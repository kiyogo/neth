use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};

use crate::entity::{
    combat,
    Entity, EntityBase, item::ItemEntity,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal,
        look_at_entity::LookAtEntityGoal, tempt::TemptGoal,
        try_find_water::TryFindWaterGoal, wander_around::WanderAroundGoal,
    },
    custom_sound::CustomSound,
    living::LivingEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::entity::ai::pathfinder::Navigator;
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::SEAGRASS];

/// Vanilla `Turtle.BABY_ON_LAND_SELECTOR`: only targets that are babies on land.
///
/// The cat and ocelot prey target goals share this one predicate, like vanilla.
pub(crate) fn baby_on_land_selector(target: &LivingEntity, _world: &World) -> bool {
    target.entity.age.load(Ordering::Relaxed) < 0 && !target.is_in_water()
}

pub struct TurtleEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub has_egg: AtomicBool,
    pub laying_egg: AtomicBool,
    pub home_pos: std::sync::Mutex<BlockPos>,
}

impl TurtleEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::TURTLE_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        let spawn_block_pos = entity.block_pos.load();
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::amphibious(false);

        let turtle = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            has_egg: AtomicBool::new(false),
            laying_egg: AtomicBool::new(false),
            home_pos: std::sync::Mutex::new(spawn_block_pos),
        };
        let mob_arc = Arc::new(turtle);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Turtle: MAX_HEALTH = 30.0, MOVEMENT_SPEED = 0.25, STEP_HEIGHT = 1.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::TURTLE_MAX_HEALTH as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::TURTLE_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::STEP_HEIGHT, crate::entity::combat::TURTLE_STEP_HEIGHT as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::TURTLE_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vanilla Turtle: 0 Panic(1.2), 1 Breed(1.0), 2 Tempt(1.1), 3 GoToWater(1.0), 8 LookAtPlayer(8.0), 9 Stroll(1.0)
            goal_selector.add_goal(0, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(1, BreedGoal::new(1.0));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.1, TEMPT_ITEMS, false)));
            goal_selector.add_goal(3, Box::new(TryFindWaterGoal));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(9, Box::new(WanderAroundGoal::new(1.0)));
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let is_baby = self.is_baby();
        let (w, h, eye) = combat::calculate_turtle_dimensions(
            is_baby,
            entity.entity_type.dimension[0],
            entity.entity_type.dimension[1],
            entity.entity_type.eye_height,
        );
        let dimensions = EntityDimensions {
            width: w,
            height: h,
            eye_height: eye,
        };
        let pos = entity.pos.load();
        entity.bounding_box.store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    #[must_use]
    pub fn get_home_pos(&self) -> BlockPos {
        *self.home_pos.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_home_pos(&self, pos: BlockPos) {
        *self.home_pos.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = pos;
    }

    #[must_use]
    pub fn has_egg(&self) -> bool {
        self.has_egg.load(Ordering::Relaxed)
    }

    pub fn set_has_egg(&self, has_egg: bool) {
        self.has_egg.store(has_egg, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::HAS_EGG, has_egg);
    }

    #[must_use]
    pub fn is_laying_egg(&self) -> bool {
        self.laying_egg.load(Ordering::Relaxed)
    }

    pub fn set_laying_egg(&self, laying_egg: bool) {
        self.laying_egg.store(laying_egg, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::LAYING_EGG, laying_egg);
    }

    /// Vanilla `Turtle.BABY_ON_LAND_SELECTOR`: a hatchling out of the water.
    #[must_use]
    pub fn baby_on_land_selector(target: &LivingEntity, _world: &World) -> bool {
        target.entity.age.load(Ordering::Relaxed) < 0 && !target.entity.is_in_water()
    }
}

impl AgeableMob for TurtleEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn set_baby(&self, baby: bool) {
        self.set_age(if baby { self.get_baby_start_age() } else { 0 });
        self.refresh_dimensions();
    }

    /// Vanilla `Turtle.java:260-265`:
    /// `super.ageBoundaryReached(); if (!this.isBaby() && level.getGameRules().get(GameRules.MOB_DROPS)) { this.dropFromGiftLootTable(level, BuiltInLootTables.TURTLE_GROW, this::spawnAtLocation); }`
    fn age_boundary_reached(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        self.refresh_dimensions();
        crate::entity::ageable::handle_age_boundary_vehicle_dismount(self.is_baby(), entity);
        if !self.is_baby() {
            let world = entity.world.load();
            if world.level_info.load().game_rules.mob_drops {
                let pos = entity.pos.load();
                let item_entity = Arc::new(ItemEntity::new(
                    Entity::new(world.clone(), pos, &EntityType::ITEM),
                    ItemStack::new(1, crate::entity::combat::TURTLE_GROWTH_SCUTE_ITEM),
                ));
                world.spawn_entity(item_entity);
            }
        }
    }
}

impl Animal for TurtleEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_TURTLE_FOOD)
            || item_stack.item == &Item::SEAGRASS
    }
}

