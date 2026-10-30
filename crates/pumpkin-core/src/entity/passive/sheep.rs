use crate::entity::combat;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU8, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::dye_color::DyeColor;
use pumpkin_data::{entity::EntityType, item::Item};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Hand;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, eat_grass::EatGrassGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
    shearable::{Shearable, shear_by_player, shearing_loot},
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::loot_table::{LootEntityPropertyValue, LootEntityTarget};

use crate::world::loot::LootContextParameters;

const TEMPT_ITEMS: &[&Item] = &[&Item::WHEAT];
/// The dye color bits in vanilla's `DATA_WOOL_ID` byte.
const COLOR_MASK: u8 = 0x0F;
/// The sheared flag in vanilla's `DATA_WOOL_ID` byte.
const SHEARED_BIT: u8 = 0x10;

/// Loot-context key holding the sheep's wool color, matched by the `entities/sheep` predicates.
const COLOR_LOOT_PROPERTY: &str = "minecraft:components/minecraft:sheep/color";
/// Loot-context key holding the sheep's sheared state, matched by the same predicates.
const SHEARED_LOOT_PROPERTY: &str = "minecraft:type_specific/sheep/sheared";

pub struct SheepEntity {
    pub mob_entity: MobEntity,
    color_and_sheared: AtomicU8,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

impl SheepEntity {
    pub const XP_REWARD: u32 = combat::SHEEP_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let sheep = Self {
            mob_entity,
            color_and_sheared: AtomicU8::new(0),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(sheep);

        // Vanilla Sheep attributes: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.23
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MAX_HEALTH,
                combat::SHEEP_MAX_HEALTH,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MOVEMENT_SPEED,
                combat::SHEEP_BASE_MOVEMENT_SPEED,
            );
            mob_arc.mob_entity.living_entity.set_health(combat::SHEEP_MAX_HEALTH as f32);

        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.1, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(EatGrassGoal::default()));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    fn get_packed_byte(&self) -> u8 {
        self.color_and_sheared.load(Ordering::Relaxed)
    }

    pub fn get_color(&self) -> u8 {
        self.get_packed_byte() & COLOR_MASK
    }

    pub fn is_sheared(&self) -> bool {
        (self.get_packed_byte() & SHEARED_BIT) != 0
    }

    // No races due to atomic byte setters
    fn sync_packed_byte(&self) {
        self.mob_entity.living_entity.entity.set_synced_data(
            pumpkin_data::tracked_data::sheep::WOOL_ID,
            self.get_packed_byte() as i8,
        );
    }

    pub fn set_color(&self, color: u8) {
        self.color_and_sheared
            .update(Ordering::Relaxed, Ordering::Relaxed, |byte| {
                (byte & !COLOR_MASK) | (color & COLOR_MASK)
            });
        self.sync_packed_byte();
    }

    pub fn set_sheared(&self, sheared: bool) {
        if sheared {
            self.color_and_sheared
                .fetch_or(SHEARED_BIT, Ordering::Relaxed);
        } else {
            self.color_and_sheared
                .fetch_and(!SHEARED_BIT, Ordering::Relaxed);
        }
        self.sync_packed_byte();
    }

    pub fn set_packed_and_sync(&self, byte: u8) {
        self.color_and_sheared.store(byte, Ordering::Relaxed);
        self.sync_packed_byte();
    }
}

impl Shearable for SheepEntity {
    fn shear(&self, sound_category: SoundCategory, tool: &ItemStack) -> bool {
        if self
            .color_and_sheared
            .fetch_or(SHEARED_BIT, Ordering::Relaxed)
            & SHEARED_BIT
            != 0
        {
            return false;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound(Sound::EntitySheepShear, sound_category, &pos);

        let color = DyeColor::by_id(self.get_color()).unwrap_or(DyeColor::White);
        let loot_key = format!("minecraft:shearing/sheep/{}", color.name()); // is there a better way to do this
        let drop_pos = Vector3::new(pos.x, pos.y + 1.0, pos.z);
        let mut rng = rand::rng();
        for drop in shearing_loot(entity, &loot_key, tool) {
            for _ in 0..drop.item_count {
                let item_entity = ItemEntity::new(
                    Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
                    drop.copy_with_count(1),
                );
                let item_base = item_entity.get_entity();
                item_base.velocity.store(
                    item_base.velocity.load()
                        + Vector3::new(
                            // magic numbers from vanilla
                            f64::from((rng.random::<f32>() - rng.random::<f32>()) * 0.1),
                            f64::from(rng.random::<f32>() * 0.05),
                            f64::from((rng.random::<f32>() - rng.random::<f32>()) * 0.1),
                        ),
                );
                world.spawn_entity(Arc::new(item_entity));
            }
        }

        // Bit is set, send change to clients
        self.set_sheared(true);
        true
    }

    fn ready_for_shearing(&self) -> bool {
        !self.is_sheared() && !self.is_baby()
    }
}

impl AgeableMob for SheepEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for SheepEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_SHEEP_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for SheepEntity {
    fn get_base_experience_reward(&self) -> u32 {
        if self.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0 {
            0
        } else {
            Self::XP_REWARD
        }
    }

