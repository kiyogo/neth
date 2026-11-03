use pumpkin_data::{BlockId, BlockStateId};

use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, OnPlaceArgs, PlacedArgs};

type LogProperties = pumpkin_data::block_properties::PaleOakWoodLikeProperties;

pub struct LogBlock;

impl BlockMetadata for LogBlock {
    fn ids() -> Box<[BlockId]> {
        let tag_ids = pumpkin_data::tag::get_tag_ids(
            pumpkin_data::tag::RegistryKey::Block,
            "minecraft:logs",
        )
        .unwrap_or_default();
        let mut ids: Vec<BlockId> = tag_ids
            .iter()
            .copied()
            .map(BlockId::new_or_air)
            .collect();
        ids.extend_from_slice(&[
            BlockId::MUDDY_MANGROVE_ROOTS,
            BlockId::BAMBOO_BLOCK,
            BlockId::STRIPPED_BAMBOO_BLOCK,
            BlockId::BASALT,
            BlockId::POLISHED_BASALT,
            BlockId::QUARTZ_PILLAR,
            BlockId::PURPUR_PILLAR,
            BlockId::BONE_BLOCK,
            BlockId::DEEPSLATE,
            BlockId::INFESTED_DEEPSLATE,
            BlockId::OCHRE_FROGLIGHT,
            BlockId::VERDANT_FROGLIGHT,
            BlockId::PEARLESCENT_FROGLIGHT,
        ]);
        ids.into_boxed_slice()
    }
}

impl BlockBehaviour for LogBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut log_props = LogProperties::default(args.block);
        log_props.axis = args.direction.to_axis();

        log_props.to_state_id(args.block)
    }
}

