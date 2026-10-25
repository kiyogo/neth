use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::{EntityPose, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::optional_int::OptionalInt;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal,
        active_target::ActiveTargetGoal,
        breed::BreedGoal, escape_danger::EscapeDangerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    ai::pathfinder::NavigatorGoal,
    combat,
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

use pumpkin_data::frog_variant::FrogVariant;

pub const FROG_FOOD: &[&Item] = &[&Item::SLIME_BALL];

/// Represents a Frog, an amphibious mob that can eat small slimes and magma cubes.
///
/// Wiki: <https://minecraft.wiki/w/Frog>
pub struct FrogEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub tongue_target_id: AtomicI32,
    pub is_pregnant: AtomicBool,
}

impl FrogEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let frog = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(FrogVariant::Temperate.id() as i32),
            tongue_target_id: AtomicI32::new(-1),
            is_pregnant: AtomicBool::new(false),
        };


        // Initialize attributes: Max Health = 10.0, Movement Speed = 1.0, Attack Damage = 10.0, Step Height = 1.0
        {
            let mut attributes = frog
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
                speed.base_value = 1.0;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 10.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(step) = attributes.get_mut(&Attributes::STEP_HEIGHT.id) {
                step.base_value = 1.0;
                step.dirty.store(true, Ordering::Relaxed);
            }
        }
        frog.mob_entity.living_entity.health.store(10.0);

        let mob_arc = Arc::new(frog);
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

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, Box::new(FrogEatGoal::new()));
            goal_selector.add_goal(3, BreedGoal::new(1.0));
            goal_selector.add_goal(4, Box::new(TemptGoal::new(1.25, FROG_FOOD, false)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(
                1,
                ActiveTargetGoal::predicated(&mob_arc.mob_entity, 10, true, |living, _world| {
                    combat::can_frog_eat_entity(
                        living.entity.entity_type,
                        living.entity.data.load(Ordering::Relaxed),
                    )
                }),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> FrogVariant {
        FrogVariant::from_id(self.variant.load(Ordering::Relaxed) as u32)
    }

    pub fn set_variant(&self, variant: FrogVariant) {
        self.variant.store(variant.id() as i32, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::frog::VARIANT,
            VarInt(variant.id() as i32),
        );
    }

    #[must_use]
    pub fn is_pregnant(&self) -> bool {
        self.is_pregnant.load(Ordering::Relaxed)
    }

    pub fn set_pregnant(&self, pregnant: bool) {
        self.is_pregnant.store(pregnant, Ordering::Relaxed);
    }
}

impl AgeableMob for FrogEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    /// Vanilla `Frog.java:251-253`: `canBeABaby() -> false`. Frogs cannot be babies;
    /// their offspring stage is Tadpole.
    fn can_be_a_baby(&self) -> bool {
        false
    }
}

impl Animal for FrogEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FROG_FOOD)
            || FROG_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for FrogEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_frog(&self) -> Option<&FrogEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("variant", self.get_variant().asset_id().to_string());
        nbt.put_bool("HasEgg", self.is_pregnant());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("variant") {
            self.set_variant(FrogVariant::from_name(variant_str).unwrap_or_default());
        }
        if let Some(pregnant) = nbt.get_bool("HasEgg").or_else(|| nbt.get_bool("IsPregnant")) {
            self.set_pregnant(pregnant);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.set_variant(FrogVariant::from_name(name).unwrap_or_default());
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla TryLaySpawnOnFluidNearLand.java:23-39: pregnant frog on ground near liquid lays spawn
        if self.is_pregnant() {
            let on_ground = entity.on_ground.load(Ordering::Relaxed);
            let in_water = entity.touching_water.load(Ordering::Relaxed) || entity.is_in_water();
            if combat::should_frog_lay_spawn_block(true, on_ground, in_water) {

                let world = entity.world.load();
                world.play_sound_fine(
                    Sound::EntityFrogLaySpawn,
                    SoundCategory::Blocks,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
                self.set_pregnant(false);
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            entity.world.load().play_sound_fine(
                Sound::EntityFrogAmbient,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                if entity.age.load(Ordering::Relaxed) < 0 {
                    1.25
                } else {
                    1.0
                },
            );
        }
    }


    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::frog::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::frog::VARIANT,
            VarInt(self.get_variant().id() as i32),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let is_lead = item_stack.item.registry_key == "lead"
            || item_stack.item.registry_key == "minecraft:lead";
        let was_leashed = self
            .get_entity()
            .leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();

        let res = self.animal_interact(player, item_stack, Sound::EntityFrogAmbient);

        if res && is_lead && !was_leashed {
            let now_leashed = self
                .get_entity()
                .leashed_to
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some();
            if now_leashed {
                let variant_id = self.get_variant().asset_id().to_string();
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::LeashedFrog {
                        variant: variant_id,
                    },
                );
            }
        }
        res
    }

    fn get_max_head_rotation(&self) -> f32 {
        5.0
    }

    fn can_drown(&self) -> bool {
        false
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    /// Vanilla `Frog.calculateFallDamage`: fall damage is reduced by 5.
    fn calculate_fall_damage(&self, fall_distance: f64, damage_modifier: f32) -> i32 {
        combat::calculate_frog_fall_damage_points(fall_distance, damage_modifier)
    }

    fn adjust_damage_amount(&self, amount: f32, damage_type: &DamageType) -> f32 {
        if *damage_type == DamageType::FALL {
            (amount - 5.0).max(0.0)
        } else {
            amount
        }
    }
}

