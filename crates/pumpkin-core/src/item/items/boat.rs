use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::vehicle::boat::BoatEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;

pub struct BoatItem;

impl BoatItem {
    /// Maps boat item to corresponding entity type
    pub(crate) fn item_to_entity(item: &Item) -> &'static EntityType {
        match item.id {
            val if val == Item::OAK_BOAT.id => &EntityType::OAK_BOAT,
            val if val == Item::OAK_CHEST_BOAT.id => &EntityType::OAK_CHEST_BOAT,
            val if val == Item::SPRUCE_BOAT.id => &EntityType::SPRUCE_BOAT,
            val if val == Item::SPRUCE_CHEST_BOAT.id => &EntityType::SPRUCE_CHEST_BOAT,
            val if val == Item::BIRCH_BOAT.id => &EntityType::BIRCH_BOAT,
            val if val == Item::BIRCH_CHEST_BOAT.id => &EntityType::BIRCH_CHEST_BOAT,
            val if val == Item::JUNGLE_BOAT.id => &EntityType::JUNGLE_BOAT,
            val if val == Item::JUNGLE_CHEST_BOAT.id => &EntityType::JUNGLE_CHEST_BOAT,
            val if val == Item::ACACIA_BOAT.id => &EntityType::ACACIA_BOAT,
            val if val == Item::ACACIA_CHEST_BOAT.id => &EntityType::ACACIA_CHEST_BOAT,
            val if val == Item::DARK_OAK_BOAT.id => &EntityType::DARK_OAK_BOAT,
            val if val == Item::DARK_OAK_CHEST_BOAT.id => &EntityType::DARK_OAK_CHEST_BOAT,
            val if val == Item::MANGROVE_BOAT.id => &EntityType::MANGROVE_BOAT,
            val if val == Item::MANGROVE_CHEST_BOAT.id => &EntityType::MANGROVE_CHEST_BOAT,
            val if val == Item::CHERRY_BOAT.id => &EntityType::CHERRY_BOAT,
            val if val == Item::CHERRY_CHEST_BOAT.id => &EntityType::CHERRY_CHEST_BOAT,
            val if val == Item::PALE_OAK_BOAT.id => &EntityType::PALE_OAK_BOAT,
            val if val == Item::PALE_OAK_CHEST_BOAT.id => &EntityType::PALE_OAK_CHEST_BOAT,
            val if val == Item::POPLAR_BOAT.id => &EntityType::POPLAR_BOAT,
            val if val == Item::POPLAR_CHEST_BOAT.id => &EntityType::POPLAR_CHEST_BOAT,
            val if val == Item::BAMBOO_RAFT.id => &EntityType::BAMBOO_RAFT,
            val if val == Item::BAMBOO_CHEST_RAFT.id => &EntityType::BAMBOO_CHEST_RAFT,
            _ => {
                tracing::error!("Unknown boat item ID: {}", item.id);
                &EntityType::OAK_BOAT
            }
        }
    }
}

impl ItemMetadata for BoatItem {
    fn ids() -> Box<[u16]> {
        [
            Item::OAK_BOAT.id,
            Item::OAK_CHEST_BOAT.id,
            Item::SPRUCE_BOAT.id,
            Item::SPRUCE_CHEST_BOAT.id,
            Item::BIRCH_BOAT.id,
            Item::BIRCH_CHEST_BOAT.id,
            Item::JUNGLE_BOAT.id,
            Item::JUNGLE_CHEST_BOAT.id,
            Item::ACACIA_BOAT.id,
            Item::ACACIA_CHEST_BOAT.id,
            Item::DARK_OAK_BOAT.id,
            Item::DARK_OAK_CHEST_BOAT.id,
            Item::MANGROVE_BOAT.id,
            Item::MANGROVE_CHEST_BOAT.id,
            Item::CHERRY_BOAT.id,
            Item::CHERRY_CHEST_BOAT.id,
            Item::PALE_OAK_BOAT.id,
            Item::PALE_OAK_CHEST_BOAT.id,
            Item::POPLAR_BOAT.id,
            Item::POPLAR_CHEST_BOAT.id,
            Item::BAMBOO_RAFT.id,
            Item::BAMBOO_CHEST_RAFT.id,
        ]
        .into()
    }
}

impl ItemBehaviour for BoatItem {
    fn normal_use(&self, item: &Item, player: &Player) {
        let (yaw, pitch) = player.rotation();
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_rotation(&self, item: &Item, player: &Player, yaw: f32, pitch: f32) {
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    /// Vanilla: `BoatItem.use()` - raycasts to find placement position
    fn normal_use_with_hand(
        &self,
        item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: pumpkin_util::Hand,
    ) {
        let world = player.world();
        let start_pos = player.eye_position();
        let view_vector = player.get_looking_vector();
        let end_pos = start_pos + view_vector * player.block_interaction_range();

        // Vanilla: BoatItem.use() clips with FluidHandling.ANY
        let hit = world.clip(&pumpkin_world::clip::ClipContext::new(
            start_pos,
            end_pos,
            pumpkin_world::clip::ClipBlock::Outline,
            pumpkin_world::clip::ClipFluid::Any,
        ));
        if hit.is_miss {
            return;
        }

        let hit_vec = hit.location;
        // Vanilla: Check for entities in the path that would block placement (BoatItem.java:38-50)
        let player_entity = player.get_entity().bounding_box.load();
        let search_box = player_entity
            .stretch(view_vector * crate::entity::combat::BOAT_PLACEMENT_REACH_DISTANCE)
            .expand_all(crate::entity::combat::BOAT_PLACEMENT_SEARCH_BOX_INFLATION);

        let entities = world.get_all_at_box(&search_box);

        for entity in &entities {
            // Vanilla: EntityPredicates.CAN_HIT = !isSpectator() && canHit()
            if entity.get_entity().entity_id == player.entity_id()
                || entity.is_spectator()
                || (!entity.can_hit() && entity.get_player().is_none())
            {
                continue;
            }

            // Vanilla: expand by getPickRadius() (0.0 for most entities, BoatItem.java:45)
            let entity_box = entity.get_entity().bounding_box.load();
            if crate::entity::combat::is_point_inside_entity_pick_box(
                start_pos,
                entity_box.min,
                entity_box.max,
                0.0,
            ) {
                // Entity is blocking the line of sight at eye position
                return;
            }
        }

        let entity_type = Self::item_to_entity(item);
        let entity = Entity::new(world.clone(), hit_vec, entity_type);
        let (player_yaw, _) = player.rotation();
        entity.set_rotation(player_yaw, 0.0);
        let boat_entity = Arc::new(BoatEntity::new(entity));
        let entity = boat_entity.get_entity();
        let boat_box = entity.bounding_box.load();

        // Vanilla: if (!level.noCollision(boat, boat.getBoundingBox())) return FAIL (BoatItem.java:59-61)
        if !world.is_space_empty(boat_box)
            || entity
                .get_boat_collisions(boat_box, boat_entity.as_ref())
                .iter()
                .any(|shape| shape.intersects(&boat_box))
        {
            return;
        }
        world.spawn_entity(boat_entity);
        world.emit_game_event(
            pumpkin_data::game_event::GameEvent::EntityPlace.name(),
            hit_vec,
        );

        let mut stack = player.inventory.get_stack_in_hand(hand);
        if !stack.is_empty() && stack.item.id == item.id {
            stack.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_stack_in_hand(hand, stack);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
