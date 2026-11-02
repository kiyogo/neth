use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU64, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    experience_orb::ExperienceOrbEntity,
    item::ItemEntity,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const SNIFFER_FOOD: &[&Item] = &[&Item::TORCHFLOWER_SEEDS, &Item::PITCHER_POD];
pub const SNIFFER_BABY_START_AGE: i32 = crate::entity::combat::SNIFFER_BABY_START_AGE;
pub const DIGGING_DROP_SEED_OFFSET_TICKS: i32 = crate::entity::combat::SNIFFER_DIGGING_DROP_SEED_OFFSET_TICKS;
pub const STEP_SOUND_VOLUME: f32 = crate::entity::combat::SNIFFER_STEP_SOUND_VOLUME;
pub const STEP_SOUND_PITCH: f32 = crate::entity::combat::SNIFFER_STEP_SOUND_PITCH;
pub const DIGGING_MIN_DURATION_TICKS: u64 = crate::entity::combat::SNIFFER_DIGGING_MIN_DURATION_TICKS;
pub const DIGGING_MAX_DURATION_TICKS: u64 = crate::entity::combat::SNIFFER_DIGGING_MAX_DURATION_TICKS;
pub const RISING_DURATION_TICKS: u64 = crate::entity::combat::SNIFFER_RISING_DURATION_TICKS;
pub const SNIFFING_COOLDOWN_TICKS: i32 = crate::entity::combat::SNIFFER_SNIFFING_COOLDOWN_TICKS;
pub const DIGGING_EVENT_ID: u8 = crate::entity::combat::SNIFFER_DIGGING_EVENT_ID;
pub const MAX_HEAD_ROTATION: f32 = crate::entity::combat::SNIFFER_MAX_HEAD_ROTATION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SnifferState {
    #[default]
    Idling = 0,
    FeelingHappy = 1,
    Scenting = 2,
    Sniffing = 3,
    Searching = 4,
    Digging = 5,
    Rising = 6,
}

impl SnifferState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::FeelingHappy,
            2 => Self::Scenting,
            3 => Self::Sniffing,
            4 => Self::Searching,
            5 => Self::Digging,
            6 => Self::Rising,
            _ => Self::Idling,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn default_duration(self) -> u64 {
        match self {
            Self::Idling => 0,
            Self::FeelingHappy => 40,
            Self::Scenting => 40,
            Self::Sniffing => 40,
            Self::Searching => 600,
            Self::Digging => 160,
            Self::Rising => 40,
        }
    }
}

/// Represents a Sniffer, a large passive mob that digs up ancient seeds.
///
/// Wiki: <https://minecraft.wiki/w/Sniffer>
pub struct SnifferEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub state: AtomicI32,
    pub drop_seed_at_tick: AtomicI32,
    pub in_state_ticks: AtomicU64,
    pub sniff_cooldown_ticks: AtomicI32,
    pub state_duration_ticks: AtomicU64,
    pub explored_positions: std::sync::Mutex<Vec<BlockPos>>,
}

