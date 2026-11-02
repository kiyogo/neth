use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    item::ItemEntity,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const ARMADILLO_FOOD: &[&Item] = &[&Item::SPIDER_EYE];
pub const ARMADILLO_BABY_START_AGE: i32 = -48000;
pub const SCARE_CHECK_INTERVAL: i32 = crate::entity::combat::ARMADILLO_SCARE_CHECK_INTERVAL;
pub const SCARE_DISTANCE_HORIZONTAL: f64 = crate::entity::combat::ARMADILLO_SCARE_DISTANCE_HORIZONTAL;
pub const SCARE_DISTANCE_VERTICAL: f64 = crate::entity::combat::ARMADILLO_SCARE_DISTANCE_VERTICAL;
pub const BALL_UP_STAY_IN_STATE: u64 = 6000;
pub const BRUSH_DURABILITY_DAMAGE: i32 = crate::entity::combat::ARMADILLO_BRUSH_DURABILITY_DAMAGE;
pub const PEEK_EVENT_ID: u8 = crate::entity::combat::ARMADILLO_PEEK_EVENT_ID;
pub const UNROLL_START_THRESHOLD_TICKS: u64 = crate::entity::combat::ARMADILLO_UNROLL_DURATION_TICKS;
pub const DANGER_THRESHOLD: i32 = crate::entity::combat::ARMADILLO_DANGER_THRESHOLD;
pub const STEP_SOUND_VOLUME: f32 = crate::entity::combat::ARMADILLO_STEP_SOUND_VOLUME;
pub const STEP_SOUND_PITCH: f32 = crate::entity::combat::ARMADILLO_STEP_SOUND_PITCH;

fn pick_next_scute_drop_time() -> i32 {
    let rand_ticks = (rand::random::<u32>() % 6000) as i32;
    rand_ticks + 6000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ArmadilloState {
    #[default]
    Idle = 0,
    Rolling = 1,
    Scared = 2,
    Unrolling = 3,
}

impl ArmadilloState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Rolling,
            2 => Self::Scared,
            3 => Self::Unrolling,
            _ => Self::Idle,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn is_threatened(self) -> bool {
        !matches!(self, Self::Idle)
    }

    #[must_use]
    pub const fn animation_duration(self) -> u64 {
        match self {
            Self::Idle => 0,
            Self::Rolling => 10,
            Self::Scared => 50,
            Self::Unrolling => 30,
        }
    }

    #[must_use]
    pub const fn should_hide_in_shell(self, ticks_in_state: u64) -> bool {
        match self {
            Self::Idle => false,
            Self::Rolling => ticks_in_state > 5,
            Self::Scared => true,
            Self::Unrolling => ticks_in_state < 26,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Rolling => "rolling",
            Self::Scared => "scared",
            Self::Unrolling => "unrolling",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "rolling" => Self::Rolling,
            "scared" => Self::Scared,
            "unrolling" => Self::Unrolling,
            _ => Self::Idle,
        }
    }
}

/// Represents an Armadillo, a passive entity that can roll into a ball when threatened.
///
/// Wiki: <https://minecraft.wiki/w/Armadillo>
pub struct ArmadilloEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub state: AtomicI32,
    pub in_state_ticks: AtomicU64,
    pub scute_time: AtomicI32,
    pub danger_detected_recently_ticks: AtomicI32,
    pub next_peek_timer: AtomicI32,
    pub danger_was_around: AtomicBool,
}

