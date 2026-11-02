use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase, EntityDimensions,
    ai::control::fish_move_control::FishMoveControl,
    ai::goal::{
        avoid_entity::AvoidEntityGoal, escape_danger::EscapeDangerGoal,
        random_swimming::RandomSwimmingGoal,
    },
    ai::pathfinder::{node::PathType, Navigator},
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::fish,
};

/// Represents Salmon size variant.
/// Mirrors vanilla `Salmon.java:156-183` (`Salmon.Variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SalmonVariant {
    Small = 0,
    #[default]
    Medium = 1,
    Large = 2,
}

impl SalmonVariant {
    pub const DEFAULT: Self = Self::Medium;

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Small,
            2 => Self::Large,
            _ => Self::Medium,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }

    #[must_use]
    pub fn scale(self) -> f32 {
        crate::entity::combat::calculate_salmon_scale(self.id())
    }
}

pub struct SalmonEntity {
    pub mob_entity: MobEntity,
    pub air_supply: AtomicI32,
    variant: AtomicI32,
    pub from_bucket: AtomicBool,
}

impl SalmonEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::FISH_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FishMoveControl::default());

        let mut navigator = Navigator::water_bound(false);
        navigator.set_pathfinding_malus(PathType::Water, 0.0);
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;

        let salmon = Self {
            mob_entity,
            air_supply: AtomicI32::new(crate::entity::combat::FISH_TOTAL_AIR_SUPPLY),
            variant: AtomicI32::new(SalmonVariant::DEFAULT.id()),
            from_bucket: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(salmon);
        mob_arc.refresh_dimensions();

        // Vanilla AbstractFish: MAX_HEALTH = 3.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::FISH_MAX_HEALTH as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::FISH_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Priority 0: PanicGoal (EscapeDangerGoal 1.25)
            goal_selector.add_goal(0, EscapeDangerGoal::new(1.25));
            // Priority 2: AvoidEntityGoal (Player, 8.0, 1.6, 1.4)
            goal_selector.add_goal(
                2,
                Box::new(AvoidEntityGoal::new(&EntityType::PLAYER, 8.0, 1.6, 1.4)),
            );
            // Priority 4: FishSwimGoal (RandomSwimmingGoal 1.0)
            goal_selector.add_goal(4, Box::new(RandomSwimmingGoal::new(1.0)));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> SalmonVariant {
        SalmonVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: SalmonVariant) {
        if self.variant.swap(variant.id(), Ordering::Relaxed) == variant.id() {
            return;
        }
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::salmon::DATA_TYPE, VarInt(variant.id()));
        self.refresh_dimensions();
    }

    #[must_use]
    pub fn get_salmon_scale(&self) -> f32 {
        self.get_variant().scale()
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let (width, height, eye_height) = crate::entity::combat::calculate_salmon_dimensions(
            self.get_variant().id(),
            entity.entity_type.dimension[0],
            entity.entity_type.dimension[1],
            entity.entity_type.eye_height,
        );

        let dimensions = EntityDimensions {
            width,
            height,
            eye_height,
        };

        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    #[must_use]
    pub fn is_from_bucket(&self) -> bool {
        self.from_bucket.load(Ordering::Relaxed)
    }

    pub fn set_from_bucket(&self, from_bucket: bool) {
        self.from_bucket.store(from_bucket, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::salmon::FROM_BUCKET, from_bucket);
    }

    pub fn finalize_spawn(&self, rng: &mut impl rand::Rng) {
        let roll: u32 = rng.random_range(0..crate::entity::combat::SALMON_TOTAL_SPAWN_WEIGHT);
        let variant = if roll < crate::entity::combat::SALMON_SPAWN_WEIGHT_SMALL {
            SalmonVariant::Small
        } else if roll < crate::entity::combat::SALMON_SPAWN_WEIGHT_SMALL + crate::entity::combat::SALMON_SPAWN_WEIGHT_MEDIUM {
            SalmonVariant::Medium
        } else {
            SalmonVariant::Large
        };
        self.set_variant(variant);
    }
}

