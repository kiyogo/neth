use std::any::Any;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::entity::projectile::arrow::ArrowPickup;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_util::GameMode;
use pumpkin_util::math::vector3::Vector3;

pub struct TridentItem;

impl ItemMetadata for TridentItem {
    fn ids() -> Box<[u16]> {
        [Item::TRIDENT.id].into()
    }
}

impl ItemBehaviour for TridentItem {
    fn normal_use(&self, item: &Item, player: &Player) {
        self.normal_use_with_hand(item, player, 0.0, 0.0, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_rotation(&self, item: &Item, player: &Player, yaw: f32, pitch: f32) {
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_hand(
        &self,
        _item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: pumpkin_util::Hand,
    ) {
        let inventory = player.inventory();
        let stack = inventory.get_stack_in_hand(hand);

        // Vanilla TridentItem.java:129-130:
        // if (riptideStrength > 0.0F && !player.isInWaterOrRain()) return FAIL;
        let mut riptide_level = 0u32;
        if let Some(enchantments) =
            stack.get_data_component::<pumpkin_data::data_component_impl::EnchantmentsImpl>()
        {
            for (enchantment, level) in enchantments.enchantment.iter() {
                if **enchantment == pumpkin_data::Enchantment::RIPTIDE {
                    riptide_level = *level as u32;
                }
            }
        }
        if riptide_level > 0 && !player.get_entity().is_in_water_or_rain() {
            return;
        }

        player
            .living_entity
            .set_active_hand(hand, stack, 72000);
    }

    fn on_stopped_using(&self, _stack: &ItemStack, player: &Player) {
        let use_ticks = player
            .living_entity
            .item_use_time
            .load(std::sync::atomic::Ordering::Relaxed);
        let use_ticks = 72000 - use_ticks;

        if use_ticks < 10 {
            return;
        }

        let active_hand = player
            .living_entity
            .active_hand
            .lock()
            .unwrap()
            .unwrap_or(pumpkin_util::Hand::Right);
        let world = player.world();
        let stack_guard = player.inventory().get_stack_in_hand(active_hand);

        // Check Riptide level
        let mut riptide_level = 0u32;
        if let Some(enchantments) =
            stack_guard.get_data_component::<pumpkin_data::data_component_impl::EnchantmentsImpl>()
        {
            for (enchantment, level) in enchantments.enchantment.iter() {
                if **enchantment == pumpkin_data::Enchantment::RIPTIDE {
                    riptide_level = *level as u32;
                }
            }
        }

        if riptide_level > 0 {
            // Vanilla TridentItem.java:70:
            // if (!(riptideStrength > 0.0F) || player.isInWaterOrRain() && !player.isPassenger())
            let in_water_or_rain = player.get_entity().is_in_water_or_rain();
            let is_passenger = player.get_entity().has_vehicle();
            if !crate::entity::combat::can_perform_riptide(
                riptide_level,
                in_water_or_rain,
                is_passenger,
            ) {
                player.living_entity.clear_active_hand();
                return;
            }

            if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                && let Some(server) = world.server.upgrade()
            {
                let mut event =
                    crate::plugin::api::events::player::player_riptide::PlayerRiptideEvent {
                        player: player_arc,
                        item_name: "trident".to_string(),
                        cancelled: false,
                    };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    player.living_entity.clear_active_hand();
                    return;
                }
            }

            let riptide_strength =
                crate::entity::combat::calculate_riptide_strength(riptide_level);
            let (yaw, pitch) = player.rotation();
            let velocity =
                crate::entity::combat::calculate_riptide_velocity(yaw, pitch, riptide_strength);

            player.living_entity.entity.velocity.store(velocity);
            player.living_entity.start_auto_spin_attack(20, 8.0);

            // Vanilla TridentItem.java:104-107:
            // if (player.onGround()) { player.move(MoverType.SELF, new Vec3(0.0, 1.1999999F, 0.0)); }
            if player
                .get_entity()
                .on_ground
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                let current_pos = player.position();
                player.living_entity.entity.set_pos(Vector3::new(
                    current_pos.x,
                    current_pos.y + 1.1999999,
                    current_pos.z,
                ));
            }

            let sound = crate::entity::combat::get_riptide_sound(riptide_level);
            world.play_sound(
                sound,
                pumpkin_data::sound::SoundCategory::Players,
                &player.position(),
            );

            let slot = EquipmentSlot::from_hand(active_hand);
            player.damage_item_in_slot(&slot, 1);
            player.living_entity.clear_active_hand();
            return;
        }

        // Normal throw - spawn thrown trident
        let (yaw, pitch) = player.rotation();
        let entity = Entity::new(world.clone(), player.position(), &EntityType::TRIDENT);
        let trident = TridentEntity::new_shot(
            entity,
            player.get_entity(),
            stack_guard.clone(),
            ArrowPickup::Allowed,
        );
        trident.set_velocity_from_rotation(pitch, yaw, 0.0, 2.5, 1.0);
        trident.apply_on_projectile_spawned(&stack_guard);
        world.spawn_entity(Arc::new(trident));

        world.play_sound(
            Sound::ItemTridentThrow,
            pumpkin_data::sound::SoundCategory::Players,
            &player.position(),
        );

        if player.gamemode.load() != GameMode::Creative {
            let inventory = player.inventory();
            let slot = match active_hand {
                pumpkin_util::Hand::Right => inventory.get_selected_slot() as usize,
                pumpkin_util::Hand::Left => {
                    pumpkin_inventory::player::player_inventory::PlayerInventory::OFF_HAND_SLOT
                }
            };
            inventory.set_slot(slot, ItemStack::EMPTY.clone());
            player.sync_hand_slot(slot, ItemStack::EMPTY.clone());
        }

        player.living_entity.clear_active_hand();
    }

    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
