use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use crate::entity::attributes::Modifier;
use crate::entity::attributes::ModifierOperation;
use pumpkin_data::{BlockStateId, attributes::Attributes};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    damage::DamageType,
    data_component_impl::EquipmentSlot,
    entity::EntityType,
    item::Item,
    particle::Particle,
    sound::{Sound, SoundCategory},
    tag,
    tag::Taggable,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use rand::RngExt;

use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::{
        goal::{
            active_target::ActiveTargetGoal, chase_player::ChasePlayerGoal,
            look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
            melee_attack::MeleeAttackGoal, pick_up_block::PickUpBlockGoal,
            place_block::PlaceBlockGoal, reset_universal_anger::ResetUniversalAngerGoal,
            revenge::RevengeGoal, swim::SwimGoal,
            teleport_towards_player::TeleportTowardsPlayerGoal,
            water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
        },
        pathfinder::node::PathType,
    },
    mob::{
        Mob, MobEntity,
        neutral::{NeutralData, NeutralMob},
    },
    player::Player,
};

const SPEED_BOOST: f64 = 0.15;
const ENDERMAN_SPEED_BOOST_ID: &str = "minecraft:attacking";

pub const ENDERMAN_EYE_HEIGHT: f64 = 2.55;
pub const ENDERMAN_BODY_Y_OFFSET: f64 = 1.45;
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;

fn is_projectile_damage(dt: DamageType) -> bool {
    pumpkin_data::tag::DamageType::MINECRAFT_IS_PROJECTILE
        .0
        .contains(&dt.message_id)
}

pub struct EndermanEntity {
    pub mob_entity: MobEntity,
    neutral_data: NeutralData,
    carried_block: AtomicCell<Option<BlockStateId>>,
    /// Synced CREEPY flag, the open-mouth pose. Anger lives in `NeutralMob`.
    creepy: AtomicBool,
    provoked: AtomicBool,
    speed_boosted: AtomicBool,
    /// Tick count when the target was last changed or acquired (Enderman.java:85).
    pub target_change_time: AtomicCell<u64>,
    /// Tick count when the stare sound was last played (Enderman.java:84).
    pub last_stare_sound: AtomicCell<u64>,
}

