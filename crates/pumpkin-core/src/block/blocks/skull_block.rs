use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::entities::skull::SkullBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, OnNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
    PlacedArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::block_properties::{
    HorizontalFacing, SkeletonSkullLikeProperties, SkeletonWallSkullLikeProperties,
};
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId};
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

pub struct SkullBlock;

pub fn is_wall_skull(block_id: BlockId) -> bool {
    matches!(
        block_id,
        BlockId::SKELETON_WALL_SKULL
            | BlockId::PLAYER_WALL_HEAD
            | BlockId::ZOMBIE_WALL_HEAD
            | BlockId::CREEPER_WALL_HEAD
            | BlockId::PIGLIN_WALL_HEAD
            | BlockId::DRAGON_WALL_HEAD
            | BlockId::WITHER_SKELETON_WALL_SKULL
    )
}

pub fn to_wall_skull(block_id: BlockId) -> Option<&'static Block> {
    match block_id {
        BlockId::SKELETON_SKULL | BlockId::SKELETON_WALL_SKULL => Some(&Block::SKELETON_WALL_SKULL),
        BlockId::PLAYER_HEAD | BlockId::PLAYER_WALL_HEAD => Some(&Block::PLAYER_WALL_HEAD),
        BlockId::ZOMBIE_HEAD | BlockId::ZOMBIE_WALL_HEAD => Some(&Block::ZOMBIE_WALL_HEAD),
        BlockId::CREEPER_HEAD | BlockId::CREEPER_WALL_HEAD => Some(&Block::CREEPER_WALL_HEAD),
        BlockId::PIGLIN_HEAD | BlockId::PIGLIN_WALL_HEAD => Some(&Block::PIGLIN_WALL_HEAD),
        BlockId::DRAGON_HEAD | BlockId::DRAGON_WALL_HEAD => Some(&Block::DRAGON_WALL_HEAD),
        BlockId::WITHER_SKELETON_SKULL | BlockId::WITHER_SKELETON_WALL_SKULL => {
            Some(&Block::WITHER_SKELETON_WALL_SKULL)
        }
        _ => None,
    }
}

impl BlockMetadata for SkullBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SKELETON_SKULL,
            BlockId::PLAYER_HEAD,
            BlockId::ZOMBIE_HEAD,
            BlockId::CREEPER_HEAD,
            BlockId::PIGLIN_HEAD,
            BlockId::DRAGON_HEAD,
            BlockId::SKELETON_WALL_SKULL,
            BlockId::PLAYER_WALL_HEAD,
            BlockId::ZOMBIE_WALL_HEAD,
            BlockId::CREEPER_WALL_HEAD,
            BlockId::PIGLIN_WALL_HEAD,
            BlockId::DRAGON_WALL_HEAD,
        ]
        .into()
    }
}

impl BlockBehaviour for SkullBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = SkullBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let is_receiving_power = block_receives_redstone_power(args.world, args.position);

        if args.direction.is_horizontal() || is_wall_skull(args.block.id) {
            let wall_block = to_wall_skull(args.block.id).unwrap_or(args.block);
            let mut props = SkeletonWallSkullLikeProperties::default(wall_block);
            props.facing = args
                .direction
                .to_horizontal_facing()
                .unwrap_or(HorizontalFacing::North);
            props.powered = is_receiving_power;
            props.to_state_id(wall_block)
        } else {
            let mut props = SkeletonSkullLikeProperties::default(args.block);
            props.rotation = args.player.get_entity().get_rotation_16();
            props.powered = is_receiving_power;
            props.to_state_id(args.block)
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let is_receiving_power = block_receives_redstone_power(args.world, args.position);

        if is_wall_skull(args.block.id) {
            let mut props = SkeletonWallSkullLikeProperties::from_state_id(state.id);
            if props.powered != is_receiving_power {
                props.powered = is_receiving_power;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_LISTENERS,
                );
            }
        } else {
            let mut props = SkeletonSkullLikeProperties::from_state_id(state.id);
            if props.powered != is_receiving_power {
                props.powered = is_receiving_power;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_LISTENERS,
                );
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

