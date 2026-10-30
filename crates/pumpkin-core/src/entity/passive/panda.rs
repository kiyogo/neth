use crate::entity::combat;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::BAMBOO];

pub const FLAG_SNEEZE: u8 = 2;
pub const FLAG_ROLL: u8 = 4;
pub const FLAG_SIT: u8 = 8;
pub const FLAG_ON_BACK: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PandaGene {
    #[default]
    Normal = 0,
    Lazy = 1,
    Worried = 2,
    Playful = 3,
    Brown = 4,
    Weak = 5,
    Aggressive = 6,
}

impl PandaGene {
    #[must_use]
    pub const fn from_id(id: u8) -> Self {
        match id {
            1 => Self::Lazy,
            2 => Self::Worried,
            3 => Self::Playful,
            4 => Self::Brown,
            5 => Self::Weak,
            6 => Self::Aggressive,
            _ => Self::Normal,
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Lazy => "lazy",
            Self::Worried => "worried",
            Self::Playful => "playful",
            Self::Brown => "brown",
            Self::Weak => "weak",
            Self::Aggressive => "aggressive",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "lazy" => Self::Lazy,
            "worried" => Self::Worried,
            "playful" => Self::Playful,
            "brown" => Self::Brown,
            "weak" => Self::Weak,
            "aggressive" => Self::Aggressive,
            _ => Self::Normal,
        }
    }

    #[must_use]
    pub fn random_gene() -> Self {
        let rand_16 = rand::random_range(0..16);
        Self::from_id(crate::entity::combat::calculate_panda_random_gene(rand_16))
    }

    #[must_use]
    pub const fn is_recessive(self) -> bool {
        matches!(self, Self::Brown | Self::Weak)
    }

    #[must_use]
    pub fn get_variant_from_genes(main: Self, hidden: Self) -> Self {
        if main.is_recessive() {
            if main == hidden {
                main
            } else {
                Self::Normal
            }
        } else {
            main
        }
    }
}

pub struct PandaEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub main_gene: AtomicU8,
    pub hidden_gene: AtomicU8,
    pub flags: AtomicU8,
    pub eat_counter: AtomicI32,
    pub sneeze_counter: AtomicI32,
    pub unhappy_counter: AtomicI32,
}

