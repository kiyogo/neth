use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::{
    attributes::Attributes,
    damage::DamageType,
    effect::StatusEffect,
    entity::{EntityPose, EntityStatus, EntityType},
    particle::Particle,
    potion::Effect,
    sound::{Sound, SoundCategory},
    tag::Taggable,
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::VarInt;
use pumpkin_util::{GameMode, math::vector3::Vector3};

use super::warden_anger::{AngerLevel, AngerManagement};
use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};
use crate::world::World;

const EMERGE_DURATION: i32 = 134;
const SNIFFING_DURATION: i32 = 84;
const ROAR_DURATION: i32 = 84;
const DIGGING_DURATION: i32 = 100;
const DARKNESS_DISPLAY_LIMIT: i32 = 200;
const DARKNESS_DURATION: i32 = 260;
const TOUCH_COOLDOWN_TICKS: i32 = 20;
const ANGERMANAGEMENT_TICK_DELAY: i32 = 20;
const DARKNESS_INTERVAL: i32 = 120;
const DARKNESS_RADIUS: f64 = 20.0;
const DEFAULT_ANGER: i32 = 35;
const ON_HURT_ANGER_BOOST: i32 = 20;

pub struct WardenEntity {
    pub mob_entity: MobEntity,
    emerge_ticks: Arc<AtomicI32>,
    sniff_ticks: Arc<AtomicI32>,
    roar_ticks: Arc<AtomicI32>,
    dig_ticks: Arc<AtomicI32>,
    darkness_tick: AtomicI32,
    heartbeat_tick: AtomicI32,
    tendril_tick: AtomicI32,
    anger: std::sync::Mutex<AngerManagement>,
    touch_cooldown: AtomicI32,
}

struct EmergeGoal {
    emerge_ticks: Arc<AtomicI32>,
}

impl Goal for EmergeGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.emerge_ticks.load(Ordering::Relaxed) > 0
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        if entity.pose.load() == EntityPose::Emerging {
            entity.set_pose(EntityPose::Standing);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.emerge_ticks.fetch_sub(1, Ordering::Relaxed);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn can_stop(&self) -> bool {
        false
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP | Controls::TARGET
    }
}

struct SniffGoal {
    sniff_ticks: Arc<AtomicI32>,
    cooldown: AtomicI32,
}

