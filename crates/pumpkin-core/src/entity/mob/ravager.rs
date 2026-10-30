use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{
        Mob, MobEntity,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            HoldGroundAttackGoal, PathfindToRaidGoal, Raider, RaiderCelebrationGoal, RaiderData,
            RaiderMoveThroughVillageGoal,
        },
    },
};

pub struct RavagerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    attack_tick: AtomicI32,
    stunned_tick: AtomicI32,
    roar_tick: AtomicI32,
}

impl RavagerEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let ravager = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            attack_tick: AtomicI32::new(0),
            stunned_tick: AtomicI32::new(0),
            roar_tick: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(ravager);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Ravager: MAX_HEALTH = 100.0, MOVEMENT_SPEED = 0.3, KNOCKBACK_RESISTANCE = 0.75, ATTACK_DAMAGE = 12.0, ATTACK_KNOCKBACK = 1.5, FOLLOW_RANGE = 32.0, STEP_HEIGHT = 1.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = f64::from(crate::entity::combat::RAVAGER_MAX_HEALTH);
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = crate::entity::combat::RAVAGER_BASE_MOVEMENT_SPEED;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(kb) = attributes.get_mut(&Attributes::KNOCKBACK_RESISTANCE.id) {
                kb.base_value = crate::entity::combat::RAVAGER_KNOCKBACK_RESISTANCE;
                kb.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = f64::from(crate::entity::combat::RAVAGER_ATTACK_DAMAGE);
                damage.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(attack_kb) = attributes.get_mut(&Attributes::ATTACK_KNOCKBACK.id) {
                attack_kb.base_value = crate::entity::combat::RAVAGER_ATTACK_KNOCKBACK;
                attack_kb.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(range) = attributes.get_mut(&Attributes::FOLLOW_RANGE.id) {
                range.base_value = crate::entity::combat::RAVAGER_FOLLOW_RANGE;
                range.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(step) = attributes.get_mut(&Attributes::STEP_HEIGHT.id) {
                step.base_value = crate::entity::combat::RAVAGER_STEP_HEIGHT;
                step.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::RAVAGER_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(2, Box::new(HoldGroundAttackGoal::new(10.0)));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            goal_selector.add_goal(4, Box::new(PathfindToRaidGoal::default()));
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(0.4)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(2, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, true),
            );
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::WANDERING_TRADER,
                    true,
                ),
            );
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_immobile(&self) -> bool {
        self.attack_tick.load(Ordering::Relaxed) > 0
            || self.stunned_tick.load(Ordering::Relaxed) > 0
            || self.roar_tick.load(Ordering::Relaxed) > 0
    }

    pub fn stun(&self) {
        self.stunned_tick.store(40, Ordering::Relaxed);
        let entity = &self.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let world = entity.world.load();
        world.play_sound(Sound::EntityRavagerStunned, SoundCategory::Hostile, &pos);
        world.send_entity_status(entity, pumpkin_data::entity::EntityStatus::RavagerStunned, None);
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    /// Vanilla `Ravager.strongKnockback`.
    pub fn strong_knockback(&self, target: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let my_pos = entity.pos.load();
        let target_entity = target.get_entity();
        let t_pos = target_entity.pos.load();
        let (push_x, push_y, push_z) = crate::entity::combat::calculate_ravager_strong_knockback(
            my_pos.x, my_pos.z, t_pos.x, t_pos.z,
        );
        let mut vel = target_entity.velocity.load();
        vel.x += push_x.clamp(-2.0, 2.0);
        vel.y += push_y;
        vel.z += push_z.clamp(-2.0, 2.0);
        target_entity.set_velocity(vel);
        target_entity.sync_velocity.store(true, Ordering::SeqCst);
        target_entity.velocity_dirty.store(true, Ordering::SeqCst);
    }

    pub fn roar(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load_full();
        let my_id = entity.entity_id;

        let roar_bb = entity.bounding_box.load().expand_all(4.0);
        let targets = world.get_entities_at_box(&roar_bb);

        for target in targets {
            let target_entity = target.get_entity();
            if target_entity.entity_id == my_id {
                continue;
            }
            if let Some(living) = target.get_living_entity() {
                if !living.entity.is_alive() {
                    continue;
                }
                // Illagers are immune to roar damage, but still get knocked back (Ravager.java:204-209)
                let is_illager = crate::entity::combat::is_illager_immune_to_ravager_roar(target_entity.entity_type);
                if !is_illager {
                    target.damage(target.as_ref(), crate::entity::combat::RAVAGER_ROAR_DAMAGE, DamageType::MOB_ATTACK);
                }

                // Vanilla Ravager.java:207: if (!(entity instanceof Player)) { this.strongKnockback(entity); }
                if crate::entity::combat::should_ravager_roar_knockback_target(
                    target_entity.entity_type == &EntityType::PLAYER,
                ) {
                    self.strong_knockback(&*target);
                }
            }
        }

        world.send_entity_status(entity, pumpkin_data::entity::EntityStatus::RavagerRoared, None);
    }
}

impl CustomSound for RavagerEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityRavagerDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityRavagerHurt)
    }
}