    /// Publishes the wool color and sheared state that `entities/sheep` predicates match on,
    /// so only the entry for this sheep's color is eligible and a sheared sheep drops none.
    fn populate_loot_context(&self, params: &mut LootContextParameters) {
        params.add_entity_property(
            LootEntityTarget::This,
            COLOR_LOOT_PROPERTY,
            LootEntityPropertyValue::String(DyeColor::from(self.get_color()).name()),
        );
        params.add_entity_property(
            LootEntityTarget::This,
            SHEARED_LOOT_PROPERTY,
            LootEntityPropertyValue::Bool(self.is_sheared()),
        );
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_sheep(&self) -> Option<&SheepEntity> {
        Some(self)
    }

    fn as_shearable(&self) -> Option<&dyn Shearable> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("Sheared", self.is_sheared());
        nbt.put_byte("Color", self.get_color() as i8);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        let sheared = nbt
            .get_bool("Sheared")
            .or_else(|| nbt.get_byte("Sheared").map(|b| b == 1))
            .unwrap_or(false);
        let color = nbt.get_byte("Color").unwrap_or(0) as u8;
        let byte = (color & COLOR_MASK) | if sheared { SHEARED_BIT } else { 0 };
        self.set_packed_and_sync(byte);
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn on_eating_grass(&self) {
        self.set_sheared(false);
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use super::animal::{Animal, get_dye_color_from_item};
        let item = item_stack.get_item();

        if item == &Item::SHEARS && self.ready_for_shearing() {
            if !shear_by_player(self, player, item_stack) {
                return false;
            }
            // Vanilla returns `SUCCESS_SERVER` here, so the client waits for the server to swing its arm.
            player.swing_hand(Hand::Right, true);
            return true;
        }

        if let Some(color) = get_dye_color_from_item(item)
            && !self.is_sheared()
            && color != self.get_color()
        {
            self.set_color(color);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            return true;
        }

        self.animal_interact(player, item_stack, Sound::EntitySheepAmbient)
    }
}

impl CustomSound for SheepEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySheepDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySheepHurt)
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::combat;

    #[test]
    fn test_sheep_parity_constants() {
        assert_eq!(crate::entity::passive::sheep::SheepEntity::XP_REWARD, 2);
        assert_eq!(combat::SHEEP_MAX_HEALTH, 8.0);
        assert_eq!(combat::SHEEP_BASE_MOVEMENT_SPEED, 0.23);
        assert_eq!(combat::SHEEP_SHEAR_WOOL_MIN, 1);
        assert_eq!(combat::SHEEP_SHEAR_WOOL_MAX, 3);
    }
    use pumpkin_util::loot_table::{
        LootCondition, LootEntityProperties, LootEntityProperty, LootEntityPropertyValue,
        LootEntityTarget,
    };

    use crate::world::loot::{LootContextParameters, generate_loot_with_context};

    const COLORS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];

    /// Builds the property snapshot a sheep of the given color and sheared state publishes.
    fn properties(color: &'static str, sheared: bool) -> LootEntityProperties {
        LootEntityProperties {
            values: vec![
                LootEntityProperty {
                    key: super::COLOR_LOOT_PROPERTY,
                    value: LootEntityPropertyValue::String(color),
                },
                LootEntityProperty {
                    key: super::SHEARED_LOOT_PROPERTY,
                    value: LootEntityPropertyValue::Bool(sheared),
                },
            ],
        }
    }

    /// Builds the loot context a sheep of the given color and sheared state would publish.
    fn context(color: &'static str, sheared: bool) -> LootContextParameters {
        let mut params = LootContextParameters::default();
        params.add_entity_property(
            LootEntityTarget::This,
            super::COLOR_LOOT_PROPERTY,
            LootEntityPropertyValue::String(color),
        );
        params.add_entity_property(
            LootEntityTarget::This,
            super::SHEARED_LOOT_PROPERTY,
            LootEntityPropertyValue::Bool(sheared),
        );
        params
    }

    /// Returns the wool items the generated sheep table drops for the given context.
    fn dropped_wool(params: &LootContextParameters, seed: i64) -> Vec<String> {
        generate_loot_with_context(&pumpkin_data::loot_table::ENTITIES_SHEEP, seed, params)
            .iter()
            .map(|stack| {
                let key = stack.item.registry_key;
                key.strip_prefix("minecraft:").unwrap_or(key).to_owned()
            })
            .filter(|key| key.ends_with("_wool"))
            .collect()
    }

    /// Rolling the real table must drop exactly the wool matching the sheep's color, and a
    /// sheared sheep must drop no wool at all. This is the behavior #3225 reported broken.
    #[test]
    fn sheep_loot_drops_only_the_wool_matching_its_color() {
        for (seed, color) in COLORS.iter().enumerate() {
            let seed = seed as i64;
            assert_eq!(
                dropped_wool(&context(color, false), seed),
                vec![format!("{color}_wool")],
                "unsheared {color} sheep must drop {color} wool"
            );
            assert!(
                dropped_wool(&context(color, true), seed).is_empty(),
                "sheared {color} sheep must not drop wool"
            );
        }
    }

    /// Every generated wool entry must accept exactly its own color on an unsheared sheep,
    /// and reject both a different color and the sheared state.
    #[test]
    fn generated_loot_matches_only_unsheared_sheep_of_the_right_color() {
        let wool_entries = pumpkin_data::loot_table::ENTITIES_SHEEP.pools[1].entries;
        assert_eq!(wool_entries.len(), COLORS.len());

        for (index, entry) in wool_entries.iter().enumerate() {
            let (target, predicate) = match &entry.condition {
                LootCondition::EntityProperties { target, predicate } => (*target, predicate),
                LootCondition::AllOf(conditions) => conditions
                    .iter()
                    .find_map(|c| match c {
                        LootCondition::EntityProperties { target, predicate } => {
                            Some((*target, predicate))
                        }
                        _ => None,
                    })
                    .expect("sheep wool entry must have an entity-properties condition"),
                _ => panic!("sheep wool entry must have an entity-properties condition"),
            };
            assert_eq!(target, LootEntityTarget::This);
            assert!(properties(COLORS[index], false).matches(*predicate));
            assert!(!properties(COLORS[(index + 1) % COLORS.len()], false).matches(*predicate));
            assert!(!properties(COLORS[index], true).matches(*predicate));
        }
    }
}