impl Goal for SniffGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.sniff_ticks.load(Ordering::Relaxed) > 0 {
            return true;
        }
        let cd = self.cooldown.load(Ordering::Relaxed);
        if cd > 0 {
            self.cooldown.store(cd - 1, Ordering::Relaxed);
            return false;
        }
        if mob.get_mob_entity().get_target().is_none() {
            let entity = &mob.get_mob_entity().living_entity.entity;
            if entity.pose.load() == EntityPose::Standing {
                self.sniff_ticks.store(SNIFFING_DURATION, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    fn start(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        entity.set_pose(EntityPose::Sniffing);
        entity.world.load().play_sound_fine(
            Sound::EntityWardenSniff,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        if entity.pose.load() == EntityPose::Sniffing {
            entity.set_pose(EntityPose::Standing);
        }
        self.cooldown.store(300, Ordering::Relaxed);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let ticks = self.sniff_ticks.fetch_sub(1, Ordering::Relaxed);
        if ticks <= 1 {
            let entity = &mob.get_mob_entity().living_entity.entity;
            let world = entity.world.load();
            let my_pos = entity.pos.load();
            let players = world.get_nearby_players(my_pos, 16.0);
            for player in players {
                if matches!(
                    player.gamemode.load(),
                    GameMode::Survival | GameMode::Adventure
                ) && player.get_entity().is_alive()
                {
                    mob.get_mob_entity().set_target(Some(player));
                    break;
                }
            }
        }
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.sniff_ticks.load(Ordering::Relaxed) > 0
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

struct RoarGoal {
    roar_ticks: Arc<AtomicI32>,
}

impl Goal for RoarGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.roar_ticks.load(Ordering::Relaxed) > 0
    }

    fn start(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        entity.set_pose(EntityPose::Roaring);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        if entity.pose.load() == EntityPose::Roaring {
            entity.set_pose(EntityPose::Standing);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let ticks_left = self.roar_ticks.fetch_sub(1, Ordering::Relaxed);
        if ticks_left == ROAR_DURATION - 25 {
            let entity = &mob.get_mob_entity().living_entity.entity;
            entity.world.load().play_sound_fine(
                Sound::EntityWardenRoar,
                SoundCategory::Hostile,
                &entity.pos.load(),
                3.0,
                1.0,
            );
        }
        if let Some(target) = mob.get_mob_entity().get_target() {
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 90.0, 90.0);
        }
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.roar_ticks.load(Ordering::Relaxed) > 0
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::TARGET
    }
}

struct DigGoal {
    dig_ticks: Arc<AtomicI32>,
}

impl Goal for DigGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.dig_ticks.load(Ordering::Relaxed) > 0
    }

    fn start(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        entity.set_pose(EntityPose::Digging);
        entity.world.load().play_sound_fine(
            Sound::EntityWardenDig,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        entity.remove();
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.dig_ticks.fetch_sub(1, Ordering::Relaxed);
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.dig_ticks.load(Ordering::Relaxed) > 0
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn can_stop(&self) -> bool {
        false
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP | Controls::TARGET
    }
}

pub struct WardenSonicBoomGoal {
    warden: Weak<WardenEntity>,
    ticks: i32,
    cooldown: i32,
}

impl WardenSonicBoomGoal {
    pub const COOLDOWN: i32 = 40;
    pub const CHARGE_DURATION: i32 = 34;
    pub const TOTAL_DURATION: i32 = 60;

    #[must_use]
    pub const fn new(warden: Weak<WardenEntity>) -> Self {
        Self {
            warden,
            ticks: 0,
            cooldown: 0,
        }
    }
}

impl Goal for WardenSonicBoomGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return false;
        }
        let Some(warden) = self.warden.upgrade() else {
            return false;
        };
        let Some(target) = warden.mob_entity.get_target() else {
            return false;
        };
        let target_entity = target.get_entity();
        if !target_entity.is_alive() {
            return false;
        }

        let my_pos = warden.mob_entity.living_entity.entity.pos.load();
        let target_pos = target_entity.pos.load();
        let dx = target_pos.x - my_pos.x;
        let dy = target_pos.y - my_pos.y;
        let dz = target_pos.z - my_pos.z;
        let hor_dist_sq = dx * dx + dz * dz;

        hor_dist_sq <= 15.0 * 15.0 && dy.abs() <= 20.0 && hor_dist_sq > 9.0
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.ticks < Self::TOTAL_DURATION
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.ticks = 0;
        let Some(warden) = self.warden.upgrade() else {
            return;
        };
        let entity = &warden.mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::SonicCharge, None);
        world.play_sound_fine(
            Sound::EntityWardenSonicCharge,
            SoundCategory::Hostile,
            &entity.pos.load(),
            3.0,
            1.0,
        );
        warden
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.cooldown = Self::COOLDOWN;
        self.ticks = 0;
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        let Some(warden) = self.warden.upgrade() else {
            return;
        };
        let Some(target) = warden.mob_entity.get_target() else {
            return;
        };
        let target_entity = target.get_entity();

        warden
            .mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity_with_range(&target, 90.0, 90.0);
        warden
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();

        self.ticks += 1;
        if self.ticks == Self::CHARGE_DURATION {
            let entity = &warden.mob_entity.living_entity.entity;
            let world = entity.world.load();
            let my_pos = entity.pos.load();
            let source_pos = Vector3::new(my_pos.x, my_pos.y + 1.6, my_pos.z);
            let target_pos = target_entity.pos.load();
            let target_eye = Vector3::new(
                target_pos.x,
                target_pos.y + target_entity.get_eye_height(),
                target_pos.z,
            );
            let delta = Vector3::new(
                target_eye.x - source_pos.x,
                target_eye.y - source_pos.y,
                target_eye.z - source_pos.z,
            );
            let dist = delta.length();
            if dist > 0.001 {
                let norm = delta.normalize();
                let steps = (dist.floor() as i32) + 7;
                for i in 1..steps {
                    let p_pos = Vector3::new(
                        norm.x.mul_add(f64::from(i), source_pos.x),
                        norm.y.mul_add(f64::from(i), source_pos.y),
                        norm.z.mul_add(f64::from(i), source_pos.z),
                    );
                    world.spawn_particle(
                        p_pos,
                        Vector3::new(0.0, 0.0, 0.0),
                        0.0,
                        1,
                        Particle::SonicBoom,
                    );
                }

                world.play_sound_fine(
                    Sound::EntityWardenSonicBoom,
                    SoundCategory::Hostile,
                    &my_pos,
                    3.0,
                    1.0,
                );

                target.damage(target.as_ref(), 10.0, DamageType::SONIC_BOOM);

                let knockback_res = target
                    .get_living_entity()
                    .map_or(0.0, |l| l.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE));
                let factor = (1.0 - knockback_res).max(0.0);
                let kb_h = 2.5 * factor;
                let kb_v = 0.5 * factor;
                let mut target_vel = target_entity.velocity.load();
                target_vel.x += norm.x * kb_h;
                target_vel.y += norm.y * kb_v;
                target_vel.z += norm.z * kb_h;
                target_entity.velocity.store(target_vel);
            }
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

impl WardenEntity {
    pub const XP_REWARD: u32 = 5;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let emerge_ticks = Arc::new(AtomicI32::new(0));
        let sniff_ticks = Arc::new(AtomicI32::new(0));
        let roar_ticks = Arc::new(AtomicI32::new(0));
        let dig_ticks = Arc::new(AtomicI32::new(0));
        let warden = Self {
            mob_entity,
            emerge_ticks: emerge_ticks.clone(),
            sniff_ticks: sniff_ticks.clone(),
            roar_ticks: roar_ticks.clone(),
            dig_ticks: dig_ticks.clone(),
            darkness_tick: AtomicI32::new(0),
            heartbeat_tick: AtomicI32::new(0),
            tendril_tick: AtomicI32::new(0),
            anger: std::sync::Mutex::new(AngerManagement::default()),
            touch_cooldown: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(warden);
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

            goal_selector.add_goal(0, Box::new(EmergeGoal { emerge_ticks }));
            goal_selector.add_goal(0, Box::new(DigGoal { dig_ticks }));
            goal_selector.add_goal(0, Box::new(RoarGoal { roar_ticks }));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                2,
                Box::new(SniffGoal {
                    sniff_ticks,
                    cooldown: AtomicI32::new(100),
                }),
            );
            goal_selector.add_goal(
                3,
                Box::new(WardenSonicBoomGoal::new(Arc::downgrade(&mob_arc))),
            );
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.5)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn emerge(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_pose(EntityPose::Emerging);
        self.emerge_ticks.store(EMERGE_DURATION, Ordering::Relaxed);
        entity.world.load().play_sound_fine(
            Sound::EntityWardenEmerge,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );
    }

    pub fn sniff(&self) {
        self.sniff_ticks.store(SNIFFING_DURATION, Ordering::Relaxed);
    }

    pub fn roar(&self) {
        self.roar_ticks.store(ROAR_DURATION, Ordering::Relaxed);
    }

    pub fn dig(&self) {
        self.dig_ticks.store(DIGGING_DURATION, Ordering::Relaxed);
    }

    pub fn apply_darkness_around(world: &World, position: Vector3<f64>, radius: f64) {
        for player in world.get_nearby_players(position, radius) {
            if !matches!(
                player.gamemode.load(),
                GameMode::Survival | GameMode::Adventure
            ) {
                continue;
            }
            let current = player.living_entity.get_effect(&StatusEffect::DARKNESS);
            if current.is_some_and(|effect| {
                effect.duration < 0 || effect.duration >= DARKNESS_DISPLAY_LIMIT
            }) {
                continue;
            }
            let darkness = Effect {
                effect_type: &StatusEffect::DARKNESS,
                duration: DARKNESS_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: false,
                show_icon: false,
                blend: true,
            };
            player.send_effect(&darkness);
            player.living_entity.add_effect(darkness);
        }
    }

    pub fn anger(&self) -> std::sync::MutexGuard<'_, AngerManagement> {
        self.anger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn target(&self) -> Option<Arc<dyn EntityBase>> {
        self.mob_entity
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_mob_target(&self, target: Option<Arc<dyn EntityBase>>) {
        *self
            .mob_entity
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = target;
    }

    pub fn can_target_entity(&self, entity: &dyn EntityBase) -> bool {
        let Some(living) = entity.get_living_entity() else {
            return false;
        };
        let target = entity.get_entity();
        let this = &self.mob_entity.living_entity.entity;
        let world = this.world.load();
        if !Arc::ptr_eq(&world, &target.world.load()) {
            return false;
        }
        if entity
            .get_player()
            .is_some_and(|player| player.is_creative() || player.is_spectator())
        {
            return false;
        }
        let position = target.pos.load();
        !(self as &dyn EntityBase).is_allied_to(entity)
            && target.entity_type.id != EntityType::ARMOR_STAND.id
            && target.entity_type.id != EntityType::WARDEN.id
            && !target.invulnerable.load(Ordering::Relaxed)
            && !living.dead.load(Ordering::Relaxed)
            && world
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains(position.x, position.z)
    }

    pub fn get_anger_level(&self) -> AngerLevel {
        AngerLevel::by_anger(self.get_active_anger())
    }

    pub fn get_active_anger(&self) -> i32 {
        let target = self.target();
        self.anger().get_active_anger(target.as_deref())
    }

    pub fn get_entity_angry_at(&self) -> Option<Arc<dyn EntityBase>> {
        if !self.get_anger_level().is_angry() {
            return None;
        }
        self.anger()
            .get_active_entity(|entity| self.can_target_entity(entity))
    }

    pub fn clear_anger(&self, entity: &dyn EntityBase) {
        self.anger().clear_anger(entity);
    }

    pub fn increase_anger_at(&self, entity: &Arc<dyn EntityBase>, amount: i32, play_sound: bool) {
        if self.mob_entity.is_no_ai() || !self.can_target_entity(entity.as_ref()) {
            return;
        }
        let maybe_switch_target = self
            .target()
            .is_none_or(|target| target.get_player().is_none());
        let new_anger = self.anger().increase_anger(entity, amount);
        if entity.get_player().is_some()
            && maybe_switch_target
            && AngerLevel::by_anger(new_anger).is_angry()
        {
            self.set_mob_target(None);
        }
        if play_sound {
            self.play_listening_sound();
        }
    }

    fn play_listening_sound(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        if entity.pose.load() != EntityPose::Roaring {
            entity.world.load().play_sound_fine(
                self.get_anger_level().listening_sound(),
                SoundCategory::Hostile,
                &entity.pos.load(),
                10.0,
                1.0,
            );
        }
    }

    fn sync_client_anger_level(&self) {
        self.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::warden::CLIENT_ANGER_LEVEL,
            VarInt(self.get_active_anger()),
        );
    }

    fn tick_touch(&self) {
        if self.touch_cooldown.load(Ordering::Relaxed) > 0 {
            self.touch_cooldown.fetch_sub(1, Ordering::Relaxed);
            return;
        }
        if self.mob_entity.is_no_ai() || self.is_digging_or_emerging() {
            return;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let bounding_box = entity.bounding_box.load();
        let touching = world
            .get_players_at_box(&bounding_box)
            .into_iter()
            .map(|player| player as Arc<dyn EntityBase>)
            .chain(world.get_entities_at_box(&bounding_box))
            .find(|other| other.get_entity().entity_id != entity.entity_id && other.is_pushable());
        if let Some(other) = touching {
            self.touch_cooldown
                .store(TOUCH_COOLDOWN_TICKS, Ordering::Relaxed);
            self.increase_anger_at(&other, DEFAULT_ANGER, true);
        }
    }

    fn update_target_from_anger(&self) {
        let target = self.target();
        if let Some(target) = &target
            && (!self.get_anger_level().is_angry() || !self.can_target_entity(target.as_ref()))
        {
            if !self.can_target_entity(target.as_ref()) {
                self.clear_anger(target.as_ref());
            }
            self.set_mob_target(None);
            return;
        }
        if target.is_none()
            && let Some(angry_at) = self.get_entity_angry_at()
        {
            self.set_mob_target(Some(angry_at));
        }
    }

    pub fn is_digging_or_emerging(&self) -> bool {
        self.emerge_ticks.load(Ordering::Relaxed) > 0 || self.dig_ticks.load(Ordering::Relaxed) > 0
    }
}

impl CustomSound for WardenEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityWardenDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityWardenHurt)
    }
}

