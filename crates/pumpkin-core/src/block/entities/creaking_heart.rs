use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicI64, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::block_properties::{Axis, CreakingHeartLikeProperties, CreakingHeartState};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use uuid::Uuid;

use crate::world::World;

use super::BlockEntity;

pub struct CreakingHeartBlockEntity {
    pub position: BlockPos,
    pub creaking_uuid: AtomicCell<Option<Uuid>>,
    pub ticks_existed: AtomicI64,
    pub ticker: AtomicI32,
    pub emitter: AtomicI32,
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

        // Hurt emitter countdown (plays hurt sound every 10 ticks down from 100)
        let emitter = self.emitter.load(Ordering::Relaxed);
        if emitter > 0 {
            if emitter % 10 == 0 {
                world.play_sound(
                    Sound::BlockCreakingHeartHurt,
                    SoundCategory::Blocks,
                    &self.position.to_f64(),
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

            if props.creaking_heart_state != new_state {
                props.creaking_heart_state = new_state;
                world.set_block_state(
                    &self.position,
                    props.to_state_id(&Block::CREAKING_HEART),
                    BlockFlags::NOTIFY_ALL,
                );
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
            }
        }
    }

    fn on_block_replaced(self: Arc<Self>, _world: &Arc<World>, _position: &BlockPos) {
        self.clear_creaking();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CreakingHeartBlockEntity {
    pub const ID: &'static str = "minecraft:creaking_heart";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            creaking_uuid: AtomicCell::new(None),
            ticks_existed: AtomicI64::new(0),
            ticker: AtomicI32::new(20),
            emitter: AtomicI32::new(0),
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

    pub fn creaking_hurt(&self, world: &Arc<World>) {
        world.play_sound(
            Sound::BlockCreakingHeartHurt,
            SoundCategory::Blocks,
            &self.position.to_f64(),
        );
        self.emitter.store(100, Ordering::Relaxed);

        let state_id = world.get_block_state_id(&self.position);
        if state_id.to_block_id() == Block::CREAKING_HEART.id {
            let props = CreakingHeartLikeProperties::from_state_id(state_id);
            if props.creaking_heart_state == CreakingHeartState::Awake {
                self.spread_resin(world);
            }
        }
    }

    fn spread_resin(&self, world: &Arc<World>) {
        let directions = [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
            BlockDirection::Up,
            BlockDirection::Down,
        ];
        let mut placed = 0;
        for &dir in &directions {
            let log_pos = Self::get_relative(&self.position, dir);
            let log_id = world.get_block_state_id(&log_pos).to_block_id();
            if Self::is_pale_oak_log(log_id) {
                for &resin_dir in &directions {
                    let resin_pos = Self::get_relative(&log_pos, resin_dir);
                    let block_id = world.get_block_state_id(&resin_pos).to_block_id();
                    if block_id == Block::AIR.id {
                        world.set_block_state(
                            &resin_pos,
                            Block::RESIN_CLUMP.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        );
                        world.play_sound(
                            Sound::BlockResinPlace,
                            SoundCategory::Blocks,
                            &resin_pos.to_f64(),
                        );
                        placed += 1;
                        if placed >= 2 {
                            return;
                        }
                    }
                }
            }
        }
    }

    pub const fn is_pale_oak_log(id: BlockId) -> bool {
        matches!(id, BlockId::PALE_OAK_LOG | BlockId::STRIPPED_PALE_OAK_LOG)
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
        if let Some(dist) = distance {
            let clamped = dist.clamp(0.0, 32.0) / 32.0;
            15 - (clamped * 15.0).floor() as i32
        } else {
            0
        }
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
