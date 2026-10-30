use pumpkin_data::attributes::Attributes;
use pumpkin_data::sound::Sound;

use crate::entity::combat::{
    MAGMA_CUBE_ARMOR_PER_SIZE, MAGMA_CUBE_ATTACK_DAMAGE_BONUS, MAGMA_CUBE_JUMP_DELAY_MULTIPLIER,
    MAGMA_CUBE_SQUISH_DECAY,
};
use crate::entity::mob::cube_mob::{ContactDamage, CubeKind, CubeSounds, SizedSound};
use crate::entity::mob::slime::SLIME;

pub const MAGMA_CUBE: CubeKind = CubeKind {
    sounds: CubeSounds {
        jump: SizedSound {
            normal: Sound::EntityMagmaCubeJump,
            small: Sound::EntityMagmaCubeJump,
        },
        squish: SizedSound {
            normal: Sound::EntityMagmaCubeSquish,
            small: Sound::EntityMagmaCubeSquishSmall,
        },
        hurt: SizedSound {
            normal: Sound::EntityMagmaCubeHurt,
            small: Sound::EntityMagmaCubeHurtSmall,
        },
        death: SizedSound {
            normal: Sound::EntityMagmaCubeDeath,
            small: Sound::EntityMagmaCubeDeathSmall,
        },
    },
    // Vanilla adds the 2 in getAttackDamage instead of the attribute.
    size_attributes: &[
        (
            Attributes::ATTACK_DAMAGE,
            |size| f64::from(size as f32 + MAGMA_CUBE_ATTACK_DAMAGE_BONUS),
        ),
        (
            Attributes::ARMOR,
            |size| f64::from(size as f32 * MAGMA_CUBE_ARMOR_PER_SIZE),
        ),
    ],
    jump_delay_multiplier: MAGMA_CUBE_JUMP_DELAY_MULTIPLIER,
    contact_damage: ContactDamage::Always,
    is_magma_cube: true,
    squish_decay: MAGMA_CUBE_SQUISH_DECAY,
    ..SLIME
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magma_cube_attributes_and_jump_parity() {
        assert_eq!(MAGMA_CUBE.jump_delay_multiplier, 4);
        assert_eq!(MAGMA_CUBE.squish_decay, 0.9);
        assert!(MAGMA_CUBE.is_magma_cube);
        let attack_fn = MAGMA_CUBE.size_attributes[0].1;
        assert_eq!(attack_fn(1), 3.0);
        assert_eq!(attack_fn(2), 4.0);
        assert_eq!(attack_fn(4), 6.0);
        let armor_fn = MAGMA_CUBE.size_attributes[1].1;
        assert_eq!(armor_fn(1), 3.0);
        assert_eq!(armor_fn(2), 6.0);
        assert_eq!(armor_fn(4), 12.0);
    }
}