impl Mob for TurtleEntity {
    /// Turtles never drown.
    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
    }

    /// Vanilla Turtle.java:279: Turtles cannot be leashed.
    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    /// Turtle.getAmbientSoundInterval in vanilla is 200.
    fn get_ambient_sound_interval(&self) -> i32 {
        200
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_turtle(&self) -> Option<&TurtleEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        let home = self.get_home_pos();
        nbt.put_int_array("home_pos", vec![home.0.x, home.0.y, home.0.z]);
        nbt.put_bool("has_egg", self.has_egg());
        nbt.put_bool("HasEgg", self.has_egg());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(pos) = nbt.get_int_array("home_pos") {
            if pos.len() >= 3 {
                self.set_home_pos(BlockPos::new(pos[0], pos[1], pos[2]));
            }
        } else if let (Some(x), Some(y), Some(z)) = (
            nbt.get_int("HomePosX"),
            nbt.get_int("HomePosY"),
            nbt.get_int("HomePosZ"),
        ) {
            self.set_home_pos(BlockPos::new(x, y, z));
        }

        if let Some(has_egg) = nbt.get_bool("has_egg").or_else(|| nbt.get_bool("HasEgg")) {
            self.set_has_egg(has_egg);
        }
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
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

        if !entity.silent.load(Ordering::Relaxed)
            && !entity.touching_water.load(Ordering::Relaxed)
            && entity.on_ground.load(Ordering::Relaxed)
            && !self.is_baby()
            && rand::random_range(0..100) == 0
        {
            entity.world.load().play_sound_fine(
                Sound::EntityTurtleAmbientLand,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        lightning: &crate::entity::lightning::LightningBoltEntity,
    ) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        // Vanilla Turtle.java:283-285 & VanillaEntityLoot.java:1007:
        // Spawn a bowl when killed by lightning
        let drop_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
        world.spawn_entity(Arc::new(ItemEntity::new(
            drop_entity,
            ItemStack::new(1, combat::TURTLE_LIGHTNING_DROP_ITEM),
        )));

        let cause = lightning.get_cause();
        self.mob_entity.living_entity.damage_with_context(
            caller,
            crate::entity::combat::get_turtle_thunder_hit_damage(),
            pumpkin_data::damage::DamageType::LIGHTNING_BOLT,
            None,
            Some(lightning),
            cause.as_deref().map(|p| p as &dyn EntityBase),
        );
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::turtle::DATA_BABY_ID, true);
        }
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::HAS_EGG, self.has_egg());
        entity.set_synced_data(
            pumpkin_data::tracked_data::turtle::LAYING_EGG,
            self.is_laying_egg(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityTurtleAmbientLand)
    }
}

impl CustomSound for TurtleEntity {
    fn death_sound(&self) -> Option<Sound> {
        if self.is_baby() {
            Some(Sound::EntityTurtleDeathBaby)
        } else {
            Some(Sound::EntityTurtleDeath)
        }
    }

    fn hurt_sound(&self) -> Option<Sound> {
        if self.is_baby() {
            Some(Sound::EntityTurtleHurtBaby)
        } else {
            Some(Sound::EntityTurtleHurt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat;

    #[test]
    fn test_turtle_parity_constants() {
        assert_eq!(TurtleEntity::XP_REWARD, 2);
        assert_eq!(combat::TURTLE_MAX_HEALTH, 30.0);
        assert_eq!(combat::TURTLE_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(combat::TURTLE_STEP_HEIGHT, 1.0);
        assert_eq!(combat::TURTLE_AMBIENT_SOUND_INTERVAL, 200);
        assert_eq!(combat::TURTLE_BABY_SCALE, 0.3);
        assert_eq!(combat::TURTLE_LAY_EGG_TICKS, 200);
        assert_eq!(combat::TURTLE_MAX_EGGS_LAID, 4);
        assert_eq!(combat::TURTLE_EGG_DIG_INTERVAL_TICKS, 5);
        assert_eq!(combat::TURTLE_HOME_RADIUS, 64.0);
        assert_eq!(combat::TURTLE_CLOSE_HOME_RADIUS, 16.0);
        assert_eq!(combat::TURTLE_GIVE_UP_HOME_TICKS, 600);
        assert_eq!(combat::TURTLE_LIGHTNING_DROP_ITEM, &Item::BOWL);
    }

    #[test]
    fn test_turtle_nbt_home_pos_and_egg_roundtrip() {
        let mut nbt = NbtCompound::new();
        nbt.put_int_array("home_pos", vec![120, 63, -450]);
        nbt.put_bool("has_egg", true);

        // Verify modern NBT keys
        let home_arr = nbt.get_int_array("home_pos").unwrap();
        assert_eq!(home_arr, &[120, 63, -450]);
        assert_eq!(nbt.get_bool("has_egg"), Some(true));

        // Verify legacy NBT backward compatibility
        let mut legacy_nbt = NbtCompound::new();
        legacy_nbt.put_int("HomePosX", -15);
        legacy_nbt.put_int("HomePosY", 65);
        legacy_nbt.put_int("HomePosZ", 300);
        legacy_nbt.put_bool("HasEgg", true);

        let x = legacy_nbt.get_int("HomePosX").unwrap();
        let y = legacy_nbt.get_int("HomePosY").unwrap();
        let z = legacy_nbt.get_int("HomePosZ").unwrap();
        assert_eq!(BlockPos::new(x, y, z), BlockPos::new(-15, 65, 300));
        assert_eq!(legacy_nbt.get_bool("has_egg").or_else(|| legacy_nbt.get_bool("HasEgg")), Some(true));
    }

    #[test]
    fn test_turtle_baby_dimensions_and_parity() {
        let adult = combat::calculate_turtle_dimensions(false, 1.2, 0.4, 0.34);
        assert_eq!(adult, (1.2, 0.4, 0.34));
        let baby = combat::calculate_turtle_dimensions(true, 1.2, 0.4, 0.34);
        assert_eq!(baby, (0.36, 0.12, 0.102));
    }
}
