use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, NormalUseArgs, PathComputationType, PlacedArgs, RandomTickArgs, UseWithItemArgs,
};
use pumpkin_data::flower_pot_transformations::get_potted_item;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockId, BlockState};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_world::world::BlockFlags;

#[must_use]
pub fn get_plant_item_from_potted_block(block: &Block) -> Option<&'static Item> {
    let plant_name = block.name.strip_prefix("potted_")?;
    let plant_name = match plant_name {
        "azalea_bush" => "azalea",
        "flowering_azalea_bush" => "flowering_azalea",
        other => other,
    };
    let item_key = format!("minecraft:{plant_name}");
    Item::from_registry_key(&item_key)
}

#[pumpkin_block_from_tag("minecraft:flower_pots")]
pub struct FlowerPotBlock;

impl FlowerPotBlock {
    pub const FLOWER_POT_HEIGHT: f64 = 6.0 / 16.0;

    #[must_use]
    pub fn is_empty(block: &Block) -> bool {
        block == &Block::FLOWER_POT
    }

    #[must_use]
    pub fn can_be_potted(item_id: u16) -> bool {
        get_potted_item(item_id) != BlockId::AIR
    }
}

impl BlockBehaviour for FlowerPotBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if args.block == &Block::FLOWER_POT {
            return BlockActionResult::Consume;
        }

        if let Some(plant_item) = get_plant_item_from_potted_block(args.block) {
            let mut plant_stack = ItemStack::new(1, plant_item);
            if !args.player.inventory.insert_stack_anywhere(&mut plant_stack)
                && !plant_stack.is_empty()
            {
                args.world
                    .drop_stack(&args.player.position().to_block_pos(), plant_stack);
            }
        }

        args.world.set_block_state(
            args.position,
            Block::FLOWER_POT.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        args.world.emit_game_event(
            GameEvent::BlockChange.name(),
            args.position.to_centered_f64(),
        );

        BlockActionResult::Success
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let item = args.item_stack.item;
        let potted_block_id = get_potted_item(item.id);

        if potted_block_id == BlockId::AIR {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        if args.block != &Block::FLOWER_POT {
            return BlockActionResult::Consume;
        }

        args.world.set_block_state(
            args.position,
            Block::from_id(potted_block_id).default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        args.world.emit_game_event(
            GameEvent::BlockChange.name(),
            args.position.to_centered_f64(),
        );
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::PotFlower as i32,
            1,
        );
        args.item_stack
            .decrement_unless_creative(args.player.gamemode.load(), 1);

        BlockActionResult::Success
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let is_open_potted = args.block.eq(&Block::POTTED_OPEN_EYEBLOSSOM);
        let is_closed_potted = args.block.eq(&Block::POTTED_CLOSED_EYEBLOSSOM);
        if !is_open_potted && !is_closed_potted {
            return;
        }

        let is_open = is_open_potted;
        let should_be_open = args.world.eyeblossom_open(args.position).unwrap_or(is_open);

        if is_open != should_be_open {
            let next_block = if should_be_open {
                &Block::POTTED_OPEN_EYEBLOSSOM
            } else {
                &Block::POTTED_CLOSED_EYEBLOSSOM
            };
            args.world.set_block_state(
                args.position,
                next_block.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );

            // Parity with vanilla FlowerPotBlock.java:139-142:
            // Potted eyeblossom transform emits particles and plays the long switch sound
            let mut rng = rand::rng();
            crate::block::blocks::plant::eyeblossom::spawn_transform_particle(
                args.world,
                next_block,
                args.position,
                &mut rng,
            );
            let sound = if is_open {
                Sound::BlockEyeblossomCloseLong
            } else {
                Sound::BlockEyeblossomOpenLong
            };
            args.world.play_sound(
                sound,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
