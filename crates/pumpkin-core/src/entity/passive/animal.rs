use std::sync::Arc;

use pumpkin_data::entity::{EntityType, entity_from_egg};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use uuid::Uuid;

use crate::entity::{EntityBase, ageable::BABY_START_AGE, mob::Mob, player::Player, r#type::from_type};
use crate::item::items::spawn_egg::prepare_egg_mob;
use crate::plugin::api::events::entity::creature_spawn::CreatureSpawnReason;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_util::math::vector3::Vector3;

/// Whether `item_stack` is the spawn egg for `entity_type`.
///
/// Vanilla `SpawnEggItem.spawnsEntity`: only an egg for the mob's own type spawns a
/// baby from it. Any other spawn egg is left to the normal use-on-block path, which
/// places a new adult next to the mob.
fn is_spawn_egg_for(item_stack: &ItemStack, entity_type: &EntityType) -> bool {
    entity_from_egg(item_stack.item.id).is_some_and(|egg_type| &egg_type == entity_type)
}

pub trait Animal: Mob {
    fn is_food(&self, item_stack: &ItemStack) -> bool;

    /// Animals prefer grass, then bright spots.
    fn animal_walk_target_value(&self, pos: &pumpkin_util::math::position::BlockPos) -> f32 {
        let world = self.get_mob_entity().living_entity.entity.world.load();
        if world.get_block(&pos.down()).id == pumpkin_data::Block::GRASS_BLOCK.id {
            return 10.0;
        }
        world.get_light_level_dependent_magic_value(pos) - 0.5
    }

    fn play_eating_sound(&self, sound: Sound) {
        let mob_entity = self.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
    }

    fn write_animal_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        let mob_entity = self.get_mob_entity();
        let in_love = mob_entity
            .love_ticks
            .load(std::sync::atomic::Ordering::Relaxed);
        nbt.put_int("InLove", in_love);
        if let Some(uuid) = mob_entity.breeder.load() {
            nbt.put_uuid("LoveCause", uuid);
        }
    }

    fn read_animal_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        let mob_entity = self.get_mob_entity();
        let in_love = nbt.get_int("InLove").unwrap_or(0);
        let love_cause = nbt.get_uuid("LoveCause");
        mob_entity.set_love_ticks(in_love, love_cause);
    }

    /// Spawns a baby of this mob's own type at its position and consumes one egg,
    /// mirroring vanilla `SpawnEggItem.spawnOffspringFromSpawnEgg`.
    fn spawn_baby_from_egg(
        &self,
        player: &Arc<Player>,
        item_stack: &mut ItemStack,
    ) {
        let entity = &self.get_mob_entity().living_entity.entity;
        let world = entity.world.load();
        let baby = from_type(
            entity.entity_type,
            entity.pos.load(),
            &world,
            Uuid::new_v4(),
        );

        // `init_data_tracker` derives the baby flag from a negative age and runs as
        // part of `spawn_entity`, so the age has to be set before spawning.
        baby.get_entity().set_age(BABY_START_AGE);
        // Vanilla `snapTo(pos, 0.0F, 0.0F)`.
        baby.get_entity().set_rotation(0.0, 0.0);
        prepare_egg_mob(item_stack, &baby, &world, Some(player.as_ref()));

        world.spawn_creature(
            baby,
            CreatureSpawnReason::SpawnerEgg,
            Some(player.clone()),
        );
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
    }

    fn animal_interact(
        &self,
        player: &Arc<Player>,
        item_stack: &mut ItemStack,
        ambient_sound: Sound,
    ) -> bool {
        let mob_entity = self.get_mob_entity();

        // Vanilla runs this in `Mob.checkAndHandleImportantInteractions`, ahead of
        // any food handling. No spawn egg is a breeding item, so the order only
        // matters for staying close to vanilla.
        if is_spawn_egg_for(item_stack, mob_entity.living_entity.entity.entity_type) {
            self.spawn_baby_from_egg(player, item_stack);
            return true;
        }

        if self.is_food(item_stack) {
            let age = mob_entity
                .living_entity
                .entity
                .age
                .load(std::sync::atomic::Ordering::Relaxed);

            if age >= 0 && mob_entity.is_breeding_ready() && !mob_entity.is_in_love() {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);

                mob_entity.set_love_ticks(600, Some(player.gameprofile.id));
                let entity = &mob_entity.living_entity.entity;
                let world = entity.world.load();
                let pos = entity.pos.load();

                world.send_entity_status(
                    entity,
                    pumpkin_data::entity::EntityStatus::InLoveHearts,
                    Some(ActorEventID::InLoveHearts),
                );

                world.spawn_particle(
                    pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                    Vector3::new(0.5, 0.5, 0.5),
                    1.0,
                    7,
                    Particle::Heart,
                );
                world.play_sound(ambient_sound, SoundCategory::Neutral, &entity.pos.load());
                return true;
            }

            if age < 0 {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                let speedup = (-age / 10).max(1);
                mob_entity
                    .living_entity
                    .entity
                    .age
                    .fetch_add(speedup, std::sync::atomic::Ordering::Relaxed);

                let entity = &mob_entity.living_entity.entity;
                let world = entity.world.load();
                let pos = entity.pos.load();

                world.spawn_particle(
                    pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                    Vector3::new(0.5, 0.5, 0.5),
                    1.0,
                    7,
                    Particle::HappyVillager,
                );
                self.play_eating_sound(ambient_sound);
                return true;
            }
        }

        mob_entity.mob_interact(player, item_stack)
    }
}