impl PandaEntity {
    pub const XP_REWARD: u32 = combat::PANDA_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let gene = PandaGene::random_gene();
        let hidden = PandaGene::random_gene();
        let panda = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            main_gene: AtomicU8::new(gene.id()),
            hidden_gene: AtomicU8::new(hidden.id()),
            flags: AtomicU8::new(0),
            eat_counter: AtomicI32::new(0),
            sneeze_counter: AtomicI32::new(0),
            unhappy_counter: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(panda);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new_environmental(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        // Vanilla Panda: MAX_HEALTH = 20.0 (weak: 10.0), MOVEMENT_SPEED = 0.15 (lazy: 0.07), ATTACK_DAMAGE = 6.0
        mob_arc.set_attributes();
        let max_hp = if mob_arc.is_weak() {
            combat::PANDA_WEAK_MAX_HEALTH as f32
        } else {
            combat::PANDA_MAX_HEALTH as f32
        };
        mob_arc.mob_entity.living_entity.set_health(max_hp);

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> PandaGene {
        PandaGene::get_variant_from_genes(self.get_main_gene(), self.get_hidden_gene())
    }

    #[must_use]
    pub fn is_lazy(&self) -> bool {
        self.get_variant() == PandaGene::Lazy
    }

    #[must_use]
    pub fn is_worried(&self) -> bool {
        self.get_variant() == PandaGene::Worried
    }

    #[must_use]
    pub fn is_playful(&self) -> bool {
        self.get_variant() == PandaGene::Playful
    }

    #[must_use]
    pub fn is_brown(&self) -> bool {
        self.get_variant() == PandaGene::Brown
    }

    #[must_use]
    pub fn is_weak(&self) -> bool {
        self.get_variant() == PandaGene::Weak
    }

    #[must_use]
    pub fn is_aggressive(&self) -> bool {
        self.get_variant() == PandaGene::Aggressive
    }

    /// Vanilla `Panda.setAttributes`: weak pandas have 10 max health, lazy pandas have 0.07 speed.
    pub fn set_attributes(&self) {
        let max_health = if self.is_weak() {
            combat::PANDA_WEAK_MAX_HEALTH
        } else {
            combat::PANDA_MAX_HEALTH
        };
        self.mob_entity.living_entity.set_attribute_base(&Attributes::MAX_HEALTH, max_health);

        let speed = if self.is_lazy() {
            combat::PANDA_LAZY_MOVEMENT_SPEED
        } else {
            combat::PANDA_BASE_MOVEMENT_SPEED
        };
        self.mob_entity.living_entity.set_attribute_base(&Attributes::MOVEMENT_SPEED, speed);

        self.mob_entity.living_entity.set_attribute_base(&Attributes::ATTACK_DAMAGE, combat::PANDA_BASE_ATTACK_DAMAGE);
    }

    #[must_use]
    pub fn get_main_gene(&self) -> PandaGene {
        PandaGene::from_id(self.main_gene.load(Ordering::Relaxed))
    }

    pub fn set_main_gene(&self, gene: PandaGene) {
        self.main_gene.store(gene.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::MAIN_GENE_ID,
            gene.id() as i8,
        );
        self.set_attributes();
        if self.is_weak() && self.mob_entity.living_entity.health.load() > 10.0 {
            self.mob_entity.living_entity.set_health(10.0);
        }
    }

    #[must_use]
    pub fn get_hidden_gene(&self) -> PandaGene {
        PandaGene::from_id(self.hidden_gene.load(Ordering::Relaxed))
    }

    pub fn set_hidden_gene(&self, gene: PandaGene) {
        self.hidden_gene.store(gene.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::HIDDEN_GENE_ID,
            gene.id() as i8,
        );
        self.set_attributes();
        if self.is_weak() && self.mob_entity.living_entity.health.load() > 10.0 {
            self.mob_entity.living_entity.set_health(10.0);
        }
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_sneezing(&self) -> bool {
        self.has_flag(FLAG_SNEEZE)
    }

    pub fn set_sneezing(&self, val: bool) {
        self.set_flag(FLAG_SNEEZE, val);
    }

    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.has_flag(FLAG_SIT)
    }

    pub fn set_sitting(&self, val: bool) {
        self.set_flag(FLAG_SIT, val);
    }

    #[must_use]
    pub fn is_on_back(&self) -> bool {
        self.has_flag(FLAG_ON_BACK)
    }

    pub fn set_on_back(&self, val: bool) {
        self.set_flag(FLAG_ON_BACK, val);
    }

    #[must_use]
    pub fn is_rolling(&self) -> bool {
        self.has_flag(FLAG_ROLL)
    }

    pub fn set_gene_from_parents(&self, parent1: &Self, parent2: Option<&Self>) {
        let mut rng = rand::rng();
        let rand_swap = rng.random_bool(0.5);
        let rand_p1 = rng.random_bool(0.5);
        let rand_p2 = rng.random_bool(0.5);
        let rand_mut_main = rng.random_range(0..32);
        let rand_mut_hidden = rng.random_range(0..32);
        let mut_main = PandaGene::random_gene().id();
        let mut_hidden = PandaGene::random_gene().id();

        let (p2_main, p2_hidden) = if let Some(p2) = parent2 {
            (p2.get_main_gene().id(), p2.get_hidden_gene().id())
        } else {
            let rand_gene = PandaGene::random_gene().id();
            (rand_gene, rand_gene)
        };

        let (new_main, new_hidden) = crate::entity::combat::select_panda_baby_genes(
            parent1.get_main_gene().id(),
            parent1.get_hidden_gene().id(),
            p2_main,
            p2_hidden,
            rand_swap,
            rand_p1,
            rand_p2,
            rand_mut_main,
            rand_mut_hidden,
            mut_main,
            mut_hidden,
        );

        self.set_main_gene(PandaGene::from_id(new_main));
        self.set_hidden_gene(PandaGene::from_id(new_hidden));
    }
}

impl AgeableMob for PandaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for PandaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_PANDA_FOOD)
            || item_stack.item == &Item::BAMBOO
            || item_stack.item == &Item::CAKE
    }
}

impl Mob for PandaEntity {
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

    fn as_panda(&self) -> Option<&PandaEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_string("MainGene", self.get_main_gene().as_str().to_string());
        nbt.put_string("HiddenGene", self.get_hidden_gene().as_str().to_string());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(main) = nbt.get_string("MainGene") {
            self.set_main_gene(PandaGene::from_name(main));
        }
        if let Some(hidden) = nbt.get_string("HiddenGene") {
            self.set_hidden_gene(PandaGene::from_name(hidden));
        }
        self.set_attributes();
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

