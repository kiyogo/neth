use pumpkin_data::attributes::Attributes;
use pumpkin_data::biome::Biome;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_util::Difficulty;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::RandomImpl;

use crate::entity::mob::cube_mob::{
    ContactDamage, CubeKind, CubeSounds, SizedSound, random_spawn_size, random_split_count,
    register_hostile_goals,
};
use crate::world::World;

pub const SLIME: CubeKind = CubeKind {
    sounds: CubeSounds {
        jump: SizedSound {
            normal: Sound::EntitySlimeJump,
            small: Sound::EntitySlimeJumpSmall,
        },
        squish: SizedSound {
            normal: Sound::EntitySlimeSquish,
            small: Sound::EntitySlimeSquishSmall,
        },
        hurt: SizedSound {
            normal: Sound::EntitySlimeHurt,
            small: Sound::EntitySlimeHurtSmall,
        },
        death: SizedSound {
            normal: Sound::EntitySlimeDeath,
            small: Sound::EntitySlimeDeathSmall,
        },
    },
    sound_category: SoundCategory::Hostile,
    health: |size| f64::from(size * size),
    size_attributes: &[(Attributes::ATTACK_DAMAGE, |size| f64::from(size))],
    jump_delay_multiplier: 1,
    contact_damage: ContactDamage::UnlessTiny,
    split_count: random_split_count,
    spawn_size: random_spawn_size,
    register_goals: register_hostile_goals,
    is_magma_cube: false,
    squish_decay: crate::entity::combat::SLIME_SQUISH_DECAY,
};

pub struct SlimeEntity;

impl SlimeEntity {
    #[must_use]
    pub fn new(entity: crate::entity::Entity) -> std::sync::Arc<dyn crate::entity::EntityBase> {
        crate::entity::mob::cube_mob::CubeMobEntity::new(entity, &SLIME)
    }
}

#[must_use]
fn surface_slime_spawn_succeeds(
    biome: &Biome,
    y: i32,
    spawn_chance: f32,
    brightness: u8,
    spawn_roll: f32,
    brightness_roll: u8,
) -> bool {
    biome.has_tag(&pumpkin_data::tag::WorldgenBiome::MINECRAFT_ALLOWS_SURFACE_SLIME_SPAWNS)
        && (51..=69).contains(&y)
        && spawn_roll < spawn_chance
        && brightness <= brightness_roll
}

pub fn check_slime_spawn_rules(world: &World, pos: &BlockPos) -> bool {
    if world.level_info.load().difficulty == Difficulty::Peaceful {
        return false;
    }

    if !crate::entity::mob::MobEntity::check_mob_spawn_rules(world, pos) {
        return false;
    }

    let biome = world.get_biome(pos);
    let brightness = world.get_max_local_raw_brightness(pos);
    if surface_slime_spawn_succeeds(
        &biome,
        pos.0.y,
        world.surface_slime_spawn_chance(pos),
        brightness,
        rand::random_range(0.0..1.0),
        rand::random_range(0..=7),
    ) {
        return true;
    }

    let chunk_pos = pos.chunk_position();
    let slime_seed = pumpkin_util::random::seed_slime_chunk(
        chunk_pos.x,
        chunk_pos.y,
        world.level.seed.0,
        987_234_911,
    );
    let mut slime_rand = pumpkin_util::random::legacy_rand::LegacyRand::from_seed(slime_seed);
    rand::random_range(0..10) == 0 && slime_rand.next_bounded_i32(10) == 0 && pos.0.y < 40
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_slime_spawn_rules_cover_biomes_height_light_and_moon_roll() {
        assert!(surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            51,
            0.5,
            0,
            0.49,
            0,
        ));
        assert!(surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::MANGROVE_SWAMP,
            69,
            0.5,
            7,
            0.49,
            7,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            50,
            0.5,
            0,
            0.0,
            0,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            70,
            0.5,
            0,
            0.0,
            0,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::PLAINS,
            60,
            0.5,
            0,
            0.0,
            0,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            60,
            0.5,
            0,
            0.5,
            0,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            60,
            0.5,
            8,
            0.0,
            7,
        ));
        assert!(!surface_slime_spawn_succeeds(
            &pumpkin_data::biome::Biome::SWAMP,
            60,
            0.0,
            0,
            0.0,
            0,
        ));
    }
}
