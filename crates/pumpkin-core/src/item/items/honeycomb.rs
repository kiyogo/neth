use std::sync::Arc;

use crate::block::UseWithItemArgs;
use crate::block::entities::BlockEntity;
use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;
use pumpkin_data::block_properties::{
    ChestLikeProperties, ChestType, DoubleBlockHalf, OakDoorLikeProperties,
};
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, tag};
use pumpkin_data::{BlockDirection, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct HoneyCombItem;

impl ItemMetadata for HoneyCombItem {
    fn ids() -> Box<[u16]> {
        [Item::HONEYCOMB.id].into()
    }
}

impl ItemBehaviour for HoneyCombItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        if try_wax_block(&world, location, block) {
            item.decrement_unless_creative(player.gamemode.load(), 1);
            player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::WaxOn);
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Waxes the block at `location` if it has a waxed equivalent, emitting the wax
/// particles and sound on success.
pub(crate) fn try_wax_block(world: &Arc<World>, location: BlockPos, block: &Block) -> bool {
    let Some(replacement) = get_waxed_equivalent(block.id) else {
        return false;
    };
    let new_block = replacement.to_block();

    let from_state_id = world.get_block_state_id(&location);
    let new_state_id = waxed_state_id(block, from_state_id, new_block);

    world.set_block_state(&location, new_state_id, BlockFlags::NOTIFY_ALL);
    world.sync_world_event(WorldEvent::ParticlesWaxOn, location, 0);
    world.play_sound(
        Sound::ItemHoneycombWaxOn,
        SoundCategory::Blocks,
        &location.to_f64(),
    );

    // 1. Double Door synchronization: wax both halves
    if block.has_tag(&tag::Block::MINECRAFT_DOORS) {
        let door_props = OakDoorLikeProperties::from_state_id(from_state_id);
        let other_half_pos = match door_props.half {
            DoubleBlockHalf::Lower => location.up(),
            DoubleBlockHalf::Upper => location.down(),
        };
        let (other_block, other_state_id) = world.get_block_and_state_id(&other_half_pos);
        if other_block == block {
            let other_new_state_id = crate::block::blocks::weathering_copper::with_properties_of(
                other_block,
                other_state_id,
                new_block,
            );
            world.set_block_state(&other_half_pos, other_new_state_id, BlockFlags::NOTIFY_ALL);
            world.sync_world_event(WorldEvent::ParticlesWaxOn, other_half_pos, 0);
        }
    }
    // 2. Double Copper Chest synchronization: wax companion chest
    else {
        let chest_blocks = [
            &Block::COPPER_CHEST,
            &Block::EXPOSED_COPPER_CHEST,
            &Block::WEATHERED_COPPER_CHEST,
            &Block::OXIDIZED_COPPER_CHEST,
        ];
        if chest_blocks.contains(&block) {
            let chest_props = ChestLikeProperties::from_state_id(from_state_id);
            if chest_props.r#type != ChestType::Single {
                let neighbor_dir = match chest_props.r#type {
                    ChestType::Left => chest_props.facing.rotate_clockwise(),
                    ChestType::Right => chest_props.facing.rotate_counter_clockwise(),
                    ChestType::Single => unreachable!(),
                };
                let neighbor_pos = location.offset(neighbor_dir.to_offset());
                let (neighbor_block, neighbor_state_id) = world.get_block_and_state_id(&neighbor_pos);
                if neighbor_block == block {
                    let neighbor_new_state_id = crate::block::blocks::weathering_copper::with_properties_of(
                        neighbor_block,
                        neighbor_state_id,
                        new_block,
                    );
                    world.set_block_state(&neighbor_pos, neighbor_new_state_id, BlockFlags::NOTIFY_ALL);
                    world.sync_world_event(WorldEvent::ParticlesWaxOn, neighbor_pos, 0);
                    world.play_sound(
                        Sound::ItemHoneycombWaxOn,
                        SoundCategory::Blocks,
                        &neighbor_pos.to_f64(),
                    );
                }
            }
        }
    }

    true
}

