use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicI64, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::block_properties::{
    Axis, CreakingHeartLikeProperties, CreakingHeartState,
    ResinClumpProperties, WaterProperties,
};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::CParticle;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;
use uuid::Uuid;

use crate::world::World;

use super::BlockEntity;
use crate::block::blocks::creaking_heart::CreakingHeartBlock;

pub struct CreakingHeartBlockEntity {
    pub position: BlockPos,
    pub creaking_uuid: AtomicCell<Option<Uuid>>,
    pub ticks_existed: AtomicI64,
    pub ticker: AtomicI32,
    pub emitter: AtomicI32,
    pub emitter_target: AtomicCell<Option<Vector3<f64>>>,
    pub output_signal: AtomicI32,
}

impl BlockEntity for CreakingHeartBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let creaking_uuid = nbt
            .get_uuid("creaking")
            .or_else(|| {
                nbt.get_string("creaking")
                    .and_then(|uuid_str| Uuid::parse_str(uuid_str).ok())
            })
            .or_else(|| {
                nbt.get_string("creaking_uuid")
                    .and_then(|uuid_str| Uuid::parse_str(uuid_str).ok())
            });
        Self {
            position,
            creaking_uuid: AtomicCell::new(creaking_uuid),
            ticks_existed: AtomicI64::new(0),
            ticker: AtomicI32::new(20),
            emitter: AtomicI32::new(0),
            emitter_target: AtomicCell::new(None),
            output_signal: AtomicI32::new(0),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(uuid) = self.creaking_uuid.load() {
            nbt.put_uuid("creaking", uuid);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Some(uuid) = self.creaking_uuid.load() {
            nbt.put_uuid("creaking", uuid);
        }
        Some(nbt)
    }

    fn tick(&self, world: &Arc<World>) {
        self.ticks_existed.fetch_add(1, Ordering::Relaxed);

        // Update output signal based on creaking protector distance (CreakingHeartBlockEntity.java:74-78, 338-346)
        let computed_signal = if let Some(uuid) = self.creaking_uuid.load() {
            if let Some(entity) = world.get_entity_by_uuid(uuid) {
                let entity_pos = entity.get_entity().pos.load();
                let heart_bottom = Vector3::new(
                    self.position.0.x as f64 + 0.5,
                    self.position.0.y as f64,
                    self.position.0.z as f64 + 0.5,
                );
                let dist = (entity_pos - heart_bottom).length();
                self.compute_analog_output_signal(Some(dist))
            } else {
                0
            }
        } else {
            0
        };

        if self.output_signal.swap(computed_signal, Ordering::Relaxed) != computed_signal {
            world.update_neighbors_at(&self.position, &Block::CREAKING_HEART, None);
        }

        // Hurt emitter countdown (plays hurt sound every 10 ticks down from 100, moving from creaking to heart)
        let emitter = self.emitter.load(Ordering::Relaxed);
        if emitter > 0 {
            if emitter > 50 {
                self.emit_particles(world, 1, true);
                self.emit_particles(world, 1, false);
            }

            if emitter % 10 == 0 {
                if let Some(uuid) = self.creaking_uuid.load() {
                    if let Some(entity) = world.get_entity_by_uuid(uuid) {
                        let bbox = entity.get_entity().bounding_box.load();
                        let center = bbox.min + (bbox.max - bbox.min) * 0.5;
                        self.emitter_target.store(Some(center));
                    }
                }

                let sound_location = if let Some(emitter_target) = self.emitter_target.load() {
                    let heart_position = Vector3::new(
                        self.position.0.x as f64 + 0.5,
                        self.position.0.y as f64 + 0.5,
                        self.position.0.z as f64 + 0.5,
                    );
                    Self::compute_hurt_sound_location(heart_position, emitter_target, emitter)
                } else {
                    self.position.to_f64()
                };

                world.play_sound(
                    Sound::BlockCreakingHeartHurt,
                    SoundCategory::Blocks,
                    &sound_location,
                );
            }
            self.emitter.fetch_sub(1, Ordering::Relaxed);
        }

        // Ticker update (every 20 ticks)
        if self.ticker.fetch_sub(1, Ordering::Relaxed) <= 0 {
            self.ticker.store(20, Ordering::Relaxed);

            let state_id = world.get_block_state_id(&self.position);
            if state_id.to_block_id() != Block::CREAKING_HEART.id {
                return;
            }

            let mut props = CreakingHeartLikeProperties::from_state_id(state_id);
            let has_logs = Self::check_active_logs(world.as_ref(), &self.position, props.axis);

            let new_state = if !has_logs && self.creaking_uuid.load().is_none() {
                CreakingHeartState::Uprooted
            } else if has_logs {
                let is_night = world
                    .level_time
                    .try_lock()
                    .map_or(false, |time| time.is_night());
                if is_night {
                    CreakingHeartState::Awake
                } else {
                    CreakingHeartState::Dormant
                }
            } else {
                props.creaking_heart_state
            };

            if new_state != props.creaking_heart_state {
                props.creaking_heart_state = new_state;
                let new_state_id = props.to_state_id(&Block::CREAKING_HEART);
                world.set_block_state(&self.position, new_state_id, BlockFlags::NOTIFY_ALL);
            }

            // Spawn protector when Awake and no protector is assigned
            if new_state == CreakingHeartState::Awake && self.creaking_uuid.load().is_none() {
                let nearby_players = world.get_nearby_players(self.position.to_f64(), 32.0);
                if !nearby_players.is_empty() {
                    let creaking_uuid = Uuid::new_v4();
                    self.creaking_uuid.store(Some(creaking_uuid));
                    world.play_sound(
                        Sound::BlockCreakingHeartSpawn,
                        SoundCategory::Blocks,
                        &self.position.to_f64(),
                    );
                }
            } else if new_state == CreakingHeartState::Dormant && self.creaking_uuid.load().is_some() {
                // Clear creaking protector during daytime
                self.creaking_uuid.store(None);
            } else if new_state == CreakingHeartState::Uprooted {
                self.remove_creaking(world);
            }
        }
    }

    fn on_block_replaced(self: Arc<Self>, world: &Arc<World>, _position: &BlockPos) {
        self.remove_creaking(world);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CreakingHeartBlockEntity {
    pub const ID: &'static str = "minecraft:creaking_heart";

    pub const RESIN_SPREAD_MAX_DEPTH: usize = 2;
    pub const RESIN_SPREAD_MAX_COUNT: usize = 64;
    pub const TRAIL_COLOR_TOWARDS_CREAKING: i32 = 16545810;
    pub const TRAIL_COLOR_TOWARDS_HEART: i32 = 6250335;
    pub const RESIN_CLUMPS_MIN: i32 = 2;
    pub const RESIN_CLUMPS_MAX: i32 = 3;

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            creaking_uuid: AtomicCell::new(None),
            ticks_existed: AtomicI64::new(0),
            ticker: AtomicI32::new(20),
            emitter: AtomicI32::new(0),
            emitter_target: AtomicCell::new(None),
            output_signal: AtomicI32::new(0),
        }
    }

    pub fn is_protector(&self, creaking_uuid: Uuid) -> bool {
        self.creaking_uuid
            .load()
            .is_none_or(|uuid| uuid == creaking_uuid)
    }

    pub fn set_creaking_uuid(&self, uuid: Option<Uuid>) {
        self.creaking_uuid.store(uuid);
    }

    pub fn clear_creaking(&self) {
        self.creaking_uuid.store(None);
    }

    pub fn remove_creaking(&self, world: &Arc<World>) {
        if let Some(uuid) = self.creaking_uuid.load() {
            if let Some(entity) = world.get_entity_by_uuid(uuid) {
                if let Some(creaking) = entity
                    .cast_any()
                    .downcast_ref::<crate::entity::mob::creaking::CreakingEntity>()
                {
                    creaking.tear_down();
                } else {
                    entity.get_entity().remove();
                }
            }
        }
        self.clear_creaking();
    }

    pub fn creaking_hurt(&self, world: &Arc<World>) {
        // Vanilla CreakingHeartBlockEntity.java:237-250: only proceed if emitter <= 0
        if self.emitter.load(Ordering::Relaxed) <= 0 {
            self.emit_particles(world, 20, false);

            let state_id = world.get_block_state_id(&self.position);
            if state_id.to_block_id() == Block::CREAKING_HEART.id {
                let props = CreakingHeartLikeProperties::from_state_id(state_id);
                if props.creaking_heart_state == CreakingHeartState::Awake {
                    let num_clumps = rand::random_range(Self::RESIN_CLUMPS_MIN..=Self::RESIN_CLUMPS_MAX);
                    for _ in 0..num_clumps {
                        if self.spread_resin(world).is_none() {
                            break;
                        }
                    }
                }
            }

            self.emitter.store(100, Ordering::Relaxed);
            if let Some(uuid) = self.creaking_uuid.load() {
                if let Some(entity) = world.get_entity_by_uuid(uuid) {
                    let bbox = entity.get_entity().bounding_box.load();
                    let center = bbox.min + (bbox.max - bbox.min) * 0.5;
                    self.emitter_target.store(Some(center));
                }
            }
        }
    }

    pub fn spread_resin(&self, world: &Arc<World>) -> Option<BlockPos> {
        Self::spread_resin_at(world, &self.position)
    }

    /// BFS resin clump spreading algorithm mirroring vanilla CreakingHeartBlockEntity.java:252-282
    /// and BlockPos.java:breadthFirstTraversal(startPos, 2, 64, ...)
    pub fn spread_resin_at(world: &Arc<World>, start_pos: &BlockPos) -> Option<BlockPos> {
        use rand::seq::SliceRandom;
        use std::collections::{HashSet, VecDeque};

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut rng = rand::rng();

        queue.push_back((*start_pos, 0));
        let mut count = 0;

        let all_dirs = [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
            BlockDirection::Up,
            BlockDirection::Down,
        ];

        while let Some((current_pos, depth)) = queue.pop_front() {
            if !visited.insert(current_pos) {
                continue;
            }

            let current_state_id = world.get_block_state_id(&current_pos);
            let current_block_id = current_state_id.to_block_id();

            // Node processor: if current position is pale oak log, try to place resin clump on neighbors
            if Self::is_pale_oak_log(current_block_id) {
                let mut shuffled_dirs = all_dirs;
                shuffled_dirs.shuffle(&mut rng);

                for dir in shuffled_dirs {
                    let neighbour_pos = Self::get_relative(&current_pos, dir);
                    let neighbour_state_id = world.get_block_state_id(&neighbour_pos);
                    let neighbour_block_id = neighbour_state_id.to_block_id();
                    let opposite = dir.opposite();

                    let mut clump_props = None;
                    if neighbour_block_id == Block::AIR.id {
                        let mut props = ResinClumpProperties {
                            down: false,
                            east: false,
                            north: false,
                            south: false,
                            up: false,
                            waterlogged: false,
                            west: false,
                        };
                        Self::set_face_property(&mut props, opposite, true);
                        clump_props = Some(props);
                    } else if neighbour_block_id == Block::WATER.id {
                        let water_props = WaterProperties::from_state_id(neighbour_state_id);
                        if water_props.level == 0 {
                            let mut props = ResinClumpProperties {
                                down: false,
                                east: false,
                                north: false,
                                south: false,
                                up: false,
                                waterlogged: true,
                                west: false,
                            };
                            Self::set_face_property(&mut props, opposite, true);
                            clump_props = Some(props);
                        }
                    } else if neighbour_block_id == Block::RESIN_CLUMP.id {
                        let mut props = ResinClumpProperties::from_state_id(neighbour_state_id);
                        if !Self::get_face_property(&props, opposite) {
                            Self::set_face_property(&mut props, opposite, true);
                            clump_props = Some(props);
                        }
                    }

                    if let Some(props) = clump_props {
                        let new_state_id = props.to_state_id(&Block::RESIN_CLUMP);
                        world.set_block_state(&neighbour_pos, new_state_id, BlockFlags::NOTIFY_ALL);
                        world.play_sound(
                            Sound::BlockResinPlace,
                            SoundCategory::Blocks,
                            &neighbour_pos.to_f64(),
                        );
                        return Some(neighbour_pos);
                    }
                }
            }

            count += 1;
            if count >= Self::RESIN_SPREAD_MAX_COUNT {
                break;
            }

            if depth < Self::RESIN_SPREAD_MAX_DEPTH {
                let mut neighbour_dirs = all_dirs;
                neighbour_dirs.shuffle(&mut rng);
                for dir in neighbour_dirs {
                    let neighbour_pos = Self::get_relative(&current_pos, dir);
                    let neighbour_state_id = world.get_block_state_id(&neighbour_pos);
                    if Self::is_pale_oak_log(neighbour_state_id.to_block_id()) {
                        queue.push_back((neighbour_pos, depth + 1));
                    }
                }
            }
        }

        None
    }

    /// Emits trail particles between Creaking entity and Creaking Heart (CreakingHeartBlockEntity.java:284-306)
    pub fn emit_particles(&self, world: &Arc<World>, count: u32, towards_creaking: bool) {
        let Some(uuid) = self.creaking_uuid.load() else {
            return;
        };
        let Some(entity) = world.get_entity_by_uuid(uuid) else {
            return;
        };

        let color = if towards_creaking {
            Self::TRAIL_COLOR_TOWARDS_CREAKING
        } else {
            Self::TRAIL_COLOR_TOWARDS_HEART
        };
        let mut rng = rand::rng();
        let bbox = entity.get_entity().bounding_box.load();
        let x_size = (bbox.max.x - bbox.min.x).max(0.1);
        let y_size = (bbox.max.y - bbox.min.y).max(0.1);
        let z_size = (bbox.max.z - bbox.min.z).max(0.1);

        for _ in 0..count {
            let mut source = bbox.min
                + Vector3::new(
                    rng.random::<f64>() * x_size,
                    rng.random::<f64>() * y_size,
                    rng.random::<f64>() * z_size,
                );
            let mut destination = Vector3::new(
                self.position.0.x as f64 + rng.random::<f64>(),
                self.position.0.y as f64 + rng.random::<f64>(),
                self.position.0.z as f64 + rng.random::<f64>(),
            );
            if towards_creaking {
                std::mem::swap(&mut source, &mut destination);
            }

            let duration = rng.random_range(10..50);
            let mut data = Vec::with_capacity(32);
            data.extend_from_slice(&destination.x.to_be_bytes());
            data.extend_from_slice(&destination.y.to_be_bytes());
            data.extend_from_slice(&destination.z.to_be_bytes());
            data.extend_from_slice(&color.to_be_bytes());
            let mut varint_val = duration as u32;
            while (varint_val & !0x7F) != 0 {
                data.push(((varint_val & 0x7F) | 0x80) as u8);
                varint_val >>= 7;
            }
            data.push(varint_val as u8);

            let packet = CParticle {
                force_spawn: true,
                important: true,
                position: source,
                offset: Vector3::new(0.0, 0.0, 0.0),
                max_speed: 0.0,
                particle_count: 1,
                particle_id: VarInt(59), // Particle::Trail
                data: &data,
            };
            world.broadcast_packet_all(&packet);
        }
    }

    pub fn get_face_property(props: &ResinClumpProperties, dir: BlockDirection) -> bool {
        match dir {
            BlockDirection::Down => props.down,
            BlockDirection::Up => props.up,
            BlockDirection::North => props.north,
            BlockDirection::South => props.south,
            BlockDirection::West => props.west,
            BlockDirection::East => props.east,
        }
    }

    pub fn set_face_property(props: &mut ResinClumpProperties, dir: BlockDirection, val: bool) {
        match dir {
            BlockDirection::Down => props.down = val,
            BlockDirection::Up => props.up = val,
            BlockDirection::North => props.north = val,
            BlockDirection::South => props.south = val,
            BlockDirection::West => props.west = val,
            BlockDirection::East => props.east = val,
        }
    }

    pub fn compute_hurt_sound_location(
        heart_position: Vector3<f64>,
        emitter_target: Vector3<f64>,
        emitter: i32,
    ) -> Vector3<f64> {
        let progress = 0.2 + 0.8 * (100 - emitter) as f64 / 100.0;
        (heart_position - emitter_target) * progress + emitter_target
    }

    pub const fn is_pale_oak_log(id: BlockId) -> bool {
        matches!(
            id,
            BlockId::PALE_OAK_LOG
                | BlockId::PALE_OAK_WOOD
                | BlockId::STRIPPED_PALE_OAK_LOG
                | BlockId::STRIPPED_PALE_OAK_WOOD
        )
    }

    pub fn check_active_logs(
        world: &dyn pumpkin_world::world::BlockAccessor,
        pos: &BlockPos,
        axis: Axis,
    ) -> bool {
        let (pos_a, pos_b) = match axis {
            Axis::X => (pos.west(), pos.east()),
            Axis::Y => (pos.down(), pos.up()),
            Axis::Z => (pos.north(), pos.south()),
        };

        let block_a = world.get_block_state_id(&pos_a).to_block_id();
        let block_b = world.get_block_state_id(&pos_b).to_block_id();

        Self::is_pale_oak_log(block_a) && Self::is_pale_oak_log(block_b)
    }

    pub fn compute_analog_output_signal(&self, distance: Option<f64>) -> i32 {
        CreakingHeartBlock::calculate_analog_output_signal(distance)
    }

    fn get_relative(pos: &BlockPos, dir: BlockDirection) -> BlockPos {
        match dir {
            BlockDirection::North => pos.north(),
            BlockDirection::South => pos.south(),
            BlockDirection::East => pos.east(),
            BlockDirection::West => pos.west(),
            BlockDirection::Up => pos.up(),
            BlockDirection::Down => pos.down(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resin_clump_multiface_orientation_and_waterlogged() {
        // 1. Multiface initialization attached to pale oak log surface
        let mut props = ResinClumpProperties {
            down: false,
            east: false,
            north: false,
            south: false,
            up: false,
            waterlogged: false,
            west: false,
        };

        // When neighbor log is North, opposite face South attaches to log
        CreakingHeartBlockEntity::set_face_property(&mut props, BlockDirection::South, true);
        assert!(CreakingHeartBlockEntity::get_face_property(&props, BlockDirection::South));
        assert!(!CreakingHeartBlockEntity::get_face_property(&props, BlockDirection::North));
        assert!(!CreakingHeartBlockEntity::get_face_property(&props, BlockDirection::Up));

        // State ID roundtrip
        let state_id = props.to_state_id(&Block::RESIN_CLUMP);
        let parsed = ResinClumpProperties::from_state_id(state_id);
        assert!(parsed.south);
        assert!(!parsed.north);
        assert!(!parsed.waterlogged);

        // 2. Preserves existing faces when another side connects
        let mut multi_face = parsed;
        CreakingHeartBlockEntity::set_face_property(&mut multi_face, BlockDirection::Up, true);
        assert!(multi_face.south);
        assert!(multi_face.up);
        assert!(!multi_face.down);

        // 3. Water source converts to waterlogged clump
        let mut waterlogged_props = ResinClumpProperties {
            down: false,
            east: false,
            north: false,
            south: false,
            up: false,
            waterlogged: true,
            west: false,
        };
        CreakingHeartBlockEntity::set_face_property(&mut waterlogged_props, BlockDirection::Down, true);
        let wl_state_id = waterlogged_props.to_state_id(&Block::RESIN_CLUMP);
        let wl_parsed = ResinClumpProperties::from_state_id(wl_state_id);
        assert!(wl_parsed.waterlogged);
        assert!(wl_parsed.down);
    }

    #[test]
    fn test_creaking_heart_resin_bfs_and_trail_constants() {
        // Parity constants from CreakingHeartBlockEntity.java:233-306
        assert_eq!(CreakingHeartBlockEntity::RESIN_SPREAD_MAX_DEPTH, 2);
        assert_eq!(CreakingHeartBlockEntity::RESIN_SPREAD_MAX_COUNT, 64);
        assert_eq!(CreakingHeartBlockEntity::TRAIL_COLOR_TOWARDS_CREAKING, 16545810);
        assert_eq!(CreakingHeartBlockEntity::TRAIL_COLOR_TOWARDS_HEART, 6250335);
        assert_eq!(CreakingHeartBlockEntity::RESIN_CLUMPS_MIN, 2);
        assert_eq!(CreakingHeartBlockEntity::RESIN_CLUMPS_MAX, 3);
    }

    #[test]
    fn test_hurt_sound_location_interpolation() {
        let heart_pos = Vector3::new(10.0, 64.0, 10.0);
        let creaking_target = Vector3::new(0.0, 64.0, 0.0);

        // At emitter = 100 (beginning of hurt), progress = 0.2 (near creaking)
        let loc_100 = CreakingHeartBlockEntity::compute_hurt_sound_location(heart_pos, creaking_target, 100);
        assert!((loc_100.x - 2.0).abs() < 1e-5);
        assert!((loc_100.y - 64.0).abs() < 1e-5);
        assert!((loc_100.z - 2.0).abs() < 1e-5);

        // At emitter = 0 (end of hurt trail), progress = 1.0 (at heart)
        let loc_0 = CreakingHeartBlockEntity::compute_hurt_sound_location(heart_pos, creaking_target, 0);
        assert!((loc_0.x - 10.0).abs() < 1e-5);
        assert!((loc_0.y - 64.0).abs() < 1e-5);
        assert!((loc_0.z - 10.0).abs() < 1e-5);
    }
}
