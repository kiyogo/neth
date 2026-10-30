use pumpkin_macros::pumpkin_block;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use crate::block::{BlockBehaviour, OnNeighborUpdateArgs, PlacedArgs};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block("minecraft:sponge")]
pub struct SpongeBlock;

impl SpongeBlock {
    /// Vanilla `SpongeBlock.java:16-17`:
    pub const MAX_DEPTH: i32 = 6;
    pub const MAX_COUNT: usize = 64;

    pub fn absorb_water(world: &Arc<crate::world::World>, position: &BlockPos) -> bool {
        let mut water_blocks = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start from the sponge position with depth 0
        queue.push_back((*position, 0));
        visited.insert(*position);

        while let Some((current_pos, depth)) = queue.pop_front() {
            if depth >= Self::MAX_DEPTH || water_blocks.len() >= Self::MAX_COUNT {
                continue;
            }

            for direction in &[
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                if water_blocks.len() >= Self::MAX_COUNT {
                    break;
                }

                let next_pos = BlockPos::new(
                    current_pos.0.x + direction.0,
                    current_pos.0.y + direction.1,
                    current_pos.0.z + direction.2,
                );

                if visited.contains(&next_pos) {
                    continue;
                }

                visited.insert(next_pos);
                let (block, _state) = world.get_block_and_state(&next_pos);

                // Only add to queue if it's water.
                if block.id == Block::WATER.id {
                    water_blocks.push(next_pos);
                    queue.push_back((next_pos, depth + 1));
                }
            }
        }

        if water_blocks.is_empty() {
            false
        } else {
            let mut event =
                crate::plugin::api::events::block::sponge_absorb::SpongeAbsorbEvent::new(*position);
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return false;
            }

            for water_pos in &water_blocks {
                world.set_block_state(water_pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            }
            world.set_block_state(
                position,
                Block::WET_SPONGE.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );

            world.play_block_sound(Sound::BlockSpongeAbsorb, SoundCategory::Blocks, *position);

            true
        }
    }
}

impl BlockBehaviour for SpongeBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        // Attempt to absorb water on placement
        Self::absorb_water(args.world, args.position);
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        // If a neighboring block changed and it's water, attempt to absorb.
        if args.source_block.id == Block::WATER.id {
            Self::absorb_water(args.world, args.position);
        }
    }
}

#[pumpkin_block("minecraft:wet_sponge")]
pub struct WetSpongeBlock;

impl WetSpongeBlock {
    pub const EVENT_WATER_EVAPORATE: u32 = 2009;
}

impl BlockBehaviour for WetSpongeBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        // Check if placed where water evaporates (EnvironmentAttributes.WATER_EVAPORATES), if so, dry out
        if args.world.dimension.water_evaporates {
            args.world.set_block_state(
                args.position,
                Block::SPONGE.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );

            args.world.sync_world_event(
                WorldEvent::ParticlesWaterEvaporating,
                *args.position,
                0,
            );

            let pitch = (1.0 + rand::random::<f32>() * 0.2) * 0.7;
            args.world.play_sound_fine(
                Sound::BlockWetSpongeDries,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
                1.0,
                pitch,
            );
        }
    }
}
