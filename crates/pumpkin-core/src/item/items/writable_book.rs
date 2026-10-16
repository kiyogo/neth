use std::any::Any;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::Hand;

pub struct WritableBookItem;

impl ItemMetadata for WritableBookItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WRITABLE_BOOK.id, Item::WRITTEN_BOOK.id])
    }
}

impl ItemBehaviour for WritableBookItem {
    fn normal_use_with_hand(
        &self,
        item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: Hand,
    ) {
        // The client opens a book and quill by itself when the player uses it.
        // Sending the open-book packet for it makes the client open the book
        // again and it lands in the read-only view, where nothing can be
        // written. A signed book is not opened by the client, so the server has
        // to send the packet for that one.
        if should_send_open_book(item) {
            player.open_book(hand);
        }
        player.world().play_sound(
            Sound::ItemBookPageTurn,
            SoundCategory::Players,
            &player.position(),
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

const fn should_send_open_book(item: &Item) -> bool {
    item.id == Item::WRITTEN_BOOK.id
}

#[cfg(test)]
mod tests {
    use super::should_send_open_book;
    use pumpkin_data::item::Item;

    #[test]
    fn only_written_books_request_the_open_book_packet() {
        assert!(!should_send_open_book(&Item::WRITABLE_BOOK));
        assert!(should_send_open_book(&Item::WRITTEN_BOOK));
    }
}

