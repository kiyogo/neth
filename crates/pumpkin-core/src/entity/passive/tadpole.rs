use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl,
    ai::pathfinder::Navigator,
    ai::goal::{
        escape_danger::EscapeDangerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, random_swimming::RandomSwimmingGoal, tempt::TemptGoal,
        try_find_water::TryFindWaterGoal,
    },
    combat,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    player::Player,
};
use crate::item::items::bucket::create_filled_result;

const TEMPT_ITEMS: &[&Item] = &[&Item::SLIME_BALL];

pub struct TadpoleEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub air_supply: AtomicI32,
}

impl TadpoleEntity {
    pub const XP_REWARD: u32 = combat::TADPOLE_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(SmoothSwimmingMoveControl::new(85, 10, 0.02, 0.1, true));

        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::water_bound(false);

        let tadpole = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            air_supply: AtomicI32::new(300),
        };
        let mob_arc = Arc::new(tadpole);

        // Vanilla Tadpole attributes: MAX_HEALTH = 6.0, MOVEMENT_SPEED = 1.0
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MAX_HEALTH,
                combat::TADPOLE_MAX_HEALTH,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MOVEMENT_SPEED,
                combat::TADPOLE_BASE_MOVEMENT_SPEED,
            );
            mob_arc.mob_entity.living_entity.set_health(combat::TADPOLE_MAX_HEALTH as f32);

        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(TryFindWaterGoal));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(3, Box::new(RandomSwimmingGoal::new(0.5)));
            goal_selector.add_goal(
                4,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(5, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    fn is_food(item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FROG_FOOD)
            || item_stack.item == &Item::SLIME_BALL
    }

    #[must_use]
    pub const fn can_drown(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn can_breathe_under_water(&self) -> bool {
        true
    }

    /// Converts this Tadpole into a mature Frog with the biome-specific variant.
    ///
    /// Mirrors vanilla `Tadpole.java:238-247`:
    /// `this.convertTo(EntityTypes.FROG, ...)`
    pub fn grow_up(&self) {
        let entity = self.get_entity();
        if !entity.is_alive() {
            return;
        }
        let world = entity.world.load();
        let pos = entity.pos.load();
        let block_pos = entity.block_pos.load();
        let biome = world.get_biome(&block_pos);
        let is_warm = biome.has_tag(&pumpkin_data::tag::WorldgenBiome::MINECRAFT_SPAWNS_WARM_VARIANT_FROGS);
        let is_cold = biome.has_tag(&pumpkin_data::tag::WorldgenBiome::MINECRAFT_SPAWNS_COLD_VARIANT_FROGS);
        let variant = combat::determine_frog_variant_from_biome_tags(is_warm, is_cold);

        let frog = crate::entity::passive::frog::FrogEntity::new(Entity::new(
            world.clone(),
            pos,
            &EntityType::FROG,
        ));
        frog.set_variant(variant);
        world.play_sound_fine(
            Sound::EntityTadpoleGrowUp,
            SoundCategory::Neutral,
            &pos,
            0.15,
            1.0,
        );
        world.spawn_entity(frog);
        entity.remove();
    }
}

impl AgeableMob for TadpoleEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Mob for TadpoleEntity {

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
    }

    /// Water animals never drown.
    fn can_breathe_underwater(&self) -> bool {
        true
    }

    /// WaterAnimal.handleAirSupply.
    fn dries_out_on_land(&self) -> bool {
        true
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_tadpole(&self) -> Option<&TadpoleEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    /// Vanilla `Tadpole.java:254-256`: `shouldDropExperience() -> false`.
    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla `Tadpole.java:233-235`: when age reaches adult threshold, mature into Frog
        if entity.age.load(Ordering::Relaxed) >= 0 {
            self.grow_up();
            return;
        }

        let in_water = entity.touching_water.load(Ordering::Relaxed);

        // Flop on land (matching vanilla AbstractFish.aiStep)
        if !in_water {
            self.mob_entity.living_entity.movement_input.store(Vector3::new(0.0, 0.0, 0.0));
        }

        if !in_water && entity.on_ground.load(Ordering::Relaxed) {
            let mut rng = rand::rng();
            let dx = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let dz = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let velo = entity.velocity.load();
            entity.set_velocity(Vector3::new(velo.x + dx as f64, 0.4, velo.z + dz as f64));
            entity.on_ground.store(false, Ordering::Relaxed);
            if !entity.silent.load(Ordering::Relaxed) {
                entity.world.load().play_sound_fine(
                    Sound::EntityTadpoleFlop,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if Self::is_food(item_stack) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let age = self
                .get_mob_entity()
                .living_entity
                .entity
                .age
                .load(Ordering::Relaxed);
            let speedup = (-age / 10).max(1);
            let new_age = self
                .get_mob_entity()
                .living_entity
                .entity
                .age
                .fetch_add(speedup, Ordering::Relaxed)
                + speedup;

            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.spawn_particle(
                pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                Particle::HappyVillager,
            );
            if new_age >= 0 {
                self.grow_up();
            }
            return true;
        }

        if item_stack.get_item() == &Item::WATER_BUCKET {
            let entity = self.get_entity();
            let world = entity.world.load();
            if let Some(server) = world.server.upgrade() {
                let mut event = crate::plugin::api::events::player::player_bucket_entity::PlayerBucketEntityEvent {
                    player: player.clone(),
                    entity_id: entity.entity_id,
                    bucket_item: "tadpole_bucket".to_string(),
                    cancelled: false,
                };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return false;
                }
            }
            create_filled_result(item_stack, player, &Item::TADPOLE_BUCKET, false);
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Used,
                Item::WATER_BUCKET.id as i32,
                1,
            );
            player.trigger_advancement(
                crate::entity::player::advancement::trigger::AdvancementTrigger::InventoryChanged,
            );
            let pos = entity.pos.load();
            world.play_sound(Sound::ItemBucketFillTadpole, SoundCategory::Neutral, &pos);
            entity.remove();
            return true;
        }

        false
    }
}

impl CustomSound for TadpoleEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTadpoleDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTadpoleHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tadpole_parity_constants() {
        assert_eq!(TadpoleEntity::XP_REWARD, 0);
        assert_eq!(combat::TADPOLE_MAX_HEALTH, 6.0);
        assert_eq!(combat::TADPOLE_BASE_MOVEMENT_SPEED, 1.0);
    }
}
