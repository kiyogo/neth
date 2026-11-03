use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, PlacedArgs};
use pumpkin_data::block_properties::NoteblockInstrument;
use pumpkin_data::sound::Sound;
use pumpkin_data::BlockId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuffSoundType {
    Tuff,
    PolishedTuff,
    TuffBricks,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TuffSoundGroup {
    pub sound_type: TuffSoundType,
    pub break_sound: Sound,
    pub step_sound: Sound,
    pub place_sound: Sound,
    pub hit_sound: Sound,
    pub fall_sound: Sound,
    pub volume: f32,
    pub pitch: f32,
}

pub struct TuffBlock;

impl TuffBlock {
    /// Vanilla: Blocks.java:4935 strength(1.5F, 6.0F)
    pub const HARDNESS: f32 = 1.5;
    pub const BLAST_RESISTANCE: f32 = 6.0;

    /// Vanilla: NoteBlockInstrument.BASEDRUM (Blocks.java:4932)
    pub const INSTRUMENT: NoteblockInstrument = NoteblockInstrument::Basedrum;

    /// Vanilla: SoundType.java:322-342 volume and pitch are 1.0F
    pub const SOUND_VOLUME: f32 = 1.0;
    pub const SOUND_PITCH: f32 = 1.0;

    /// All 14 Tuff family blocks introduced/expanded in 1.21 update
    pub const TUFF_FAMILY: [BlockId; 14] = [
        BlockId::TUFF,
        BlockId::TUFF_SLAB,
        BlockId::TUFF_STAIRS,
        BlockId::TUFF_WALL,
        BlockId::CHISELED_TUFF,
        BlockId::POLISHED_TUFF,
        BlockId::POLISHED_TUFF_SLAB,
        BlockId::POLISHED_TUFF_STAIRS,
        BlockId::POLISHED_TUFF_WALL,
        BlockId::TUFF_BRICKS,
        BlockId::TUFF_BRICK_SLAB,
        BlockId::TUFF_BRICK_STAIRS,
        BlockId::TUFF_BRICK_WALL,
        BlockId::CHISELED_TUFF_BRICKS,
    ];

    #[must_use]
    pub const fn is_tuff_family(block_id: BlockId) -> bool {
        matches!(
            block_id,
            BlockId::TUFF
                | BlockId::TUFF_SLAB
                | BlockId::TUFF_STAIRS
                | BlockId::TUFF_WALL
                | BlockId::CHISELED_TUFF
                | BlockId::POLISHED_TUFF
                | BlockId::POLISHED_TUFF_SLAB
                | BlockId::POLISHED_TUFF_STAIRS
                | BlockId::POLISHED_TUFF_WALL
                | BlockId::TUFF_BRICKS
                | BlockId::TUFF_BRICK_SLAB
                | BlockId::TUFF_BRICK_STAIRS
                | BlockId::TUFF_BRICK_WALL
                | BlockId::CHISELED_TUFF_BRICKS
        )
    }

    #[must_use]
    pub const fn sound_type_for_block(block_id: BlockId) -> Option<TuffSoundType> {
        match block_id {
            BlockId::TUFF
            | BlockId::TUFF_SLAB
            | BlockId::TUFF_STAIRS
            | BlockId::TUFF_WALL
            | BlockId::CHISELED_TUFF => Some(TuffSoundType::Tuff),

            BlockId::POLISHED_TUFF
            | BlockId::POLISHED_TUFF_SLAB
            | BlockId::POLISHED_TUFF_STAIRS
            | BlockId::POLISHED_TUFF_WALL => Some(TuffSoundType::PolishedTuff),

            BlockId::TUFF_BRICKS
            | BlockId::TUFF_BRICK_SLAB
            | BlockId::TUFF_BRICK_STAIRS
            | BlockId::TUFF_BRICK_WALL
            | BlockId::CHISELED_TUFF_BRICKS => Some(TuffSoundType::TuffBricks),

            _ => None,
        }
    }

    #[must_use]
    pub const fn sound_group_for_type(sound_type: TuffSoundType) -> TuffSoundGroup {
        match sound_type {
            TuffSoundType::Tuff => TuffSoundGroup {
                sound_type,
                break_sound: Sound::BlockTuffBreak,
                step_sound: Sound::BlockTuffStep,
                place_sound: Sound::BlockTuffPlace,
                hit_sound: Sound::BlockTuffHit,
                fall_sound: Sound::BlockTuffFall,
                volume: Self::SOUND_VOLUME,
                pitch: Self::SOUND_PITCH,
            },
            TuffSoundType::PolishedTuff => TuffSoundGroup {
                sound_type,
                break_sound: Sound::BlockPolishedTuffBreak,
                step_sound: Sound::BlockPolishedTuffStep,
                place_sound: Sound::BlockPolishedTuffPlace,
                hit_sound: Sound::BlockPolishedTuffHit,
                fall_sound: Sound::BlockPolishedTuffFall,
                volume: Self::SOUND_VOLUME,
                pitch: Self::SOUND_PITCH,
            },
            TuffSoundType::TuffBricks => TuffSoundGroup {
                sound_type,
                break_sound: Sound::BlockTuffBricksBreak,
                step_sound: Sound::BlockTuffBricksStep,
                place_sound: Sound::BlockTuffBricksPlace,
                hit_sound: Sound::BlockTuffBricksHit,
                fall_sound: Sound::BlockTuffBricksFall,
                volume: Self::SOUND_VOLUME,
                pitch: Self::SOUND_PITCH,
            },
        }
    }

    #[must_use]
    pub const fn sound_group_for_block(block_id: BlockId) -> Option<TuffSoundGroup> {
        match Self::sound_type_for_block(block_id) {
            Some(st) => Some(Self::sound_group_for_type(st)),
            None => None,
        }
    }

    /// Validates if an input block can be cut into an output block via Stonecutter (Vanilla recipes)
    #[must_use]
    pub const fn is_valid_stonecutting_recipe(input: BlockId, output: BlockId) -> bool {
        match input {
            BlockId::TUFF => matches!(
                output,
                BlockId::CHISELED_TUFF
                    | BlockId::POLISHED_TUFF
                    | BlockId::POLISHED_TUFF_SLAB
                    | BlockId::POLISHED_TUFF_STAIRS
                    | BlockId::POLISHED_TUFF_WALL
                    | BlockId::TUFF_BRICKS
                    | BlockId::TUFF_BRICK_SLAB
                    | BlockId::TUFF_BRICK_STAIRS
                    | BlockId::TUFF_BRICK_WALL
                    | BlockId::CHISELED_TUFF_BRICKS
                    | BlockId::TUFF_SLAB
                    | BlockId::TUFF_STAIRS
                    | BlockId::TUFF_WALL
            ),
            BlockId::POLISHED_TUFF => matches!(
                output,
                BlockId::POLISHED_TUFF_SLAB
                    | BlockId::POLISHED_TUFF_STAIRS
                    | BlockId::POLISHED_TUFF_WALL
                    | BlockId::TUFF_BRICKS
                    | BlockId::TUFF_BRICK_SLAB
                    | BlockId::TUFF_BRICK_STAIRS
                    | BlockId::TUFF_BRICK_WALL
                    | BlockId::CHISELED_TUFF_BRICKS
            ),
            BlockId::TUFF_BRICKS => matches!(
                output,
                BlockId::TUFF_BRICK_SLAB
                    | BlockId::TUFF_BRICK_STAIRS
                    | BlockId::TUFF_BRICK_WALL
                    | BlockId::CHISELED_TUFF_BRICKS
            ),
            _ => false,
        }
    }
}

impl BlockMetadata for TuffBlock {
    fn ids() -> Box<[BlockId]> {
        Box::new(TuffBlock::TUFF_FAMILY)
    }
}

impl BlockBehaviour for TuffBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn test_tuff_family_properties_sounds_and_stonecutting_parity() {
        // 1. Constants parity (Blocks.java:4930-4949)
        assert_eq!(TuffBlock::HARDNESS, 1.5);
        assert_eq!(TuffBlock::BLAST_RESISTANCE, 6.0);
        assert_eq!(TuffBlock::INSTRUMENT, NoteblockInstrument::Basedrum);
        assert_eq!(TuffBlock::SOUND_VOLUME, 1.0);
        assert_eq!(TuffBlock::SOUND_PITCH, 1.0);

        // 2. Family identification & registry
        assert_eq!(TuffBlock::TUFF_FAMILY.len(), 14);
        for id in TuffBlock::TUFF_FAMILY {
            assert!(TuffBlock::is_tuff_family(id));
            let block = Block::from_id(id);
            assert_eq!(block.hardness, 1.5);
            assert_eq!(block.blast_resistance, 6.0);
            assert_eq!(block.default_state.instrument, NoteblockInstrument::Basedrum);
        }
        assert!(!TuffBlock::is_tuff_family(BlockId::STONE));
        assert!(!TuffBlock::is_tuff_family(BlockId::HEAVY_CORE));

        // 3. Sound type mappings parity
        // Base Tuff sound type
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF), Some(TuffSoundType::Tuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_SLAB), Some(TuffSoundType::Tuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_STAIRS), Some(TuffSoundType::Tuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_WALL), Some(TuffSoundType::Tuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::CHISELED_TUFF), Some(TuffSoundType::Tuff));

        // Polished Tuff sound type
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::POLISHED_TUFF), Some(TuffSoundType::PolishedTuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::POLISHED_TUFF_SLAB), Some(TuffSoundType::PolishedTuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::POLISHED_TUFF_STAIRS), Some(TuffSoundType::PolishedTuff));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::POLISHED_TUFF_WALL), Some(TuffSoundType::PolishedTuff));

        // Tuff Bricks sound type
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_BRICKS), Some(TuffSoundType::TuffBricks));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_BRICK_SLAB), Some(TuffSoundType::TuffBricks));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_BRICK_STAIRS), Some(TuffSoundType::TuffBricks));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::TUFF_BRICK_WALL), Some(TuffSoundType::TuffBricks));
        assert_eq!(TuffBlock::sound_type_for_block(BlockId::CHISELED_TUFF_BRICKS), Some(TuffSoundType::TuffBricks));

        // 4. Sound events parity
        let tuff_group = TuffBlock::sound_group_for_block(BlockId::TUFF).unwrap();
        assert_eq!(tuff_group.break_sound, Sound::BlockTuffBreak);
        assert_eq!(tuff_group.step_sound, Sound::BlockTuffStep);
        assert_eq!(tuff_group.place_sound, Sound::BlockTuffPlace);
        assert_eq!(tuff_group.hit_sound, Sound::BlockTuffHit);
        assert_eq!(tuff_group.fall_sound, Sound::BlockTuffFall);

        let polished_group = TuffBlock::sound_group_for_block(BlockId::POLISHED_TUFF).unwrap();
        assert_eq!(polished_group.break_sound, Sound::BlockPolishedTuffBreak);
        assert_eq!(polished_group.step_sound, Sound::BlockPolishedTuffStep);
        assert_eq!(polished_group.place_sound, Sound::BlockPolishedTuffPlace);
        assert_eq!(polished_group.hit_sound, Sound::BlockPolishedTuffHit);
        assert_eq!(polished_group.fall_sound, Sound::BlockPolishedTuffFall);

        let brick_group = TuffBlock::sound_group_for_block(BlockId::TUFF_BRICKS).unwrap();
        assert_eq!(brick_group.break_sound, Sound::BlockTuffBricksBreak);
        assert_eq!(brick_group.step_sound, Sound::BlockTuffBricksStep);
        assert_eq!(brick_group.place_sound, Sound::BlockTuffBricksPlace);
        assert_eq!(brick_group.hit_sound, Sound::BlockTuffBricksHit);
        assert_eq!(brick_group.fall_sound, Sound::BlockTuffBricksFall);

        // 5. Stonecutter recipes parity (All 25 Vanilla recipes)
        // From TUFF (13 outputs)
        let tuff_outputs = [
            BlockId::CHISELED_TUFF,
            BlockId::POLISHED_TUFF,
            BlockId::POLISHED_TUFF_SLAB,
            BlockId::POLISHED_TUFF_STAIRS,
            BlockId::POLISHED_TUFF_WALL,
            BlockId::TUFF_BRICKS,
            BlockId::TUFF_BRICK_SLAB,
            BlockId::TUFF_BRICK_STAIRS,
            BlockId::TUFF_BRICK_WALL,
            BlockId::CHISELED_TUFF_BRICKS,
            BlockId::TUFF_SLAB,
            BlockId::TUFF_STAIRS,
            BlockId::TUFF_WALL,
        ];
        assert_eq!(tuff_outputs.len(), 13);
        for out in tuff_outputs {
            assert!(TuffBlock::is_valid_stonecutting_recipe(BlockId::TUFF, out));
        }

        // From POLISHED_TUFF (8 outputs)
        let polished_outputs = [
            BlockId::POLISHED_TUFF_SLAB,
            BlockId::POLISHED_TUFF_STAIRS,
            BlockId::POLISHED_TUFF_WALL,
            BlockId::TUFF_BRICKS,
            BlockId::TUFF_BRICK_SLAB,
            BlockId::TUFF_BRICK_STAIRS,
            BlockId::TUFF_BRICK_WALL,
            BlockId::CHISELED_TUFF_BRICKS,
        ];
        assert_eq!(polished_outputs.len(), 8);
        for out in polished_outputs {
            assert!(TuffBlock::is_valid_stonecutting_recipe(BlockId::POLISHED_TUFF, out));
        }

        // From TUFF_BRICKS (4 outputs)
        let brick_outputs = [
            BlockId::TUFF_BRICK_SLAB,
            BlockId::TUFF_BRICK_STAIRS,
            BlockId::TUFF_BRICK_WALL,
            BlockId::CHISELED_TUFF_BRICKS,
        ];
        assert_eq!(brick_outputs.len(), 4);
        for out in brick_outputs {
            assert!(TuffBlock::is_valid_stonecutting_recipe(BlockId::TUFF_BRICKS, out));
        }

        // Total stonecutter recipes = 13 + 8 + 4 = 25
        assert_eq!(tuff_outputs.len() + polished_outputs.len() + brick_outputs.len(), 25);

        // Invalid recipe checks
        assert!(!TuffBlock::is_valid_stonecutting_recipe(BlockId::TUFF, BlockId::STONE));
        assert!(!TuffBlock::is_valid_stonecutting_recipe(BlockId::TUFF_BRICKS, BlockId::POLISHED_TUFF));
        assert!(!TuffBlock::is_valid_stonecutting_recipe(BlockId::POLISHED_TUFF, BlockId::TUFF));
    }
}