impl ArmadilloEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::ARMADILLO_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let armadillo = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            state: AtomicI32::new(ArmadilloState::Idle.id()),
            in_state_ticks: AtomicU64::new(0),
            scute_time: AtomicI32::new(pick_next_scute_drop_time()),
            danger_detected_recently_ticks: AtomicI32::new(0),
            next_peek_timer: AtomicI32::new(0),
            danger_was_around: AtomicBool::new(false),
        };

        // Initialize attributes: Max Health = 12.0, Movement Speed = 0.14
        armadillo
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::ARMADILLO_MAX_HEALTH as f64);
        armadillo
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::ARMADILLO_BASE_MOVEMENT_SPEED);
        armadillo
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::ARMADILLO_MAX_HEALTH);

        let mob_arc = Arc::new(armadillo);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new_environmental(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, ARMADILLO_FOOD, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let is_baby = self.is_baby();
        let (w, h, eye) = crate::entity::combat::calculate_armadillo_dimensions(
            is_baby,
            entity.entity_type.dimension[0],
            entity.entity_type.dimension[1],
            entity.entity_type.eye_height,
        );
        let dimensions = EntityDimensions {
            width: w,
            height: h,
            eye_height: eye,
        };
        let pos = entity.pos.load();
        entity.bounding_box.store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    #[must_use]
    pub fn get_state(&self) -> ArmadilloState {
        ArmadilloState::from_id(self.state.load(Ordering::Relaxed))
    }

    pub fn switch_to_state(&self, state: ArmadilloState) {
        self.state.store(state.id(), Ordering::Relaxed);
        self.in_state_ticks.store(0, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::armadillo::ARMADILLO_STATE,
            VarInt(state.id()),
        );
    }

    #[must_use]
    pub fn is_scared(&self) -> bool {
        self.get_state() != ArmadilloState::Idle
    }

    #[must_use]
    pub fn should_hide_in_shell(&self) -> bool {
        self.get_state()
            .should_hide_in_shell(self.in_state_ticks.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn should_switch_to_scared_state(&self) -> bool {
        self.get_state() == ArmadilloState::Rolling
            && self.in_state_ticks.load(Ordering::Relaxed)
                > ArmadilloState::Rolling.animation_duration()
    }

    pub const fn can_stay_rolled_up_predicate(
        is_panicking: bool,
        in_liquid: bool,
        is_leashed: bool,
        is_passenger: bool,
        is_vehicle: bool,
    ) -> bool {
        !is_panicking && !in_liquid && !is_leashed && !is_passenger && !is_vehicle
    }

    pub const fn is_scared_by_predicate(
        is_undead: bool,
        is_last_attacker: bool,
        is_player: bool,
        is_spectator: bool,
        is_sprinting: bool,
        is_passenger: bool,
    ) -> bool {
        if is_undead || is_last_attacker {
            true
        } else if is_player {
            if is_spectator {
                false
            } else {
                is_sprinting || is_passenger
            }
        } else {
            false
        }
    }

    pub fn can_stay_rolled_up(&self) -> bool {
        !self.is_panicking()
            && !self.mob_entity.living_entity.is_in_water()
            && !self.get_entity().has_vehicle()
    }

    pub fn roll_up(&self) {
        if !self.is_scared() {
            self.mob_entity.reset_love_ticks();
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityArmadilloRoll,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            self.switch_to_state(ArmadilloState::Rolling);
        }
    }

    pub fn roll_out(&self) {
        if self.is_scared() {
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityArmadilloUnrollFinish,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            self.switch_to_state(ArmadilloState::Idle);
        }
    }

    #[must_use]
    pub fn pick_next_peek_timer(&self) -> i32 {
        ArmadilloState::Scared.animation_duration() as i32
            + rand::random_range(
                crate::entity::combat::ARMADILLO_PEEK_MIN_TICKS
                    ..=crate::entity::combat::ARMADILLO_PEEK_MAX_TICKS,
            )
    }

    pub fn brush_off_scute(&self, player: &Arc<Player>) -> bool {
        if self.is_baby() {
            return false;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let item_entity = Arc::new(ItemEntity::new(
            Entity::new(world.clone(), pos, &EntityType::ITEM),
            ItemStack::new(1, &Item::ARMADILLO_SCUTE),
        ));
        world.spawn_entity(item_entity);
        world.play_sound(Sound::EntityArmadilloBrush, SoundCategory::Neutral, &pos);
        player.damage_held_item(BRUSH_DURABILITY_DAMAGE);
        true
    }

    pub fn is_scared_by(&self, living_entity: &dyn EntityBase) -> bool {
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let target_pos = living_entity.get_entity().pos.load();
        let dx = (pos.x - target_pos.x).abs();
        let dy = (pos.y - target_pos.y).abs();
        let dz = (pos.z - target_pos.z).abs();

        if dx > SCARE_DISTANCE_HORIZONTAL
            || dz > SCARE_DISTANCE_HORIZONTAL
            || dy > SCARE_DISTANCE_VERTICAL
        {
            return false;
        }

        let target_type = living_entity.get_entity().entity_type;
        if target_type.has_tag(&tag::EntityType::MINECRAFT_UNDEAD) {
            return true;
        }

        let last_attacker_id = self
            .mob_entity
            .living_entity
            .last_attacker_id
            .load(Ordering::Relaxed);
        if last_attacker_id != 0 && living_entity.get_entity().entity_id == last_attacker_id {
            return true;
        }

        if target_type == &EntityType::PLAYER {
            let target_ent = living_entity.get_entity();
            if let Some(player) = living_entity.get_player() {
                if player.gamemode.load() == pumpkin_util::GameMode::Spectator {
                    return false;
                }
            }
            if target_ent.is_sprinting() || target_ent.has_vehicle() {
                return true;
            }
        }

        false
    }
}

impl AgeableMob for ArmadilloEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn get_baby_start_age(&self) -> i32 {
        ARMADILLO_BABY_START_AGE
    }

    fn set_baby(&self, baby: bool) {
        self.set_age(if baby { self.get_baby_start_age() } else { 0 });
        self.refresh_dimensions();
    }

    fn age_boundary_reached(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        self.refresh_dimensions();
        crate::entity::ageable::handle_age_boundary_vehicle_dismount(self.is_baby(), entity);
    }
}

impl Animal for ArmadilloEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_ARMADILLO_FOOD)
            || ARMADILLO_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for ArmadilloEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_armadillo(&self) -> Option<&ArmadilloEntity> {
        Some(self)
    }

    fn can_be_leashed(&self) -> bool {
        true
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_string("state", self.get_state().name().to_string());
        nbt.put_int("scute_time", self.scute_time.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(state_name) = nbt.get_string("state") {
            self.switch_to_state(ArmadilloState::from_name(state_name));
        } else if let Some(state_id) = nbt.get_int("state") {
            self.switch_to_state(ArmadilloState::from_id(state_id));
        }
        if let Some(scute_time) = nbt.get_int("scute_time") {
            self.scute_time.store(scute_time, Ordering::Relaxed);
        }
        self.refresh_dimensions();
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn modify_incoming_damage(&self, amount: f32, _damage_type: DamageType) -> f32 {
        if self.is_scared() {
            crate::entity::combat::calculate_armadillo_scared_damage(amount)
        } else {
            amount
        }
    }

    fn on_damage(&self, damage_type: DamageType, source: Option<&dyn EntityBase>) {
        if self.get_entity().is_alive() {
            if let Some(src) = source {
                if src.get_entity().entity_type != &EntityType::ITEM {
                    self.danger_detected_recently_ticks
                        .store(SCARE_CHECK_INTERVAL, Ordering::Relaxed);
                    if self.can_stay_rolled_up() {
                        self.roll_up();
                    }
                }
            } else if crate::entity::combat::is_armadillo_panic_environmental_damage(&damage_type) {
                self.roll_out();
            }
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        self.in_state_ticks.fetch_add(1, Ordering::Relaxed);
        let danger_ticks = self.danger_detected_recently_ticks.load(Ordering::Relaxed);
        if danger_ticks > 0 {
            self.danger_detected_recently_ticks
                .store(danger_ticks - 1, Ordering::Relaxed);
        }

        let peek_timer = self.next_peek_timer.load(Ordering::Relaxed);
        if peek_timer > 0 {
            self.next_peek_timer
                .store(peek_timer - 1, Ordering::Relaxed);
        }

        let entity = self.get_entity();
        let world = entity.world.load();

        // Threat sensing every 5 ticks (mirrors SensorType.ARMADILLO_SCARE_DETECTED)
        let age = entity.age.load(Ordering::Relaxed);
        if age % 5 == 0 && entity.is_alive() && self.can_stay_rolled_up() {
            let pos = entity.pos.load();
            let mut threat_detected = false;

            for player in world.get_nearby_players(pos, SCARE_DISTANCE_HORIZONTAL) {
                if self.is_scared_by(player.as_ref()) {
                    threat_detected = true;
                    break;
                }
            }

            if !threat_detected {
                let nearby = world.get_nearby_entities(pos, SCARE_DISTANCE_HORIZONTAL);
                for (_, nearby_ent) in nearby {
                    if self.is_scared_by(nearby_ent.as_ref()) {
                        threat_detected = true;
                        break;
                    }
                }
            }

            if threat_detected {
                self.danger_detected_recently_ticks
                    .store(SCARE_CHECK_INTERVAL, Ordering::Relaxed);
                if self.get_state() == ArmadilloState::Idle {
                    self.roll_up();
                }
            }
        }

        // Natural scute shed drop (5 to 10 minutes)
        if entity.is_alive() && !self.is_baby() {
            let scute_time = self.scute_time.fetch_sub(1, Ordering::Relaxed) - 1;
            if scute_time <= 0 {
                let pos = entity.pos.load();
                let item_entity = Arc::new(ItemEntity::new(
                    Entity::new(world.clone(), pos, &EntityType::ITEM),
                    ItemStack::new(1, &Item::ARMADILLO_SCUTE),
                ));
                world.spawn_entity_non_save(item_entity as Arc<dyn EntityBase>);
                world.play_sound(
                    Sound::EntityArmadilloScuteDrop,
                    SoundCategory::Neutral,
                    &pos,
                );
                self.scute_time
                    .store(pick_next_scute_drop_time(), Ordering::Relaxed);
            }
        }

        // State machine updates (mirrors ArmadilloAi.java:143-171)
        if self.should_switch_to_scared_state() {
            self.switch_to_state(ArmadilloState::Scared);
            if entity.on_ground.load(Ordering::Relaxed) {
                world.play_sound(
                    Sound::EntityArmadilloLand,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
            }
        } else {
            let state = self.get_state();
            let current_danger_ticks = self.danger_detected_recently_ticks.load(Ordering::Relaxed);
            let danger_is_around = current_danger_ticks > DANGER_THRESHOLD;
            let was_around = self.danger_was_around.load(Ordering::Relaxed);

            if danger_is_around != was_around {
                self.next_peek_timer
                    .store(self.pick_next_peek_timer(), Ordering::Relaxed);
            }
            self.danger_was_around
                .store(danger_is_around, Ordering::Relaxed);

            match state {
                ArmadilloState::Scared => {
                    if !self.can_stay_rolled_up() {
                        self.roll_out();
                    } else {
                        if self.next_peek_timer.load(Ordering::Relaxed) == 0
                            && entity.on_ground.load(Ordering::Relaxed)
                            && danger_is_around
                        {
                            world.broadcast_entity_status_id(&entity, PEEK_EVENT_ID as i8);
                            self.next_peek_timer
                                .store(self.pick_next_peek_timer(), Ordering::Relaxed);
                        }

                        if (current_danger_ticks as u64) < ArmadilloState::Unrolling.animation_duration() {
                            world.play_sound(
                                Sound::EntityArmadilloUnrollStart,
                                SoundCategory::Neutral,
                                &entity.pos.load(),
                            );
                            self.switch_to_state(ArmadilloState::Unrolling);
                        }
                    }
                }
                ArmadilloState::Unrolling => {
                    if (current_danger_ticks as u64) > ArmadilloState::Unrolling.animation_duration() {
                        self.switch_to_state(ArmadilloState::Scared);
                    } else if self.in_state_ticks.load(Ordering::Relaxed) > ArmadilloState::Unrolling.animation_duration() {
                        self.roll_out();
                    }
                }
                ArmadilloState::Rolling | ArmadilloState::Idle => {}
            }
        }

        if entity.is_alive()
            && !entity.silent.load(Ordering::Relaxed)
            && !self.is_scared()
            && rand::random_range(0..100) == 0
        {
            world.play_sound_fine(
                Sound::EntityArmadilloAmbient,
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
            entity.set_synced_data(pumpkin_data::tracked_data::armadillo::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::armadillo::ARMADILLO_STATE,
            VarInt(self.get_state().id()),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.item == &Item::BRUSH && self.brush_off_scute(player) {
            return true;
        }
        if self.is_scared() {
            return false;
        }
        self.animal_interact(player, item_stack, Sound::EntityArmadilloAmbient)
    }

    fn get_max_head_rotation(&self) -> f32 {
        if self.is_scared() { 0.0 } else { 32.5 }
    }

    fn adjust_damage_amount(&self, amount: f32, _damage_type: &DamageType) -> f32 {
        if self.is_scared() {
            crate::entity::combat::calculate_armadillo_scared_damage(amount)
        } else {
            amount
        }
    }
}

impl CustomSound for ArmadilloEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityArmadilloDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(if self.is_scared() {
            Sound::EntityArmadilloHurtReduced
        } else {
            Sound::EntityArmadilloHurt
        })
    }

    fn step_sound(&self) -> Option<Sound> {
        Some(Sound::EntityArmadilloStep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armadillo_scared_damage_reduction() {
        assert_eq!(ArmadilloState::from_id(0), ArmadilloState::Idle);
        assert_eq!(ArmadilloState::from_id(1), ArmadilloState::Rolling);
        assert_eq!(ArmadilloState::from_id(2), ArmadilloState::Scared);
        assert_eq!(ArmadilloState::from_id(3), ArmadilloState::Unrolling);

        // Vanilla Armadillo.java:279-283: damage = (damage - 1.0F) / 2.0F
        assert_eq!(crate::entity::combat::calculate_armadillo_scared_damage(1.0), 0.0);
        assert_eq!(crate::entity::combat::calculate_armadillo_scared_damage(5.0), 2.0);
        assert_eq!(crate::entity::combat::calculate_armadillo_scared_damage(11.0), 5.0);
        assert_eq!(crate::entity::combat::calculate_armadillo_scared_damage(0.0), 0.0);
    }

    #[test]
    fn test_armadillo_parity_constants() {
        assert_eq!(ArmadilloEntity::XP_REWARD, 2);
        assert_eq!(crate::entity::combat::ARMADILLO_MAX_HEALTH, 12.0);
        assert_eq!(crate::entity::combat::ARMADILLO_BASE_MOVEMENT_SPEED, 0.14);
        assert_eq!(crate::entity::combat::ARMADILLO_ROLL_DURATION_TICKS, 10);
        assert_eq!(crate::entity::combat::ARMADILLO_SCARED_DURATION_TICKS, 50);
        assert_eq!(crate::entity::combat::ARMADILLO_UNROLL_DURATION_TICKS, 30);
        assert_eq!(STEP_SOUND_VOLUME, 0.15);
        assert_eq!(STEP_SOUND_PITCH, 1.0);
        assert_eq!(BRUSH_DURABILITY_DAMAGE, 16);
        assert_eq!(DANGER_THRESHOLD, 75);
        assert_eq!(SCARE_CHECK_INTERVAL, 80);
        assert_eq!(SCARE_DISTANCE_HORIZONTAL, 7.0);
        assert_eq!(SCARE_DISTANCE_VERTICAL, 2.0);
        assert_eq!(PEEK_EVENT_ID, 64);
        assert_eq!(crate::entity::combat::ARMADILLO_PEEK_MIN_TICKS, 100);
        assert_eq!(crate::entity::combat::ARMADILLO_PEEK_MAX_TICKS, 400);
        assert_eq!(crate::entity::combat::ARMADILLO_MIN_SCUTE_DROP_TICKS, 6000);
        assert_eq!(crate::entity::combat::ARMADILLO_MAX_SCUTE_DROP_TICKS, 12000);
    }

    #[test]
    fn test_armadillo_roll_up_scared_mechanics_and_threat_parity() {
        // State durations
        assert_eq!(ArmadilloState::Idle.animation_duration(), 0);
        assert_eq!(ArmadilloState::Rolling.animation_duration(), 10);
        assert_eq!(ArmadilloState::Scared.animation_duration(), 50);
        assert_eq!(ArmadilloState::Unrolling.animation_duration(), 30);

        // Shell hiding states
        assert!(!ArmadilloState::Idle.should_hide_in_shell(0));
        assert!(!ArmadilloState::Rolling.should_hide_in_shell(5));
        assert!(ArmadilloState::Rolling.should_hide_in_shell(6));
        assert!(ArmadilloState::Scared.should_hide_in_shell(0));
        assert!(ArmadilloState::Scared.should_hide_in_shell(50));
        assert!(ArmadilloState::Unrolling.should_hide_in_shell(25));
        assert!(!ArmadilloState::Unrolling.should_hide_in_shell(26));

        // Predicate checks
        assert!(ArmadilloEntity::is_scared_by_predicate(true, false, false, false, false, false));
        assert!(ArmadilloEntity::is_scared_by_predicate(false, true, false, false, false, false));
        assert!(!ArmadilloEntity::is_scared_by_predicate(false, false, true, true, true, true));
        assert!(!ArmadilloEntity::is_scared_by_predicate(false, false, true, false, false, false));
        assert!(ArmadilloEntity::is_scared_by_predicate(false, false, true, false, true, false));
        assert!(ArmadilloEntity::is_scared_by_predicate(false, false, true, false, false, true));

        // Can stay rolled up checks
        assert!(ArmadilloEntity::can_stay_rolled_up_predicate(false, false, false, false, false));
        assert!(!ArmadilloEntity::can_stay_rolled_up_predicate(true, false, false, false, false));
        assert!(!ArmadilloEntity::can_stay_rolled_up_predicate(false, true, false, false, false));
        assert!(!ArmadilloEntity::can_stay_rolled_up_predicate(false, false, true, false, false));
        assert!(!ArmadilloEntity::can_stay_rolled_up_predicate(false, false, false, true, false));
        assert!(!ArmadilloEntity::can_stay_rolled_up_predicate(false, false, false, false, true));

        // Environmental panic damage check
        assert!(crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::IN_FIRE));
        assert!(crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::LAVA));
        assert!(crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::CACTUS));
        assert!(crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::FREEZE));
        assert!(!crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::GENERIC));
        assert!(!crate::entity::combat::is_armadillo_panic_environmental_damage(&DamageType::PLAYER_ATTACK));
    }

    #[test]
    fn test_armadillo_baby_dimensions_and_parity() {
        let adult = crate::entity::combat::calculate_armadillo_dimensions(false, 0.7, 0.65, 0.26);
        assert_eq!(adult, (0.7, 0.65, 0.26));
        let baby = crate::entity::combat::calculate_armadillo_dimensions(true, 0.7, 0.65, 0.26);
        assert_eq!(baby, (0.42, 0.39, 0.21875));
    }
}
