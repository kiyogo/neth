use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::BundleContentsImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::statistic::StatisticCategory;
use pumpkin_data::tag;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_util::Hand;

pub struct BundleItem;

impl BundleItem {
    // Vanilla `BundleItem.FULL_BAR_COLOR`.
    pub const FULL_BAR_COLOR: u32 = 0xFFFF_5454;
    // Vanilla `BundleItem.BAR_COLOR`.
    pub const BAR_COLOR: u32 = 0xFF70_87FF;
    // Vanilla `BundleItem.MAX_SHOWN_GRID_ITEMS_X`.
    pub const MAX_SHOWN_GRID_ITEMS_X: usize = 4;
    // Vanilla `BundleItem.MAX_SHOWN_GRID_ITEMS_Y`.
    pub const MAX_SHOWN_GRID_ITEMS_Y: usize = 3;
    // Vanilla `BundleItem.MAX_SHOWN_GRID_ITEMS`.
    pub const MAX_SHOWN_GRID_ITEMS: usize = 12;
    // Vanilla `BundleItem.OVERFLOWING_MAX_SHOWN_GRID_ITEMS`.
    pub const OVERFLOWING_MAX_SHOWN_GRID_ITEMS: usize = 11;
    // Vanilla `BundleItem.Self::TICKS_MAX_THROW_DURATION`.
    pub const TICKS_MAX_THROW_DURATION: i32 = 200;
    // Vanilla `BundleItem.Self::TICKS_AFTER_FIRST_THROW`.
    pub const TICKS_AFTER_FIRST_THROW: i32 = 10;
    // Vanilla `BundleItem.Self::TICKS_BETWEEN_THROWS`.
    pub const TICKS_BETWEEN_THROWS: i32 = 2;

    #[must_use]
    pub const fn is_bar_visible(weight: u32) -> bool {
        weight > 0
    }

    #[must_use]
    pub const fn get_bar_width(weight: u32) -> u32 {
        let width = 1 + (weight * 12) / 64;
        if width > 13 {
            13
        } else {
            width
        }
    }

    #[must_use]
    pub const fn get_bar_color(weight: u32) -> u32 {
        if weight >= 64 {
            Self::FULL_BAR_COLOR
        } else {
            Self::BAR_COLOR
        }
    }

    #[must_use]
    pub fn get_fullness_display(weight: u32) -> f32 {
        (weight as f32) / 64.0
    }

    #[must_use]
    pub const fn get_number_of_items_to_show(size: usize) -> usize {
        let available_items_to_show = if size > 12 { 11 } else { 12 };
        let items_on_non_full_row = size % 4;
        let empty_space = if items_on_non_full_row == 0 {
            0
        } else {
            4 - items_on_non_full_row
        };
        let max_allowed = if available_items_to_show >= empty_space {
            available_items_to_show - empty_space
        } else {
            0
        };
        if size < max_allowed {
            size
        } else {
            max_allowed
        }
    }
}

impl ItemMetadata for BundleItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_BUNDLES.1.into()
    }
}

impl ItemBehaviour for BundleItem {
    // Vanilla `BundleItem.use`, which only starts the use: the contents come out while the button
    // is held, in [`Self::on_use_tick`].
    fn normal_use_with_hand(
        &self,
        _item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: Hand,
    ) {
        let stack = player.inventory.get_stack_in_hand(hand);
        player
            .living_entity
            .set_active_hand(hand, stack, Self::TICKS_MAX_THROW_DURATION);
    }

    // Vanilla `BundleItem.onUseTick`.
    fn on_use_tick(&self, _stack: &ItemStack, player: &Player, remaining_use_ticks: i32) {
        let is_first_tick = remaining_use_ticks == Self::TICKS_MAX_THROW_DURATION;
        if is_first_tick
            || (remaining_use_ticks < Self::TICKS_MAX_THROW_DURATION - Self::TICKS_AFTER_FIRST_THROW
                && remaining_use_ticks % Self::TICKS_BETWEEN_THROWS == 0)
        {
            Self::drop_content(player);
        }
    }

    fn get_use_duration(&self) -> i32 {
        Self::TICKS_MAX_THROW_DURATION
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BundleItem {
    // Vanilla `BundleItem.dropContent` through `removeOneItemFromBundle`: the stack in the hand is
    // the one written back, so the dropped item actually leaves the bundle.
    fn drop_content(player: &Player) {
        let hand = player
            .living_entity
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unwrap_or(Hand::Right);
        let slot_index = if hand == Hand::Left {
            PlayerInventory::OFF_HAND_SLOT
        } else {
            player.inventory.get_selected_slot() as usize
        };

        let mut bundle = player.inventory.get_stack_in_hand(hand);
        let Some(contents) = bundle.get_data_component_mut::<BundleContentsImpl>() else {
            return;
        };
        let Some(item) = contents.remove_one() else {
            return;
        };

        let updated_bundle = bundle.clone();
        player
            .inventory
            .set_stack_in_hand(hand, updated_bundle.clone());
        player.sync_hand_slot(slot_index, updated_bundle);

        Self::play_sound(player, Sound::ItemBundleRemoveOne);
        player.drop_item(item);
        Self::play_sound(player, Sound::ItemBundleDropContents);
        player.increment_stat(StatisticCategory::Used, bundle.item.id as i32, 1);
    }

    // Vanilla `BundleItem.playRemoveOneSound` and the sounds around it.
    fn play_sound(player: &Player, sound: Sound) {
        player.play_item_sound(sound, 0.8, 0.8 + rand::random_range(0.0..1.0) * 0.4);
    }
}
