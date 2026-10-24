use pumpkin_data::damage::DamageType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::mob::Mob;

pub struct GuardianAttackGoal {
    attack_time: i32,
    is_elder: bool,
}

impl GuardianAttackGoal {
    #[must_use]
    pub const fn new(is_elder: bool) -> Self {
        Self {
            attack_time: -10,
            is_elder,
        }
    }
}

impl Goal for GuardianAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        target.get_entity().is_alive()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }
        let my_pos = mob.get_entity().pos.load();
        let target_pos = target.get_entity().pos.load();
        let dist_sq = my_pos.squared_distance_to_vec(&target_pos);
        // Elder guardian attacks at any distance within follow range; normal guardian stays > 9 blocks away
        self.is_elder || dist_sq > 9.0
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.attack_time = -10;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();

        if let Some(target) = mob.get_mob_entity().get_target() {
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 90.0, 90.0);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.attack_time = -10;
        let entity = mob.get_entity();
        entity.set_synced_data(tracked_data::guardian::ID_ATTACK_TARGET, VarInt(0));
        *mob.get_mob_entity()
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return;
        };

        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();

        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity_with_range(&target, 90.0, 90.0);

        if !mob.has_line_of_sight(target.get_entity()) {
            mob.get_entity()
                .set_synced_data(tracked_data::guardian::ID_ATTACK_TARGET, VarInt(0));
            *mob.get_mob_entity()
                .target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            return;
        }

        self.attack_time += 1;

        if self.attack_time == 0 {
            // Activate laser beam on client
            let target_id = target.get_entity().entity_id;
            mob.get_entity()
                .set_synced_data(tracked_data::guardian::ID_ATTACK_TARGET, VarInt(target_id));

            let pos = mob.get_entity().pos.load();
            mob.get_entity().world.load().play_sound(
                Sound::EntityGuardianAttack,
                SoundCategory::Hostile,
                &pos,
            );
        } else if self.attack_time >= 80 {
            // Complete laser blast
            let world = mob.get_entity().world.load();
            let difficulty = world.level_info.load().difficulty;
            let magic_damage = crate::entity::combat::calculate_guardian_laser_damage(difficulty, self.is_elder);

            let guardian_base = mob.get_entity();

            // Vanilla Guardian.java:413-414:
            // 1. Indirect magic damage
            target.damage_with_context(
                guardian_base,
                magic_damage,
                DamageType::INDIRECT_MAGIC,
                Some(mob.get_entity().pos.load()),
                Some(guardian_base),
                Some(guardian_base),
            );

            // 2. Physical melee damage via doHurtTarget (mob.try_attack)
            mob.get_mob_entity().try_attack(guardian_base, target.as_ref());

            // Reset laser beam and cooldown
            mob.get_entity()
                .set_synced_data(tracked_data::guardian::ID_ATTACK_TARGET, VarInt(0));
            *mob.get_mob_entity()
                .target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            self.attack_time = -10;
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
