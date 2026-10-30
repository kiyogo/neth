use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;

use crate::entity::custom_sound::CustomSound;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::mob::{abstract_piglin, piglin_brute_ai};
use crate::entity::{
    Entity, EntityBase,
    mob::{Mob, MobEntity},
};
use crate::world::World;

pub struct PiglinBruteEntity {
    pub mob_entity: MobEntity,
    pub immune_to_zombification: AtomicBool,
    pub time_in_overworld: AtomicI32,
}

impl PiglinBruteEntity {
    pub const CONVERSION_TIME: i32 = 300;
    pub const XP_REWARD: u32 = 20;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let piglin = Self {
            mob_entity,
            immune_to_zombification: AtomicBool::new(false),
            time_in_overworld: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(piglin);

        // Vanilla PiglinBrute: MAX_HEALTH = 50.0, MOVEMENT_SPEED = 0.35, ATTACK_DAMAGE = 7.0, FOLLOW_RANGE = 12.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 50.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.35;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 7.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(follow) = attributes.get_mut(&Attributes::FOLLOW_RANGE.id) {
                follow.base_value = 12.0;
                follow.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(50.0);

        mob_arc.mob_entity.init_brain(mob_arc.as_ref());
        mob_arc
            .mob_entity
            .with_brain(mob_arc.as_ref(), piglin_brute_ai::init_memories);
        mob_arc
    }

    #[must_use]
    pub fn is_immune_to_zombification(&self) -> bool {
        self.immune_to_zombification.load(Ordering::Relaxed)
    }

    pub fn set_immune_to_zombification(&self, immune: bool) {
        self.immune_to_zombification
            .store(immune, Ordering::Relaxed);
        self.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::piglin_brute::DATA_IMMUNE_TO_ZOMBIFICATION,
            immune,
        );
    }

    #[must_use]
    pub fn is_converting(&self, world: &World) -> bool {
        !self.is_immune_to_zombification()
            && !self.mob_entity.is_no_ai()
            && world.dimension.piglins_zombify
    }

    pub fn play_angry_sound(&self) {
        self.play_sound(Sound::EntityPiglinBruteAngry);
    }

    fn play_sound(&self, sound: Sound) {
        let entity = &self.mob_entity.living_entity.entity;
        entity
            .world
            .load()
            .play_sound(sound, SoundCategory::Hostile, &entity.pos.load());
    }

    pub fn check_piglin_brute_spawn_rules(world: &World, pos: &BlockPos) -> bool {
        let below = BlockPos::new(pos.0.x, pos.0.y - 1, pos.0.z);
        let state = world.get_block_state(&below);
        state.id != Block::NETHER_WART_BLOCK.default_state.id
    }

    fn convert_to_zombified(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let zombified = crate::entity::r#type::from_type(
            &EntityType::ZOMBIFIED_PIGLIN,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let zombified_base = zombified.get_entity();
        zombified_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        zombified_base.head_yaw.store(entity.head_yaw.load());
        zombified_base.velocity.store(entity.velocity.load());

        if let Some(living) = zombified.get_living_entity() {
            living.set_health(self.mob_entity.living_entity.health.load());
            living.add_effect(crate::entity::combat::create_piglin_conversion_effect());
        }

        if let Some(custom_name) = &**entity.custom_name.load() {
            zombified_base.set_custom_name(custom_name.clone());
        }

        {
            let src_equip = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = zombified.get_living_entity() {
                let mut dst_equip = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (slot, item) in &src_equip.equipment {
                    dst_equip.put(slot, item.clone());
                }
            }
        }

        world.spawn_entity(zombified);
        entity.remove();
    }
}

impl CustomSound for PiglinBruteEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPiglinBruteDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPiglinBruteHurt)
    }
}

impl abstract_piglin::AbstractPiglin for PiglinBruteEntity {
    fn is_adult(&self) -> bool {
        true
    }

    fn can_hunt(&self) -> bool {
        true
    }

    fn is_converting(&self, world: &World) -> bool {
        Self::is_converting(self, world)
    }

    fn is_immune_to_zombification(&self) -> bool {
        Self::is_immune_to_zombification(self)
    }

