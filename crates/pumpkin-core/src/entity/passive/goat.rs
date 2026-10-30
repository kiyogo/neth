use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    combat,
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::item::items::bucket::create_filled_result;

const TEMPT_ITEMS: &[&Item] = &[&Item::WHEAT];

pub struct GoatEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub is_screaming: AtomicBool,
    pub has_left_horn: AtomicBool,
    pub has_right_horn: AtomicBool,
}

impl GoatEntity {
    pub const XP_REWARD: u32 = combat::GOAT_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        let is_screaming = combat::should_spawn_screaming_goat(rand::random_range(0.0..1.0));
        let mut has_left = true;
        let mut has_right = true;
        if combat::should_goat_spawn_unihorn(is_baby, rand::random_range(0.0..1.0)) {
            if rand::random() {
                has_left = false;
            } else {
                has_right = false;
            }
        }

        let mob_entity = MobEntity::new(entity);
        let goat = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            is_screaming: AtomicBool::new(is_screaming),
            has_left_horn: AtomicBool::new(has_left),
            has_right_horn: AtomicBool::new(has_right),
        };
        let mob_arc = Arc::new(goat);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        // Vanilla Goat: MAX_HEALTH = 10.0, MOVEMENT_SPEED = 0.2, ATTACK_DAMAGE = 2.0 (baby: 1.0)
        let is_baby = mob_arc.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0;
        let attack_dmg = if is_baby {
            combat::GOAT_BABY_ATTACK_DAMAGE
        } else {
            combat::GOAT_BASE_ATTACK_DAMAGE
        };
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::MAX_HEALTH,
            combat::GOAT_MAX_HEALTH,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::MOVEMENT_SPEED,
            combat::GOAT_BASE_MOVEMENT_SPEED,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::ATTACK_DAMAGE,
            attack_dmg,
        );
            mob_arc.mob_entity.living_entity.set_health(combat::GOAT_MAX_HEALTH as f32);

        mob_arc
    }

    #[must_use]
    pub fn is_screaming(&self) -> bool {
        self.is_screaming.load(Ordering::Relaxed)
    }

    pub fn set_screaming(&self, screaming: bool) {
        self.is_screaming.store(screaming, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_IS_SCREAMING_GOAT,
            screaming,
        );
    }

    #[must_use]
    pub fn has_left_horn(&self) -> bool {
        self.has_left_horn.load(Ordering::Relaxed)
    }

    pub fn set_has_left_horn(&self, has_horn: bool) {
        self.has_left_horn.store(has_horn, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_LEFT_HORN,
            has_horn,
        );
    }

    #[must_use]
    pub fn has_right_horn(&self) -> bool {
        self.has_right_horn.load(Ordering::Relaxed)
    }

    pub fn set_has_right_horn(&self, has_horn: bool) {
        self.has_right_horn.store(has_horn, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_RIGHT_HORN,
            has_horn,
        );
    }

    pub fn drop_horn(&self) -> bool {
        if self.is_baby() {
            return false;
        }

        let has_left = self.has_left_horn();
        let has_right = self.has_right_horn();
        let horn_to_drop = combat::select_goat_horn_to_drop(has_left, has_right, rand::random());
        match horn_to_drop {
            Some(true) => self.set_has_left_horn(false),
            Some(false) => self.set_has_right_horn(false),
            None => return false,
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound(
            Sound::EntityGoatHornBreak,
            SoundCategory::Neutral,
            &pos,
        );

        let delta_x = rand::random_range(-0.2..0.2);
        let delta_y = rand::random_range(0.3..0.7);
        let delta_z = rand::random_range(-0.2..0.2);
        let drop_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
        drop_entity
            .velocity
            .store(Vector3::new(delta_x, delta_y, delta_z));
        world.spawn_entity(Arc::new(ItemEntity::new(
            drop_entity,
            ItemStack::new(1, &Item::GOAT_HORN),
        )));
        true
    }
}

