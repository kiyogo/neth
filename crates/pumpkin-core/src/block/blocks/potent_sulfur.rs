use pumpkin_data::block_properties::{PotentSulfurLikeProperties, PotentSulfurState};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{BlockBehaviour, OnNeighborUpdateArgs, OnPlaceArgs, PlacedArgs};

#[pumpkin_block("minecraft:potent_sulfur")]
pub struct PotentSulfurBlock;

impl PotentSulfurBlock {
    #[must_use]
    pub fn valid_block_state(
        block: &Block,
        world: &dyn BlockAccessor,
        pos: &BlockPos,
    ) -> BlockStateId {
        let above = pos.up();
        let block_above = world.get_block_state_id(&above).to_block();
        let is_water_above = *block_above == Block::WATER;

        let mut props = PotentSulfurLikeProperties::default(block);
        if !is_water_above {
            props.r#potent_sulfur_state = PotentSulfurState::Dry;
            return props.to_state_id(block);
        }

        let below = pos.down();
        let block_below = world.get_block_state_id(&below).to_block();
        if *block_below == Block::LAVA {
            props.r#potent_sulfur_state = PotentSulfurState::Continuous;
        } else if *block_below == Block::MAGMA_BLOCK {
            let current_state =
                PotentSulfurLikeProperties::from_state_id(world.get_block_state_id(pos))
                    .r#potent_sulfur_state;
            if current_state == PotentSulfurState::Erupting {
                props.r#potent_sulfur_state = PotentSulfurState::Erupting;
            } else {
                props.r#potent_sulfur_state = PotentSulfurState::Dormant;
            }
        } else {
            props.r#potent_sulfur_state = PotentSulfurState::Wet;
        }

        props.to_state_id(block)
    }
}

impl BlockBehaviour for PotentSulfurBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        Self::valid_block_state(args.block, args.world, args.position)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let props = PotentSulfurLikeProperties::from_state_id(state_id);
        if props.r#potent_sulfur_state == PotentSulfurState::Erupting
            || props.r#potent_sulfur_state == PotentSulfurState::Continuous
        {
            let sound = if props.r#potent_sulfur_state == PotentSulfurState::Continuous {
                Sound::BlockPotentSulfurGeyserContinuousEruption
            } else {
                Sound::BlockPotentSulfurGeyserEruption
            };
            args.world
                .play_sound(sound, SoundCategory::Blocks, &args.position.to_centered_f64());
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let new_state_id =
            Self::valid_block_state(args.block, args.world.as_ref(), args.position);
        if state_id != new_state_id {
            args.world
                .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL);
        }
    }
}
