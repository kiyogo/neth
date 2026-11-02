use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::block::blocks::chests::get_container;
use crate::block::entities::hopper::HopperBlockEntity;
use crate::entity::{Entity, player::Player};
use pumpkin_data::translation;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::BlockDirection;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;

use super::container::{self, MinecartInventory};

/// Vanilla `MinecartHopper`: 5 slots inventory, suck item rate 1 item/tick.
#[derive(Clone)]
pub(super) struct HopperMinecart {
    enabled: Arc<AtomicBool>,
    consumed_item_this_frame: Arc<AtomicBool>,
    inventory: Arc<MinecartInventory>,
}

impl HopperMinecart {
    pub const DEFAULT_ENABLED: bool = true;
    #[allow(dead_code)]
    pub const DEFAULT_DISPLAY_OFFSET: i32 = 1;
    pub const CONTAINER_SIZE: usize = 5;

    pub(super) fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(Self::DEFAULT_ENABLED)),
            consumed_item_this_frame: Arc::new(AtomicBool::new(false)),
            inventory: Arc::new(MinecartInventory::new(Self::CONTAINER_SIZE)),
        }
    }

    pub(super) const fn inventory(&self) -> &Arc<MinecartInventory> {
        &self.inventory
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub(super) fn reset_consumed_item_this_frame(&self) {
        self.consumed_item_this_frame.store(false, Ordering::Relaxed);
    }

    pub(super) fn has_consumed_item_this_frame(&self) -> bool {
        self.consumed_item_this_frame.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub(super) fn tick(&self, entity: &Entity) {
        self.reset_consumed_item_this_frame();
        self.try_consume_items(entity);
    }

    /// Vanilla `MinecartHopper.tryConsumeItems()`: consumes at most 1 item per frame/tick.
    pub(super) fn try_consume_items(&self, entity: &Entity) -> bool {
        if !entity.is_alive()
            || !self.is_enabled()
            || self.consumed_item_this_frame.load(Ordering::Relaxed)
        {
            return false;
        }

        if self.suck_in_items(entity) {
            self.consumed_item_this_frame.store(true, Ordering::Relaxed);
            return true;
        }

        false
    }

    /// Vanilla `MinecartHopper.suckInItems()`:
    /// 1. Tries `HopperBlockEntity.suckInItems(level, this)`:
    ///    a. Block container directly above at `(x, y + 1.5, z)`
    ///    b. Entity container directly above within 1x1x1 AABB
    ///    c. If no container was above: item entities in SUCK_AABB above
    /// 2. If above returned false: item entities in cart hitbox inflated by (0.25, 0.0, 0.25).
    pub(super) fn suck_in_items(&self, entity: &Entity) -> bool {
        let world = entity.world.load();
        let pos = entity.pos.load();
        let source_pos = BlockPos::floored(pos.x, pos.y + 1.5, pos.z);
        let inventory = &self.inventory;

        // 1a. Try pulling from block container directly above (vanilla getSourceContainer -> getBlockContainer)
        if let Some(block_entity) = world.get_block_entity(&source_pos) {
            if let Some(container) = get_container(&world, &source_pos, &block_entity, true)
                .or_else(|| block_entity.clone().get_inventory())
            {
                let slots: Vec<usize> = if let Some(face_slots) =
                    container.get_slots_for_face(BlockDirection::Down)
                {
                    face_slots.to_vec()
                } else {
                    (0..container.size()).collect()
                };
                for slot in slots {
                    let item = container.get_stack(slot);
                    if !item.is_empty()
                        && container.can_take_item_through_face(slot, &item, BlockDirection::Down)
                        && container.can_transfer_to(inventory.as_ref(), slot, &item)
                    {
                        let backup = item.clone();
                        let one = container.remove_stack_specific(slot, 1);
                        if HopperBlockEntity::add_one_item(
                            container.as_ref(),
                            inventory.as_ref(),
                            &one,
                        ) {
                            return true;
                        }
                        container.set_stack(slot, backup);
                    }
                }
                return false;
            }
        }

        // 1b. Try pulling from entity container directly above (vanilla getSourceContainer -> getEntityContainer)
        let above_entity_box = BoundingBox::new(
            Vector3::new(pos.x - 0.5, pos.y + 1.0, pos.z - 0.5),
            Vector3::new(pos.x + 0.5, pos.y + 2.0, pos.z + 0.5),
        );
        for other in world.get_entities_at_box(&above_entity_box) {
            if other.get_entity().entity_id != entity.entity_id && other.get_entity().is_alive() {
                if let Some(container) = other.get_container_inventory() {
                    let slots: Vec<usize> = if let Some(face_slots) =
                        container.get_slots_for_face(BlockDirection::Down)
                    {
                        face_slots.to_vec()
                    } else {
                        (0..container.size()).collect()
                    };
                    for slot in slots {
                        let item = container.get_stack(slot);
                        if !item.is_empty()
                            && container.can_take_item_through_face(slot, &item, BlockDirection::Down)
                            && container.can_transfer_to(inventory.as_ref(), slot, &item)
                        {
                            let backup = item.clone();
                            let one = container.remove_stack_specific(slot, 1);
                            if HopperBlockEntity::add_one_item(
                                container.as_ref(),
                                inventory.as_ref(),
                                &one,
                            ) {
                                return true;
                            }
                            container.set_stack(slot, backup);
                        }
                    }
                    return false;
                }
            }
        }

        // 1c. If no container was above: items at and above (vanilla getItemsAtAndAbove)
        let suction_box = BoundingBox::new(
            Vector3::new(pos.x - 0.5, pos.y + 0.6875, pos.z - 0.5),
            Vector3::new(pos.x + 0.5, pos.y + 2.0, pos.z + 0.5),
        );
        if Self::pick_up_item_internal(&world, inventory, &suction_box) {
            return true;
        }

        // 2. Items in/around the cart hitbox (vanilla this.getBoundingBox().inflate(0.25, 0.0, 0.25))
        let cart_box = entity.bounding_box.load().expand(0.25, 0.0, 0.25);
        Self::pick_up_item_internal(&world, inventory, &cart_box)
    }

    fn pick_up_item_internal(
        world: &Arc<crate::world::World>,
        inventory: &Arc<MinecartInventory>,
        search_box: &BoundingBox,
    ) -> bool {
        for entity in world.get_entities_at_box(search_box) {
            let Some(item) = entity.get_item_entity() else {
                continue;
            };
            if item
                .insert_into(|stack| HopperBlockEntity::add_stack(None, inventory.as_ref(), stack))
            {
                return true;
            }
        }
        false
    }

    pub(super) fn interact(
        &self,
        custom_name: Option<TextComponent>,
        player: &Arc<Player>,
    ) -> bool {
        container::open(
            custom_name,
            player,
            &self.inventory,
            TextComponent::translate_cross(
                translation::java::ENTITY_MINECRAFT_HOPPER_MINECART,
                translation::bedrock::ENTITY_HOPPER_MINECART_NAME,
                [],
            ),
            true,
        )
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.inventory.write_nbt(nbt);
        nbt.put_bool("Enabled", self.enabled.load(Ordering::Relaxed));
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.inventory.read_nbt(nbt);
        self.enabled
            .store(nbt.get_bool("Enabled").unwrap_or(Self::DEFAULT_ENABLED), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::HopperMinecart;
    use pumpkin_inventory::Inventory;
    use pumpkin_nbt::compound::NbtCompound;

    #[test]
    fn hopper_minecart_inventory_has_five_slots() {
        assert_eq!(HopperMinecart::new().inventory.size(), 5);
    }

    #[test]
    fn hopper_minecart_enabled_toggle_and_nbt() {
        let cart = HopperMinecart::new();
        assert!(cart.is_enabled());

        cart.set_enabled(false);
        assert!(!cart.is_enabled());

        let mut nbt = NbtCompound::new();
        cart.write_nbt(&mut nbt);
        assert_eq!(nbt.get_bool("Enabled"), Some(false));

        let cart2 = HopperMinecart::new();
        cart2.read_nbt(&nbt);
        assert!(!cart2.is_enabled());
    }

    #[test]
    fn hopper_minecart_rate_limiting_consumed_per_frame() {
        let cart = HopperMinecart::new();
        assert!(!cart.has_consumed_item_this_frame());

        cart.consumed_item_this_frame.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(cart.has_consumed_item_this_frame());

        cart.reset_consumed_item_this_frame();
        assert!(!cart.has_consumed_item_this_frame());
    }
}