impl EndermanEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, 40.0);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, 0.3);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, 7.0);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::FOLLOW_RANGE, 64.0);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::STEP_HEIGHT, 1.0);
        mob_entity.living_entity.health.store(40.0);

        let entity = Self {
            mob_entity,
            neutral_data: NeutralData::default(),
            carried_block: AtomicCell::new(None),
            creepy: AtomicBool::new(false),
            provoked: AtomicBool::new(false),
            speed_boosted: AtomicBool::new(false),
            target_change_time: AtomicCell::new(0),
            last_stare_sound: AtomicCell::new(0),
        };
        let mob_arc = Arc::new(entity);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        let mut navigator = mob_arc
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.set_mob_dimensions(0.6, 2.9);
        navigator.set_pathfinding_malus(PathType::Water, -1.0);
        drop(navigator);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(ChasePlayerGoal::new(mob_arc.clone())));
            goal_selector.add_goal(2, Box::new(MeleeAttackGoal::new(1.0, false)));
            goal_selector.add_goal(
                7,
                Box::new(WaterAvoidingRandomStrollGoal::with_probability(1.0, 0.0)),
            );
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(10, Box::new(PlaceBlockGoal::new(mob_arc.clone())));
            goal_selector.add_goal(11, Box::new(PickUpBlockGoal::new(mob_arc.clone())));

            target_selector.add_goal(1, Box::new(TeleportTowardsPlayerGoal::new(mob_arc.clone())));
            target_selector.add_goal(2, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::ENDERMITE, true),
            );
            target_selector.add_goal(4, ResetUniversalAngerGoal::new(false));
        };

        mob_arc
    }

    pub fn teleport_randomly(&self) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let (x, y, z) = {
            let mut rng = self.get_random();
            (
                pos.x + (rng.random_range(0.0..1.0) - 0.5) * 64.0,
                pos.y + (rng.random_range(0i32..64) - 32) as f64,
                pos.z + (rng.random_range(0.0..1.0) - 0.5) * 64.0,
            )
        };

        self.teleport_to(x, y, z)
    }

    pub fn teleport_towards(&self, target: &dyn EntityBase) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let target_pos = target.get_entity().pos.load();

        let dx = pos.x - target_pos.x;
        let dy = (pos.y + ENDERMAN_BODY_Y_OFFSET) - (target_pos.y + PLAYER_EYE_HEIGHT);
        let dz = pos.z - target_pos.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if dist < 1e-6 {
            return false;
        }

        let nx = dx / dist;
        let ny = dy / dist;
        let nz = dz / dist;
        let (x, y, z) = {
            let mut rng = self.get_random();
            (
                pos.x + (rng.random_range(0.0..1.0) - 0.5) * 8.0 - nx * 16.0,
                pos.y + (rng.random_range(0i32..16) - 8) as f64 - ny * 16.0,
                pos.z + (rng.random_range(0.0..1.0) - 0.5) * 8.0 - nz * 16.0,
            )
        };

        self.teleport_to(x, y, z)
    }

    pub fn teleport_to(&self, x: f64, y: f64, z: f64) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let origin = entity.pos.load();
        let world = entity.world.load();

        let min_y = f64::from(world.dimension.min_y);
        let max_y = f64::from(world.dimension.min_y + world.dimension.height - 1);
        let mut target_y = y.clamp(min_y, max_y);

        let block_x = x.floor() as i32;
        let mut block_y = target_y.floor() as i32;
        let block_z = z.floor() as i32;
        let ground_pos = loop {
            let below_pos = BlockPos::new(block_x, block_y - 1, block_z);
            let below_state = world.get_block_state(&below_pos);
            if below_state.is_solid() {
                break Some(below_pos);
            }
            if block_y <= world.dimension.min_y {
                break None;
            }
            block_y -= 1;
            target_y = block_y as f64;
        };
        let Some(ground_pos) = ground_pos else {
            return false;
        };

        if world
            .get_fluid(&ground_pos)
            .has_tag(&tag::Fluid::MINECRAFT_WATER)
        {
            return false;
        }

        let bb = BoundingBox::new_from_pos(x, target_y, z, &entity.entity_dimension.load());
        if !world.is_space_empty(bb) || world.contains_any_liquid(bb) {
            return false;
        }

        let new_pos = Vector3::new(x, target_y, z);

        for pos in &[origin, new_pos] {
            world.spawn_particle(
                *pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                128,
                Particle::Portal,
            );
            world.play_sound(Sound::EntityEndermanTeleport, SoundCategory::Hostile, pos);
        }

        if let Some(server) = world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::entity::entity_teleport::EntityTeleportEvent::new(
                    entity.entity_id,
                    origin,
                    new_pos,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return false;
            }
        }

        entity.teleport(new_pos, None, None, &world);

        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();

        true
    }

    pub fn set_target(&self, target: Option<Arc<dyn EntityBase>>) {
        let is_some = target.is_some();
        let mut mob_target = self
            .mob_entity
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *mob_target = target;
        drop(mob_target);

        if is_some {
            let tick_count = self
                .mob_entity
                .living_entity
                .entity
                .world
                .load()
                .level_info
                .load()
                .day_time as u64;
            self.target_change_time.store(tick_count);
            self.set_creepy(true);
            // Use attribute modifier instead of direct speed arithmetic
            if !self.speed_boosted.swap(true, Ordering::Relaxed) {
                let living = &self.mob_entity.living_entity;
                let modifier = Modifier {
                    id: ENDERMAN_SPEED_BOOST_ID.to_string(),
                    amount: SPEED_BOOST,
                    operation: ModifierOperation::Add,
                    permanent: false,
                };

                living.update_attribute(&Attributes::MOVEMENT_SPEED, |inst| {
                    inst.add_or_replace_modifier(modifier);
                });

                crate::entity::attributes::send_attribute_updates_for_living(
                    living,
                    vec![Attributes::MOVEMENT_SPEED],
                );
            }
        } else {
            self.target_change_time.store(0);
            self.set_creepy(false);
            self.set_provoked(false);
            if self.speed_boosted.swap(false, Ordering::Relaxed) {
                let living = &self.mob_entity.living_entity;

                living.update_attribute(&Attributes::MOVEMENT_SPEED, |inst| {
                    inst.remove_modifier(ENDERMAN_SPEED_BOOST_ID);
                });

                crate::entity::attributes::send_attribute_updates_for_living(
                    living,
                    vec![Attributes::MOVEMENT_SPEED],
                );
            }
        }
    }

    /// Plays the Enderman stare sound when gazed at by a player (Enderman.java:176-183).
    pub fn play_stare_sound(&self, tick_count: u64) {
        let last = self.last_stare_sound.load();
        let entity = &self.mob_entity.living_entity.entity;
        if crate::entity::combat::should_enderman_play_stare_sound(
            tick_count,
            last,
            entity.silent.load(Ordering::Relaxed),
        ) {
            self.last_stare_sound.store(tick_count);
            entity.world.load().play_sound_fine(
                Sound::EntityEndermanStare,
                SoundCategory::Hostile,
                &entity.pos.load(),
                crate::entity::combat::ENDERMAN_STARE_SOUND_VOLUME,
                1.0,
            );
        }
    }

    pub fn set_creepy(&self, creepy: bool) {
        self.creepy.store(creepy, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(pumpkin_data::tracked_data::enderman::CREEPY, creepy);
    }

    pub fn is_creepy(&self) -> bool {
        self.creepy.load(Ordering::Relaxed)
    }

    pub fn set_provoked(&self, provoked: bool) {
        self.provoked.store(provoked, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(pumpkin_data::tracked_data::enderman::STARED_AT, provoked);
    }

    pub fn set_carried_block(&self, block_state: Option<BlockStateId>) {
        self.carried_block.store(block_state);
        let value = block_state.map_or(VarInt(0), |id| VarInt(id.as_u16() as i32));
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(pumpkin_data::tracked_data::enderman::CARRY_STATE, value);
    }

    pub fn get_carried_block(&self) -> Option<BlockStateId> {
        self.carried_block.load()
    }

    pub fn is_player_staring(&self, player: &Player) -> bool {
        let equipment = player.living_entity.entity_equipment.try_lock();
        if let Ok(equipment) = equipment
            && let Some(head_stack) = equipment.equipment.get(&EquipmentSlot::HEAD)
            && !head_stack.is_empty()
            && head_stack.item == &Item::CARVED_PUMPKIN
        {
            return false;
        }

        let entity = &self.mob_entity.living_entity.entity;
        let enderman_pos = entity.pos.load();
        let enderman_eye_y = enderman_pos.y + ENDERMAN_EYE_HEIGHT;

        let player_entity = player.get_entity();
        let player_pos = player_entity.pos.load();
        let player_eye_y = player_pos.y + PLAYER_EYE_HEIGHT;

        let pitch = player_entity.pitch.load().to_radians();
        let yaw = -player_entity.yaw.load().to_radians();

        let cos_pitch = pitch.cos();
        let look_dir = Vector3::new(
            (yaw.sin() * cos_pitch) as f64,
            (-pitch.sin()) as f64,
            (yaw.cos() * cos_pitch) as f64,
        );

        let dx = enderman_pos.x - player_pos.x;
        let dy = enderman_eye_y - player_eye_y;
        let dz = enderman_pos.z - player_pos.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        if distance < 0.1 {
            return false;
        }

        let dir_x = dx / distance;
        let dir_y = dy / distance;
        let dir_z = dz / distance;

        let dot = look_dir.x * dir_x + look_dir.y * dir_y + look_dir.z * dir_z;

        if dot <= 1.0 - 0.025 / distance {
            return false;
        }

        let enderman_eye_pos = Vector3::new(enderman_pos.x, enderman_eye_y, enderman_pos.z);
        let player_eye_pos = Vector3::new(player_pos.x, player_eye_y, player_pos.z);
        let world = entity.world.load();
        world
            .clip(&pumpkin_world::clip::ClipContext::new(
                enderman_eye_pos,
                player_eye_pos,
                pumpkin_world::clip::ClipBlock::Visual,
                pumpkin_world::clip::ClipFluid::None,
            ))
            .is_miss
    }
}

impl NeutralMob for EndermanEntity {
    fn get_neutral_data(&self) -> &NeutralData {
        &self.neutral_data
    }
}

impl CustomSound for EndermanEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityEndermanDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityEndermanHurt)
    }
}