#[must_use]
pub fn get_dye_color_from_item(item: &pumpkin_data::item::Item) -> Option<u8> {
    match item.registry_key {
        "white_dye" => Some(0),
        "orange_dye" => Some(1),
        "magenta_dye" => Some(2),
        "light_blue_dye" => Some(3),
        "yellow_dye" => Some(4),
        "lime_dye" => Some(5),
        "pink_dye" => Some(6),
        "gray_dye" => Some(7),
        "light_gray_dye" => Some(8),
        "cyan_dye" => Some(9),
        "purple_dye" => Some(10),
        "blue_dye" => Some(11),
        "brown_dye" => Some(12),
        "green_dye" => Some(13),
        "red_dye" => Some(14),
        "black_dye" => Some(15),
        _ => None,
    }
}

#[must_use]
pub const fn get_wool_item_for_color(color: u8) -> &'static pumpkin_data::item::Item {
    match color {
        0 => &pumpkin_data::item::Item::WHITE_WOOL,
        1 => &pumpkin_data::item::Item::ORANGE_WOOL,
        2 => &pumpkin_data::item::Item::MAGENTA_WOOL,
        3 => &pumpkin_data::item::Item::LIGHT_BLUE_WOOL,
        4 => &pumpkin_data::item::Item::YELLOW_WOOL,
        5 => &pumpkin_data::item::Item::LIME_WOOL,
        6 => &pumpkin_data::item::Item::PINK_WOOL,
        7 => &pumpkin_data::item::Item::GRAY_WOOL,
        8 => &pumpkin_data::item::Item::LIGHT_GRAY_WOOL,
        9 => &pumpkin_data::item::Item::CYAN_WOOL,
        10 => &pumpkin_data::item::Item::PURPLE_WOOL,
        11 => &pumpkin_data::item::Item::BLUE_WOOL,
        12 => &pumpkin_data::item::Item::BROWN_WOOL,
        13 => &pumpkin_data::item::Item::GREEN_WOOL,
        14 => &pumpkin_data::item::Item::RED_WOOL,
        _ => &pumpkin_data::item::Item::BLACK_WOOL,
    }
}

#[must_use]
pub fn get_carpet_color_from_item(item: &pumpkin_data::item::Item) -> Option<u8> {
    match item.registry_key {
        "white_carpet" => Some(0),
        "orange_carpet" => Some(1),
        "magenta_carpet" => Some(2),
        "light_blue_carpet" => Some(3),
        "yellow_carpet" => Some(4),
        "lime_carpet" => Some(5),
        "pink_carpet" => Some(6),
        "gray_carpet" => Some(7),
        "light_gray_carpet" => Some(8),
        "cyan_carpet" => Some(9),
        "purple_carpet" => Some(10),
        "blue_carpet" => Some(11),
        "brown_carpet" => Some(12),
        "green_carpet" => Some(13),
        "red_carpet" => Some(14),
        "black_carpet" => Some(15),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::is_spawn_egg_for;
    use pumpkin_data::entity::EntityType;
    use pumpkin_data::item::Item;
    use pumpkin_data::item_stack::ItemStack;

    #[test]
    fn matching_egg_is_recognized() {
        let stack = ItemStack::new(1, &Item::COW_SPAWN_EGG);
        assert!(is_spawn_egg_for(&stack, &EntityType::COW));
    }

    /// The wrong egg has to fall through, otherwise using a cow egg on a pig would
    /// spawn a baby pig instead of placing a cow.
    #[test]
    fn egg_for_another_mob_is_not_recognized() {
        let stack = ItemStack::new(1, &Item::COW_SPAWN_EGG);
        assert!(!is_spawn_egg_for(&stack, &EntityType::PIG));
        assert!(!is_spawn_egg_for(&stack, &EntityType::CHICKEN));
    }

    #[test]
    fn a_non_egg_item_is_not_recognized() {
        let stack = ItemStack::new(1, &Item::WHEAT);
        assert!(!is_spawn_egg_for(&stack, &EntityType::COW));
    }
}
