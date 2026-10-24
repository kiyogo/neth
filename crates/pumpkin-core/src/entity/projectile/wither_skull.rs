use std::sync::atomic::{AtomicBool, Ordering};

use pumpkin_data::{damage::DamageType, effect::StatusEffect, potion::Effect, tracked_data};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::{
    entity::{
        Entity, EntityBase,
        projectile::{ProjectileHit, ThrownItemEntity},
    },
    server::Server,
    world::ExplosionInteraction,
};

const GRAVITY: f64 = 0.0;

pub struct WitherSkullEntity {
    pub thrown: ThrownItemEntity,
    pub dangerous: AtomicBool,
}

impl WitherSkullEntity {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            has_left_owner: AtomicBool::new(true),
            gravity: GRAVITY,
        };

        Self {
            thrown,
            dangerous: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn new_shot(
        entity: Entity,
        shooter: &Entity,
        dangerous: bool,
        direction: Vector3<f64>,
    ) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        let speed = 0.95;
        let vel = direction.normalize().multiply(speed, speed, speed);
        thrown.entity.velocity.store(vel);

        let len = vel.horizontal_length();
        thrown.entity.set_rotation(
            vel.x.atan2(vel.z) as f32 * 57.295_776,
            vel.y.atan2(len) as f32 * 57.295_776,
        );

        Self {
            thrown,
            dangerous: AtomicBool::new(dangerous),
        }
    }

    #[must_use]
    pub fn is_dangerous(&self) -> bool {
        self.dangerous.load(Ordering::Relaxed)
    }

    pub fn set_dangerous(&self, dangerous: bool) {
        self.dangerous.store(dangerous, Ordering::Relaxed);
        self.thrown
            .entity
            .set_synced_data(tracked_data::wither_skull::DATA_DANGEROUS, dangerous);
    }
}

impl EntityBase for WitherSkullEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("dangerous", self.is_dangerous());
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(dangerous) = nbt.get_bool("dangerous") {
            self.dangerous.store(dangerous, Ordering::Relaxed);
        }
    }

    fn init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            tracked_data::wither_skull::DATA_DANGEROUS,
            self.is_dangerous(),
        );
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = self.get_entity();
        let mut velocity = entity.velocity.load();

        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            0.8
        } else {
            crate::entity::combat::get_wither_skull_inertia(self.is_dangerous())
        };

        let speed = velocity.length();
        if speed > 1e-6 {
            let norm = velocity.normalize();
            velocity = norm
                .multiply(0.1, 0.1, 0.1)
                .add(&velocity)
                .multiply(inertia, inertia, inertia);
            entity.velocity.store(velocity);
        }

        self.thrown.process_move_and_collision(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn on_hit(&self, hit: ProjectileHit) {
        let world = self.get_entity().world.load();

        if let ProjectileHit::Entity {
            ref entity,
            hit_pos,
            ..
        } = hit
        {
            let difficulty = world.level_info.load().difficulty;
            let shooter = self
                .thrown
                .owner_id
                .and_then(|id| world.get_entity_by_id(id));

            let damaged = entity.damage_with_context(
                entity.as_ref(),
                8.0,
                DamageType::WITHER_SKULL,
                Some(hit_pos),
                Some(self),
                shooter.as_deref(),
            );

            if damaged {
                // Vanilla WitherSkull.java:70-72:
                // If victim was killed, heal living owner by 5.0 HP
                if !entity.get_entity().is_alive()
                    && let Some(living_owner) = shooter.as_ref().and_then(|s| s.get_living_entity())
                {
                    living_owner.heal(crate::entity::combat::get_wither_skull_kill_heal_amount());
                }

                if let Some(living) = entity.get_living_entity() {
                    let duration = crate::entity::combat::get_wither_skull_effect_duration(difficulty);
                    if duration > 0 {
                        let effect = Effect {
                            effect_type: &StatusEffect::WITHER,
                            duration,
                            amplifier: 1,
                            ambient: false,
                            show_particles: true,
                            show_icon: true,
                            blend: true,
                        };
                        if let Some(player) = entity.get_player() {
                            player.add_effect(effect);
                        } else {
                            living.add_effect(effect);
                        }
                    }
                }
            }
        }

        let hit_pos = hit.hit_pos();
        world.explode(hit_pos, 1.0, ExplosionInteraction::Mob);

        // Vanilla WitherSkull.java:98: discard projectile after exploding
        self.get_entity().remove();
    }
}