fn waxed_state_id(
    from_block: &Block,
    from_state_id: BlockStateId,
    to_block: &Block,
) -> BlockStateId {
    crate::block::blocks::weathering_copper::with_properties_of(from_block, from_state_id, to_block)
}

impl HoneyCombItem {
    pub fn apply_to_sign(
        &self,
        args: &UseWithItemArgs<'_>,
        block_entity: &Arc<dyn BlockEntity>,
        sign_entity: &crate::block::entities::sign::SignEntityRef<'_>,
    ) -> BlockActionResult {
        if sign_entity.is_waxed() {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        sign_entity.set_waxed(true);

        args.world.update_block_entity(block_entity);
        args.world
            .sync_world_event(WorldEvent::ParticlesWaxOn, *args.position, 0);

        BlockActionResult::Success
    }
}

pub const fn get_waxed_equivalent(id: BlockId) -> Option<BlockId> {
    match id {
        // 1. Copper Block
        BlockId::COPPER_BLOCK => Some(BlockId::WAXED_COPPER_BLOCK),
        BlockId::EXPOSED_COPPER => Some(BlockId::WAXED_EXPOSED_COPPER),
        BlockId::WEATHERED_COPPER => Some(BlockId::WAXED_WEATHERED_COPPER),
        BlockId::OXIDIZED_COPPER => Some(BlockId::WAXED_OXIDIZED_COPPER),

        // 2. Cut Copper
        BlockId::CUT_COPPER => Some(BlockId::WAXED_CUT_COPPER),
        BlockId::EXPOSED_CUT_COPPER => Some(BlockId::WAXED_EXPOSED_CUT_COPPER),
        BlockId::WEATHERED_CUT_COPPER => Some(BlockId::WAXED_WEATHERED_CUT_COPPER),
        BlockId::OXIDIZED_CUT_COPPER => Some(BlockId::WAXED_OXIDIZED_CUT_COPPER),

        // 3. Cut Copper Slab
        BlockId::CUT_COPPER_SLAB => Some(BlockId::WAXED_CUT_COPPER_SLAB),
        BlockId::EXPOSED_CUT_COPPER_SLAB => Some(BlockId::WAXED_EXPOSED_CUT_COPPER_SLAB),
        BlockId::WEATHERED_CUT_COPPER_SLAB => Some(BlockId::WAXED_WEATHERED_CUT_COPPER_SLAB),
        BlockId::OXIDIZED_CUT_COPPER_SLAB => Some(BlockId::WAXED_OXIDIZED_CUT_COPPER_SLAB),

        // 4. Cut Copper Stairs
        BlockId::CUT_COPPER_STAIRS => Some(BlockId::WAXED_CUT_COPPER_STAIRS),
        BlockId::EXPOSED_CUT_COPPER_STAIRS => Some(BlockId::WAXED_EXPOSED_CUT_COPPER_STAIRS),
        BlockId::WEATHERED_CUT_COPPER_STAIRS => Some(BlockId::WAXED_WEATHERED_CUT_COPPER_STAIRS),
        BlockId::OXIDIZED_CUT_COPPER_STAIRS => Some(BlockId::WAXED_OXIDIZED_CUT_COPPER_STAIRS),

        // 5. Chiseled Copper
        BlockId::CHISELED_COPPER => Some(BlockId::WAXED_CHISELED_COPPER),
        BlockId::EXPOSED_CHISELED_COPPER => Some(BlockId::WAXED_EXPOSED_CHISELED_COPPER),
        BlockId::WEATHERED_CHISELED_COPPER => Some(BlockId::WAXED_WEATHERED_CHISELED_COPPER),
        BlockId::OXIDIZED_CHISELED_COPPER => Some(BlockId::WAXED_OXIDIZED_CHISELED_COPPER),

        // 6. Copper Door
        BlockId::COPPER_DOOR => Some(BlockId::WAXED_COPPER_DOOR),
        BlockId::EXPOSED_COPPER_DOOR => Some(BlockId::WAXED_EXPOSED_COPPER_DOOR),
        BlockId::WEATHERED_COPPER_DOOR => Some(BlockId::WAXED_WEATHERED_COPPER_DOOR),
        BlockId::OXIDIZED_COPPER_DOOR => Some(BlockId::WAXED_OXIDIZED_COPPER_DOOR),

        // 7. Copper Trapdoor
        BlockId::COPPER_TRAPDOOR => Some(BlockId::WAXED_COPPER_TRAPDOOR),
        BlockId::EXPOSED_COPPER_TRAPDOOR => Some(BlockId::WAXED_EXPOSED_COPPER_TRAPDOOR),
        BlockId::WEATHERED_COPPER_TRAPDOOR => Some(BlockId::WAXED_WEATHERED_COPPER_TRAPDOOR),
        BlockId::OXIDIZED_COPPER_TRAPDOOR => Some(BlockId::WAXED_OXIDIZED_COPPER_TRAPDOOR),

        // 8. Copper Grate
        BlockId::COPPER_GRATE => Some(BlockId::WAXED_COPPER_GRATE),
        BlockId::EXPOSED_COPPER_GRATE => Some(BlockId::WAXED_EXPOSED_COPPER_GRATE),
        BlockId::WEATHERED_COPPER_GRATE => Some(BlockId::WAXED_WEATHERED_COPPER_GRATE),
        BlockId::OXIDIZED_COPPER_GRATE => Some(BlockId::WAXED_OXIDIZED_COPPER_GRATE),

        // 9. Copper Bulb
        BlockId::COPPER_BULB => Some(BlockId::WAXED_COPPER_BULB),
        BlockId::EXPOSED_COPPER_BULB => Some(BlockId::WAXED_EXPOSED_COPPER_BULB),
        BlockId::WEATHERED_COPPER_BULB => Some(BlockId::WAXED_WEATHERED_COPPER_BULB),
        BlockId::OXIDIZED_COPPER_BULB => Some(BlockId::WAXED_OXIDIZED_COPPER_BULB),

        // 10. Copper Bars
        BlockId::COPPER_BARS => Some(BlockId::WAXED_COPPER_BARS),
        BlockId::EXPOSED_COPPER_BARS => Some(BlockId::WAXED_EXPOSED_COPPER_BARS),
        BlockId::WEATHERED_COPPER_BARS => Some(BlockId::WAXED_WEATHERED_COPPER_BARS),
        BlockId::OXIDIZED_COPPER_BARS => Some(BlockId::WAXED_OXIDIZED_COPPER_BARS),

        // 11. Copper Chain
        BlockId::COPPER_CHAIN => Some(BlockId::WAXED_COPPER_CHAIN),
        BlockId::EXPOSED_COPPER_CHAIN => Some(BlockId::WAXED_EXPOSED_COPPER_CHAIN),
        BlockId::WEATHERED_COPPER_CHAIN => Some(BlockId::WAXED_WEATHERED_COPPER_CHAIN),
        BlockId::OXIDIZED_COPPER_CHAIN => Some(BlockId::WAXED_OXIDIZED_COPPER_CHAIN),

        // 12. Copper Lantern
        BlockId::COPPER_LANTERN => Some(BlockId::WAXED_COPPER_LANTERN),
        BlockId::EXPOSED_COPPER_LANTERN => Some(BlockId::WAXED_EXPOSED_COPPER_LANTERN),
        BlockId::WEATHERED_COPPER_LANTERN => Some(BlockId::WAXED_WEATHERED_COPPER_LANTERN),
        BlockId::OXIDIZED_COPPER_LANTERN => Some(BlockId::WAXED_OXIDIZED_COPPER_LANTERN),

        // 13. Copper Chest
        BlockId::COPPER_CHEST => Some(BlockId::WAXED_COPPER_CHEST),
        BlockId::EXPOSED_COPPER_CHEST => Some(BlockId::WAXED_EXPOSED_COPPER_CHEST),
        BlockId::WEATHERED_COPPER_CHEST => Some(BlockId::WAXED_WEATHERED_COPPER_CHEST),
        BlockId::OXIDIZED_COPPER_CHEST => Some(BlockId::WAXED_OXIDIZED_COPPER_CHEST),

        // 14. Copper Golem Statue
        BlockId::COPPER_GOLEM_STATUE => Some(BlockId::WAXED_COPPER_GOLEM_STATUE),
        BlockId::EXPOSED_COPPER_GOLEM_STATUE => Some(BlockId::WAXED_EXPOSED_COPPER_GOLEM_STATUE),
        BlockId::WEATHERED_COPPER_GOLEM_STATUE => Some(BlockId::WAXED_WEATHERED_COPPER_GOLEM_STATUE),
        BlockId::OXIDIZED_COPPER_GOLEM_STATUE => Some(BlockId::WAXED_OXIDIZED_COPPER_GOLEM_STATUE),

        // 15. Lightning Rod
        BlockId::LIGHTNING_ROD => Some(BlockId::WAXED_LIGHTNING_ROD),
        BlockId::EXPOSED_LIGHTNING_ROD => Some(BlockId::WAXED_EXPOSED_LIGHTNING_ROD),
        BlockId::WEATHERED_LIGHTNING_ROD => Some(BlockId::WAXED_WEATHERED_LIGHTNING_ROD),
        BlockId::OXIDIZED_LIGHTNING_ROD => Some(BlockId::WAXED_OXIDIZED_LIGHTNING_ROD),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::{Half, HorizontalFacing, OakTrapdoorLikeProperties};

    #[test]
    fn waxing_preserves_trapdoor_properties() {
        for (from, to) in [
            (BlockId::COPPER_TRAPDOOR, BlockId::WAXED_COPPER_TRAPDOOR),
            (
                BlockId::EXPOSED_COPPER_TRAPDOOR,
                BlockId::WAXED_EXPOSED_COPPER_TRAPDOOR,
            ),
            (
                BlockId::WEATHERED_COPPER_TRAPDOOR,
                BlockId::WAXED_WEATHERED_COPPER_TRAPDOOR,
            ),
            (
                BlockId::OXIDIZED_COPPER_TRAPDOOR,
                BlockId::WAXED_OXIDIZED_COPPER_TRAPDOOR,
            ),
        ] {
            let from_block = from.to_block();
            let to_block = to.to_block();

            let mut props = OakTrapdoorLikeProperties::default(from_block);
            props.facing = HorizontalFacing::East;
            props.half = Half::Top;
            props.open = true;
            props.powered = true;
            props.waterlogged = true;
            let state_id = props.to_state_id(from_block);

            let waxed = waxed_state_id(from_block, state_id, to_block);
            assert_eq!(Block::from_state_id(waxed), to_block);

            let waxed_props = OakTrapdoorLikeProperties::from_state_id(waxed);
            assert_eq!(waxed_props.facing, HorizontalFacing::East);
            assert_eq!(waxed_props.half, Half::Top);
            assert!(waxed_props.open);
            assert!(waxed_props.powered);
            assert!(waxed_props.waterlogged);
        }
    }

    #[test]
    fn waxing_other_blocks_uses_default_state() {
        let waxed = waxed_state_id(
            &Block::COPPER_BLOCK,
            Block::COPPER_BLOCK.default_state.id,
            &Block::WAXED_COPPER_BLOCK,
        );
        assert_eq!(waxed, Block::WAXED_COPPER_BLOCK.default_state.id);
    }
}
