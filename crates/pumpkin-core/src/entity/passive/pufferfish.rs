use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::GameEvent;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::ai::control::fish_move_control::FishMoveControl;
use crate::entity::ai::goal::avoid_entity::AvoidEntityGoal;
use crate::entity::ai::goal::escape_danger::EscapeDangerGoal;
use crate::entity::ai::goal::pufferfish_puff::PufferfishPuffGoal;
use crate::entity::ai::goal::random_swimming::RandomSwimmingGoal;
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::ai::pathfinder::Navigator;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::custom_sound::CustomSound;
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::passive::fish;
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, EntityDimensions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PuffState {
    Small,
    Mid,
    Full,
}

impl PuffState {
    #[must_use]
    pub const fn id(self) -> i32 {
        match self {
            Self::Small => 0,
            Self::Mid => 1,
            Self::Full => 2,
        }
    }

    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Small,
            1 => Self::Mid,
            _ => Self::Full,
        }
    }
}

/// Represents a Pufferfish, a passive aquatic mob that can inflate when threatened.
///
/// Wiki: <https://minecraft.wiki/w/Pufferfish>
pub struct PufferfishEntity {
    pub mob_entity: MobEntity,
    pub air_supply: AtomicI32,
    puff_state: AtomicI32,
    inflate_counter: AtomicI32,
    deflate_timer: AtomicI32,
}

impl PufferfishEntity {
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

        let pufferfish = Self {
            mob_entity,
            air_supply: AtomicI32::new(300),
            puff_state: AtomicI32::new(PuffState::Small.id()),
            inflate_counter: AtomicI32::new(0),
            deflate_timer: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(pufferfish);
        mob_arc.refresh_dimensions();

        // Vanilla Pufferfish (AbstractFish): MAX_HEALTH = 3.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 3.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(3.0);

        let puffer_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Priority 0: PanicGoal (EscapeDangerGoal 1.25)
            goal_selector.add_goal(0, EscapeDangerGoal::new(1.25));
            // Priority 1: PufferfishPuffGoal
            goal_selector.add_goal(1, Box::new(PufferfishPuffGoal::new(puffer_weak)));
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
    pub fn get_puff_state(&self) -> PuffState {
        PuffState::from_id(self.puff_state.load(Ordering::Relaxed))
    }

    pub fn set_puff_state(&self, state: PuffState) {
        if self.puff_state.swap(state.id(), Ordering::Relaxed) == state.id() {
            return;
        }
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::pufferfish::PUFF_STATE, VarInt(state.id()));
        self.refresh_dimensions();
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let scale = match self.get_puff_state() {
            PuffState::Small => 0.5,
            PuffState::Mid => 0.7,
            PuffState::Full => 1.0,
        };

        let dimensions = EntityDimensions {
            width: entity.entity_type.dimension[0] * scale,
            height: entity.entity_type.dimension[1] * scale,
            eye_height: entity.entity_type.eye_height * scale,
        };

        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    pub(crate) fn start_inflating(&self) {
        self.inflate_counter.store(1, Ordering::Relaxed);
        self.deflate_timer.store(0, Ordering::Relaxed);
    }

    pub(crate) fn stop_inflating(&self) {
        self.inflate_counter.store(0, Ordering::Relaxed);
    }

    fn is_alive(&self) -> bool {
        let living = &self.mob_entity.living_entity;
        living.entity.is_alive() && !living.dead.load(Ordering::Relaxed)
    }

    fn make_sound(&self, sound: Sound) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_silent() {
            entity.play_sound(sound);
        }
    }