    fn arm_pose(&self) -> abstract_piglin::PiglinArmPose {
        let holding_melee_weapon = self
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::MAIN_HAND)
            .get_data_component::<pumpkin_data::data_component_impl::ToolImpl>()
            .is_some();
        if self.mob_entity.is_attacking() && holding_melee_weapon {
            abstract_piglin::PiglinArmPose::AttackingWithMeleeWeapon
        } else {
            abstract_piglin::PiglinArmPose::Default
        }
    }

    fn play_converted_sound(&self) {
        self.play_sound(Sound::EntityPiglinBruteConvertedToZombified);
    }

    fn finish_conversion(&self) {
        self.convert_to_zombified();
    }
}

impl Mob for PiglinBruteEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_piglin_brute(&self) -> Option<&PiglinBruteEntity> {
        Some(self)
    }

    fn as_abstract_piglin(&self) -> Option<&dyn abstract_piglin::AbstractPiglin> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        _difficulty: &RegionalDifficulty,
    ) {
        let living = &self.mob_entity.living_entity;
        let mut equipment = living
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, &Item::GOLDEN_AXE));
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        if self.is_immune_to_zombification() {
            entity.set_synced_data(
                tracked_data::piglin_brute::DATA_IMMUNE_TO_ZOMBIFICATION,
                true,
            );
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if self.is_immune_to_zombification() {
            nbt.put_bool("IsImmuneToZombification", true);
        }
        let time_in_overworld = self.time_in_overworld.load(Ordering::Relaxed);
        if time_in_overworld > 0 {
            nbt.put_int("TimeInOverworld", time_in_overworld);
        }
        nbt.put_bool("CanPickUpLoot", true);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(immune) = nbt.get_bool("IsImmuneToZombification") {
            self.set_immune_to_zombification(immune);
        }
        if let Some(time) = nbt.get_int("TimeInOverworld") {
            self.time_in_overworld.store(time, Ordering::Relaxed);
        }
    }

    fn make_brain(
        &self,
        packed: &crate::entity::ai::brain::memory::PackedMemories,
    ) -> crate::entity::ai::brain::Brain {
        piglin_brute_ai::PIGLIN_BRUTE_PROVIDER.make_brain(self, packed)
    }

    fn after_brain_tick(&self, tick: &mut crate::entity::ai::brain::BrainTick<'_>) {
        piglin_brute_ai::update_activity(tick);
        piglin_brute_ai::maybe_play_activity_sound(tick);
    }

    fn custom_server_ai_step(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !crate::entity::ai::brain::behavior::utils::is_alive(self) {
            // No AI while dying, but still drain the inbox
            self.mob_entity.apply_brain_inbox(self);
            return;
        }
        self.mob_entity.tick_brain(self);
        let world = entity.world.load();
        abstract_piglin::tick_conversion(self, &world, &self.time_in_overworld);
    }

    fn on_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        source: Option<&dyn EntityBase>,
    ) {
        let Some(attacker) = source else {
            return;
        };
        let world = self.mob_entity.living_entity.entity.world.load_full();
        let Some(attacker) = world
            .get_entity_by_id(attacker.get_entity().entity_id)
            .filter(|attacker| attacker.get_living_entity().is_some())
        else {
            return;
        };
        // Queued, see PiglinEntity::on_damage
        self.mob_entity.post_to_brain(Box::new(move |tick| {
            piglin_brute_ai::was_hurt_by(tick, &attacker);
        }));
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piglin_brute_attributes_and_conversion_parity() {
        assert_eq!(PiglinBruteEntity::CONVERSION_TIME, 300);
        assert_eq!(PiglinBruteEntity::XP_REWARD, 20);

        // Constants verification
        assert_eq!(crate::entity::combat::PIGLIN_BRUTE_MAX_HEALTH, 50.0);
        assert_eq!(crate::entity::combat::PIGLIN_BRUTE_MOVEMENT_SPEED, 0.35);
        assert_eq!(crate::entity::combat::PIGLIN_BRUTE_ATTACK_DAMAGE, 7.0);
        assert_eq!(crate::entity::combat::PIGLIN_BRUTE_FOLLOW_RANGE, 12.0);
        assert_eq!(crate::entity::combat::PIGLIN_BRUTE_XP_REWARD, 20);

        let effect = crate::entity::combat::create_piglin_conversion_effect();
        assert_eq!(effect.effect_type, &pumpkin_data::effect::StatusEffect::NAUSEA);
        assert_eq!(effect.duration, 200);
    }
}
