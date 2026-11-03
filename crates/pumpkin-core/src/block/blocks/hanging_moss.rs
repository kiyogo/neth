use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, PlacedArgs, RandomTickArgs,
};
use pumpkin_data::block_properties::PaleHangingMossLikeProperties;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

/// Idle ambient sound chance: 1 in 500 ticks when hanging from pale oak logs or leaves.
/// Mirroring vanilla `HangingMossBlock.java:39`: `if (random.nextInt(500) == 0)`.
pub const PALE_HANGING_MOSS_IDLE_SOUND_CHANCE: u32 = 500;

/// Centered width for hanging moss bounding box (14 / 16 = 0.875).
/// Mirroring vanilla `HangingMossBlock.java:23-24`: `Block.column(14.0, ...)`.
pub const PALE_HANGING_MOSS_WIDTH: f64 = 14.0 / 16.0;

/// Base hanging moss height (16 / 16 = 1.0).
pub const PALE_HANGING_MOSS_BASE_HEIGHT: f64 = 1.0;

/// Tip hanging moss height (14 / 16 = 0.875, from y = 2/16 to 16/16).
pub const PALE_HANGING_MOSS_TIP_HEIGHT: f64 = 14.0 / 16.0;

pub struct HangingMossBlock;

impl HangingMossBlock {
    pub const IDLE_SOUND_CHANCE: u32 = PALE_HANGING_MOSS_IDLE_SOUND_CHANCE;
    pub const WIDTH: f64 = PALE_HANGING_MOSS_WIDTH;
    pub const BASE_HEIGHT: f64 = PALE_HANGING_MOSS_BASE_HEIGHT;
    pub const TIP_HEIGHT: f64 = PALE_HANGING_MOSS_TIP_HEIGHT;

    #[must_use]
    pub fn should_play_idle_sound(above_block: &Block, roll: u32) -> bool {
        should_play_idle_sound(above_block, roll)
    }

    #[must_use]
    pub fn is_pale_oak_support(block: &Block) -> bool {
        is_pale_oak_support(block)
    }
}

impl BlockMetadata for HangingMossBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::PALE_HANGING_MOSS].into()
    }
}

pub fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let above = pos.up();
    let (block_above, state_above) = world.get_block_and_state(&above);
    block_above == &Block::PALE_HANGING_MOSS || state_above.is_side_solid(BlockDirection::Down)
}

/// Checks whether the supporting block directly above counts as a pale oak tree log/leaf
/// for emitting ambient idle sounds, mirroring vanilla `HangingMossBlock.java:41`:
/// `above.is(BlockTags.PALE_OAK_LOGS) || above.is(Blocks.PALE_OAK_LEAVES)`.
#[must_use]
pub fn is_pale_oak_support(block: &Block) -> bool {
    block == &Block::PALE_OAK_LOG
        || block == &Block::STRIPPED_PALE_OAK_LOG
        || block == &Block::PALE_OAK_WOOD
        || block == &Block::STRIPPED_PALE_OAK_WOOD
        || block == &Block::PALE_OAK_LEAVES
}

/// Checks whether the ambient idle sound should play, mirroring vanilla `HangingMossBlock.java:39-44`.
#[must_use]
pub fn should_play_idle_sound(above_block: &Block, roll: u32) -> bool {
    roll == 0 && is_pale_oak_support(above_block)
}

impl BlockBehaviour for HangingMossBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if can_survive(args.world, args.position) {
            let below = args.position.down();
            let block_below = args.world.get_block(&below);
            let mut props = PaleHangingMossLikeProperties::default(args.block);
            props.tip = block_below != &Block::PALE_HANGING_MOSS;
            props.to_state_id(args.block)
        } else {
            BlockStateId::AIR
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_survive(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !can_survive(args.world, args.position) {
            // Parity with vanilla HangingMossBlock.java:74-76:
            // Schedule a 1-tick delay before breaking instead of instantly disappearing.
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            args.state_id
        } else {
            let below = args.position.down();
            let block_below = args.world.get_block(&below);
            let mut props = PaleHangingMossLikeProperties::from_state_id(args.state_id);
            props.tip = block_below != &Block::PALE_HANGING_MOSS;
            props.to_state_id(args.block)
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_survive(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let mut rng = rand::rng();
        let above = args.world.get_block(&args.position.up());
        if should_play_idle_sound(above, rng.random_range(0..PALE_HANGING_MOSS_IDLE_SOUND_CHANCE)) {
            args.world.play_sound(
                Sound::BlockPaleHangingMossIdle,
                SoundCategory::Ambient,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let mut cur = *args.position;
        while args.world.get_block(&cur) == &Block::PALE_HANGING_MOSS {
            cur = cur.down();
        }
        args.world.get_block_state(&cur).is_air()
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let mut cur = *args.position;
        while args.world.get_block(&cur) == &Block::PALE_HANGING_MOSS {
            cur = cur.down();
        }
        if args.world.get_block_state(&cur).is_air() {
            let mut props = PaleHangingMossLikeProperties::default(args.block);
            props.tip = true;
            args.world.set_block_state(
                &cur,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            // Update above block from tip to body if it was hanging moss
            let above = cur.up();
            if args.world.get_block(&above) == &Block::PALE_HANGING_MOSS {
                let mut above_props = PaleHangingMossLikeProperties::default(args.block);
                above_props.tip = false;
                args.world.set_block_state(
                    &above,
                    above_props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Block, HangingMossBlock, PALE_HANGING_MOSS_BASE_HEIGHT,
        PALE_HANGING_MOSS_IDLE_SOUND_CHANCE, PALE_HANGING_MOSS_TIP_HEIGHT,
        PALE_HANGING_MOSS_WIDTH, is_pale_oak_support, should_play_idle_sound,
    };

    #[test]
    fn test_pale_hanging_moss_parity() {
        assert_eq!(PALE_HANGING_MOSS_IDLE_SOUND_CHANCE, 500);
        assert_eq!(HangingMossBlock::IDLE_SOUND_CHANCE, 500);

        // Dimensions
        assert_eq!(PALE_HANGING_MOSS_WIDTH, 14.0 / 16.0);
        assert_eq!(PALE_HANGING_MOSS_BASE_HEIGHT, 1.0);
        assert_eq!(PALE_HANGING_MOSS_TIP_HEIGHT, 14.0 / 16.0);

        // Supporting blocks
        assert!(is_pale_oak_support(&Block::PALE_OAK_LOG));
        assert!(is_pale_oak_support(&Block::STRIPPED_PALE_OAK_LOG));
        assert!(is_pale_oak_support(&Block::PALE_OAK_WOOD));
        assert!(is_pale_oak_support(&Block::STRIPPED_PALE_OAK_WOOD));
        assert!(is_pale_oak_support(&Block::PALE_OAK_LEAVES));
        assert!(!is_pale_oak_support(&Block::OAK_LOG));
        assert!(!is_pale_oak_support(&Block::STONE));

        // Idle sound conditions
        assert!(should_play_idle_sound(&Block::PALE_OAK_LOG, 0));
        assert!(should_play_idle_sound(&Block::PALE_OAK_LEAVES, 0));
        assert!(!should_play_idle_sound(&Block::PALE_OAK_LOG, 1));
        assert!(!should_play_idle_sound(&Block::OAK_LOG, 0));
    }
}