impl AgeableMob for GoatEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for GoatEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_GOAT_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for GoatEntity {
    fn get_base_experience_reward(&self) -> u32 {
        if self.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0 {
            0
        } else {
            Self::XP_REWARD
        }
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

    fn as_goat(&self) -> Option<&GoatEntity> {
        Some(self)
    }

    /// Vanilla `Goat.calculateFallDamage`: fall damage is reduced by 10.
    fn calculate_fall_damage(&self, fall_distance: f64, damage_modifier: f32) -> i32 {
        combat::calculate_goat_fall_damage_points(fall_distance, damage_modifier)
    }

    /// Vanilla `Goat.calculateFallDamage`: fall damage is reduced by 10.
    fn adjust_damage_amount(&self, amount: f32, damage_type: &DamageType) -> f32 {
        if *damage_type == DamageType::FALL {
            (amount - 10.0).max(0.0)
        } else {
            amount
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("IsScreamingGoat", self.is_screaming());
        nbt.put_bool("HasLeftHorn", self.has_left_horn());
        nbt.put_bool("HasRightHorn", self.has_right_horn());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(screaming) = nbt.get_bool("IsScreamingGoat") {
            self.set_screaming(screaming);
        }
        if let Some(left) = nbt.get_bool("HasLeftHorn") {
            self.set_has_left_horn(left);
        }
        if let Some(right) = nbt.get_bool("HasRightHorn") {
            self.set_has_right_horn(right);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        if !self.is_baby()
            && (self.has_left_horn() || self.has_right_horn())
            && entity.horizontal_collision.load(Ordering::Relaxed)
        {
            let world = entity.world.load();
            let yaw_rad = entity.yaw.load().to_radians();
            let look_dir_x = -yaw_rad.sin();
            let look_dir_z = yaw_rad.cos();
            let facing_pos = BlockPos::floored(
                entity.pos.load().x + look_dir_x as f64,
                f64::from(entity.block_pos.load().0.y),
                entity.pos.load().z + look_dir_z as f64,
            );
            let facing_above = facing_pos.up();
            let state_facing = world.get_block_state(&facing_pos);
            let state_above = world.get_block_state(&facing_above);
            let block_facing = Block::from_state_id(state_facing.id);
            let block_above = Block::from_state_id(state_above.id);

            let snaps_horn = block_facing.has_tag(&tag::Block::MINECRAFT_SNAPS_GOAT_HORN)
                || block_above.has_tag(&tag::Block::MINECRAFT_SNAPS_GOAT_HORN);

            if snaps_horn {
                self.drop_horn();
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if self.is_screaming() {
                Sound::EntityGoatScreamingAmbient
            } else {
                Sound::EntityGoatAmbient
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                if entity.age.load(Ordering::Relaxed) < 0 {
                    1.25
                } else {
                    1.0
                },
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::goat::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_IS_SCREAMING_GOAT,
            self.is_screaming(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_LEFT_HORN,
            self.has_left_horn(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_RIGHT_HORN,
            self.has_right_horn(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        if item == &Item::BUCKET && !self.is_baby() {
            create_filled_result(item_stack, player, &Item::MILK_BUCKET, true);
            let entity = self.get_entity();
            let world = entity.world.load();
            let sound = if self.is_screaming() {
                Sound::EntityGoatScreamingMilk
            } else {
                Sound::EntityGoatMilk
            };
            world.play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
            let mut milk = ItemStack::new(1, &Item::MILK_BUCKET);
            let was_added = player.inventory.insert_stack_anywhere(&mut milk);
            if !was_added && !milk.is_empty() {
                world.drop_stack(&entity.block_pos.load(), milk);
            }
            return true;
        }

        let eat_sound = if self.is_screaming() {
            Sound::EntityGoatScreamingEat
        } else {
            Sound::EntityGoatEat
        };
        self.animal_interact(player, item_stack, eat_sound)
    }

}

impl CustomSound for GoatEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(if self.is_screaming() {
            Sound::EntityGoatScreamingDeath
        } else {
            Sound::EntityGoatDeath
        })
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(if self.is_screaming() {
            Sound::EntityGoatScreamingHurt
        } else {
            Sound::EntityGoatHurt
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goat_parity_constants() {
        assert_eq!(GoatEntity::XP_REWARD, 2);
        assert_eq!(combat::GOAT_MAX_HEALTH, 10.0);
        assert_eq!(combat::GOAT_BASE_MOVEMENT_SPEED, 0.2);
        assert_eq!(combat::GOAT_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(combat::GOAT_BABY_ATTACK_DAMAGE, 1.0);
        assert_eq!(combat::GOAT_FALL_DAMAGE_REDUCTION, 10);
        assert_eq!(combat::GOAT_SCREAMING_CHANCE, 0.02);
    }
}
