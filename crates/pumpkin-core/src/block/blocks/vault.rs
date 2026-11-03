use std::sync::Arc;

use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{VaultLikeProperties, VaultState};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::vault::VaultBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BrokenArgs, NormalUseArgs, OnPlaceArgs, PlacedArgs, UseWithItemArgs};

#[pumpkin_block("minecraft:vault")]
pub struct VaultBlock;

impl VaultBlock {
    pub const ACTIVATION_RANGE: f64 = 4.0;
    pub const DEACTIVATION_RANGE: f64 = 4.5;

    #[must_use]
    pub fn is_valid_key(item_id: u16, is_ominous: bool) -> bool {
        if is_ominous {
            item_id == Item::OMINOUS_TRIAL_KEY.id
        } else {
            item_id == Item::TRIAL_KEY.id
        }
    }

    #[must_use]
    pub const fn can_insert_key(state: VaultState) -> bool {
        !matches!(state, VaultState::Ejecting | VaultState::Unlocking)
    }

    #[must_use]
    pub fn is_within_activation_range(distance_sq: f64) -> bool {
        distance_sq <= Self::ACTIVATION_RANGE * Self::ACTIVATION_RANGE
    }

    #[must_use]
    pub fn is_within_deactivation_range(distance_sq: f64) -> bool {
        distance_sq <= Self::DEACTIVATION_RANGE * Self::DEACTIVATION_RANGE
    }

    pub const INSERT_FAIL_SOUND_COOLDOWN_TICKS: i64 = 15;
    pub const DISPLAY_ITEM_CYCLE_TICK_RATE: u32 = 20;
    pub const DELAY_BEFORE_FIRST_EJECTION_TICKS: u32 = 20;
    pub const DELAY_BETWEEN_EJECTIONS_TICKS: u32 = 20;
    pub const DELAY_AFTER_LAST_EJECTION_TICKS: u32 = 20;
    pub const UNLOCKING_DELAY_TICKS: u32 = 14;
    pub const UPDATE_CONNECTED_PLAYERS_TICK_RATE: u32 = 20;
    pub const AMBIENT_SOUND_CHANCE: f32 = 0.02;
    pub const KEYHOLE_Y_OFFSET: f64 = 1.75;
    pub const KEYHOLE_FACING_OFFSET: f64 = 0.5;
    pub const MIN_CONNECTION_PARTICLES: i32 = 2;
    pub const MAX_CONNECTION_PARTICLES: i32 = 5;
    pub const EVENT_VAULT_ACTIVATE: i32 = 3015;
    pub const EVENT_VAULT_DEACTIVATE: i32 = 3016;
    pub const EVENT_VAULT_EJECT_ITEM: i32 = 3017;
    pub const LIGHT_LEVEL_HALF_LIT: u8 = 6;
    pub const LIGHT_LEVEL_LIT: u8 = 12;
    pub const EJECTION_Y_OFFSET: f64 = 1.2;
    pub const EJECTION_VELOCITY: f64 = 2.0;

    #[must_use]
    pub fn get_light_level(state: VaultState) -> u8 {
        match state {
            VaultState::Inactive => Self::LIGHT_LEVEL_HALF_LIT,
            VaultState::Active | VaultState::Unlocking | VaultState::Ejecting => {
                Self::LIGHT_LEVEL_LIT
            }
        }
    }

    #[must_use]
    pub fn can_play_insert_fail_sound(current_tick: i64, last_fail_tick: i64) -> bool {
        current_tick >= last_fail_tick + Self::INSERT_FAIL_SOUND_COOLDOWN_TICKS
    }

    #[must_use]
    pub fn calculate_ejection_progress(remaining_items: usize, total_ejections: usize) -> f32 {
        if total_ejections <= 1 {
            1.0
        } else {
            let rem = remaining_items.clamp(1, total_ejections);
            1.0 - (rem - 1) as f32 / (total_ejections - 1) as f32
        }
    }

    #[must_use]
    pub fn calculate_ejection_pitch(remaining_items: usize, total_ejections: usize) -> f32 {
        let progress = Self::calculate_ejection_progress(remaining_items, total_ejections);
        0.8 + 0.4 * progress
    }

    #[must_use]
    pub fn should_cycle_display_item(game_time: i64, state: VaultState) -> bool {
        game_time % (Self::DISPLAY_ITEM_CYCLE_TICK_RATE as i64) == 0 && state == VaultState::Active
    }

    #[must_use]
    pub fn calculate_keyhole_position(
        pos: pumpkin_util::math::position::BlockPos,
        facing_dx: f64,
        facing_dz: f64,
    ) -> pumpkin_util::math::vector3::Vector3<f64> {
        pumpkin_util::math::vector3::Vector3::new(
            f64::from(pos.0.x) + 0.5 + facing_dx * Self::KEYHOLE_FACING_OFFSET,
            f64::from(pos.0.y) + Self::KEYHOLE_Y_OFFSET,
            f64::from(pos.0.z) + 0.5 + facing_dz * Self::KEYHOLE_FACING_OFFSET,
        )
    }

    #[must_use]
    pub fn is_valid_to_insert(
        inserted_item_id: u16,
        inserted_count: u32,
        key_item_id: u16,
        key_count: u32,
    ) -> bool {
        inserted_item_id == key_item_id && inserted_count >= key_count
    }