impl Mob for EndermanEntity {
    fn as_enderman(&self) -> Option<&EndermanEntity> {
        Some(self)
    }

    fn as_neutral(&self) -> Option<&dyn NeutralMob> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn is_sensitive_to_water(&self) -> bool {
        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(block_state) = self.carried_block.load() {
            nbt.put_int("carriedBlockState", block_state.as_u16() as i32);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(block_state) = nbt.get_int("carriedBlockState") {
            self.set_carried_block(BlockStateId::from_raw(block_state as u16));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn set_mob_target(&self, target: Option<Arc<dyn EntityBase>>) {
        self.set_target(self.as_valid_target(target));
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load_full();

        // Carried block drop on death
        if !entity.is_alive() {
            if let Some(block_state) = self.carried_block.load() {
                self.set_carried_block(None);
                let block = pumpkin_data::Block::from_state_id(block_state);
                if block.item_id != 0
                    && let Some(item) = Item::from_id(block.item_id)
                {
                    world.drop_stack(
                        &entity.block_pos.load(),
                        pumpkin_data::item_stack::ItemStack::new(1, item),
                    );
                }
            }
            return;
        }

        let raining_at_feet = world.is_raining_at(&entity.block_pos.load());
        let raining_at_head = world.is_raining_at(&entity.bounding_box.load().max_block_pos());
        if entity.touching_water.load(Ordering::SeqCst) || raining_at_feet || raining_at_head {
            // Environmental water/rain damage has no attacker entity
            caller.damage_with_context(caller, 1.0, DamageType::DROWN, None, None, None);
        }

        // Vanilla sunlight avoidance (Enderman.java:247-254):
        // When in bright daylight and seeing sky, Enderman de-aggros and teleports away
        // if tickCount >= targetChangeTime + 600 and random * 30 < (br - 0.4) * 2.
        let current_tick = world.level_info.load().day_time as u64;
        let target_change = self.target_change_time.load();
        if self.mob_entity.is_in_daylight()
            && world.can_see_sky(&entity.block_pos.load())
            && crate::entity::combat::can_enderman_deaggro_in_sunlight(current_tick, target_change)
        {
            let br = 1.0f32;
            let rand_val = rand::random_range(0.0..1.0);
            if crate::entity::combat::should_enderman_daylight_teleport(br, rand_val) {
                self.set_target(None);
                self.teleport_randomly();
            }
        }

        // Vanilla ambient sound: screaming when creepy, normal ambient when calm
        if !entity.silent.load(Ordering::Relaxed) {
            if self.is_creepy() {
                if rand::random_range(0..80) == 0 {
                    world.play_sound(
                        Sound::EntityEndermanScream,
                        SoundCategory::Hostile,
                        &entity.pos.load(),
                    );
                }
            } else if rand::random_range(0..100) == 0 {
                world.play_sound(
                    Sound::EntityEndermanAmbient,
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                );
            }
        }
    }

    fn pre_damage(&self, damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        let is_projectile = is_projectile_damage(damage_type)
            || source.is_some_and(|s| {
                let et = s.get_entity().entity_type;
                et == &EntityType::ARROW
                    || et == &EntityType::SPECTRAL_ARROW
                    || et == &EntityType::TRIDENT
                    || et == &EntityType::SNOWBALL
                    || et == &EntityType::EGG
                    || et == &EntityType::WIND_CHARGE
                    || et == &EntityType::BREEZE_WIND_CHARGE
                    || et == &EntityType::SMALL_FIREBALL
                    || et == &EntityType::FIREBALL
                    || et == &EntityType::DRAGON_FIREBALL
                    || et == &EntityType::WITHER_SKULL
                    || et == &EntityType::SHULKER_BULLET
                    || et == &EntityType::LLAMA_SPIT
            });

        let is_passenger = self.mob_entity.living_entity.entity.has_vehicle();
        let (should_cancel, should_teleport) =
            crate::entity::combat::should_enderman_evade_damage(
                is_projectile,
                is_passenger,
                true,
                0,
            );

        if should_teleport {
            for _ in 0..64 {
                if self.teleport_randomly() {
                    break;
                }
            }
        }

        if should_cancel {
            return false;
        }

        true
    }

    fn on_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) {
        let has_living_attacker = source.is_some_and(|s| s.get_living_entity().is_some());
        let (_, should_teleport) = crate::entity::combat::should_enderman_evade_damage(
            false,
            false,
            has_living_attacker,
            rand::random::<u32>(),
        );

        if should_teleport {
            self.teleport_randomly();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_enderman_sunlight_and_stare_parity() {
        assert_eq!(crate::entity::combat::ENDERMAN_MIN_DEAGGRESSION_TIME, 600);
        assert_eq!(crate::entity::combat::ENDERMAN_DELAY_BETWEEN_CREEPY_STARE_SOUND, 400);
        assert_eq!(crate::entity::combat::ENDERMAN_STARE_SOUND_VOLUME, 2.5);

        assert!(crate::entity::combat::can_enderman_deaggro_in_sunlight(1000, 400));
        assert!(crate::entity::combat::can_enderman_deaggro_in_sunlight(1000, 0));
        assert!(!crate::entity::combat::can_enderman_deaggro_in_sunlight(999, 400));

        assert!(crate::entity::combat::should_enderman_daylight_teleport(1.0, 0.03));
        assert!(!crate::entity::combat::should_enderman_daylight_teleport(0.4, 0.01));

        assert!(crate::entity::combat::should_enderman_play_stare_sound(500, 100, false));
        assert!(!crate::entity::combat::should_enderman_play_stare_sound(499, 100, false));
        assert!(!crate::entity::combat::should_enderman_play_stare_sound(500, 100, true));
    }
}