    fn tick_puff(&self) {
        let puff_state = self.get_puff_state();
        let inflate_counter = self.inflate_counter.load(Ordering::Relaxed);

        if inflate_counter > 0 {
            if puff_state == PuffState::Small {
                self.make_sound(Sound::EntityPufferFishBlowUp);
                self.set_puff_state(PuffState::Mid);
            } else if inflate_counter > 40 && puff_state == PuffState::Mid {
                self.make_sound(Sound::EntityPufferFishBlowUp);
                self.set_puff_state(PuffState::Full);
            }
            self.inflate_counter.fetch_add(1, Ordering::Relaxed);
        } else if puff_state != PuffState::Small {
            let deflate_timer = self.deflate_timer.load(Ordering::Relaxed);

            if deflate_timer > 60 && puff_state == PuffState::Full {
                self.make_sound(Sound::EntityPufferFishBlowOut);
                self.set_puff_state(PuffState::Mid);
            } else if deflate_timer > 100 && puff_state == PuffState::Mid {
                self.make_sound(Sound::EntityPufferFishBlowOut);
                self.set_puff_state(PuffState::Small);
            }
            self.deflate_timer.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn touch_nearby_mobs(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let bounding_box = entity.bounding_box.load().expand_all(0.3);

        for target in world.get_entities_at_box(&bounding_box) {
            if target.get_mob().is_some()
                && self.is_scary(caller, target.as_ref())
                && target.get_entity().is_alive()
            {
                self.touch(caller, target.as_ref());
            }
        }
    }

    fn touch(&self, caller: &dyn EntityBase, mob: &dyn EntityBase) {
        let puff_state = self.get_puff_state();
        if mob.damage_with_context(
            mob,
            (1 + puff_state.id()) as f32,
            DamageType::MOB_ATTACK,
            None,
            Some(caller),
            Some(caller),
        ) {
            if let Some(living) = mob.get_living_entity() {
                living.add_effect(poison(puff_state));
            }
            self.make_sound(Sound::EntityPufferFishSting);
        }
    }

    pub fn is_scary(&self, tester: &dyn EntityBase, target: &dyn EntityBase) -> bool {
        let world = self.mob_entity.living_entity.entity.world.load();

        TargetPredicate::create_non_attackable()
            .ignore_visibility()
            .test(&world, Some(tester), target)
            && !target.get_player().is_some_and(Player::is_creative)
            && !target
                .get_entity()
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_NOT_SCARY_FOR_PUFFERFISH)
    }
}

impl Mob for PufferfishEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_pufferfish(&self) -> Option<&PufferfishEntity> {
        Some(self)
    }

    fn mob_travel_in_water(&self, caller: &dyn EntityBase) -> bool {
        fish::travel_in_water(self, caller)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn dries_out_on_land(&self) -> bool {
        true
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let world = entity.world.load_full();
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
                world.play_sound_fine(
                    Sound::EntityPufferFishFlop,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        }

        if self.is_alive() && !self.mob_entity.is_no_ai() {
            self.tick_puff();
        }

        if self.is_alive() && self.get_puff_state() != PuffState::Small {
            self.touch_nearby_mobs(caller);
        }
    }

    fn mob_player_collision(&self, player: &Arc<Player>) {
        let puff_state = self.get_puff_state();
        if puff_state == PuffState::Small {
            return;
        }

        let caller: &dyn EntityBase = self;
        if player.damage_with_context(
            &**player,
            (1 + puff_state.id()) as f32,
            DamageType::MOB_ATTACK,
            None,
            Some(caller),
            Some(caller),
        ) {
            if !self.mob_entity.living_entity.entity.is_silent() {
                player.send_game_event(GameEvent::PlayPufferfishStringSound, 0.0);
            }
            player.add_effect(poison(puff_state));
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("PuffState", self.puff_state.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(state_id) = nbt.get_int("PuffState") {
            self.set_puff_state(PuffState::from_id(state_id));
        }
    }
}

impl CustomSound for PufferfishEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPufferFishDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPufferFishHurt)
    }
}

const fn poison(puff_state: PuffState) -> Effect {
    Effect {
        effect_type: &StatusEffect::POISON,
        duration: 60 * puff_state.id(),
        amplifier: 0,
        ambient: false,
        show_particles: true,
        show_icon: true,
        blend: false,
    }
}