    #[must_use]
    pub fn is_connected_player_in_range(distance_sq: f64, range: f64) -> bool {
        distance_sq <= range * range
    }
}

impl BlockBehaviour for VaultBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = VaultLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = VaultBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
            args.world.update_neighbors(args.position, None);
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
        args.world.update_neighbors(args.position, None);
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = VaultLikeProperties::from_state_id(state_id);

        if props.vault_state == VaultState::Ejecting || props.vault_state == VaultState::Unlocking {
            return BlockActionResult::Pass;
        }

        let item_id = args.item_stack.item.id;
        // Parity with vanilla 26.3:
        // An ominous vault requires an ominous trial key; a regular vault requires a regular trial key.
        let is_valid_key = if props.ominous {
            item_id == Item::OMINOUS_TRIAL_KEY.id
        } else {
            item_id == Item::TRIAL_KEY.id
        };

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(vault_entity) = block_entity.as_any().downcast_ref::<VaultBlockEntity>()
        {
            let current_tick = args.world.get_world_age();
            if !is_valid_key {
                if vault_entity.can_play_insert_fail_sound(current_tick) {
                    args.world.play_sound(
                        Sound::BlockVaultInsertItemFail,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    vault_entity.set_last_insert_fail_timestamp(current_tick);
                }
                return BlockActionResult::Success;
            }

            let player_uuid = args.player.gameprofile.id;

            if vault_entity.has_rewarded(&player_uuid) {
                if vault_entity.can_play_insert_fail_sound(current_tick) {
                    args.world.play_sound(
                        Sound::BlockVaultRejectRewardedPlayer,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                    vault_entity.set_last_insert_fail_timestamp(current_tick);
                }
                return BlockActionResult::Success;
            }

            vault_entity.mark_rewarded(player_uuid);

            args.item_stack
                .decrement_unless_creative(args.player.gamemode.load(), 1);

            args.world.play_sound(
                Sound::BlockVaultInsertItem,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            args.world.play_sound(
                Sound::BlockVaultOpenShutter,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Used,
                item_id as i32,
                1,
            );
            if props.ominous {
                args.player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::Revaulting,
                );
            } else {
                args.player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::UnderLockAndKey,
                );
            }

            props.vault_state = VaultState::Ejecting;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            // Generate loot from Loot Table according to vanilla parity
            let loot_table_key = vault_entity.get_loot_table_key(props.ominous);
            let seed = (args.position.0.x as i64).wrapping_mul(3129871)
                ^ (args.position.0.z as i64).wrapping_mul(618791)
                ^ (args.position.0.y as i64)
                ^ rand::random::<i64>();

            let mut loot_stacks = Vec::new();
            if let Some(table_handle) = args.world.get_loot_table(&loot_table_key) {
                let mut params = crate::world::loot::LootContextParameters::default();
                params.position = Some(args.position.to_centered_f64());
                params.tool = Some(args.item_stack.clone());
                loot_stacks =
                    crate::world::loot::generate_loot_from_handle(&table_handle, seed, &params);
            }

            if loot_stacks.is_empty() {
                // Fallback trial vault loot
                loot_stacks = if props.ominous {
                    vec![
                        ItemStack::new(5, &Item::EMERALD),
                        ItemStack::new(2, &Item::DIAMOND),
                        ItemStack::new(3, &Item::IRON_INGOT),
                        ItemStack::new(8, &Item::WIND_CHARGE),
                    ]
                } else {
                    vec![
                        ItemStack::new(4, &Item::EMERALD),
                        ItemStack::new(1, &Item::DIAMOND),
                        ItemStack::new(2, &Item::IRON_INGOT),
                    ]
                };
            }

            let total_drops = loot_stacks.len();
            for (idx, stack) in loot_stacks.into_iter().enumerate() {
                let remaining = total_drops - idx;
                let pitch = VaultBlock::calculate_ejection_pitch(remaining, total_drops);
                args.world.play_sound_fine(
                    Sound::BlockVaultEjectItem,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                    1.0,
                    pitch,
                );
                args.world.sync_world_event(
                    WorldEvent::AnimationVaultEjectItem,
                    *args.position,
                    0,
                );
                let base_pos = pumpkin_util::math::vector3::Vector3::new(
                    f64::from(args.position.0.x) + 0.5,
                    f64::from(args.position.0.y) + VaultBlock::EJECTION_Y_OFFSET,
                    f64::from(args.position.0.z) + 0.5,
                );
                args.world.drop_stack_dispensed(
                    base_pos,
                    pumpkin_data::BlockDirection::Up,
                    crate::entity::combat::DISPENSE_ITEM_ACCURACY,
                    stack,
                );
            }

            args.world.play_sound(
                Sound::BlockVaultCloseShutter,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            props.vault_state = VaultState::Active;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            return BlockActionResult::Success;
        }

        BlockActionResult::Pass
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let props = VaultLikeProperties::from_state_id(state_id);
        if props.vault_state == VaultState::Inactive {
            return BlockActionResult::Pass;
        }

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(vault_entity) = block_entity.as_any().downcast_ref::<VaultBlockEntity>()
        {
            let current_tick = args.world.get_world_age();
            if vault_entity.can_play_insert_fail_sound(current_tick) {
                args.world.play_sound(
                    Sound::BlockVaultInsertItemFail,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
                vault_entity.set_last_insert_fail_timestamp(current_tick);
            }
        }
        BlockActionResult::Success
    }
}