impl Mob for RavagerEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_ravager(&self) -> Option<&RavagerEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_patrolling_monster(&self) -> Option<&dyn PatrollingMonster> {
        Some(self)
    }

    fn as_raider(&self) -> Option<&dyn Raider> {
        Some(self)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let world = entity.world.load_full();

        // Vanilla leaf trample griefing
        if world.level_info.load().game_rules.mob_griefing {
            let bb = entity.bounding_box.load().expand_all(0.2);
            let min_pos = bb.min_block_pos();
            let max_pos = bb.max_block_pos();
            for x in min_pos.0.x..=max_pos.0.x {
                for y in min_pos.0.y..=max_pos.0.y {
                    for z in min_pos.0.z..=max_pos.0.z {
                        let block_pos = BlockPos::new(x, y, z);
                        let block = world.get_block(&block_pos);
                        if block.has_tag(&tag::Block::MINECRAFT_LEAVES) {
                            world.break_block(&block_pos, None, BlockFlags::NOTIFY_ALL);
                        }
                    }
                }
            }
        }

        // Roar countdown
        let r_tick = self.roar_tick.load(Ordering::Relaxed);
        if r_tick > 0 {
            let next = r_tick - 1;
            self.roar_tick.store(next, Ordering::Relaxed);
            if next == 10 {
                self.roar();
            }
        }

        // Stunned countdown
        let s_tick = self.stunned_tick.load(Ordering::Relaxed);
        if s_tick > 0 {
            let next = s_tick - 1;
            self.stunned_tick.store(next, Ordering::Relaxed);
            if next == 0 {
                let pos = entity.pos.load();
                world.play_sound(
                    Sound::EntityRavagerRoar,
                    SoundCategory::Hostile,
                    &pos,
                );
                self.roar_tick.store(20, Ordering::Relaxed);
            }
        }

        if self.attack_tick.load(Ordering::Relaxed) > 0 {
            self.attack_tick.fetch_sub(1, Ordering::Relaxed);
        }

        // Vanilla Ravager.aiStep: movement speed transitions (0.0 when immobile, 0.35 when targeting, 0.3 normal)
        let target_speed = if self.is_immobile() {
            0.0
        } else if self.mob_entity.get_target().is_some() {
            0.35
        } else {
            0.3
        };
        {
            let mut attributes = self
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(speed_attr) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                if (speed_attr.base_value - target_speed).abs() > 1e-4 {
                    speed_attr.base_value = target_speed;
                    speed_attr.dirty.store(true, Ordering::Relaxed);
                }
            }
        }

        // Ambient sound
        if self.roar_tick.load(Ordering::Relaxed) <= 0
            && self.stunned_tick.load(Ordering::Relaxed) <= 0
            && !entity.silent.load(Ordering::Relaxed)
            && rand::random_range(0..100) == 0
        {
            world.play_sound_fine(
                Sound::EntityRavagerAmbient,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        self.attack_tick.store(10, Ordering::Relaxed);
        let entity = self.get_entity();
        let world = entity.world.load();
        world.send_entity_status(entity, pumpkin_data::entity::EntityStatus::StartAttacking, None);
        world.play_sound_fine(
            Sound::EntityRavagerAttack,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::get_ravager_xp_reward()
    }

    fn on_blocked_by_shield(&self, defender: &dyn EntityBase, _fully_blocked: bool) {
        if self.roar_tick.load(Ordering::Relaxed) == 0 {
            if rand::random::<f64>() < crate::entity::combat::RAVAGER_STUN_CHANCE {
                self.stun();
                defender.push(&self.mob_entity.living_entity.entity);
                self.mob_entity.living_entity.entity.sync_velocity.store(true, Ordering::SeqCst);
                self.mob_entity.living_entity.entity.velocity_dirty.store(true, Ordering::SeqCst);
            } else {
                self.strong_knockback(defender);
            }
            defender.get_entity().sync_velocity.store(true, Ordering::SeqCst);
            defender.get_entity().velocity_dirty.store(true, Ordering::SeqCst);
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_raider_nbt(nbt);
        nbt.put_int("AttackTick", self.attack_tick.load(Ordering::Relaxed));
        nbt.put_int("StunTick", self.stunned_tick.load(Ordering::Relaxed));
        nbt.put_int("RoarTick", self.roar_tick.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
        if let Some(tick) = nbt.get_int("AttackTick") {
            self.attack_tick.store(tick, Ordering::Relaxed);
        }
        if let Some(tick) = nbt.get_int("StunTick") {
            self.stunned_tick.store(tick, Ordering::Relaxed);
        }
        if let Some(tick) = nbt.get_int("RoarTick") {
            self.roar_tick.store(tick, Ordering::Relaxed);
        }
    }
}

impl PatrollingMonster for RavagerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }

    fn can_be_leader(&self) -> bool {
        false
    }
}

impl Raider for RavagerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityRavagerCelebrate
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::combat::{
        RAVAGER_ATTACK_DAMAGE, RAVAGER_ATTACK_DURATION_TICKS, RAVAGER_ATTACK_KNOCKBACK,
        RAVAGER_ATTACK_MOVEMENT_SPEED, RAVAGER_BASE_MOVEMENT_SPEED, RAVAGER_FOLLOW_RANGE,
        RAVAGER_KNOCKBACK_RESISTANCE, RAVAGER_MAX_HEALTH, RAVAGER_ROAR_BURST_TICK,
        RAVAGER_ROAR_DAMAGE, RAVAGER_ROAR_DURATION_TICKS, RAVAGER_ROAR_RADIUS,
        RAVAGER_STEP_HEIGHT, RAVAGER_STUN_CHANCE, RAVAGER_STUN_DURATION_TICKS,
        RAVAGER_XP_REWARD, calculate_ravager_target_speed,
        is_illager_immune_to_ravager_roar, should_ravager_roar_knockback_target,
    };

    #[test]
    fn test_ravager_combat_stun_and_roar_parity() {
        assert_eq!(RAVAGER_MAX_HEALTH, 100.0);
        assert_eq!(RAVAGER_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(RAVAGER_ATTACK_MOVEMENT_SPEED, 0.35);
        assert_eq!(RAVAGER_KNOCKBACK_RESISTANCE, 0.75);
        assert_eq!(RAVAGER_ATTACK_DAMAGE, 12.0);
        assert_eq!(RAVAGER_ATTACK_KNOCKBACK, 1.5);
        assert_eq!(RAVAGER_FOLLOW_RANGE, 32.0);
        assert_eq!(RAVAGER_STEP_HEIGHT, 1.0);
        assert_eq!(RAVAGER_XP_REWARD, 20);

        assert_eq!(RAVAGER_ATTACK_DURATION_TICKS, 10);
        assert_eq!(RAVAGER_STUN_DURATION_TICKS, 40);
        assert_eq!(RAVAGER_ROAR_DURATION_TICKS, 20);
        assert_eq!(RAVAGER_ROAR_BURST_TICK, 10);
        assert_eq!(RAVAGER_ROAR_RADIUS, 4.0);
        assert_eq!(RAVAGER_ROAR_DAMAGE, 6.0);
        assert_eq!(RAVAGER_STUN_CHANCE, 0.5);

        // Target speed states
        assert_eq!(calculate_ravager_target_speed(true, false), 0.0);
        assert_eq!(calculate_ravager_target_speed(true, true), 0.0);
        assert_eq!(calculate_ravager_target_speed(false, true), 0.35);
        assert_eq!(calculate_ravager_target_speed(false, false), 0.3);

        // Roar targets
        assert!(!should_ravager_roar_knockback_target(true)); // Player not pushed
        assert!(should_ravager_roar_knockback_target(false)); // Other mobs pushed

        // Illagers immune to roar damage
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::PILLAGER));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::VINDICATOR));
        assert!(!is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::VILLAGER));
        assert!(!is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::PLAYER));
    }
}

