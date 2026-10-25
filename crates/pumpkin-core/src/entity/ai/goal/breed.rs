use std::sync::Arc;

use uuid::Uuid;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityStatus;
use rand::RngExt;

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::passive::tamable::TamableAnimal;
use crate::entity::{EntityBase, ai::pathfinder::NavigatorGoal, combat, mob::Mob, r#type::from_type};

use super::{Controls, Goal};

pub struct BreedGoal {
    speed: f64,
    mate: Option<Arc<dyn EntityBase>>,
    timer: i32,
}

impl BreedGoal {
    #[must_use]
    pub fn new(speed: f64) -> Box<Self> {
        Box::new(Self {
            speed,
            mate: None,
            timer: 0,
        })
    }

    fn find_mate(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let mob_entity = mob.get_mob_entity();
        if !mob_entity.is_in_love() {
            return None;
        }

        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let world = entity.world.load();
        let my_type = entity.entity_type;
        let my_uuid = entity.entity_uuid;

        let nearby = world.get_nearby_entities(pos, 8.0);
        let mut closest: Option<(f64, Arc<dyn EntityBase>)> = None;

        for candidate in nearby.values() {
            let c_entity = candidate.get_entity();
            if c_entity.entity_uuid == my_uuid {
                continue;
            }
            let c_type = c_entity.entity_type;
            let can_breed = c_type == my_type || combat::can_equine_mate(my_type, c_type);
            if !can_breed {
                continue;
            }
            if !candidate.is_in_love() || !candidate.is_breeding_ready() || candidate.is_panicking()
            {
                continue;
            }

            let dist = pos.squared_distance_to_vec(&c_entity.pos.load());
            match &closest {
                Some((best_dist, _)) if dist >= *best_dist => {}
                _ => closest = Some((dist, candidate.clone())),
            }
        }

        closest.map(|(_, e)| e)
    }

    fn breed(mob: &dyn Mob, mate: &dyn EntityBase) {
        let mob_entity = mob.get_mob_entity();
        let entity = mob.get_entity();
        let world = entity.world.load();

        let player_opt = mob_entity
            .breeder
            .load()
            .and_then(|uuid| world.get_player_by_uuid(uuid));
        if let Some(player) = player_opt {
            let entity_type_name = entity.entity_type.resource_name;
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::AnimalsBred as i32,
                1,
            );

            player.trigger_advancement_criterion(
                pumpkin_data::advancement::Advancement::HUSBANDRY_BREED_AN_ANIMAL,
                "bred",
            );
            player.trigger_advancement_criterion(
                pumpkin_data::advancement::Advancement::HUSBANDRY_BRED_ALL_ANIMALS,
                &format!("minecraft:{entity_type_name}"),
            );
        }

        mob_entity.reset_love_ticks();
        mob_entity
            .breeding_cooldown
            .store(6000, std::sync::atomic::Ordering::Relaxed);

        mate.reset_love();
        mate.set_breeding_cooldown(6000);

        let parent_pos = entity.pos.load();
        let baby_type: &'static pumpkin_data::entity::EntityType =
            if combat::can_equine_mate(entity.entity_type, mate.get_entity().entity_type) {
                combat::determine_equine_baby_type(entity.entity_type, mate.get_entity().entity_type)
            } else {
                entity.entity_type
            };
        let baby = from_type(baby_type, parent_pos, &world, Uuid::new_v4());
        baby.get_entity().set_age(-24000);

        // Vanilla Axolotl.java:346-361: inherit variant from parents or 1/1200 rare blue mutation
        if let Some(axolotl_a) = mob.as_axolotl()
            && let Some(axolotl_b) = mate.cast_any().downcast_ref::<crate::entity::passive::axolotl::AxolotlEntity>()
            && let Some(baby_axolotl) = baby.cast_any().downcast_ref::<crate::entity::passive::axolotl::AxolotlEntity>()
        {
            let rand_1200 = rand::random_range(0..1200);
            let rand_bool = rand::random();
            let variant = combat::select_axolotl_baby_variant(
                axolotl_a.get_variant(),
                axolotl_b.get_variant(),
                rand_1200,
                rand_bool,
            );
            baby_axolotl.set_variant(variant);
        }

        // Vanilla Goat.java:156-158: 2% screaming goat chance or inherited from parent
        if let Some(goat_a) = mob.as_goat()
            && let Some(goat_b) = mate.cast_any().downcast_ref::<crate::entity::passive::goat::GoatEntity>()
            && let Some(baby_goat) = baby.cast_any().downcast_ref::<crate::entity::passive::goat::GoatEntity>()
        {
            let is_screaming = combat::should_spawn_screaming_goat_from_parents(
                goat_a.is_screaming(),
                goat_b.is_screaming(),
                rand::random(),
                rand::random_range(0.0..1.0),
            );
            baby_goat.set_screaming(is_screaming);
        }

        // Vanilla Panda.java:231-255: inherit Panda genes from parents
        if let Some(panda_a) = mob.as_panda()
            && let Some(panda_b) = mate.cast_any().downcast_ref::<crate::entity::passive::panda::PandaEntity>()
            && let Some(baby_panda) = baby.cast_any().downcast_ref::<crate::entity::passive::panda::PandaEntity>()
        {
            baby_panda.set_gene_from_parents(panda_a, Some(panda_b));
        }

        // Vanilla Cat.java:235-245: inherit variant and collar color from parents
        if let Some(cat_a) = mob.as_cat()
            && let Some(cat_b) = mate.cast_any().downcast_ref::<crate::entity::passive::cat::CatEntity>()
            && let Some(baby_cat) = baby.cast_any().downcast_ref::<crate::entity::passive::cat::CatEntity>()
        {
            let rand_variant = rand::random();
            let rand_collar = rand::random();
            let (variant, collar) = combat::select_cat_baby_traits(
                cat_a.variant.load(std::sync::atomic::Ordering::Relaxed),
                cat_b.variant.load(std::sync::atomic::Ordering::Relaxed),
                cat_a.get_collar_color(),
                cat_b.get_collar_color(),
                rand_variant,
                rand_collar,
            );
            baby_cat.set_variant(variant);
            if cat_a.is_tame() {
                baby_cat.set_tame(true, cat_a.get_owner());
                baby_cat.set_collar_color(collar);
            }
        }

        // Vanilla Wolf.java:271-285: inherit variant and collar color from parents, tame to owner
        if let Some(wolf_a) = mob.as_wolf()
            && let Some(wolf_b) = mate.cast_any().downcast_ref::<crate::entity::passive::wolf::WolfEntity>()
            && let Some(baby_wolf) = baby.cast_any().downcast_ref::<crate::entity::passive::wolf::WolfEntity>()
        {
            let rand_variant = rand::random();
            let rand_collar = rand::random();
            let (variant_id, collar) = combat::select_wolf_baby_traits(
                wolf_a.variant.load(std::sync::atomic::Ordering::Relaxed),
                wolf_b.variant.load(std::sync::atomic::Ordering::Relaxed),
                wolf_a.get_collar_color(),
                wolf_b.get_collar_color(),
                rand_variant,
                rand_collar,
            );
            if let Some(variant) = pumpkin_data::wolf_variant::WolfVariant::all().get(variant_id as usize).copied() {
                baby_wolf.set_variant(variant);
            }
            if wolf_a.is_tame() {
                if let Some(owner) = wolf_a.get_owner() {
                    TamableAnimal::tame(baby_wolf, owner);
                    baby_wolf.set_collar_color(collar);
                }
            }
        }

        // Vanilla Fox.java:231-236: inherit variant from either parent (50/50)
        if let Some(fox_a) = mob.as_fox()
            && let Some(fox_b) = mate.cast_any().downcast_ref::<crate::entity::passive::fox::FoxEntity>()
            && let Some(baby_fox) = baby.cast_any().downcast_ref::<crate::entity::passive::fox::FoxEntity>()
        {
            let rand_bool = rand::random();
            let variant_id = combat::select_fox_baby_variant(
                fox_a.get_variant().id(),
                fox_b.get_variant().id(),
                rand_bool,
            );
            baby_fox.set_variant(crate::entity::passive::fox::FoxVariant::from_id(variant_id));
        }

        // Vanilla Horse.java: getBreedOffspring - inherit variant and markings from parents
        if let Some(horse_a) = mob.as_horse()
            && let Some(horse_b) = mate.cast_any().downcast_ref::<crate::entity::passive::horse::HorseEntity>()
            && let Some(baby_horse) = baby.cast_any().downcast_ref::<crate::entity::passive::horse::HorseEntity>()
        {
            let rand_skin = rand::random_range(0..9);
            let rand_markings = rand::random_range(0..5);
            let rand_variant = rand::random();
            let rand_marking = rand::random();
            let variant = combat::select_horse_baby_variant_and_markings(
                horse_a.get_variant(),
                horse_b.get_variant(),
                rand_skin,
                rand_markings,
                rand_variant,
                rand_marking,
            );
            baby_horse.set_variant(variant);
        }

        // Vanilla Llama.java: getBreedOffspring & setOffspringAttributes - inherit variant (50/50) and strength (max + 3% mutation)
        if let Some(llama_a) = mob.as_llama()
            && let Some(llama_b) = mate.cast_any().downcast_ref::<crate::entity::passive::llama::LlamaEntity>()
            && let Some(baby_llama) = baby.cast_any().downcast_ref::<crate::entity::passive::llama::LlamaEntity>()
        {
            let rand_bool = rand::random();
            let variant_id = combat::select_llama_baby_variant(
                llama_a.get_variant().id(),
                llama_b.get_variant().id(),
                rand_bool,
            );
            baby_llama.set_variant(crate::entity::passive::llama::LlamaVariant::from_id(variant_id));

            let rand_int = rand::random();
            let rand_float = rand::random();
            let strength = combat::calculate_llama_baby_strength(
                llama_a.get_strength(),
                llama_b.get_strength(),
                rand_int,
                rand_float,
            );
            baby_llama.set_strength(strength);
        }

        // Vanilla AbstractHorse.java: setOffspringAttributes - inherit health, jump, speed
        if combat::can_equine_mate(entity.entity_type, mate.get_entity().entity_type) {
            let parent_a_living = &mob.get_mob_entity().living_entity;
            let parent_b_living = mate.get_mob().map(|m| &m.get_mob_entity().living_entity);
            let baby_living = baby.get_mob().map(|m| &m.get_mob_entity().living_entity);

            if let (Some(parent_b), Some(baby_l)) = (parent_b_living, baby_living) {
                // MAX_HEALTH: 15.0 .. 30.0
                let health_a = parent_a_living.get_attribute_base(&Attributes::MAX_HEALTH);
                let health_b = parent_b.get_attribute_base(&Attributes::MAX_HEALTH);
                let baby_health = combat::calculate_offspring_attribute(
                    health_a,
                    health_b,
                    15.0,
                    30.0,
                    rand::random(),
                    rand::random(),
                    rand::random(),
                );
                baby_l.set_max_health(baby_health as f32);
                baby_l.set_health(baby_health as f32);

                // JUMP_STRENGTH: 0.4 .. 1.0
                let jump_a = parent_a_living.get_attribute_base(&Attributes::JUMP_STRENGTH);
                let jump_b = parent_b.get_attribute_base(&Attributes::JUMP_STRENGTH);
                let baby_jump = combat::calculate_offspring_attribute(
                    jump_a,
                    jump_b,
                    0.4,
                    1.0,
                    rand::random(),
                    rand::random(),
                    rand::random(),
                );
                let mut attr = baby_l.attributes.write().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(j) = attr.get_mut(&Attributes::JUMP_STRENGTH.id) {
                    j.base_value = baby_jump;
                    j.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                }

                // MOVEMENT_SPEED: 0.1125 .. 0.3375
                let speed_a = parent_a_living.get_attribute_base(&Attributes::MOVEMENT_SPEED);
                let speed_b = parent_b.get_attribute_base(&Attributes::MOVEMENT_SPEED);
                let baby_speed = combat::calculate_offspring_attribute(
                    speed_a,
                    speed_b,
                    0.1125,
                    0.3375,
                    rand::random(),
                    rand::random(),
                    rand::random(),
                );
                if let Some(s) = attr.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                    s.base_value = baby_speed;
                    s.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        let world_full = entity.world.load_full();
        world_full.spawn_entity(baby);

        world_full.send_entity_status(entity, EntityStatus::InLoveHearts, None);
        // TODO: gate on the `animalBreedingDropsXp` game rule once it exists.
        ExperienceOrbEntity::spawn(&world_full, parent_pos, mob.get_random().random_range(1..8));
    }
}

impl Goal for BreedGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        if !mob_entity.is_breeding_ready() || !mob_entity.is_in_love() {
            return false;
        }

        self.mate = Self::find_mate(mob);
        self.mate.is_some()
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(mate) = &self.mate else {
            return false;
        };

        if !mate.get_entity().is_alive() || mate.is_panicking() {
            return false;
        }

        mate.is_in_love() && self.timer < 60
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.timer = 0;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.mate = None;
        self.timer = 0;
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(mate) = &self.mate else {
            return;
        };

        let mob_entity = mob.get_mob_entity();
        let mate_pos = mate.get_entity().pos.load();

        {
            let mut look_control = mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            look_control.look_at_entity(mob, mate);
        };

        let my_pos = mob.get_entity().pos.load();
        let dist_sq = my_pos.squared_distance_to_vec(&mate_pos);

        {
            let mut navigator = mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_progress(NavigatorGoal::new(my_pos, mate_pos, self.speed));
        };

        self.timer += 1;

        if self.timer >= self.get_tick_count(60) && dist_sq < 9.0 {
            Self::breed(mob, mate.as_ref());
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
