use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Potion;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data;

use crate::entity::attributes::{Modifier, ModifierOperation};
use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, ranged_attack::RangedAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{
        Mob, MobEntity, RangedAttackMob,
        patrol::{PatrolData, PatrollingMonster},
        raider::{
            PathfindToRaidGoal, Raider, RaiderCelebrationGoal, RaiderData,
            RaiderMoveThroughVillageGoal,
        },
    },
    projectile::splash_potion::SplashPotionEntity,
};

fn create_potion_stack(item: &'static Item, potion: &'static Potion) -> ItemStack {
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::data_component_impl::{DataComponentImpl, PotionContentsImpl};
    let mut stack = ItemStack::new(1, item);
    stack.patch.push((
        DataComponent::PotionContents,
        Some(
            PotionContentsImpl {
                potion_id: Some(i32::from(potion.id)),
                custom_color: None,
                custom_effects: Vec::new(),
                custom_name: None,
            }
            .to_dyn(),
        ),
    ));
    stack
}

/// Represents a Witch, a hostile ranged mob that throws splash potions and drinks restorative potions.
///
/// Wiki: <https://minecraft.wiki/w/Witch>
pub struct WitchEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    drinking_potion: AtomicBool,
    using_time: AtomicI32,
}

impl WitchEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, 26.0);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, 0.25);
        mob_entity.living_entity.health.store(26.0);

        let witch = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            drinking_potion: AtomicBool::new(false),
            using_time: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(witch);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let ranged_weak: Weak<dyn RangedAttackMob> = {
            let ranged_arc: Arc<dyn RangedAttackMob> = mob_arc.clone();
            Arc::downgrade(&ranged_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                2,
                Box::new(RangedAttackGoal::new(ranged_weak, 1.0, 60, 10.0)),
            );
            goal_selector.add_goal(2, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(3, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            goal_selector.add_goal(4, Box::new(PathfindToRaidGoal::default()));
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(
                1,
                Box::new(RevengeGoal::new(true).ignoring(|entity_type| {
                    entity_type.has_tag(&tag::EntityType::MINECRAFT_RAIDERS)
                })),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    pub fn set_drinking_potion(&self, drinking: bool) {
        self.drinking_potion.store(drinking, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::witch::DATA_USING_ITEM, drinking);
    }

    #[must_use]
    pub fn is_drinking_potion(&self) -> bool {
        self.drinking_potion.load(Ordering::Relaxed)
    }

    pub fn throw_potion(&self, target: &Arc<dyn EntityBase>) {
        if self.is_drinking_potion() {
            return;
        }

        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load_full();

        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let target_vel = target_entity.velocity.load();
        let witch_pos = entity.pos.load();

        let xd = target_pos.x + target_vel.x - witch_pos.x;
        let yd = target_pos.y + target_entity.get_eye_height() - 1.1 - witch_pos.y;
        let zd = target_pos.z + target_vel.z - witch_pos.z;
        let dist = xd.hypot(zd);

        let potion = if let Some(target_living) = target.get_living_entity() {
            let is_raider = target
                .get_entity()
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_RAIDERS);
            if is_raider {
                *self
                    .mob_entity
                    .target
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }
            crate::entity::combat::determine_witch_thrown_potion(
                is_raider,
                target_living.health.load(),
                dist,
                target_living.has_effect(&StatusEffect::SLOWNESS),
                target_living.has_effect(&StatusEffect::POISON),
                target_living.has_effect(&StatusEffect::WEAKNESS),
                rand::random(),
            )
        } else {
            &Potion::HARMING
        };

        let potion_stack = create_potion_stack(&Item::SPLASH_POTION, potion);

        let splash_entity = Entity::new(world.clone(), witch_pos, &EntityType::SPLASH_POTION);
        let splash = SplashPotionEntity::new_shot(splash_entity, entity);
        splash.set_item_stack(potion_stack);

        let speed = if dist <= 2.0 { 0.45 } else { 0.75 };
        let yo = dist * 0.2;

        splash.thrown.set_velocity(xd, yd + yo, zd, speed, 8.0);

        if !entity.silent.load(Ordering::Relaxed) {
            let rand_pitch: f32 = rand::random();
            let pitch = crate::entity::combat::calculate_witch_throw_pitch(rand_pitch);
            world.play_sound_fine(
                Sound::EntityWitchThrow,
                SoundCategory::Hostile,
                &witch_pos,
                1.0,
                pitch,
            );
        }

        let splash_arc: Arc<dyn EntityBase> = Arc::new(splash);
        world.spawn_entity(splash_arc);
    }
}

impl CustomSound for WitchEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityWitchDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityWitchHurt)
    }
}