impl Mob for WardenEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        false
    }

    fn check_spawn_obstruction(&self, world: &World) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let bounding_box = entity.bounding_box.load();
        !world.contains_any_liquid(bounding_box)
            && world.get_entities_at_box(&bounding_box).is_empty()
            && world.is_space_empty(bounding_box)
    }

    fn pre_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) -> bool {
        !self.is_digging_or_emerging()
            || damage_type.has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
    }

    fn on_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) {
        if self.mob_entity.is_no_ai() || self.is_digging_or_emerging() {
            return;
        }
        let Some(source) = source else {
            return;
        };
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let source_id = source.get_entity().entity_id;
        let attacker = world
            .get_player_by_id(source_id)
            .map(|player| player as Arc<dyn EntityBase>)
            .or_else(|| world.get_entity_by_id(source_id));
        let Some(attacker) = attacker else {
            return;
        };
        self.increase_anger_at(
            &attacker,
            AngerLevel::Angry.minimum_anger() + ON_HURT_ANGER_BOOST,
            false,
        );
        if self.target().is_none() && self.can_target_entity(attacker.as_ref()) {
            self.set_mob_target(Some(attacker));
        }
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
        world.play_sound_fine(
            Sound::EntityWardenAttackImpact,
            SoundCategory::Hostile,
            &entity.pos.load(),
            10.0,
            1.0,
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let world = entity.world.load();
        let pos = entity.pos.load();
        let age = entity.age.load(Ordering::Relaxed);

        self.tick_touch();

        // Darkness pulsing every 120 ticks
        let dark_t = self.darkness_tick.fetch_add(1, Ordering::Relaxed) + 1;
        if dark_t >= DARKNESS_INTERVAL {
            self.darkness_tick.store(0, Ordering::Relaxed);
            Self::apply_darkness_around(&world, pos, DARKNESS_RADIUS);
        }

        if age % ANGERMANAGEMENT_TICK_DELAY == 0 {
            self.anger()
                .tick(&world, |suspect| self.can_target_entity(suspect));
            self.sync_client_anger_level();
        }

        self.update_target_from_anger();

        // Heartbeat pulsing sound
        let hb_t = self.heartbeat_tick.fetch_add(1, Ordering::Relaxed) + 1;
        let hb_interval = match self.get_anger_level() {
            AngerLevel::Angry => 20,
            AngerLevel::Agitated => 40,
            AngerLevel::Calm => 60,
        };
        if hb_t >= hb_interval {
            self.heartbeat_tick.store(0, Ordering::Relaxed);
            if !entity.silent.load(Ordering::Relaxed) {
                world.play_sound_fine(
                    Sound::EntityWardenHeartbeat,
                    SoundCategory::Hostile,
                    &pos,
                    5.0,
                    1.0,
                );
            }
        }

        // Tendril clicks & shivering every 80 ticks when idle
        if self.mob_entity.get_target().is_none() {
            let tc_t = self.tendril_tick.fetch_add(1, Ordering::Relaxed) + 1;
            if tc_t >= 80 {
                self.tendril_tick.store(0, Ordering::Relaxed);
                if !entity.silent.load(Ordering::Relaxed) {
                    world.send_entity_status(entity, EntityStatus::TendrilsShiver, None);
                    world.play_sound_fine(
                        Sound::EntityWardenTendrilClicks,
                        SoundCategory::Hostile,
                        &pos,
                        5.0,
                        1.0,
                    );
                }
            }
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_compound("anger", self.anger().to_nbt());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(anger) = nbt.get_compound("anger") {
            *self.anger() = AngerManagement::from_nbt(anger);
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }
}