impl SnifferEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::SNIFFER_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let sniffer = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            state: AtomicI32::new(SnifferState::Idling.id()),
            drop_seed_at_tick: AtomicI32::new(0),
            in_state_ticks: AtomicU64::new(0),
            sniff_cooldown_ticks: AtomicI32::new(0),
            state_duration_ticks: AtomicU64::new(0),
            explored_positions: std::sync::Mutex::new(Vec::new()),
        };

        // Initialize attributes: Max Health = 14.0, Movement Speed = 0.1
        sniffer
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::SNIFFER_MAX_HEALTH as f64);
        sniffer
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::SNIFFER_BASE_MOVEMENT_SPEED);
        sniffer
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::SNIFFER_MAX_HEALTH);

        let mob_arc = Arc::new(sniffer);
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
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, SNIFFER_FOOD, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
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
        let is_digging = self.get_state() == SnifferState::Digging;
        let (w, h, eye) = crate::entity::combat::calculate_sniffer_dimensions(
            is_baby,
            is_digging,
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
    pub fn get_state(&self) -> SnifferState {
        SnifferState::from_id(self.state.load(Ordering::Relaxed))
    }

    pub fn set_state(&self, state: SnifferState) {
        self.state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::STATE,
            VarInt(state.id()),
        );
        self.refresh_dimensions();
    }

    pub fn transition_to(&self, state: SnifferState) {
        self.in_state_ticks.store(0, Ordering::Relaxed);
        self.state_duration_ticks.store(state.default_duration(), Ordering::Relaxed);

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();

        match state {
            SnifferState::Idling => {
                self.set_state(SnifferState::Idling);
            }
            SnifferState::FeelingHappy => {
                world.play_sound(Sound::EntitySnifferHappy, SoundCategory::Neutral, &pos);
                self.set_state(SnifferState::FeelingHappy);
            }
            SnifferState::Scenting => {
                self.set_state(SnifferState::Scenting);
                self.on_scenting_start();
            }
            SnifferState::Sniffing => {
                world.play_sound(Sound::EntitySnifferSniffing, SoundCategory::Neutral, &pos);
                self.set_state(SnifferState::Sniffing);
            }
            SnifferState::Searching => {
                self.set_state(SnifferState::Searching);
            }
            SnifferState::Digging => {
                self.set_state(SnifferState::Digging);
                self.on_digging_start();
            }
            SnifferState::Rising => {
                world.play_sound(
                    Sound::EntitySnifferDiggingStop,
                    SoundCategory::Neutral,
                    &pos,
                );
                self.set_state(SnifferState::Rising);
            }
        }
    }

    fn on_scenting_start(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let pitch = if self.is_baby() { 1.3 } else { 1.0 };
        world.play_sound_fine(
            Sound::EntitySnifferScenting,
            SoundCategory::Neutral,
            &pos,
            1.0,
            pitch,
        );
    }

    fn on_digging_start(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let current_ticks = world.level_time.try_lock().map_or(0, |t| t.world_age);
        let drop_tick = current_ticks as i32 + DIGGING_DROP_SEED_OFFSET_TICKS;
        self.drop_seed_at_tick.store(drop_tick, Ordering::Relaxed);
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::DROP_SEED_AT_TICK,
            VarInt(drop_tick),
        );
        world.broadcast_entity_status_id(&entity, DIGGING_EVENT_ID as i8);
    }

    pub fn on_digging_complete(&self, success: bool) {
        if success {
            let head_block = self.get_head_block().down();
            self.store_explored_position(head_block);
            self.sniff_cooldown_ticks
                .store(SNIFFING_COOLDOWN_TICKS, Ordering::Relaxed);
        }
    }

    #[must_use]
    pub fn get_head_position(&self) -> Vector3<f64> {
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let yaw = f64::from(entity.yaw.load());
        let yaw_rad = yaw.to_radians();
        let forward = Vector3::new(-yaw_rad.sin(), 0.0, yaw_rad.cos());
        pos + forward * 2.25
    }

    #[must_use]
    pub fn get_head_block(&self) -> BlockPos {
        let head_pos = self.get_head_position();
        let entity = self.get_entity();
        let pos = entity.pos.load();
        BlockPos::floored(head_pos.x, pos.y + 0.2, head_pos.z)
    }

    #[must_use]
    pub fn can_dig(&self) -> bool {
        let entity = self.get_entity();
        !self.is_panicking()
            && !self.is_baby()
            && !self.mob_entity.living_entity.is_in_water()
            && entity.on_ground.load(Ordering::Relaxed)
            && self.can_dig_at(self.get_head_block().down())
    }

    fn can_dig_at(&self, pos: BlockPos) -> bool {
        let entity = self.get_entity();
        let world = entity.world.load();
        let block_state = world.get_block_state(&pos);
        let block = pumpkin_data::Block::from_state_id(block_state.id);

        if !block.has_tag(&tag::Block::MINECRAFT_SNIFFER_DIGGABLE_BLOCK) {
            return false;
        }

        let explored = self
            .explored_positions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !explored.contains(&pos)
    }

    fn store_explored_position(&self, pos: BlockPos) {
        let mut explored = self
            .explored_positions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if explored.len() >= 20 {
            explored.pop();
        }
        explored.insert(0, pos);
    }

    pub fn drop_seed(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let current_tick = world.get_world_age() as i32;

        if self.drop_seed_at_tick.load(Ordering::Relaxed) == current_tick {
            let head_pos = self.get_head_position();
            let seed_item = if rand::random::<bool>() {
                &Item::TORCHFLOWER_SEEDS
            } else {
                &Item::PITCHER_POD
            };
            let item_stack = ItemStack::new(1, seed_item);

            let item_entity = Entity::new(world.clone(), head_pos, &EntityType::ITEM);
            let item_arc = Arc::new(ItemEntity::new(item_entity, item_stack));
            world.spawn_entity_non_save(item_arc as Arc<dyn EntityBase>);

            world.play_sound(
                Sound::EntitySnifferDropSeed,
                SoundCategory::Neutral,
                &head_pos,
            );
        }
    }

    pub fn spawn_child_from_breeding(&self, partner: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();

        let item_stack = ItemStack::new(1, &Item::SNIFFER_EGG);
        let egg_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
        let egg_arc = Arc::new(ItemEntity::new(egg_entity, item_stack));
        world.spawn_entity_non_save(egg_arc as Arc<dyn EntityBase>);

        let breeder_id = self
            .mob_entity
            .breeder
            .load()
            .or_else(|| partner.get_breeder());
        if let Some(uuid) = breeder_id {
            if let Some(player) = world.get_player_by_uuid(uuid) {
                player.increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::AnimalsBred as i32,
                    1,
                );
                player.trigger_advancement_criterion(
                    pumpkin_data::advancement::Advancement::HUSBANDRY_BREED_AN_ANIMAL,
                    "bred",
                );
                player.trigger_advancement_criterion(
                    pumpkin_data::advancement::Advancement::HUSBANDRY_BRED_ALL_ANIMALS,
                    "minecraft:sniffer",
                );
            }
        }

        let xp = rand::random_range(1..=7);
        ExperienceOrbEntity::spawn(&world, pos, xp);

        let pitch = crate::entity::combat::get_sniffer_egg_plop_pitch(rand::random(), rand::random());
        world.play_sound_fine(Sound::BlockSnifferEggPlop, SoundCategory::Neutral, &pos, 1.0, pitch);

        self.mob_entity.reset_love_ticks();
        self.mob_entity
            .breeding_cooldown
            .store(6000, Ordering::Relaxed);
        partner.reset_love();
        partner.set_breeding_cooldown(6000);
    }
}