impl CustomSound for FrogEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityFrogDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityFrogHurt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrogEatState {
    MoveToTarget,
    CatchAnimation,
    EatAnimation,
    Done,
}

/// AI goal for Frog to shoot its tongue, pull and eat small slimes and magma cubes.
///
/// Mirrors vanilla `ShootTongue.java` & `FrogAi.java`.
pub struct FrogEatGoal {
    goal_control: Controls,
    target: Option<Arc<dyn EntityBase>>,
    state: FrogEatState,
    eat_animation_timer: i32,
    calculate_path_counter: i32,
}

impl Default for FrogEatGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl FrogEatGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            goal_control: Controls::MOVE.union(Controls::LOOK),
            target: None,
            state: FrogEatState::Done,
            eat_animation_timer: 0,
            calculate_path_counter: 0,
        }
    }
}

impl Goal for FrogEatGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };

        if !target.get_entity().is_alive() {
            return false;
        }

        let size = target.get_entity().data.load(Ordering::Relaxed);
        if !combat::can_frog_eat_entity(target.get_entity().entity_type, size) {
            return false;
        }

        self.target = Some(target);
        self.state = FrogEatState::MoveToTarget;
        self.calculate_path_counter = 10;
        self.eat_animation_timer = 0;
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(target) = &self.target else {
            return false;
        };

        if !target.get_entity().is_alive() {
            return false;
        }

        self.state != FrogEatState::Done
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = &self.target {
            let target_id = target.get_entity().entity_id;
            if let Some(frog) = mob.as_frog() {
                frog.tongue_target_id.store(target_id, Ordering::Relaxed);
            }
            mob.get_entity().set_synced_data(
                pumpkin_data::tracked_data::frog::DATA_TONGUE_TARGET_ID,
                OptionalInt(Some(target_id)),
            );
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        mob.get_entity().set_pose(EntityPose::Standing);
        mob.get_entity().set_synced_data(
            pumpkin_data::tracked_data::frog::DATA_TONGUE_TARGET_ID,
            OptionalInt(None),
        );
        if let Some(frog) = mob.as_frog() {
            frog.tongue_target_id.store(-1, Ordering::Relaxed);
        }
        mob.get_mob_entity().set_target(None);
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.stop();
        self.target = None;
        self.state = FrogEatState::Done;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = &self.target else {
            return;
        };

        let mob_entity = mob.get_mob_entity();
        mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity_with_range(target, 45.0, 90.0);

        let frog_pos = mob.get_entity().pos.load();
        let target_pos = target.get_entity().pos.load();

        match self.state {
            FrogEatState::MoveToTarget => {
                let dist = frog_pos.squared_distance_to_vec(&target_pos).sqrt();
                if combat::should_frog_start_eating(dist) {
                    let world = mob.get_entity().world.load();
                    world.play_sound_fine(
                        Sound::EntityFrogTongue,
                        SoundCategory::Neutral,
                        &frog_pos,
                        2.0,
                        1.0,
                    );
                    mob.get_entity().set_pose(EntityPose::UsingTongue);
                    let pull_vel = combat::calculate_frog_tongue_pull_velocity(target_pos, frog_pos);
                    target.get_entity().velocity.store(pull_vel);
                    self.eat_animation_timer = 0;
                    self.state = FrogEatState::CatchAnimation;
                } else if self.calculate_path_counter <= 0 {
                    let mut navigator = mob_entity
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    navigator.set_progress(NavigatorGoal::new(frog_pos, target_pos, 2.0));
                    self.calculate_path_counter = 10;
                } else {
                    self.calculate_path_counter -= 1;
                }
            }
            FrogEatState::CatchAnimation => {
                self.eat_animation_timer += 1;
                if self.eat_animation_timer >= 6 {
                    self.state = FrogEatState::EatAnimation;
                    let world = mob.get_entity().world.load();
                    world.play_sound_fine(
                        Sound::EntityFrogEat,
                        SoundCategory::Neutral,
                        &frog_pos,
                        2.0,
                        1.0,
                    );

                    let victim_type = target.get_entity().entity_type;
                    let variant = mob.as_frog().map(|f| f.get_variant()).unwrap_or_default();
                    if let Some(drop_item) = combat::get_frog_drop_item(victim_type, variant) {
                        let drop_entity = Entity::new(world.clone(), target_pos, &EntityType::ITEM);
                        world.spawn_entity(Arc::new(ItemEntity::new(
                            drop_entity,
                            ItemStack::new(1, drop_item),
                        )));
                    }
                    target.get_entity().kill(target.as_ref());
                }
            }
            FrogEatState::EatAnimation => {
                self.eat_animation_timer += 1;
                if self.eat_animation_timer >= 10 {
                    self.state = FrogEatState::Done;
                }
            }
            FrogEatState::Done => {}
        }
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frog_variants_and_oviparous() {
        assert_eq!(FrogVariant::Cold.id(), 0);
        assert_eq!(FrogVariant::Temperate.id(), 1);
        assert_eq!(FrogVariant::Warm.id(), 2);
        assert_eq!(FrogVariant::from_id(0), FrogVariant::Cold);
        assert_eq!(FrogVariant::from_id(1), FrogVariant::Temperate);
        assert_eq!(FrogVariant::from_id(2), FrogVariant::Warm);
    }
}