impl Mob for SalmonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_salmon(&self) -> Option<&SalmonEntity> {
        Some(self)
    }

    fn mob_init_data_tracker(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_synced_data(
            tracked_data::salmon::DATA_TYPE,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            tracked_data::salmon::FROM_BUCKET,
            self.is_from_bucket(),
        );
    }

    fn mob_travel_in_water(&self, caller: &dyn EntityBase) -> bool {
        fish::travel_in_water(self, caller)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn dries_out_on_land(&self) -> bool {
        true
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let in_water =
            entity.touching_water.load(Ordering::Relaxed) || entity.is_in_water();

        if !in_water && entity.on_ground.load(Ordering::Relaxed) {
            let mut rng = rand::rng();
            let dx = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let dz = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let velo = entity.velocity.load();
            entity.set_velocity(Vector3::new(velo.x + dx as f64, 0.4, velo.z + dz as f64));
            entity.on_ground.store(false, Ordering::Relaxed);
            if !entity.silent.load(Ordering::Relaxed) {
                entity.world.load().play_sound_fine(
                    Sound::EntitySalmonFlop,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        } else if in_water
            && !entity.silent.load(Ordering::Relaxed)
            && rand::random_range(0..100) == 0
        {
            entity.world.load().play_sound_fine(
                Sound::EntitySalmonAmbient,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("type", self.get_variant().name().to_string());
        nbt.put_bool("FromBucket", self.is_from_bucket());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("type") {
            self.set_variant(SalmonVariant::from_name(variant_str));
        } else if let Some(variant_id) = nbt.get_int("type") {
            self.set_variant(SalmonVariant::from_id(variant_id));
        }
        if let Some(from_bucket) = nbt.get_bool("FromBucket") {
            self.set_from_bucket(from_bucket);
        }
        self.refresh_dimensions();
    }
}

impl CustomSound for SalmonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySalmonDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySalmonHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat;

    #[test]
    fn test_salmon_parity_constants() {
        assert_eq!(SalmonEntity::XP_REWARD, 2);
        assert_eq!(combat::FISH_MAX_HEALTH, 3.0);
        assert_eq!(combat::SALMON_SMALL_SCALE, 0.5);
        assert_eq!(combat::SALMON_MEDIUM_SCALE, 1.0);
        assert_eq!(combat::SALMON_LARGE_SCALE, 1.5);
        assert_eq!(combat::SALMON_MAX_SCHOOL_SIZE, 5);
        assert_eq!(combat::SALMON_DEFAULT_WIDTH, 0.7);
        assert_eq!(combat::SALMON_DEFAULT_HEIGHT, 0.4);
        assert_eq!(combat::SALMON_DEFAULT_EYE_HEIGHT, 0.26);
    }

    #[test]
    fn test_salmon_variants_and_scales() {
        assert_eq!(SalmonVariant::Small.id(), 0);
        assert_eq!(SalmonVariant::Medium.id(), 1);
        assert_eq!(SalmonVariant::Large.id(), 2);

        assert_eq!(SalmonVariant::Small.scale(), 0.5);
        assert_eq!(SalmonVariant::Medium.scale(), 1.0);
        assert_eq!(SalmonVariant::Large.scale(), 1.5);

        assert_eq!(SalmonVariant::from_name("small"), SalmonVariant::Small);
        assert_eq!(SalmonVariant::from_name("medium"), SalmonVariant::Medium);
        assert_eq!(SalmonVariant::from_name("large"), SalmonVariant::Large);
        assert_eq!(SalmonVariant::from_name("unknown"), SalmonVariant::Medium);

        assert_eq!(SalmonVariant::Small.name(), "small");
        assert_eq!(SalmonVariant::Medium.name(), "medium");
        assert_eq!(SalmonVariant::Large.name(), "large");
    }

    #[test]
    fn test_salmon_dimensions_scaling() {
        let (w_s, h_s, eye_s) = combat::calculate_salmon_dimensions(0, 0.7, 0.4, 0.26);
        assert_eq!(w_s, 0.35);
        assert_eq!(h_s, 0.2);
        assert_eq!(eye_s, 0.13);

        let (w_m, h_m, eye_m) = combat::calculate_salmon_dimensions(1, 0.7, 0.4, 0.26);
        assert_eq!(w_m, 0.7);
        assert_eq!(h_m, 0.4);
        assert_eq!(eye_m, 0.26);

        let (w_l, h_l, eye_l) = combat::calculate_salmon_dimensions(2, 0.7, 0.4, 0.26);
        assert_eq!(w_l, 1.05);
        assert_eq!(h_l, 0.6);
        assert_eq!(eye_l, 0.39);
    }

    #[test]
    fn test_salmon_nbt_roundtrip() {
        let mut nbt = NbtCompound::new();
        nbt.put_string("type", "large".to_string());
        nbt.put_bool("FromBucket", true);

        assert_eq!(nbt.get_string("type"), Some("large"));
        assert_eq!(nbt.get_bool("FromBucket"), Some(true));

        assert_eq!(SalmonVariant::from_name(nbt.get_string("type").unwrap()), SalmonVariant::Large);
    }
}
