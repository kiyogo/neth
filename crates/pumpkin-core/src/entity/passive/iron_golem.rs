use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::{EntityStatus, EntityType, MobCategory};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::GameMode;

use crate::entity::{
    Entity, EntityBase,
    ai::behavior::neutral::apply_targets,
    ai::goal::{
        active_target::ActiveTargetGoal, defend_village::DefendVillageTargetGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, move_towards_target::MoveTowardsTargetGoal,
        offer_flower::OfferFlowerGoal, revenge::RevengeGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{
        Mob, MobEntity,
        neutral::{NeutralData, NeutralMob},
    },
    player::Player,
};

/// Represents an Iron Golem, a powerful neutral mob that protects villagers and players.
///
/// Wiki: <https://minecraft.wiki/w/Iron_Golem>
pub struct IronGolemEntity {
    pub mob_entity: MobEntity,
    pub neutral_data: NeutralData,
    pub player_created: AtomicBool,
    pub attack_animation_tick: AtomicI32,
    pub offer_flower_tick: AtomicI32,
    pub crackiness: AtomicCell<u8>,
}

impl IronGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let iron_golem = Self {
            mob_entity,
            neutral_data: NeutralData::default(),
            player_created: AtomicBool::new(false),
            attack_animation_tick: AtomicI32::new(0),
            offer_flower_tick: AtomicI32::new(0),
            crackiness: AtomicCell::new(0),
        };
        let mob_arc = Arc::new(iron_golem);
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
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(1, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(2, Box::new(MoveTowardsTargetGoal::new(0.9, 32.0)));
            goal_selector.add_goal(4, Box::new(WaterAvoidingRandomStrollGoal::new(0.6)));
            goal_selector.add_goal(5, Box::new(OfferFlowerGoal::new()));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(DefendVillageTargetGoal::new()));
            target_selector.add_goal(2, Box::new(RevengeGoal::new(true)));
            apply_targets(&mut target_selector, &mob_arc.mob_entity, 3, 4, false);
            // Mirrors NearestAttackableTargetGoal<Mob>: any monster but creepers.
            target_selector.add_goal(
                3,
                ActiveTargetGoal::predicated(&mob_arc.mob_entity, 5, false, |living, _world| {
                    let entity_type = living.entity.entity_type;
                    entity_type.category == &MobCategory::MONSTER
                        && entity_type != &EntityType::CREEPER
                }),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_player_created(&self) -> bool {
        self.player_created.load(Ordering::Relaxed)
    }

    pub fn set_player_created(&self, value: bool) {
        self.player_created.store(value, Ordering::Relaxed);
        let entity = self.get_entity();
        let flag: u8 = u8::from(value);
        entity.set_synced_data(pumpkin_data::tracked_data::iron_golem::FLAGS_ID, flag);
    }

    pub fn offer_flower(&self, offer: bool) {
        let entity = self.get_entity();
        let world = entity.world.load();
        if offer {
            self.offer_flower_tick.store(400, Ordering::Relaxed);
            world.send_entity_status(entity, EntityStatus::OfferFlower, None);
        } else {
            self.offer_flower_tick.store(0, Ordering::Relaxed);
            world.send_entity_status(entity, EntityStatus::StopOfferFlower, None);
        }
    }

    #[must_use]
    pub fn get_offer_flower_tick(&self) -> i32 {
        self.offer_flower_tick.load(Ordering::Relaxed)
    }

    /// Trigger the attack animation for the iron golem.
    /// This sends `EntityStatus::StartAttacking` to clients and sets the `attack_animation_tick` counter.
    pub fn trigger_attack_animation(&self) {
        self.attack_animation_tick.store(10, Ordering::Relaxed);
        let entity = self.get_entity();
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
    }

    #[must_use]
    pub fn get_crackiness(&self) -> u8 {
        self.crackiness.load()
    }
}

impl NeutralMob for IronGolemEntity {
    fn get_neutral_data(&self) -> &NeutralData {
        &self.neutral_data
    }
}

impl Mob for IronGolemEntity {
    fn can_attack(&self, target: &dyn EntityBase) -> bool {
        let target_type = target.get_entity().entity_type;
        if self.is_player_created() && target_type == &EntityType::PLAYER {
            return false;
        }
        // Vanilla's `super.canAttack` goes through `Mob` crepper and ghast
        target_type != &EntityType::CREEPER
            && target_type != &EntityType::GHAST
            && self.get_mob_entity().living_entity.can_attack(target)
    }

    fn as_neutral(&self) -> Option<&dyn NeutralMob> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("PlayerCreated", self.is_player_created());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(created) = nbt.get_bool("PlayerCreated") {
            self.set_player_created(created);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_iron_golem(&self) -> Option<&crate::entity::passive::iron_golem::IronGolemEntity> {
        Some(self)
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let attack_tick = self.attack_animation_tick.load(Ordering::Relaxed);
        if attack_tick > 0 {
            self.attack_animation_tick.fetch_sub(1, Ordering::Relaxed);
        }

        let flower_tick = self.offer_flower_tick.load(Ordering::Relaxed);
        if flower_tick > 0 {
            self.offer_flower_tick.fetch_sub(1, Ordering::Relaxed);
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let flag: u8 = u8::from(self.is_player_created());
        entity.set_synced_data(pumpkin_data::tracked_data::iron_golem::FLAGS_ID, flag);
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.item.id == Item::IRON_INGOT.id {
            let living = &self.mob_entity.living_entity;
            let current_health = living.health.load();
            let max_health = living.get_max_health();
            if current_health < max_health {
                let new_health = (current_health + 25.0).min(max_health);
                living.set_health(new_health);
                self.crackiness.store(crate::entity::combat::calculate_iron_golem_crackiness(
                    new_health,
                    max_health,
                ));
                let entity = self.get_entity();
                let world = entity.world.load();
                let pos = entity.pos.load();
                let pitch = crate::entity::combat::calculate_iron_golem_repair_pitch(
                    rand::random::<f32>(),
                    rand::random::<f32>(),
                );
                world.play_sound_fine(
                    Sound::EntityIronGolemRepair,
                    SoundCategory::Neutral,
                    &pos,
                    1.0,
                    pitch,
                );
                if player.gamemode.load() != GameMode::Creative {
                    item_stack.item_count = item_stack.item_count.saturating_sub(1);
                }
                return true;
            }
        }
        false
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        let kb_res = target
            .get_living_entity()
            .map_or(0.0, |l| l.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE));
        let current_vel = target.get_entity().velocity.load();
        let new_y = crate::entity::combat::calculate_iron_golem_fling_velocity_y(current_vel.y, kb_res);
        let mut vel = current_vel;
        vel.y = new_y;
        target.get_entity().velocity.store(vel);

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound(Sound::EntityIronGolemAttack, SoundCategory::Neutral, &pos);
    }

    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        let living = &self.mob_entity.living_entity;
        let new_crackiness = crate::entity::combat::calculate_iron_golem_crackiness(
            living.health.load(),
            living.get_max_health(),
        );
        let old_crackiness = self.crackiness.swap(new_crackiness);
        if new_crackiness != old_crackiness {
            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.play_sound(Sound::EntityIronGolemDamage, SoundCategory::Neutral, &pos);
        }
    }
}

impl CustomSound for IronGolemEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityIronGolemDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityIronGolemHurt)
    }
}
