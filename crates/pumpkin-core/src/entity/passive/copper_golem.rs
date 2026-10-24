use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicI64, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    player::Player,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum WeatherState {
    #[default]
    Unaffected = 0,
    Exposed = 1,
    Weathered = 2,
    Oxidized = 3,
}

impl WeatherState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Exposed,
            2 => Self::Weathered,
            3 => Self::Oxidized,
            _ => Self::Unaffected,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Unaffected => Self::Exposed,
            Self::Exposed => Self::Weathered,
            Self::Weathered | Self::Oxidized => Self::Oxidized,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Oxidized => Self::Weathered,
            Self::Weathered => Self::Exposed,
            Self::Exposed | Self::Unaffected => Self::Unaffected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CopperGolemState {
    #[default]
    Idle = 0,
    GettingItem = 1,
    GettingNoItem = 2,
    DroppingItem = 3,
    DroppingNoItem = 4,
}

impl CopperGolemState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::GettingItem,
            2 => Self::GettingNoItem,
            3 => Self::DroppingItem,
            4 => Self::DroppingNoItem,
            _ => Self::Idle,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }
}

/// Represents a Copper Golem, a passive creation mob made of copper blocks and lightning rod.
///
/// Wiki: <https://minecraft.wiki/w/Copper_Golem>
pub struct CopperGolemEntity {
    pub mob_entity: MobEntity,
    pub weather_state: AtomicI32,
    pub state: AtomicI32,
    pub next_weathering_tick: AtomicI64,
    last_lightning_bolt_uuid: AtomicCell<Option<Uuid>>,
}

impl CopperGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let golem = Self {
            mob_entity,
            weather_state: AtomicI32::new(WeatherState::Unaffected.id()),
            state: AtomicI32::new(CopperGolemState::Idle.id()),
            next_weathering_tick: AtomicI64::new(-1),
            last_lightning_bolt_uuid: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(golem);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla CopperGolem: MOVEMENT_SPEED = 0.2, STEP_HEIGHT = 1.0, MAX_HEALTH = 12.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 12.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.2;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(step) = attributes.get_mut(&Attributes::STEP_HEIGHT.id) {
                step.base_value = 1.0;
                step.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(12.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_weather_state(&self) -> WeatherState {
        WeatherState::from_id(self.weather_state.load(Ordering::Relaxed))
    }

    pub fn set_weather_state(&self, state: WeatherState) {
        self.weather_state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::WEATHER_STATE,
            VarInt(state.id()),
        );
    }

    #[must_use]
    pub fn get_state(&self) -> CopperGolemState {
        CopperGolemState::from_id(self.state.load(Ordering::Relaxed))
    }

    pub fn set_state(&self, state: CopperGolemState) {
        self.state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::COPPER_GOLEM_STATE,
            VarInt(state.id()),
        );
    }

    #[must_use]
    pub fn step_sound(&self) -> Sound {
        match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemStep,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredStep,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedStep,
        }
    }
}

impl CustomSound for CopperGolemEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemHurt,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredHurt,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedHurt,
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemDeath,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredDeath,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedDeath,
        })
    }
}

impl Mob for CopperGolemEntity {
    fn as_custom_sound(&self) -> Option<&dyn crate::entity::custom_sound::CustomSound> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_long(
            "next_weather_age",
            self.next_weathering_tick.load(Ordering::Relaxed),
        );
        nbt.put_int("weather_state", self.get_weather_state().id());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(next) = nbt.get_long("next_weather_age") {
            self.next_weathering_tick.store(next, Ordering::Relaxed);
        }
        if let Some(state) = nbt.get_int("weather_state") {
            self.set_weather_state(WeatherState::from_id(state));
        }
    }

    fn as_copper_golem(&self) -> Option<&CopperGolemEntity> {
        Some(self)
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        lightning: &crate::entity::lightning::LightningBoltEntity,
    ) {
        let lightning_uuid = lightning.get_entity().entity_uuid;
        if self.last_lightning_bolt_uuid.load() != Some(lightning_uuid) {
            self.last_lightning_bolt_uuid.store(Some(lightning_uuid));
            let weather_state = self.get_weather_state();
            if weather_state != WeatherState::Unaffected {
                self.next_weathering_tick.store(-1, Ordering::Relaxed);
                self.set_weather_state(weather_state.previous());
            }
        }
        self.mob_entity
            .living_entity
            .on_lightning_strike(caller, lightning);
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::WEATHER_STATE,
            VarInt(self.get_weather_state().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::COPPER_GOLEM_STATE,
            VarInt(self.get_state().id()),
        );
    }

    fn mob_interact(&self, _player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let entity = self.get_entity();
        let world = entity.world.load();

        // Honeycomb waxing
        if item_stack.item.id == Item::HONEYCOMB.id
            && self.next_weathering_tick.load(Ordering::Relaxed) != -2
        {
            self.next_weathering_tick.store(-2, Ordering::Relaxed);
            let pos = entity.pos.load();
            world.play_sound(Sound::ItemHoneycombWaxOn, SoundCategory::Blocks, &pos);
            return true;
        }

        // Axe scraping
        if item_stack.item.has_tag(&tag::Item::MINECRAFT_AXES) {
            let current_next = self.next_weathering_tick.load(Ordering::Relaxed);
            if current_next == -2 {
                self.next_weathering_tick.store(-1, Ordering::Relaxed);
                let pos = entity.pos.load();
                world.play_sound(Sound::ItemAxeScrape, SoundCategory::Blocks, &pos);
                return true;
            }

            let weather_state = self.get_weather_state();
            if weather_state != WeatherState::Unaffected {
                self.set_weather_state(weather_state.previous());
                self.next_weathering_tick.store(-1, Ordering::Relaxed);
                let pos = entity.pos.load();
                world.play_sound(Sound::ItemAxeScrape, SoundCategory::Blocks, &pos);
                return true;
            }
        }

        false
    }
}
