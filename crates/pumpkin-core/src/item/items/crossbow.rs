use std::any::Any;
use std::sync::atomic::Ordering;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::items::projectile_weapon::ProjectileWeaponItem;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::ChargedProjectilesImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_util::{GameMode, Hand};

pub struct CrossbowItem;

impl ItemMetadata for CrossbowItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CROSSBOW.id])
    }
}

impl ItemBehaviour for CrossbowItem {
    fn normal_use(&self, item: &Item, player: &Player) {
        self.normal_use_with_hand(item, player, 0.0, 0.0, Hand::Right);
    }

    fn normal_use_with_rotation(&self, item: &Item, player: &Player, yaw: f32, pitch: f32) {
        self.normal_use_with_hand(item, player, yaw, pitch, Hand::Right);
    }

    fn normal_use_with_hand(
        &self,
        _item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: Hand,
    ) {
        let inventory = player.inventory();
        let stack = inventory.get_stack_in_hand(hand);

        // Every crossbow carries a ChargedProjectiles component by default, so its mere
        // presence does not mean the crossbow is loaded. Vanilla checks the list is also
        // non-empty (CrossbowItem.java:68).
        if stack
            .get_data_component::<ChargedProjectilesImpl>()
            .is_some_and(|charged| !charged.projectiles.is_empty())
        {
            Self::fire_projectiles(player, hand);
            return;
        }

        let has_ammo = player.find_crossbow_ammo().is_some();
        if !has_ammo && player.gamemode.load() != GameMode::Creative {
            return;
        }

        player
            .living_entity
            .set_active_hand(hand, stack, 72000);
    }

    fn on_stopped_using(&self, _stack: &ItemStack, player: &Player) {
        let use_ticks = player.living_entity.item_use_time.load(Ordering::Relaxed);
        let use_ticks = 72000 - use_ticks;

        let active_hand = player
            .living_entity
            .active_hand
            .lock()
            .unwrap()
            .unwrap_or(Hand::Right);
        let mut stack = player.inventory().get_stack_in_hand(active_hand);
        let charge_time =
            crate::enchantment::EnchantmentHelper::modify_crossbow_charge_time(&stack, 25);

        if use_ticks >= charge_time {
            let ammo_slot = player.find_crossbow_ammo();
            let gamemode = player.gamemode.load();
            let is_creative = gamemode == GameMode::Creative;

            if ammo_slot.is_some() || is_creative {
                let projectile = ammo_slot.map_or_else(
                    || ItemStack::new(1, &Item::ARROW),
                    |slot| {
                        let inventory = player.inventory();
                        inventory.get_slot(slot).copy_with_count(1)
                    },
                );

                let drawn = ProjectileWeaponItem::draw(&stack, &projectile, is_creative);
                if !drawn.is_empty() {
                    let mut charged_nbts = Vec::new();
                    for item in drawn {
                        let mut arrow_nbt = pumpkin_nbt::compound::NbtCompound::new();
                        item.write_item_stack(&mut arrow_nbt);
                        charged_nbts.push(arrow_nbt);
                    }

                    stack.patch.push((
                        DataComponent::ChargedProjectiles,
                        Some(Box::new(ChargedProjectilesImpl {
                            projectiles: charged_nbts,
                        })),
                    ));
                    player.inventory().set_stack_in_hand(active_hand, stack);

                    if let Some(slot) = ammo_slot
                        && !is_creative
                    {
                        player.consume_arrow(slot);
                    }

                    player.world().play_sound(
                        Sound::ItemCrossbowLoadingEnd,
                        SoundCategory::Players,
                        &player.position(),
                    );
                }
            }
        }
        player.living_entity.clear_active_hand();
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CrossbowItem {
    pub const ARROW_POWER: f32 = 3.15;
    pub const FIREWORK_POWER: f32 = 1.6;

    fn fire_projectiles(player: &Player, hand: Hand) {
        let mut held = player.inventory().get_stack_in_hand(hand);
        let charged_opt = held.get_data_component::<ChargedProjectilesImpl>().cloned();

        if let Some(charged) = charged_opt {
            let mut projectiles = Vec::new();
            for projectile_nbt in charged.projectiles {
                if let Some(projectile) = ItemStack::read_item_stack(&projectile_nbt) {
                    projectiles.push(projectile);
                }
            }

            if !projectiles.is_empty() {
                let world = player.world();
                world.play_sound(
                    Sound::ItemCrossbowShoot,
                    SoundCategory::Players,
                    &player.position(),
                );

                let is_firework = projectiles
                    .iter()
                    .any(|p| p.item.id == Item::FIREWORK_ROCKET.id);
                let power = if is_firework {
                    Self::FIREWORK_POWER
                } else {
                    Self::ARROW_POWER
                };

                let is_creative = player.gamemode.load() == GameMode::Creative;
                ProjectileWeaponItem::shoot_projectiles(
                    &world,
                    player.get_entity(),
                    &held,
                    &projectiles,
                    power,
                    1.0,
                    false,
                    is_creative,
                );

                held.patch
                    .retain(|(id, _)| *id != DataComponent::ChargedProjectiles);
                player.inventory().set_stack_in_hand(hand, held);

                let durability_use = if is_firework { 3 } else { 1 };
                let slot = EquipmentSlot::from_hand(hand);
                player.damage_item_in_slot(&slot, durability_use);

                player.increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Used,
                    Item::CROSSBOW.id as i32,
                    1,
                );
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::ShotCrossbow,
                );
            }
        }
    }
}