        // Vanilla Panda.java:446-455: Baby panda sneeze and slime ball drop
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            if !self.is_sneezing() {
                let rand_val = rand::random_range(0..6000);
                if crate::entity::combat::should_panda_sneeze(true, self.is_weak(), rand_val) {
                    self.set_sneezing(true);
                }
            } else {
                let cnt = self.sneeze_counter.fetch_add(1, Ordering::Relaxed) + 1;
                entity.set_synced_data(
                    pumpkin_data::tracked_data::panda::SNEEZE_COUNTER,
                    VarInt(cnt),
                );
                if cnt == 1 {
                    entity.world.load().play_sound_fine(
                        Sound::EntityPandaPreSneeze,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                        1.0,
                        1.0,
                    );
                } else if cnt > 20 {
                    self.set_sneezing(false);
                    self.sneeze_counter.store(0, Ordering::Relaxed);
                    entity.set_synced_data(
                        pumpkin_data::tracked_data::panda::SNEEZE_COUNTER,
                        VarInt(0),
                    );

                    let pos = entity.pos.load();
                    let world = entity.world.load();
                    world.play_sound_fine(
                        Sound::EntityPandaSneeze,
                        SoundCategory::Neutral,
                        &pos,
                        1.0,
                        1.0,
                    );

                    // Vanilla Panda.java: afterSneeze scares nearby adult pandas (jump from ground)
                    let bb = entity.bounding_box.load().expand(10.0, 10.0, 10.0);
                    let nearby = world.get_entities_at_box(&bb);
                    for other in nearby {
                        if other.get_entity().entity_type == &EntityType::PANDA
                            && other.get_entity().age.load(Ordering::Relaxed) >= 0
                            && other.get_entity().on_ground.load(Ordering::Relaxed)
                        {
                            let vel = other.get_entity().velocity.load();
                            other.get_entity().velocity.store(Vector3::new(vel.x, 0.42, vel.z));
                        }
                    }

                    // Vanilla panda_sneeze.json: 1/700 chance to drop a Slime Ball
                    if crate::entity::combat::should_panda_drop_slime_ball(rand::random_range(0..700)) {
                        let delta_x = rand::random_range(-0.1..0.1);
                        let delta_y = rand::random_range(0.2..0.4);
                        let delta_z = rand::random_range(-0.1..0.1);
                        let drop_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
                        drop_entity
                            .velocity
                            .store(Vector3::new(delta_x, delta_y, delta_z));
                        world.spawn_entity(Arc::new(ItemEntity::new(
                            drop_entity,
                            ItemStack::new(1, &Item::SLIME_BALL),
                        )));
                    }
                }
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if self.is_aggressive() {
                Sound::EntityPandaAggressiveAmbient
            } else if self.is_worried() {
                Sound::EntityPandaWorriedAmbient
            } else {
                Sound::EntityPandaAmbient
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
            entity.set_synced_data(pumpkin_data::tracked_data::panda::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::MAIN_GENE_ID,
            self.main_gene.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::HIDDEN_GENE_ID,
            self.hidden_gene.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::UNHAPPY_COUNTER,
            VarInt(self.unhappy_counter.load(Ordering::Relaxed)),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::SNEEZE_COUNTER,
            VarInt(self.sneeze_counter.load(Ordering::Relaxed)),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::EAT_COUNTER,
            VarInt(self.eat_counter.load(Ordering::Relaxed)),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityPandaAmbient)
    }
}

impl CustomSound for PandaEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPandaDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPandaHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panda_parity_constants() {
        assert_eq!(PandaEntity::XP_REWARD, 1);
        assert_eq!(combat::PANDA_MAX_HEALTH, 20.0);
        assert_eq!(combat::PANDA_WEAK_MAX_HEALTH, 10.0);
        assert_eq!(combat::PANDA_BASE_MOVEMENT_SPEED, 0.15);
        assert_eq!(combat::PANDA_LAZY_MOVEMENT_SPEED, 0.07);
        assert_eq!(combat::PANDA_BASE_ATTACK_DAMAGE, 6.0);
        assert_eq!(combat::PANDA_TOTAL_ROLL_STEPS, 32);
        assert_eq!(combat::PANDA_TOTAL_UNHAPPY_TIME, 32);
        assert_eq!(combat::PANDA_EAT_TICK_INTERVAL, 5);
        assert_eq!(combat::PANDA_BREED_TARGET_RANGE, 8.0);
        assert_eq!(combat::PANDA_FLAG_SNEEZE, 2);
        assert_eq!(combat::PANDA_FLAG_ROLL, 4);
        assert_eq!(combat::PANDA_FLAG_SIT, 8);
        assert_eq!(combat::PANDA_FLAG_ON_BACK, 16);
        assert!(!combat::is_mob_leashable(&EntityType::PANDA));
    }
}