impl Mob for WitchEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_witch(&self) -> Option<&WitchEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_patrolling_monster(&self) -> Option<&dyn PatrollingMonster> {
        Some(self)
    }

    fn as_raider(&self) -> Option<&dyn Raider> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        self.write_raider_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        self.read_raider_nbt(nbt);
    }

    fn pre_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        // Vanilla Witch.java:211:
        // if (damageSource.getEntity() == this) { damage = 0.0F; }
        if let Some(src) = source {
            let self_id = self.mob_entity.living_entity.entity.entity_id;
            if src.get_entity().entity_id == self_id {
                return false;
            }
        }
        true
    }

    fn modify_incoming_damage(&self, amount: f32, damage_type: DamageType) -> f32 {
        // Vanilla `Witch.getDamageAfterMagicAbsorb`.
        let is_witch_resistant = damage_type.has_tag(&tag::DamageType::MINECRAFT_WITCH_RESISTANT_TO);
        crate::entity::combat::calculate_witch_damage_received(amount, false, is_witch_resistant)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let living = &self.mob_entity.living_entity;
        let world = entity.world.load();

        if self.is_drinking_potion() {
            let remaining = self.using_time.fetch_sub(1, Ordering::Relaxed) - 1;
            if remaining <= 0 {
                self.set_drinking_potion(false);
                living.update_attribute(&Attributes::MOVEMENT_SPEED, |inst| {
                    inst.remove_modifier("minecraft:drinking");
                });
                let mut equipment = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let stack = equipment.get(&EquipmentSlot::MAIN_HAND);
                equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::EMPTY.clone());
                drop(equipment);
                living.send_equipment_changes(&[(
                    EquipmentSlot::MAIN_HAND,
                    ItemStack::EMPTY.clone(),
                )]);

                let effects = crate::item::potion::PotionContents::read_potion_effects(&stack);
                crate::item::potion::PotionContents::apply_effects_to(
                    living,
                    effects,
                    1.0,
                    crate::item::potion::PotionApplicationSource::Normal,
                );
            }
        } else {
            let mut potion: Option<&'static Potion> = None;
            let r: f32 = rand::random();

            let eye_in_water = entity.is_submerged_in_water();
            let is_on_fire = entity.fire_ticks.load(Ordering::Relaxed) > 0;

            if crate::entity::combat::should_witch_drink_water_breathing(
                r,
                eye_in_water,
                living.has_effect(&StatusEffect::WATER_BREATHING),
            ) {
                potion = Some(&Potion::WATER_BREATHING);
            } else if crate::entity::combat::should_witch_drink_fire_resistance(
                r,
                is_on_fire,
                false,
                living.has_effect(&StatusEffect::FIRE_RESISTANCE),
            ) {
                potion = Some(&Potion::FIRE_RESISTANCE);
            } else if crate::entity::combat::should_witch_drink_healing(
                r,
                living.health.load(),
                living.get_max_health(),
            ) {
                potion = Some(&Potion::HEALING);
            } else if let Some(target) = self.mob_entity.get_target() {
                let target_pos = target.get_entity().pos.load();
                let self_pos = entity.pos.load();
                if crate::entity::combat::should_witch_drink_speed(
                    r,
                    true,
                    self_pos.squared_distance_to_vec(&target_pos),
                    living.has_effect(&StatusEffect::SPEED),
                ) {
                    potion = Some(&Potion::SWIFTNESS);
                }
            }

            if let Some(potion) = potion {
                let stack = create_potion_stack(&Item::POTION, potion);
                let mut equipment = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equipment.put(&EquipmentSlot::MAIN_HAND, stack.clone());
                drop(equipment);
                living.send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, stack)]);

                self.using_time.store(32, Ordering::Relaxed);
                self.set_drinking_potion(true);
                living.update_attribute(&Attributes::MOVEMENT_SPEED, |inst| {
                    inst.add_or_replace_modifier(Modifier {
                        id: "minecraft:drinking".to_string(),
                        amount: -0.25,
                        operation: ModifierOperation::Add,
                        permanent: false,
                    });
                });

                if !entity.silent.load(Ordering::Relaxed) {
                    let pos = entity.pos.load();
                    let rand_pitch: f32 = rand::random();
                    let pitch = crate::entity::combat::calculate_witch_drink_pitch(rand_pitch);
                    world.play_sound_fine(
                        Sound::EntityWitchDrink,
                        SoundCategory::Hostile,
                        &pos,
                        1.0,
                        pitch,
                    );
                }
            } else if rand::random_range(0..100) == 0 && !entity.silent.load(Ordering::Relaxed) {
                world.play_sound_fine(
                    Sound::EntityWitchAmbient,
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::WITCH_XP_REWARD
    }
}

impl RangedAttackMob for WitchEntity {
    fn perform_ranged_attack(&self, target: &Arc<dyn EntityBase>, _power: f32) {
        self.throw_potion(target);
    }
}

impl PatrollingMonster for WitchEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }

    fn can_be_leader(&self) -> bool {
        false
    }
}

impl Raider for WitchEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityWitchCelebrate
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_witch_constants_and_xp_parity() {
        assert_eq!(crate::entity::combat::WITCH_MAX_HEALTH, 26.0);
        assert_eq!(crate::entity::combat::WITCH_MOVEMENT_SPEED, 0.25);
        assert_eq!(crate::entity::combat::WITCH_XP_REWARD, 5);
    }
}