impl AgeableMob for SnifferEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn get_baby_start_age(&self) -> i32 {
        SNIFFER_BABY_START_AGE
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

impl Animal for SnifferEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_SNIFFER_FOOD)
            || SNIFFER_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for SnifferEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_sniffer(&self) -> Option<&SnifferEntity> {
        Some(self)
    }

    fn can_be_leashed(&self) -> bool {
        true
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("State", self.get_state().id());
        nbt.put_int(
            "DropSeedAtTick",
            self.drop_seed_at_tick.load(Ordering::Relaxed),
        );
        nbt.put_int(
            "SniffCooldown",
            self.sniff_cooldown_ticks.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(state_id) = nbt.get_int("State") {
            self.state.store(state_id, Ordering::Relaxed);
        }
        if let Some(drop_tick) = nbt.get_int("DropSeedAtTick") {
            self.drop_seed_at_tick.store(drop_tick, Ordering::Relaxed);
        }
        if let Some(cd) = nbt.get_int("SniffCooldown") {
            self.sniff_cooldown_ticks.store(cd, Ordering::Relaxed);
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_max_head_rotation(&self) -> f32 {
        MAX_HEAD_ROTATION
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        self.in_state_ticks.fetch_add(1, Ordering::Relaxed);

        let cd = self.sniff_cooldown_ticks.load(Ordering::Relaxed);
        if cd > 0 {
            self.sniff_cooldown_ticks.store(cd - 1, Ordering::Relaxed);
        }

        let state = self.get_state();
        let ticks_in_state = self.in_state_ticks.load(Ordering::Relaxed);
        let target_duration = self.state_duration_ticks.load(Ordering::Relaxed);

        match state {
            SnifferState::Searching => {
                let entity = self.get_entity();
                let world = entity.world.load();
                let ticks = world.get_world_age();
                if ticks % 20 == 0 {
                    world.play_sound(
                        Sound::EntitySnifferSearching,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                }
                if ticks_in_state >= target_duration {
                    if self.can_dig() {
                        self.transition_to(SnifferState::Digging);
                    } else {
                        self.transition_to(SnifferState::Idling);
                    }
                }
            }
            SnifferState::Digging => {
                self.drop_seed();
                if ticks_in_state >= target_duration {
                    self.transition_to(SnifferState::Rising);
                }
            }
            SnifferState::Rising => {
                if ticks_in_state >= target_duration {
                    self.on_digging_complete(true);
                    self.transition_to(SnifferState::FeelingHappy);
                }
            }
            SnifferState::FeelingHappy => {
                if ticks_in_state >= target_duration {
                    self.transition_to(SnifferState::Idling);
                }
            }
            SnifferState::Scenting | SnifferState::Sniffing => {
                if ticks_in_state >= target_duration {
                    self.transition_to(SnifferState::Idling);
                }
            }
            SnifferState::Idling => {
                // Periodically initiate sniffing if ready and on diggable ground
                if !self.is_panicking()
                    && !self.is_baby()
                    && self.sniff_cooldown_ticks.load(Ordering::Relaxed) == 0
                    && self.can_dig()
                    && rand::random_range(0..200) == 0
                {
                    self.transition_to(SnifferState::Sniffing);
                }
            }
        }

        let entity = self.get_entity();
        if entity.is_alive()
            && !entity.silent.load(Ordering::Relaxed)
            && state != SnifferState::Digging
            && state != SnifferState::Searching
            && rand::random_range(0..100) == 0
        {
            entity.world.load().play_sound_fine(
                Sound::EntitySnifferIdle,
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
            entity.set_synced_data(pumpkin_data::tracked_data::sniffer::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::STATE,
            VarInt(self.get_state().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::DROP_SEED_AT_TICK,
            VarInt(self.drop_seed_at_tick.load(Ordering::Relaxed)),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let is_baby = self.get_entity().age.load(std::sync::atomic::Ordering::Relaxed) < 0;
        let is_food = self.is_food(item_stack);
        let res = self.animal_interact(player, item_stack, Sound::EntitySnifferEat);
        if res && is_food {
            // Vanilla Sniffer.java:360: Eating sound pitch is randomized between 0.8 and 1.2
            let entity = self.get_entity();
            let pitch = crate::entity::combat::calculate_sniffer_eat_pitch(rand::random());
            entity.world.load().play_sound_fine(
                Sound::EntitySnifferEat,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                pitch,
            );
            if is_baby {
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::FeedSnifflet,
                );
            }
        }
        res
    }
}

impl CustomSound for SnifferEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySnifferDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySnifferHurt)
    }

    fn step_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySnifferStep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniffer_eating_sound_pitch_and_constants() {
        assert_eq!(SNIFFER_BABY_START_AGE, -48000);
        assert_eq!(DIGGING_DROP_SEED_OFFSET_TICKS, 120);

        let pitch_low = crate::entity::combat::calculate_sniffer_eat_pitch(0.0);
        let pitch_high = crate::entity::combat::calculate_sniffer_eat_pitch(1.0);
        let pitch_mid = crate::entity::combat::calculate_sniffer_eat_pitch(0.5);

        assert!((pitch_low - 0.8).abs() < 1e-6);
        assert!((pitch_high - 1.2).abs() < 1e-6);
        assert!((pitch_mid - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_sniffer_parity_constants() {
        assert_eq!(SnifferEntity::XP_REWARD, 2);
        assert_eq!(crate::entity::combat::SNIFFER_MAX_HEALTH, 14.0);
        assert_eq!(crate::entity::combat::SNIFFER_BASE_MOVEMENT_SPEED, 0.1);
        assert_eq!(crate::entity::combat::SNIFFER_BABY_START_AGE, -48000);
        assert_eq!(crate::entity::combat::SNIFFER_DIGGING_DROP_SEED_OFFSET_TICKS, 120);
        assert_eq!(STEP_SOUND_VOLUME, 0.15);
        assert_eq!(STEP_SOUND_PITCH, 1.0);
        assert_eq!(DIGGING_MIN_DURATION_TICKS, 160);
        assert_eq!(DIGGING_MAX_DURATION_TICKS, 180);
        assert_eq!(RISING_DURATION_TICKS, 40);
        assert_eq!(SNIFFING_COOLDOWN_TICKS, 9600);
        assert_eq!(DIGGING_EVENT_ID, 63);
        assert_eq!(MAX_HEAD_ROTATION, 50.0);
        assert_eq!(crate::entity::combat::SNIFFER_SPEED_MULTIPLIER_WHEN_SNIFFING, 1.25);
        assert_eq!(crate::entity::combat::SNIFFER_SPEED_MULTIPLIER_WHEN_TEMPTED, 1.25);
        assert_eq!(crate::entity::combat::SNIFFER_SPEED_MULTIPLIER_WHEN_PANICKING, 2.0);
        assert_eq!(crate::entity::combat::SNIFFER_DIGGING_PARTICLES_DELAY_MS, 1700);
        assert_eq!(crate::entity::combat::SNIFFER_DIGGING_PARTICLES_DURATION_MS, 6000);
        assert_eq!(crate::entity::combat::SNIFFER_DIGGING_PARTICLES_AMOUNT, 30);
        assert_eq!(crate::entity::combat::SNIFFER_FEELING_HAPPY_MIN_TICKS, 40);
        assert_eq!(crate::entity::combat::SNIFFER_FEELING_HAPPY_MAX_TICKS, 100);
        assert_eq!(crate::entity::combat::SNIFFER_SCENTING_MIN_TICKS, 40);
        assert_eq!(crate::entity::combat::SNIFFER_SCENTING_MAX_TICKS, 80);
        assert_eq!(crate::entity::combat::SNIFFER_SNIFFING_MIN_TICKS, 40);
        assert_eq!(crate::entity::combat::SNIFFER_SNIFFING_MAX_TICKS, 80);
        assert_eq!(crate::entity::combat::SNIFFER_SEARCHING_MAX_TICKS, 600);
    }

    #[test]
    fn test_sniffer_digging_durations_and_state_transitions_parity() {
        // States and IDs
        assert_eq!(SnifferState::from_id(0), SnifferState::Idling);
        assert_eq!(SnifferState::from_id(1), SnifferState::FeelingHappy);
        assert_eq!(SnifferState::from_id(2), SnifferState::Scenting);
        assert_eq!(SnifferState::from_id(3), SnifferState::Sniffing);
        assert_eq!(SnifferState::from_id(4), SnifferState::Searching);
        assert_eq!(SnifferState::from_id(5), SnifferState::Digging);
        assert_eq!(SnifferState::from_id(6), SnifferState::Rising);

        // State durations
        assert_eq!(SnifferState::Idling.default_duration(), 0);
        assert_eq!(SnifferState::FeelingHappy.default_duration(), 40);
        assert_eq!(SnifferState::Scenting.default_duration(), 40);
        assert_eq!(SnifferState::Sniffing.default_duration(), 40);
        assert_eq!(SnifferState::Searching.default_duration(), 600);
        assert_eq!(SnifferState::Digging.default_duration(), 160);
        assert_eq!(SnifferState::Rising.default_duration(), 40);

        // Digging & Cooldown parity constants
        assert_eq!(DIGGING_MIN_DURATION_TICKS, 160);
        assert_eq!(DIGGING_MAX_DURATION_TICKS, 180);
        assert_eq!(RISING_DURATION_TICKS, 40);
        assert_eq!(SNIFFING_COOLDOWN_TICKS, 9600);
        assert_eq!(DIGGING_DROP_SEED_OFFSET_TICKS, 120);
    }

    #[test]
    fn test_sniffer_baby_and_digging_dimensions_parity() {
        let adult_stand = crate::entity::combat::calculate_sniffer_dimensions(false, false, 1.9, 1.75, 1.05);
        assert_eq!(adult_stand, (1.9, 1.75, 1.05));
        let adult_dig = crate::entity::combat::calculate_sniffer_dimensions(false, true, 1.9, 1.75, 1.05);
        assert_eq!(adult_dig, (1.9, 1.35, 0.81));
        let baby_stand = crate::entity::combat::calculate_sniffer_dimensions(true, false, 1.9, 1.75, 1.05);
        assert_eq!(baby_stand, (0.95, 0.875, 0.525));
        let baby_dig = crate::entity::combat::calculate_sniffer_dimensions(true, true, 1.9, 1.75, 1.05);
        assert_eq!(baby_dig, (0.95, 0.675, 0.405));
    }
}
