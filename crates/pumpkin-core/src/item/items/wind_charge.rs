use std::sync::Arc;

use crate::entity::player::Player;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::Sound;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::item::{ItemBehaviour, ItemMetadata};

pub struct WindChargeItem;

impl WindChargeItem {
    pub const PROJECTILE_SHOOT_POWER: f32 = 1.5;
    pub const COOLDOWN_TICKS: i32 = 10;
    pub const DISPENSE_EVENT_ID: i32 = 1051;
    pub const DISPENSER_UNCERTAINTY: f32 = 6.6666665;
    pub const DISPENSER_POWER: f32 = 1.0;
    pub const PLAYER_EXPLOSION_RADIUS: f32 = 1.2;
    pub const BREEZE_EXPLOSION_RADIUS: f32 = 3.0;
    pub const DEFLECT_COOLDOWN_TICKS: u8 = 5;

    #[must_use]
    pub const fn get_cooldown_ticks() -> i32 {
        Self::COOLDOWN_TICKS
    }

    /// Vanilla WindChargeItem.java:49: 0.4F / (level.getRandom().nextFloat() * 0.4F + 0.8F)
    #[must_use]
    pub fn calculate_throw_pitch(random_sample: f32) -> f32 {
        0.4 / (random_sample * 0.4 + 0.8)
    }
}

impl ItemMetadata for WindChargeItem {
    fn ids() -> Box<[u16]> {
        [Item::WIND_CHARGE.id].into()
    }
}

impl ItemBehaviour for WindChargeItem {
    fn normal_use(&self, item: &Item, player: &Player) {
        let (yaw, pitch) = player.rotation();
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_rotation(&self, item: &Item, player: &Player, yaw: f32, pitch: f32) {
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_hand(
        &self,
        _block: &Item,
        player: &Player,
        yaw: f32,
        pitch: f32,
        hand: pumpkin_util::Hand,
    ) {
        if player.has_item_cooldown("minecraft:wind_charge") {
            return;
        }

        let world = player.world();
        let player_pos = player.position();
        let spawn_pos = player.get_eye_pos();

        let rand_sample = rand::random::<f32>();
        let sound_pitch = crate::entity::combat::calculate_wind_charge_throw_pitch(rand_sample);
        world.play_sound_fine(
            Sound::EntityWindChargeThrow,
            pumpkin_data::sound::SoundCategory::Neutral,
            &player_pos,
            0.5,
            sound_pitch,
        );

        let entity = Entity::new(world.clone(), spawn_pos, &EntityType::WIND_CHARGE);

        let wind_charge = ThrownItemEntity::new(entity, player.get_entity(), WIND_CHARGE_GRAVITY);
        wind_charge.set_velocity_from(pitch, yaw, 0.0, Self::PROJECTILE_SHOOT_POWER, 1.0);

        world.spawn_entity(Arc::new(WindChargeEntity::new_normal(wind_charge)));

        player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            Item::WIND_CHARGE.id as i32,
            1,
        );

        let mut stack = player.inventory.get_stack_in_hand(hand);
        if !stack.is_empty() && stack.item.id == Item::WIND_CHARGE.id {
            stack.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_stack_in_hand(hand, stack);
        }

        // Vanilla Items.java:2233 useCooldown(0.5F) -> 10 ticks cooldown
        player.set_item_cooldown(
            "minecraft:wind_charge",
            crate::entity::combat::calculate_wind_charge_cooldown_ticks(),
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
