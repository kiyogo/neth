use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl,
    ai::pathfinder::Navigator,
    ai::goal::{
        avoid_entity::AvoidEntityGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, random_swimming::RandomSwimmingGoal,
        revenge::RevengeGoal, tempt::TemptGoal, try_find_water::TryFindWaterGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::COD, &Item::SALMON, &Item::TROPICAL_FISH];

pub struct DolphinEntity {
    pub mob_entity: MobEntity,
    pub got_fish: AtomicBool,
    pub moistness_level: AtomicI32,
}

impl DolphinEntity {
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
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::water_bound(true);

        let dolphin = Self {
            mob_entity,
            got_fish: AtomicBool::new(false),
            moistness_level: AtomicI32::new(2400),
        };
        let mob_arc = Arc::new(dolphin);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Dolphin: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 1.2, ATTACK_DAMAGE = 3.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 10.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 1.2;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 3.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(10.0);
        mob_arc
            .mob_entity
            .living_entity
            .breath
            .air_supply
            .store(4800, Ordering::Relaxed);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vanilla Dolphin: 0 BreathAir & TryFindLiquid, 4 RandomSwimming(1.0) & RandomLookAround, 5 LookAtPlayer(6.0), 6 MeleeAttack(1.2), 9 Avoid(Guardian, 8.0)
            goal_selector.add_goal(0, Box::new(TryFindWaterGoal));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.2, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(RandomSwimmingGoal::new(1.0)));
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(
                5,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(6, Box::new(MeleeAttackGoal::new(1.2, true)));
            goal_selector.add_goal(
                9,
                Box::new(AvoidEntityGoal::new(&EntityType::GUARDIAN, 8.0, 1.0, 1.0)),
            );
            goal_selector.add_goal(
                9,
                Box::new(AvoidEntityGoal::new(
                    &EntityType::ELDER_GUARDIAN,
                    8.0,
                    1.0,
                    1.0,
                )),
            );
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
        };

        mob_arc
    }

    #[must_use]
    pub fn got_fish(&self) -> bool {
        self.got_fish.load(Ordering::Relaxed)
    }

    pub fn set_got_fish(&self, val: bool) {
        self.got_fish.store(val, Ordering::Relaxed);
    }
}

impl Mob for DolphinEntity {
    /// `Dolphin.getMaxAirSupply`.
    fn max_air_supply(&self) -> i32 {
        4800
    }

    /// `Dolphin.increaseAirSupply`.
    fn increase_air_supply(&self, _current: i32) -> i32 {
        4800
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("GotFish", self.got_fish());
        nbt.put_int("Moistness", self.moistness_level.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(got_fish) = nbt.get_bool("GotFish") {
            self.set_got_fish(got_fish);
        }
        if let Some(moistness) = nbt.get_int("Moistness") {
            self.moistness_level.store(moistness, Ordering::Relaxed);
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_dolphin(&self) -> Option<&DolphinEntity> {
        Some(self)
    }

    fn get_max_head_rotation(&self) -> f32 {
        1.0
    }

    fn can_attack(&self, target: &dyn EntityBase) -> bool {
        if self.get_entity().age.load(Ordering::Relaxed) < 0 {
            return false;
        }
        let target_entity = target.get_entity();
        if target_entity.entity_type == &EntityType::GHAST {
            return false;
        }
        self.get_mob_entity().living_entity.can_attack(target)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla `Dolphin.tick`: handle moistness and flopping on land.
        if self.mob_entity.is_no_ai() {
            self.mob_entity
                .living_entity
                .breath
                .air_supply
                .store(self.max_air_supply(), Ordering::Relaxed);
        } else if entity.is_in_water_or_rain() {
            self.moistness_level.store(2400, Ordering::Relaxed);
        } else {
            let moist = self.moistness_level.fetch_sub(1, Ordering::Relaxed) - 1;
            if moist <= 0 {
                self.mob_entity
                    .living_entity
                    .damage(caller, 1.0, DamageType::DRY_OUT);
            }

            if entity.on_ground.load(Ordering::Relaxed) {
                let mut rng = rand::rng();
                let dx = (rng.random::<f32>() * 2.0 - 1.0) * 0.2;
                let dz = (rng.random::<f32>() * 2.0 - 1.0) * 0.2;
                let velo = entity.velocity.load();
                entity.set_velocity(Vector3::new(velo.x + dx as f64, 0.5, velo.z + dz as f64));
                entity.yaw.store(rng.random::<f32>() * 360.0);
                entity.on_ground.store(false, Ordering::Relaxed);
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if entity.touching_water.load(Ordering::Relaxed) {
                Sound::EntityDolphinAmbientWater
            } else {
                Sound::EntityDolphinAmbient
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        if TEMPT_ITEMS.iter().any(|i| i.id == item.id) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            self.set_got_fish(true);
            self.mob_entity.living_entity.heal(2.0);

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
            world.play_sound(Sound::EntityDolphinEat, SoundCategory::Neutral, &pos);
            return true;
        }

        false
    }
}

impl CustomSound for DolphinEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityDolphinDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityDolphinHurt)
    }
}
