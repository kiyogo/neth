use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::item::Item;

use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::player::Player;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REQUIRES: &[MemoryModuleId] = &[types::TEMPTING_PLAYER.id()];
pub const DEFAULT_TEMPT_RANGE: f64 = 10.0;

/// Predicate function checking whether an item stack tempts the mob.
pub type TemptPredicate =
    Box<dyn Fn(&dyn Mob, &pumpkin_data::item_stack::ItemStack) -> bool + Send + Sync>;

/// Mirrors vanilla `TemptingSensor.java`.
/// Scans players within `Attributes::TEMPT_RANGE` holding a temptation item and sets `TEMPTING_PLAYER`.
pub struct TemptingSensor {
    temptations: Option<TemptPredicate>,
}

impl TemptingSensor {
    #[must_use]
    pub fn new() -> Self {
        Self { temptations: None }
    }

    #[must_use]
    pub fn for_animal() -> Self {
        Self { temptations: None }
    }

    #[must_use]
    pub fn with_predicate(
        predicate: impl Fn(&dyn Mob, &pumpkin_data::item_stack::ItemStack) -> bool
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self {
            temptations: Some(Box::new(predicate)),
        }
    }

    #[must_use]
    pub fn with_items(items: &'static [&'static Item]) -> Self {
        Self::with_predicate(move |_mob, stack| items.iter().any(|i| i.id == stack.item.id))
    }

    fn is_temptation(&self, mob: &dyn Mob, stack: &pumpkin_data::item_stack::ItemStack) -> bool {
        if stack.item_count == 0 {
            return false;
        }
        if let Some(predicate) = &self.temptations {
            predicate(mob, stack)
        } else if let Some(animal) = mob.as_animal() {
            animal.is_food(stack)
        } else {
            false
        }
    }

    fn player_holding_temptation(&self, mob: &dyn Mob, player: &Player) -> bool {
        let inv = player.inventory();
        let main_hand = inv.held_item();
        let off_hand = inv.off_hand_item();
        self.is_temptation(mob, &main_hand) || self.is_temptation(mob, &off_hand)
    }
}

impl Default for TemptingSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl Sensor for TemptingSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let tempt_range = tick
            .mob
            .get_mob_entity()
            .living_entity
            .get_attribute_value(&Attributes::TEMPT_RANGE);
        let range = if tempt_range > 0.0 {
            tempt_range
        } else {
            DEFAULT_TEMPT_RANGE
        };
        let range_squared = range * range;
        let body_pos = tick.mob.get_entity().pos.load();

        let mut closest_player: Option<(f64, Arc<Player>)> = None;

        for player in tick.world.players.load().iter() {
            if !player.living_entity.is_part_of_game() {
                continue;
            }
            if tick.mob.get_entity().has_passenger(player.get_entity().entity_id) {
                continue;
            }
            if !self.player_holding_temptation(tick.mob, player) {
                continue;
            }
            let dist_sq = player
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&body_pos);
            if dist_sq < range_squared {
                if let Some((best_dist, _)) = &closest_player {
                    if dist_sq < *best_dist {
                        closest_player = Some((dist_sq, player.clone()));
                    }
                } else {
                    closest_player = Some((dist_sq, player.clone()));
                }
            }
        }

        if let Some((_, player)) = closest_player {
            tick.brain.set(types::TEMPTING_PLAYER, player);
        } else {
            tick.brain.erase(types::TEMPTING_PLAYER.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tempting_sensor_requires_parity() {
        assert_eq!(TemptingSensor::new().requires(), &[types::TEMPTING_PLAYER.id()]);
        assert_eq!(DEFAULT_TEMPT_RANGE, 10.0);
    }
}
