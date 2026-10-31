use uuid::Uuid;

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;
use crate::entity::mob::cube_mob::CubeMobEntity;
use crate::entity::r#type::from_type;

pub struct OozingMobEffect;

impl OozingMobEffect {
    pub const SLIME_SIZE: i32 = 2;
    pub const SLIMES_COUNT: usize = 2;
    pub const SPAWN_Y_OFFSET: f64 = 0.5;
    pub const COLOR: i32 = 10092451;

    #[must_use]
    pub const fn number_of_slimes_to_spawn(
        max_entity_cramming: i32,
        nearby_slimes: i32,
        number_requested: i32,
    ) -> i32 {
        if max_entity_cramming < 1 {
            number_requested
        } else {
            let available = max_entity_cramming - nearby_slimes;
            if available <= 0 {
                0
            } else if available > number_requested {
                number_requested
            } else {
                available
            }
        }
    }
}

impl MobEffect for OozingMobEffect {
    fn on_mob_death(&self, living: &LivingEntity, _amplifier: u8, _damage_type: &DamageType) {
        // Slimes are immune
        if living.entity.entity_type == &EntityType::SLIME {
            return;
        }

        let world = living.entity.world.load();
        let pos = living.entity.pos.load();
        let spawn_pos = Vector3::new(pos.x, pos.y + 0.5, pos.z);

        // Spawns 2 slimes of size 2 (medium slimes)
        for _ in 0..2 {
            let entity_arc = from_type(&EntityType::SLIME, spawn_pos, &world, Uuid::new_v4());
            let entity = entity_arc.get_entity();
            entity.set_pos(spawn_pos);
            entity.yaw.store(rand::random::<f32>() * 360.0);
            entity.pitch.store(0.0);

            if let Some(slime) = entity_arc.cast_any().downcast_ref::<CubeMobEntity>() {
                slime.set_size(2, true);
            }

            world.spawn_entity(entity_arc);
        }
    }
}
