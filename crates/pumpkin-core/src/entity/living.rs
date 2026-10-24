use crate::entity::velocity;
use pumpkin_data::item::Item;
use pumpkin_data::particle::Particle;
use pumpkin_data::potion::Effect;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data;
use pumpkin_inventory::build_equipment_slots;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_protocol::bedrock::client::take_item_actor::CTakeItemActor;
use pumpkin_protocol::bedrock::server::actor_event::{ActorEventID, SActorEvent};
use pumpkin_protocol::codec::var_ulong::VarULong;
use pumpkin_util::Difficulty;
use pumpkin_util::GameMode;
use pumpkin_util::Hand;
use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicU8,
    Ordering::{Relaxed, SeqCst},
};
use tracing::warn;

use super::breath::BreathManager;
use super::experience_orb::ExperienceOrbEntity;
use super::{Entity, EntityBase, NBTStorageInit};
use crate::block::OnLandedUponArgs;
use crate::entity::NBTStorage;
use crate::entity::ageable::AgeableMob;
use crate::entity::attributes::AttributeInstance;
use crate::entity::attributes::Modifier;
use crate::entity::attributes::ModifierOperation;
use crate::entity::combat::{
    self, CombatRules, CombatTracker, FallLocation, WitherRosePlacementAction, can_cause_mob_anger,
    can_place_wither_rose, determine_wither_rose_action, has_vanishing_curse,
    knockback_after_resistance, should_entity_drop_experience,
};
use crate::entity::mob::equipment::DEFAULT_EQUIPMENT_DROP_CHANCE;
use crate::entity::player::statistics::{CustomStatistic, StatisticCategory};
use crate::server::Server;
use crate::world::loot::LootContextParameters;
use crossbeam::atomic::AtomicCell;
use pumpkin_data::AttributeModifierSlot;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::Operation;
use pumpkin_data::data_component_impl::food::{ConsumableImpl, ConsumeEffect};
use pumpkin_data::data_component_impl::{
    AttributeModifiersImpl, BlocksAttacksImpl, DamageResistantImpl, DeathProtectionImpl,
    EnchantmentsImpl, EquipmentSlot, EquippableImpl, FoodImpl, GliderImpl, UseRemainderImpl,
};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityPose, EntityStatus, EntityType};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::game_rules::{GameRule, GameRuleValue};
use pumpkin_data::item_stack::{DamageResult, ItemStack};
use pumpkin_data::sound::SoundCategory;
use pumpkin_data::{Block, Enchantment};
use pumpkin_data::{damage::DamageType, sound::Sound};
use pumpkin_inventory::entity_equipment::EntityEquipment;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{
    CEntityStatus, CHurtAnimation, CSetPlayerInventory, CTakeItemEntity, CUpdateMobEffect,
};
use pumpkin_protocol::{
    codec::item_stack_seralizer::ItemStackSerializer,
    java::client::play::{CSetEquipment, MetadataSerializer},
    ser::{NetworkWriteExt, WritingError},
};
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use rand::RngExt;
use std::sync::RwLock;

/// Represents a living entity within the game world.
///
/// This struct encapsulates the core properties and behaviors of living entities, including players, mobs, and other creatures.
pub struct LivingEntity {
    /// The underlying entity object, providing basic entity information and functionality.
    pub entity: Entity,
    /// Air supply, drowning and dry-out damage (`LivingEntity.baseTick`).
    pub breath: BreathManager,
    /// Tracks the remaining time until the entity can regenerate health.
    pub hurt_cooldown: AtomicI32,
    /// Stores the amount of damage the entity last received.
    pub last_damage_taken: AtomicCell<f32>,
    /// The current health level of the entity.
    pub health: AtomicCell<f32>,
    /// The current absorption (yellow hearts) on the entity.
    pub absorption: AtomicCell<f32>,
    pub item_use_time: AtomicI32,
    pub auto_spin_attack_ticks: AtomicI32,
    pub fall_fly_ticks: AtomicI32,
    pub item_in_use: std::sync::Mutex<Option<ItemStack>>,
    pub active_hand: std::sync::Mutex<Option<Hand>>,
    pub recent_kinetic_enemies: std::sync::Mutex<FxHashMap<i32, i32>>,
    pub death_time: AtomicU8,
    /// Indicates whether the entity is dead. (`on_death` called)
    pub dead: AtomicBool,
    /// Plugin switch. False: neither pushed nor pushing.
    pub collides: AtomicBool,
    /// The distance the entity has been falling.
    pub fall_distance: AtomicCell<f32>,
    pub active_effects: std::sync::Mutex<FxHashMap<&'static StatusEffect, Effect>>,
    pub entity_equipment: Arc<std::sync::Mutex<EntityEquipment>>,
    pub equipment_drop_chances: Arc<std::sync::Mutex<FxHashMap<EquipmentSlot, f32>>>,
    pub movement_input: AtomicCell<Vector3<f64>>,
    pub equipment_slots: Arc<FxHashMap<usize, EquipmentSlot>>,

    pub jumping: AtomicBool,

    pub jumping_cooldown: AtomicU8,

    pub climbing: AtomicBool,

    /// The position where the entity was last climbing, used for death messages
    pub climbing_pos: AtomicCell<Option<BlockPos>>,

    /// The entity ID of the entity that last attacked this living entity.
    pub last_attacker_id: AtomicI32,
    /// The tick at which this entity was last attacked (entity age).
    pub last_attacked_time: AtomicI32,
    last_damage_type: std::sync::Mutex<Option<DamageType>>,
    last_damage_stamp: std::sync::atomic::AtomicI64,

    /// The entity ID of the entity this living entity last attacked.
    pub last_attacking_id: AtomicI32,
    /// The tick at which this entity last attacked something (entity age).
    pub last_attack_time: AtomicI32,

    /// Tracks combat entries, assisted falls, kill credit, and death messages.
    pub combat_tracker: std::sync::Mutex<CombatTracker>,

    /// The ID of the player that last hurt this entity.
    pub last_hurt_by_player_id: AtomicI32,
    /// The tick at which this entity was last hurt by a player.
    pub last_hurt_by_player_time: AtomicI64,
    /// The ID of the mob/entity that last hurt this entity.
    pub last_hurt_by_mob_id: AtomicI32,
    /// The tick at which this entity was last hurt by a mob/entity.
    pub last_hurt_by_mob_time: AtomicI64,

    /// Number of arrows stuck in this entity (`LivingEntity.java:2001-2007`).
    pub arrow_count: AtomicI32,
    /// Number of bee stingers stuck in this entity (`LivingEntity.java:2009-2015`).
    pub stinger_count: AtomicI32,
    /// Timer for removing stuck arrows (`LivingEntity.java:2757-2765`).
    pub remove_arrow_time: AtomicI32,
    /// Timer for removing stuck stingers (`LivingEntity.java:2769-2777`).
    pub remove_stinger_time: AtomicI32,

    water_movement_speed_multiplier: f32,
    livings_flags: AtomicU8,

    /// The last block position the entity occupied, used to trigger location changed effects.
    pub last_block_pos: AtomicCell<Option<BlockPos>>,

    /// The attributes of the entity
    pub attributes: RwLock<FxHashMap<u8, AttributeInstance>>,
    /// Modifier ids applied from the current item in each equipment slot.
    /// Used to remove them on unequip without the previous stack.
    equipment_attribute_modifier_ids: std::sync::Mutex<FxHashMap<EquipmentSlot, Vec<(u8, String)>>>,
    /// Modifier ids applied from the currently held weapon.
    weapon_attribute_modifier_ids: std::sync::Mutex<Vec<(u8, String)>>,
}

#[derive(Clone)]
struct EffectParticle {
    particle_id: VarInt,
    color: i32,
}

#[derive(Clone)]
struct EffectParticles(Vec<EffectParticle>);

impl MetadataSerializer for EffectParticles {
    fn write_metadata(
        &self,
        writer: &mut impl std::io::Write,
        _version: &pumpkin_util::version::JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let count = i32::try_from(self.0.len())
            .map_err(|_| WritingError::Message("Too many effect particles".into()))?;
        writer.write_var_int(&VarInt(count))?;
        for particle in &self.0 {
            writer.write_var_int(&particle.particle_id)?;
            writer.write_i32(particle.color)?;
        }
        Ok(())
    }
}

impl EffectParticle {
    const fn from_effect(effect: &Effect) -> Self {
        Self {
            particle_id: VarInt(Particle::EntityEffect as i32),
            color: (((if effect.ambient { 38 } else { 255 }) as u32) << 24
                | effect.effect_type.color as u32) as i32,
        }
    }
}

/// Vanilla `LivingEntity.canBeSeenAsEnemy`, with `Player`'s `abilities.invulnerable` override.
#[must_use]
pub fn can_be_seen_as_enemy(target: &dyn EntityBase) -> bool {
    target.get_player().map_or_else(
        || {
            target
                .get_living_entity()
                .is_some_and(LivingEntity::can_take_damage)
        },
        crate::entity::player::Player::can_be_seen_as_enemy,
    )
}

impl LivingEntity {
    const USING_ITEM_FLAG: u8 = 1;
    const OFF_HAND_ACTIVE_FLAG: u8 = 2;
    const RANDOM_TELEPORT_ATTEMPTS: usize = 16;
    pub const USING_RIPTIDE_FLAG: u8 = 4;

    const PREVENT_AREA_FALL_DAMAGE_BLOCKS: [&'static Block; 4] = [
        &Block::COBWEB,
        &Block::LADDER,
        &Block::POWDER_SNOW,
        &Block::SLIME_BLOCK,
    ];

    fn hurt_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.hurt_sound.unwrap_or(Sound::EntityGenericHurt)
    }

    fn death_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.death_sound.unwrap_or(Sound::EntityGenericDeath)
    }

    pub(crate) fn sound_category(&self, is_player: bool) -> SoundCategory {
        if is_player {
            SoundCategory::Players
        } else if self.entity.entity_type.category == &pumpkin_data::entity::MobCategory::MONSTER {
            SoundCategory::Hostile
        } else {
            SoundCategory::Neutral
        }
    }

    pub(crate) fn get_pitch(&self) -> f32 {
        let is_baby = self
            .get_mob()
            .and_then(|x| x.as_ageable())
            .is_some_and(AgeableMob::is_baby);

        let mut rng = rand::rng();
        if is_baby {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.5
        } else {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.0
        }
    }

    /// New living entity with default attributes.
    pub fn new(entity: Entity) -> Self {
        let water_movement_speed_multiplier = if entity.entity_type == &EntityType::POLAR_BEAR {
            0.98
        } else if entity.entity_type == &EntityType::SKELETON_HORSE {
            0.96
        } else {
            0.8
        };
        let mut max_health: f32 = 20.0; // Overridden by attribute base below
        Self {
            // Populate local attribute instances from the default registry and get initial vars
            attributes: {
                let mut m = FxHashMap::default();

                for (attr, base) in entity.entity_type.attributes {
                    if attr.id == Attributes::MAX_HEALTH.id {
                        max_health = *base as f32;
                    }
                    m.insert(attr.id, AttributeInstance::new(*base));
                }
                std::sync::RwLock::new(m)
            },
            health: AtomicCell::new(max_health), // Initial health value from attributes
            entity,
            hurt_cooldown: AtomicI32::new(0),
            last_damage_taken: AtomicCell::new(0.0),
            absorption: AtomicCell::new(0.0),
            fall_distance: AtomicCell::new(0.0),
            death_time: AtomicU8::new(0),
            dead: AtomicBool::new(false),
            collides: AtomicBool::new(true),
            item_use_time: AtomicI32::new(0),
            auto_spin_attack_ticks: AtomicI32::new(0),
            fall_fly_ticks: AtomicI32::new(0),
            item_in_use: std::sync::Mutex::new(None),
            active_hand: std::sync::Mutex::new(None),
            recent_kinetic_enemies: std::sync::Mutex::new(FxHashMap::default()),
            livings_flags: AtomicU8::new(0),
            active_effects: std::sync::Mutex::new(FxHashMap::default()),
            entity_equipment: Arc::new(std::sync::Mutex::new(EntityEquipment::new())),
            equipment_drop_chances: Arc::new(std::sync::Mutex::new(FxHashMap::default())),
            equipment_slots: Arc::new(build_equipment_slots()),
            jumping: AtomicBool::new(false),
            jumping_cooldown: AtomicU8::new(0),
            climbing: AtomicBool::new(false),
            climbing_pos: AtomicCell::new(None),
            last_attacker_id: AtomicI32::new(0),
            last_attacked_time: AtomicI32::new(0),
            last_damage_type: std::sync::Mutex::new(None),
            last_damage_stamp: std::sync::atomic::AtomicI64::new(0),
            last_attacking_id: AtomicI32::new(0),
            last_attack_time: AtomicI32::new(0),
            combat_tracker: std::sync::Mutex::new(CombatTracker::new()),
            last_hurt_by_player_id: AtomicI32::new(0),
            last_hurt_by_player_time: AtomicI64::new(0),
            last_hurt_by_mob_id: AtomicI32::new(0),
            last_hurt_by_mob_time: AtomicI64::new(0),
            arrow_count: AtomicI32::new(0),
            stinger_count: AtomicI32::new(0),
            remove_arrow_time: AtomicI32::new(0),
            remove_stinger_time: AtomicI32::new(0),
            movement_input: AtomicCell::new(Vector3::default()),
            water_movement_speed_multiplier,
            last_block_pos: AtomicCell::new(None),
            equipment_attribute_modifier_ids: std::sync::Mutex::new(FxHashMap::default()),
            weapon_attribute_modifier_ids: std::sync::Mutex::new(Vec::new()),
            breath: BreathManager::default(),
        }
    }

    /// Returns the entity that should receive kill credit for this entity's death.
    /// Following vanilla Java logic (`LivingEntity.getKillCredit`):
    /// 1. Prioritize `last_hurt_by_player` if hurt within the last 100 ticks (5 seconds).
    /// 2. Then `last_hurt_by_mob` if hurt within the last 100 ticks.
    /// 3. Fall back to combat tracker's killer entry if available.
    pub fn get_kill_credit(&self) -> Option<Arc<dyn EntityBase>> {
        let world = self.entity.world.load();
        // Vanilla LivingEntity.java:1368 & 1445:
        // Use the entity's age tick count consistently with last_hurt_by_player_time
        let current_tick = self.entity.age.load(Relaxed) as i64;

        let player_id = self.last_hurt_by_player_id.load(Relaxed);
        let player_time = self.last_hurt_by_player_time.load(Relaxed);
        if crate::entity::combat::is_kill_credit_active(player_id, player_time, current_tick)
            && let Some(player) = world.get_entity_by_id(player_id)
        {
            return Some(player);
        }

        let mob_id = self.last_hurt_by_mob_id.load(Relaxed);
        let mob_time = self.last_hurt_by_mob_time.load(Relaxed);
        if crate::entity::combat::is_kill_credit_active(mob_id, mob_time, current_tick)
            && let Some(mob) = world.get_entity_by_id(mob_id)
        {
            return Some(mob);
        }

        let tracker = self
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(killer) = tracker.get_killer_entry()
            && let Some(killer_id) = killer.attacker_id
        {
            return world.get_entity_by_id(killer_id);
        }

        None
    }

    /// Triggers location-based enchantment effects (e.g. Frost Walker) when the entity's block position changes.
    pub fn on_changed_block(&self, caller: &dyn EntityBase, _pos: BlockPos) {
        let pos_f64 = self.entity.pos.load();
        if let Some(player) = caller.get_player() {
            let boots = player.inventory.get_slot(36);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        } else {
            let boots = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&EquipmentSlot::FEET);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        }
    }

    /// Emits vanilla equipment side effects for an equipment slot change.
    pub fn on_equip_item(&self, slot: &EquipmentSlot, previous: &ItemStack, current: &ItemStack) {
        if previous.are_items_and_components_equal(current) {
            return;
        }

        let world = self.entity.world.load();
        let position = self.entity.pos.load();

        if let Some(equippable) = current.get_data_component::<EquippableImpl>()
            && *equippable.slot == *slot
        {
            world.play_sound_event(&equippable.equip_sound, SoundCategory::Neutral, &position);
        }

        if current.is_empty() {
            if !previous.is_empty() {
                world.emit_game_event(GameEvent::Unequip.name(), position);
            }
        } else {
            world.emit_game_event(GameEvent::Equip.name(), position);
        }
    }

    pub fn send_equipment_changes(&self, equipment: &[(EquipmentSlot, ItemStack)]) {
        if equipment.is_empty() {
            return;
        }
        self.apply_and_send_equipment_attribute_modifiers(equipment);

        if equipment
            .iter()
            .any(|(slot, _)| *slot == EquipmentSlot::FEET)
        {
            let pos_f64 = self.entity.pos.load();
            for (slot, stack) in equipment {
                if *slot == EquipmentSlot::FEET && !stack.is_empty() {
                    crate::enchantment::EnchantmentHelper::on_location_changed(
                        &self.entity,
                        stack,
                        pos_f64,
                    );
                }
            }
        }

        let equipment_java: Vec<(i8, ItemStackSerializer)> = equipment
            .iter()
            .map(|(slot, stack)| {
                (
                    slot.discriminant(),
                    ItemStackSerializer::from(stack.clone()),
                )
            })
            .collect();
        let je_packet = CSetEquipment::new(self.entity_id().into(), equipment_java);

        let mut sent_editioned = false;
        for (slot, stack) in equipment {
            if *slot == EquipmentSlot::MAIN_HAND && self.entity.entity_type != &EntityType::PLAYER {
                self.update_weapon_attributes(stack);
            }
            if *slot == EquipmentSlot::MAIN_HAND || *slot == EquipmentSlot::OFF_HAND {
                let window_id = if *slot == EquipmentSlot::OFF_HAND {
                    120
                } else {
                    0
                };

                let be_packet = pumpkin_protocol::bedrock::client::CMobEquipment {
                    target_runtime_id: (self.entity_id() as u64).into(),
                    item: pumpkin_protocol::bedrock::network_item::NetworkItemStackDescriptor::from(
                        stack,
                    ),
                    slot: 0,
                    selected_slot: 0,
                    container_id: window_id,
                };
                self.entity.world.load().send_to_tracking_players_editioned(
                    &self.entity,
                    &je_packet,
                    &be_packet,
                );
                sent_editioned = true;
            }
        }

        if !sent_editioned {
            self.entity
                .world
                .load()
                .send_to_tracking_players(&self.entity, &je_packet);
        }
    }

    /// Applies the held item's attack attribute modifiers to this entity's
    /// attribute map and sends the changed attributes to clients. Without this
    /// the client never sees the reduced attack speed and does not show the
    /// crosshair attack indicator.
    fn update_weapon_attributes(&self, stack: &ItemStack) {
        let component = stack.get_data_component::<AttributeModifiersImpl>();

        let mut changed: Vec<Attributes> = Vec::new();

        // Remove only the modifiers the previously held stack applied; replacing
        // the whole list would drop permanent modifiers from commands and effects.
        let previous = std::mem::take(
            &mut *self
                .weapon_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for (attribute_id, modifier_id) in previous {
            if let Some(attribute) = attributes_by_id(attribute_id) {
                self.update_attribute(attribute, |inst| inst.remove_modifier(&modifier_id));
                push_unique_attribute(&mut changed, attribute);
            }
        }

        let mut applied = Vec::new();
        for modifier in component
            .into_iter()
            .flat_map(|c| c.attribute_modifiers.iter())
        {
            let attribute = modifier.r#type;
            if attribute != &Attributes::ATTACK_SPEED && attribute != &Attributes::ATTACK_DAMAGE {
                continue;
            }
            let operation = match modifier.operation {
                Operation::AddValue => ModifierOperation::Add,
                Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
            };
            self.update_attribute(attribute, |inst| {
                inst.add_or_replace_modifier(Modifier {
                    id: modifier.id.to_string(),
                    amount: modifier.amount,
                    operation,
                    permanent: false,
                });
            });
            applied.push((attribute.id, modifier.id.to_string()));
            push_unique_attribute(&mut changed, attribute);
        }

        *self
            .weapon_attribute_modifier_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = applied;

        if !changed.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, changed);
        }
    }

    /// Applies item `attribute_modifiers` for the given slots and notifies clients.
    ///
    /// The local HUD armor bar is driven by `minecraft:armor` / `minecraft:armor_toughness`
    /// on `UPDATE_ATTRIBUTES`, not by `SET_EQUIPMENT`.
    pub fn apply_and_send_equipment_attribute_modifiers(
        &self,
        equipment: &[(EquipmentSlot, ItemStack)],
    ) {
        let mut touched = Vec::new();
        for (slot, stack) in equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
        if !touched.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, touched);
        }
    }

    /// Re-applies modifiers from every currently equipped stack without notifying clients.
    pub fn apply_current_equipment_attribute_modifiers(&self) {
        let equipment = self.snapshot_equipped_stacks();
        let mut touched = Vec::new();
        for (slot, stack) in &equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
    }

    /// Re-applies modifiers from every currently equipped stack and sends updates.
    pub fn send_current_equipment_attribute_modifiers(&self) {
        self.apply_and_send_equipment_attribute_modifiers(&self.snapshot_equipped_stacks());
    }

    fn snapshot_equipped_stacks(&self) -> Vec<(EquipmentSlot, ItemStack)> {
        let guard = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .equipment
            .iter()
            .map(|(slot, stack)| (slot.clone(), stack.clone()))
            .collect()
    }

    fn apply_equipment_slot_attribute_modifiers(
        &self,
        slot: &EquipmentSlot,
        stack: &ItemStack,
        touched: &mut Vec<Attributes>,
    ) {
        let previous = {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.remove(slot).unwrap_or_default()
        };
        for (attr_id, modifier_id) in previous {
            if let Some(attr) = attributes_by_id(attr_id) {
                self.update_attribute(attr, |inst| inst.remove_modifier(&modifier_id));
                push_unique_attribute(touched, attr);
            }
        }

        if stack.is_empty() {
            return;
        }
        let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>() else {
            return;
        };

        let mut applied = Vec::new();
        for item_mod in modifiers.attribute_modifiers.iter() {
            if !attribute_modifier_slot_matches(&item_mod.slot, slot) {
                continue;
            }
            let operation = match item_mod.operation {
                Operation::AddValue => ModifierOperation::Add,
                Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
            };
            self.update_attribute(item_mod.r#type, |inst| {
                inst.add_or_replace_modifier(Modifier {
                    id: item_mod.id.to_string(),
                    amount: item_mod.amount,
                    operation,
                    permanent: false,
                });
            });
            applied.push((item_mod.r#type.id, item_mod.id.to_string()));
            push_unique_attribute(touched, item_mod.r#type);
        }

        if !applied.is_empty() {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.insert(slot.clone(), applied);
        }
    }

    /// Fires the cancellable pickup event for picking up an Item entity or XP Orb. Callers
    /// must check this - and do so before mutating any inventory, item stack or stat, not
    /// just before the pickup animation - or a cancelled pickup still happens in every way
    /// that matters.
    pub fn can_pickup(&self, item: &Entity, stack_amount: u32) -> bool {
        let mut pickup_event =
            crate::plugin::api::events::entity::entity_pickup_item::EntityPickupItemEvent::new(
                self.entity.entity_id,
                item.entity_type.id.to_string(),
                stack_amount as u8,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut pickup_event);
            if pickup_event.cancelled {
                return false;
            }
        }
        true
    }

    /// Broadcasts the "item flies to the player" pickup animation. Call only once the pickup
    /// has actually happened: the event already allowed it (see [`Self::can_pickup`]) and the
    /// amount here is what was actually taken, not just offered.
    pub fn broadcast_pickup(&self, item: &Entity, stack_amount: u32) {
        let chunk_pos = self.entity.chunk_pos.load();
        self.entity.world.load().broadcast_to_chunk_editioned(
            chunk_pos,
            &CTakeItemEntity::new(
                item.entity_id.into(),
                self.entity.entity_id.into(),
                VarInt(stack_amount as i32),
            ),
            &CTakeItemActor {
                item_runtime_id: VarULong(item.entity_id as u64),
                actor_runtime_id: VarULong(self.entity.entity_id as u64),
            },
        );
    }

    /// Picks up an Item entity or XP Orb: fires the cancellable pickup event and, if it's
    /// allowed, broadcasts the pickup animation. Returns whether the pickup was allowed.
    /// Only correct for callers with nothing to mutate between the event check and the
    /// animation (e.g. an XP orb); a caller that inserts into an inventory first needs the
    /// amount actually taken, so it must call [`Self::can_pickup`] before mutating anything
    /// and [`Self::broadcast_pickup`] after, instead of this.
    pub fn pickup(&self, item: &Entity, stack_amount: u32) -> bool {
        if !self.can_pickup(item, stack_amount) {
            return false;
        }
        self.broadcast_pickup(item, stack_amount);
        true
    }

    /// Sends the Hand animation to all others, used when Eating for example
    pub fn set_active_hand(&self, hand: Hand, stack: ItemStack, duration: i32) {
        self.item_use_time.store(duration, Ordering::Relaxed);
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stack);
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(hand);
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.set_living_flag(Self::USING_ITEM_FLAG, true);
        self.set_living_flag(Self::OFF_HAND_ACTIVE_FLAG, hand == Hand::Left);
    }

    fn set_living_flag(&self, flag: u8, value: bool) {
        let index = flag;
        let mut b = self.livings_flags.load(Ordering::Relaxed);
        if value {
            b |= index;
        } else {
            b &= !index;
        }
        self.livings_flags.store(b, Ordering::Relaxed);

        let bedrock_meta = (flag == Self::USING_ITEM_FLAG).then(|| {
            let index =
                pumpkin_protocol::bedrock::client::set_actor_data::entity_data_flag::USING_ITEM;
            let mask = 1i64 << index;
            if value {
                self.entity.bedrock_flags.fetch_or(mask, Ordering::Relaxed);
            } else {
                self.entity
                    .bedrock_flags
                    .fetch_and(!mask, Ordering::Relaxed);
            }

            let mut meta =
                pumpkin_protocol::bedrock::client::set_actor_data::SyncedActorDataList::new();
            meta.set(
                pumpkin_protocol::bedrock::client::set_actor_data::entity_data_key::FLAGS,
                pumpkin_protocol::bedrock::client::set_actor_data::MetadataValue::Int64(
                    self.entity.bedrock_flags.load(Ordering::Relaxed),
                ),
            );
            meta
        });

        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_LIVING_ENTITY_FLAGS, b);
        if let Some(bedrock_meta) = &bedrock_meta {
            self.entity.send_bedrock_actor_data(bedrock_meta);
        }
    }

    #[must_use]
    pub fn is_auto_spin_attack(&self) -> bool {
        (self.livings_flags.load(Ordering::Relaxed) & Self::USING_RIPTIDE_FLAG) != 0
    }

    pub fn set_auto_spin_attack(&self, value: bool) {
        if !value {
            self.auto_spin_attack_ticks.store(0, Ordering::Relaxed);
        }
        self.set_living_flag(Self::USING_RIPTIDE_FLAG, value);
    }

    /// Starts auto spin attack (Riptide) state for the given duration,
    /// mirroring vanilla `Player.startAutoSpinAttack(activationTicks, dmg, itemStackUsed)`.
    pub fn start_auto_spin_attack(&self, activation_ticks: i32, _dmg: f32) {
        self.auto_spin_attack_ticks.store(activation_ticks, Ordering::Relaxed);
        self.set_living_flag(Self::USING_RIPTIDE_FLAG, true);
    }

    pub fn clear_active_hand(&self) {
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.item_use_time.store(0, Ordering::Relaxed);

        self.set_living_flag(Self::USING_ITEM_FLAG, false);
    }

    pub fn was_recently_stabbed(&self, target_id: i32, now: i32, allowed_ticks: i32) -> bool {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&target_id)
            .is_some_and(|stabbed_at| now - stabbed_at < allowed_ticks)
    }

    pub fn remember_stabbed_entity(&self, target_id: i32, now: i32) {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(target_id, now);
    }

    /// Returns the number of arrows stuck in this entity, mirroring vanilla `LivingEntity.getArrowCount`.
    #[must_use]
    pub fn get_arrow_count(&self) -> i32 {
        self.arrow_count.load(Ordering::Relaxed)
    }

    /// Sets the number of arrows stuck in this entity and syncs the tracked data,
    /// mirroring vanilla `LivingEntity.setArrowCount`.
    pub fn set_arrow_count(&self, count: i32) {
        let clamped = count.max(0);
        self.arrow_count.store(clamped, Ordering::Relaxed);
        self.entity.set_synced_data(
            tracked_data::living_entity::DATA_ARROW_COUNT_ID,
            VarInt(clamped),
        );
    }

    /// Returns the number of bee stingers stuck in this entity, mirroring vanilla `LivingEntity.getStingerCount`.
    #[must_use]
    pub fn get_stinger_count(&self) -> i32 {
        self.stinger_count.load(Ordering::Relaxed)
    }

    /// Sets the number of bee stingers stuck in this entity and syncs the tracked data,
    /// mirroring vanilla `LivingEntity.setStingerCount`.
    pub fn set_stinger_count(&self, count: i32) {
        let clamped = count.max(0);
        self.stinger_count.store(clamped, Ordering::Relaxed);
        self.entity.set_synced_data(
            tracked_data::living_entity::DATA_STINGER_COUNT_ID,
            VarInt(clamped),
        );
    }

    pub fn is_blocking(&self) -> bool {
        let item_in_use = self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(item) = item_in_use.as_ref()
            && item.get_data_component::<BlocksAttacksImpl>().is_some()
        {
            let dyn_self = self
                .entity
                .world
                .load()
                .get_entity_by_id(self.entity.entity_id);
            let is_bedrock = if let Some(player) = dyn_self
                .as_ref()
                .and_then(|e| e.cast_any().downcast_ref::<crate::entity::player::Player>())
            {
                if player.has_item_cooldown(item.item.registry_key)
                    || player.has_item_cooldown("minecraft:shield")
                {
                    return false;
                }
                matches!(
                    player.client.as_ref(),
                    crate::net::ClientPlatform::Bedrock(_)
                )
            } else {
                false
            };
            let required_time = if is_bedrock { 0 } else { 5 };
            let use_time = self.item_use_time.load(Ordering::Relaxed);
            return item.get_max_use_time() - use_time >= required_time;
        }
        false
    }

    /// Vanilla `LivingEntity.getVisibilityPercent`:
    /// Returns a factor in `0.0..=1.0` representing how visible this entity is to `tester`.
    /// Sneaking reduces visibility by 20% (0.8x).
    /// Invisibility reduces visibility based on armor coverage (0.7x * armor_cover, min 0.1).
    /// Wearing a matching mob head reduces visibility to that mob by 50% (0.5x).
    pub fn get_visibility_percent(&self, tester: Option<&dyn EntityBase>) -> f64 {
        let mut percent = 1.0;
        if self.entity.is_sneaking() {
            percent *= 0.8;
        }

        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if self.entity.invisible.load(Ordering::Relaxed) {
            let worn = [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ]
            .iter()
            .filter(|slot| !equipment.get(slot).is_empty())
            .count();
            let cover = (worn as f64 / 4.0).max(0.1);
            percent *= 0.7 * cover;
        }

        if let Some(tester_base) = tester {
            let head = equipment.get(&EquipmentSlot::HEAD);
            if !head.is_empty() {
                let tester_type = tester_base.get_entity().entity_type;
                if (tester_type == &EntityType::SKELETON && head.item.id == Item::SKELETON_SKULL.id)
                    || (tester_type == &EntityType::ZOMBIE && head.item.id == Item::ZOMBIE_HEAD.id)
                    || (tester_type == &EntityType::CREEPER
                        && head.item.id == Item::CREEPER_HEAD.id)
                    || ((tester_type == &EntityType::PIGLIN
                        || tester_type == &EntityType::PIGLIN_BRUTE)
                        && head.item.id == Item::PIGLIN_HEAD.id)
                {
                    percent *= 0.5;
                }
            }
        }

        percent
    }

    /// Heals the entity by `additional_health`, mirroring vanilla `LivingEntity.heal`.
    pub fn heal(&self, additional_health: f32) {
        if additional_health <= 0.0 {
            return;
        }
        let current_health = self.health.load();
        if current_health <= 0.0 {
            return;
        }
        let mut event =
            crate::plugin::api::events::entity::entity_regain_health::EntityRegainHealthEvent::new(
                self.entity.entity_id,
                additional_health,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        self.set_health(current_health + additional_health);
    }

    #[must_use]
    pub fn get_health(&self) -> f32 {
        self.health.load()
    }

    pub fn set_health(&self, health: f32) {
        // Clamp to [0, max_health]
        let max_health = self.get_max_health();
        let clamped = health.max(0.0).min(max_health);
        self.health.store(clamped);
        // tell everyone entities health changed
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, clamped);
    }

    /// Returns the current maximum health for this entity
    pub fn get_max_health(&self) -> f32 {
        self.get_attribute_value(&Attributes::MAX_HEALTH) as f32
    }

    /// Sets the maximum health for this entity
    pub fn set_max_health(&self, max_health: f32) {
        // Update base attribute
        self.set_attribute_base(&Attributes::MAX_HEALTH, max_health as f64);

        // Broadcast the attribute change
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_HEALTH],
        );

        // Clamp current health to new max if needed and send metadata update
        let current_health = self.health.load();
        if current_health > max_health {
            self.set_health(max_health);
        }
    }

    /// Returns the current absorption amount for this entity (yellow hearts)
    pub fn get_absorption(&self) -> f32 {
        self.absorption.load()
    }

    /// Sets the current absorption amount for this entity (yellow hearts)
    pub fn set_absorption(&self, new_abs: f32) {
        // Must be at least 0
        let new_abs = new_abs.max(0.0);

        // Set local state
        self.absorption.store(new_abs);

        // Broadcast attribute update for max_absorption so clients receive
        // the updated absorption value via the attribute packet.
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_ABSORPTION],
        );

        // Send absorption metadata for players (visual yellow hearts)
        if self.entity.entity_type == &EntityType::PLAYER {
            self.entity
                .set_synced_data(tracked_data::player::DATA_PLAYER_ABSORPTION_ID, new_abs);
        }
    }

    /// Convenience helper to mutate an attribute instance. Automatically inserts
    /// a new instance populated from the registry base if needed.
    pub fn update_attribute<F: FnOnce(&mut AttributeInstance)>(
        &self,
        attribute: &Attributes,
        f: F,
    ) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let inst = map.entry(attribute.id).or_insert_with(|| {
            let base = self
                .entity
                .entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map_or_else(
                    || {
                        tracing::debug!(
                            "Entity type {} has no base value for attribute {}; falling back to default {}",
                            self.entity.entity_type.resource_name,
                            attribute.name,
                            attribute.default_value,
                        );
                        attribute.default_value
                    },
                    |a| a.1,
                );
            AttributeInstance::new(base)
        });

        f(inst);
        inst.dirty.store(true, Ordering::Relaxed);
    }

    /// Returns the computed value for `attribute` using the local instance, falling back
    /// to `attribute.default_value` if no local instance exists.
    pub fn get_attribute_value(&self, attribute: &Attributes) -> f64 {
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(&attribute.id)
            .map_or(attribute.default_value, AttributeInstance::value)
    }

    /// Returns the base attribute value for `attribute` for this entity's type.
    pub fn get_attribute_base(&self, attribute: &Attributes) -> f64 {
        // Check the local base value first (could be modified)
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(instance) = map.get(&attribute.id) {
            return instance.base_value;
        }

        // Fall back to registry base value if no local instance exists
        self.entity
            .entity_type
            .attributes
            .iter()
            .find(|a| a.0.id == attribute.id)
            .map_or(attribute.default_value, |a| a.1)
    }

    /// Update or insert the base value for an attribute on this entity.
    /// If the attribute doesn't exist locally yet, it will be inserted.
    pub fn set_attribute_base(&self, attribute: &Attributes, new_base: f64) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(inst) = map.get_mut(&attribute.id) {
            inst.base_value = new_base;
            inst.dirty.store(true, Ordering::Relaxed);
        } else {
            let ai = AttributeInstance::new(new_base);
            ai.dirty.store(true, Ordering::Relaxed);
            map.insert(attribute.id, ai);
        }
    }

    pub fn reset_effects_and_attributes(&self) {
        // Clear active effects and reset modified attributes
        let effects_to_remove: Vec<_> = {
            let lock = self
                .active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.keys().copied().collect()
        };

        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }
    }

    pub const fn entity_id(&self) -> i32 {
        self.entity.entity_id
    }

    // canBeAffected, with the mob overrides folded in
    fn can_be_affected(&self, effect: &Effect) -> bool {
        let entity_type = self.entity.entity_type;
        if (entity_type == &EntityType::WITHER_SKELETON || entity_type == &EntityType::WITHER)
            && effect.effect_type == &StatusEffect::WITHER
        {
            return false;
        }

        if entity_type == &EntityType::PARCHED && effect.effect_type == &StatusEffect::WEAKNESS {
            return false;
        }

        // Vanilla Spider.java:126-128, AbstractNautilus.java:462-464: Spiders and Nautiluses are immune to Poison
        if crate::entity::combat::is_spider_or_nautilus_immune_to_poison(entity_type, effect.effect_type) {
            return false;
        }

        if entity_type.has_tag(&tag::EntityType::MINECRAFT_IMMUNE_TO_INFESTED) {
            effect.effect_type != &StatusEffect::INFESTED
        } else if entity_type.has_tag(&tag::EntityType::MINECRAFT_IMMUNE_TO_OOZING) {
            effect.effect_type != &StatusEffect::OOZING
        } else if entity_type.has_tag(&tag::EntityType::MINECRAFT_IGNORES_POISON_AND_REGEN) {
            effect.effect_type != &StatusEffect::REGENERATION
                && effect.effect_type != &StatusEffect::POISON
        } else {
            true
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn add_effect(&self, effect: Effect) {
        if !self.can_be_affected(&effect) {
            return;
        }

        let mut effect_event =
            crate::plugin::api::events::entity::entity_potion_effect::EntityPotionEffectEvent::new(
                self.entity.entity_id,
                effect.effect_type.translation_key.to_string(),
                effect.duration,
                effect.amplifier,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut effect_event);
        }
        if effect_event.cancelled {
            return;
        }

        // Apply instant effects immediately before storing
        if effect.effect_type == &StatusEffect::INSTANT_HEALTH {
            let heal_amount = 4.0 * (1 << effect.amplifier) as f32;
            self.heal(heal_amount);
            // Like vanilla, instant effects are never sent or stored as active effects.
            return;
        } else if effect.effect_type == &StatusEffect::INSTANT_DAMAGE {
            let damage_amount = 6.0 * (1 << effect.amplifier) as f32;
            let dyn_self = self
                .entity
                .world
                .load()
                .get_entity_by_id(self.entity.entity_id);
            if let Some(dyn_self) = dyn_self {
                let _ = dyn_self.damage(&*dyn_self, damage_amount, DamageType::MAGIC);
            }
            return;
        }

        // Apply non-instant effects

        // Effects that modify attributes (ex. speed) should also update the
        // entity's attribute instances (server-side) and then notify clients.
        if !effect.effect_type.attribute_modifiers.is_empty() {
            // Apply each attribute modifier into the local AttributeInstance
            for m in effect.effect_type.attribute_modifiers {
                let id = m.id.to_string();
                let op = match m.operation {
                    Operation::AddValue => ModifierOperation::Add,
                    Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                    Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
                };
                let scaled_amount = m.base_value * (f64::from(effect.amplifier) + 1.);
                let mod_inst = Modifier {
                    id,
                    amount: scaled_amount,
                    operation: op,
                    // Vanilla `MobEffect.addAttributeModifiers` adds these as permanent.
                    permanent: true,
                };

                self.update_attribute(m.attribute, |inst| {
                    inst.add_or_replace_modifier(mod_inst.clone());
                });
            }

            // Recompute packet modifiers from active effects for each affected attribute
            let mut touched_attrs: Vec<pumpkin_data::attributes::Attributes> = Vec::new();
            for m in effect.effect_type.attribute_modifiers {
                if !touched_attrs.iter().any(|a| a.id == m.attribute.id) {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // Apply absorption effect (+4 absorption per level)
        if effect.effect_type == &StatusEffect::ABSORPTION {
            let added = 4.0 * (effect.amplifier as f32 + 1.0);
            let max_abs = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
            let new_abs = (self.absorption.load() + added).min(max_abs);
            self.set_absorption(new_abs);
        }

        // Apply invisible effect
        if effect.effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(true);
        }

        // Apply glowing effect
        if effect.effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(true);
        }

        // Broadcast effect to nearby players
        self.broadcast_effect(&effect);
        if effect.effect_type != &StatusEffect::INSTANT_HEALTH
            && effect.effect_type != &StatusEffect::INSTANT_DAMAGE
        {
            self.active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(effect.effect_type, effect);
        }
        self.sync_effect_particles();
    }

    /// Sends an effect packet to every player tracking this entity, mirroring
    /// vanilla `LivingEntity.sendEffectToPassengers`.
    fn broadcast_effect(&self, effect: &Effect) {
        let mut flag: i8 = 0;
        if effect.ambient {
            flag |= 1;
        }
        if effect.show_particles {
            flag |= 2;
        }
        if effect.show_icon {
            flag |= 4;
        }
        if effect.blend {
            flag |= 8;
        }

        let je_packet = CUpdateMobEffect::new(
            self.entity.entity_id.into(),
            VarInt(i32::from(effect.effect_type.id)),
            effect.amplifier.into(),
            effect.duration.into(),
            flag,
        );

        let be_packet = pumpkin_protocol::bedrock::client::CMobEffect {
            target_runtime_id: VarULong(self.entity.entity_id as u64),
            event_id: pumpkin_protocol::bedrock::client::CMobEffect::EVENT_ADD,
            effect_id: VarInt(effect.effect_type.to_bedrock_id()),
            effect_amplifier: VarInt(i32::from(effect.amplifier)),
            show_particles: effect.show_particles,
            effect_duration_ticks: VarInt(effect.duration),
            tick: VarULong(0),
            ambient: effect.ambient,
        };

        let chunk_pos = self.entity.chunk_pos.load();
        self.entity
            .world
            .load()
            .broadcast_to_chunk_editioned(chunk_pos, &je_packet, &be_packet);
    }

    fn sync_effect_particles(&self) {
        let effects = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let has_effects = !effects.is_empty();
        let particles = EffectParticles(
            effects
                .values()
                .filter(|effect| effect.show_particles)
                .map(EffectParticle::from_effect)
                .collect(),
        );
        let ambient = effects
            .values()
            .filter(|effect| effect.show_particles)
            .all(|effect| effect.ambient);
        drop(effects);

        self.entity
            .set_synced_data(tracked_data::living_entity::EFFECT_PARTICLES, particles);
        if has_effects {
            self.entity
                .set_synced_data(tracked_data::living_entity::EFFECT_AMBIENCE_ID, ambient);
        }
    }

    pub fn remove_effect(&self, effect_type: &'static StatusEffect) -> bool {
        // Remove the effect
        let succeeded = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&effect_type)
            .is_some();

        // Broadcast effect removal
        self.entity
            .world
            .load()
            .send_remove_mob_effect(&self.entity, effect_type);

        // Remove attribute modifiers, if any
        if !effect_type.attribute_modifiers.is_empty() {
            let mut touched_attrs = Vec::new();

            for m in effect_type.attribute_modifiers {
                let id = m.id.to_string();

                // Clean local server state
                self.update_attribute(m.attribute, |inst| {
                    inst.remove_modifier(&id);
                });

                // Track unique attributes for the packet update
                if !touched_attrs
                    .iter()
                    .any(|a: &Attributes| a.id == m.attribute.id)
                {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            // Sync the clean state to the client
            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // If absorption effect removed, clear current absorption amount and notify clients
        if effect_type == &StatusEffect::ABSORPTION {
            self.set_absorption(0.0);
        }

        // If health boost effect removed, clamp current health to new max and notify clients
        if effect_type == &StatusEffect::HEALTH_BOOST {
            let new_max = self.get_max_health();
            if self.health.load() > new_max {
                // Update local health and send both health and absorption metadata together
                self.set_health(new_max.max(0.0));
            }
        }

        // If invisible effect removed, disable invisibility
        if effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(false);
        }

        // If glowing effect removed, disable glowing
        if effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(false);
        }

        if succeeded {
            self.sync_effect_particles();
        }

        succeeded
    }

    pub fn has_effect(&self, effect: &'static StatusEffect) -> bool {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&effect)
    }

    pub fn get_effect(&self, effect: &'static StatusEffect) -> Option<Effect> {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&effect)
            .cloned()
    }

    pub fn is_in_fall_damage_resetting(&self) -> (bool, &Block) {
        let block_pos = self.entity.block_pos.load();
        let block = self.entity.world.load().get_block(&block_pos);
        (
            block.has_tag(&tag::Block::MINECRAFT_FALL_DAMAGE_RESETTING),
            block,
        )
    }

    // Check if the entity is in water
    pub fn is_in_water(&self) -> bool {
        self.entity.touching_water.load(Ordering::Relaxed)
    }

    /// Mirrors vanilla's elytra-eligibility gate (`LivingEntity.tryToStartFallFlying`, minus the
    /// actual state transition): flight requires a non-broken item with the `glider` data
    /// component in the chest slot (not just an elytra by item id - a datapack can grant any
    /// item that component), and is blocked while underwater or under Levitation (which
    /// already provides its own upward motion). The caller is still responsible for checking
    /// that the entity is airborne.
    pub fn can_glide(&self) -> bool {
        if self.is_in_water() || self.has_effect(&StatusEffect::LEVITATION) {
            return false;
        }
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(&EquipmentSlot::CHEST)
            .is_some_and(|stack| {
                stack.get_data_component::<GliderImpl>().is_some()
                    && stack
                        .get_max_damage()
                        .is_none_or(|max| max.saturating_sub(stack.get_damage()) > 1)
            })
    }

    /// Whether a glider item (of any durability) is in the chest slot. An already-active glide
    /// continues until it's removed, even at 1 durability; `can_glide` additionally requires a
    /// non-broken one to *start* one.
    pub fn has_elytra_equipped(&self) -> bool {
        self.entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .equipment
            .get(&EquipmentSlot::CHEST)
            .is_some_and(|stack| stack.get_data_component::<GliderImpl>().is_some())
    }

    // Check if the entity is in powder snow
    pub fn is_in_powder_snow(&self) -> bool {
        self.entity.is_in_powder_snow.load(Ordering::Relaxed)
    }

    pub fn should_prevent_fall_damage(&self) -> bool {
        // Vanilla LivingEntity.java:3120-3122:
        // Slow falling and levitation effects reset fall distance and prevent fall damage.
        if self.has_effect(&StatusEffect::SLOW_FALLING)
            || self.has_effect(&StatusEffect::LEVITATION)
        {
            return true;
        }

        let (prevents, block) = self.is_in_fall_damage_resetting();

        if block == &Block::SCAFFOLDING && !self.entity.is_sneaking() {
            return false;
        }

        if block == &Block::WATER {
            return true;
        }

        if self.entity.entity_type == &EntityType::PLAYER {
            if block == &Block::END_GATEWAY || block == &Block::END_PORTAL {
                return true;
            }

            if block == &Block::NETHER_PORTAL {
                let world = self.entity.world.load();
                let level_info = world.level_info.load();

                return level_info.game_rules.players_nether_portal_default_delay == 0;
            }
        }

        prevents
    }

    pub fn should_prevent_fall_damage_in_area(&self) -> bool {
        let world = self.entity.world.load();
        let block_pos = self.entity.block_pos.load().down();
        let entity_pos = self.entity.pos.load();

        let min = BlockPos(Vector3::new(
            block_pos.0.x - 1,
            block_pos.0.y,
            block_pos.0.z - 1,
        ));
        let max = BlockPos(Vector3::new(
            block_pos.0.x + 1,
            block_pos.0.y,
            block_pos.0.z + 1,
        ));
        let pos_iter = BlockPos::iterate(min, max);

        // FIXME: it seems the java server checks all blocks around with a raycast and check if miss or hit,
        // then added to a collision checker to handle in the tick handler
        for pos in pos_iter {
            let block = world.get_block(&pos);

            if Self::PREVENT_AREA_FALL_DAMAGE_BLOCKS.contains(&block) {
                let block_center = Vector3::new(
                    f64::from(pos.0.x) + 0.5,
                    f64::from(pos.0.y) + 0.5,
                    f64::from(pos.0.z) + 0.5,
                );
                let distance = entity_pos.squared_distance_to_vec(&block_center);

                // Fetch safe fall distance from attribute
                let safe_distance = self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE);
                return distance.sqrt() <= safe_distance * safe_distance;
            }
        }

        false
    }

    pub fn is_immune_to_fall_damage(&self) -> bool {
        self.entity
            .entity_type
            .has_tag(&tag::EntityType::MINECRAFT_FALL_DAMAGE_IMMUNE)
    }

    fn get_effective_gravity(&self, caller: &dyn EntityBase) -> f64 {
        let final_gravity = caller.get_gravity();

        if self.entity.velocity.load().y <= 0.0 && self.has_effect(&StatusEffect::SLOW_FALLING) {
            final_gravity.min(0.01)
        } else {
            final_gravity
        }
    }

    pub fn swing_hand(&self) {
        let world = self.entity.world.load();
        let entity_id = self.entity_id();

        let je_packet =
            pumpkin_protocol::java::client::play::CSwingArm::new(entity_id.into(), false);
        let be_packet = pumpkin_protocol::bedrock::server::animate::SAnimate {
            action: pumpkin_protocol::bedrock::server::animate::AnimateAction::SwingArm,
            target_actor_runtime_id: pumpkin_protocol::codec::var_ulong::VarULong(entity_id as u64),
            data: 0.0,
            swing_source: None,
        };

        world.broadcast_editioned(&je_packet, &be_packet);
    }

    pub fn swing_off_hand(&self) {
        let world = self.entity.world.load();
        let entity_id = self.entity_id();

        let je_packet = pumpkin_protocol::java::client::play::CEntityAnimation::new(
            entity_id.into(),
            pumpkin_protocol::java::client::play::Animation::SwingOffhand,
        );
        let be_packet = pumpkin_protocol::bedrock::server::animate::SAnimate {
            action: pumpkin_protocol::bedrock::server::animate::AnimateAction::SwingArm,
            target_actor_runtime_id: pumpkin_protocol::codec::var_ulong::VarULong(entity_id as u64),
            data: 0.0,
            swing_source: None,
        };

        world.broadcast_editioned(&je_packet, &be_packet);
    }

    fn tick_movement(&self, caller: &dyn EntityBase) {
        if self.jumping_cooldown.load(Relaxed) != 0 {
            self.jumping_cooldown.fetch_sub(1, Relaxed);
        }

        let should_swim_in_fluids = caller.get_player().is_none_or(|player| !player.is_flying());

        // Mobs did this before their AI step.
        if caller.get_mob().is_none() {
            self.entity.check_zero_velo();
        }

        let mut movement_input = self.movement_input.load();

        movement_input.x *= 0.98;

        movement_input.z *= 0.98;

        self.movement_input.store(movement_input);

        // TODO: Tick AI

        if self.jumping.load(SeqCst) && should_swim_in_fluids {
            let in_lava = self.entity.touching_lava.load(SeqCst);

            let in_water = self.entity.touching_water.load(SeqCst);

            let fluid_height = if in_lava {
                self.entity.lava_height.load()
            } else {
                self.entity.water_height.load()
            };

            let swim_height = self.get_swim_height();

            let on_ground = self.entity.on_ground.load(SeqCst);

            if (in_water || in_lava) && (!on_ground || fluid_height > swim_height) {
                // Swim upward

                let mut velo = self.entity.velocity.load();

                velo.y += 0.04;

                self.entity.velocity.store(velo);
            } else if (on_ground || in_water && fluid_height <= swim_height)
                && self.jumping_cooldown.load(SeqCst) == 0
            {
                self.jump();

                self.jumping_cooldown.store(10, SeqCst);
            }
        } else {
            self.jumping_cooldown.store(0, SeqCst);
        }

        if self.has_effect(&StatusEffect::SLOW_FALLING)
            || self.has_effect(&StatusEffect::LEVITATION)
        {
            self.fall_distance.store(0.0);
        }

        let touching_water = self.entity.touching_water.load(SeqCst);

        // Vanilla Mob.isEffectiveAi: NoAI mobs don't travel, so they neither move nor fall.
        let effective_ai = caller
            .get_mob()
            .is_none_or(|mob| !mob.get_mob_entity().is_no_ai());

        // Strider is the only entity that has canWalkOnFluid = false

        if !effective_ai {
            // No travel.
        } else if let Some(mob) = caller.get_mob()
            && mob.custom_travel(caller)
        {
            // Handled by custom mob travel (e.g. Squid)
        } else if touching_water && caller.travel_in_water() {
            // The entity swam on its own.
        } else if (touching_water || self.entity.touching_lava.load(SeqCst))
            && should_swim_in_fluids
            && self.entity.entity_type != &EntityType::STRIDER
        {
            if self.entity.is_fall_flying() {
                self.entity.set_fall_flying(false);
            }
            self.travel_in_fluid(caller, touching_water);
        } else if self.entity.is_fall_flying() {
            self.travel_fall_flying(caller);
        } else {
            self.travel_in_air(caller);
        }

        let suffocating = self.entity.tick_block_collisions(caller);

        if suffocating
            && !caller.get_player().is_some_and(|p| p.is_sleeping())
            && self.entity.pose.load() != pumpkin_data::entity::EntityPose::Sleeping
        {
            caller.damage(caller, 1.0, DamageType::IN_WALL);
        }

        self.push_entities(caller);
    }

    fn push_entities(&self, dyn_self: &dyn EntityBase) {
        let world = self.entity.world.load();
        let pushable = velocity::pushable_entities(dyn_self, &self.entity.bounding_box.load());
        if pushable.is_empty() {
            return;
        }

        // Entity cramming check
        let max_cramming = match world.get_game_rule(&GameRule::MaxEntityCramming) {
            GameRuleValue::Int(value) => value,
            GameRuleValue::Bool(_) => 0,
        };
        if max_cramming > 0
            && pushable.len() as i64 > max_cramming - 1
            && rand::random::<u32>().is_multiple_of(4)
        {
            let count = pushable
                .iter()
                .filter(|entity| !entity.is_passenger())
                .count();
            if count as i64 > max_cramming - 1 {
                dyn_self.damage(dyn_self, 6.0, DamageType::CRAMMING);
            }
        }

        for entity in pushable {
            entity.push(dyn_self);
        }
    }

    /// Decays player velocity like vanilla `travelInAir` friction.
    fn apply_travel_friction(&self) {
        let mut velo = self.entity.velocity.load();
        if velo.x == 0.0 && velo.z == 0.0 {
            return;
        }

        let friction = if self.entity.on_ground.load(Relaxed) {
            f64::from(
                self.entity
                    .get_block_with_y_offset(0.500_001)
                    .1
                    .slipperiness,
            ) * 0.91
        } else {
            0.91
        };

        velo.x *= friction;

        velo.z *= friction;

        self.entity.velocity.store(velo);
    }

    fn travel_in_air(&self, caller: &dyn EntityBase) {
        // applyMovementInput

        let effective_speed = self.get_attribute_value(&Attributes::MOVEMENT_SPEED);

        let (speed, friction) = if self.entity.on_ground.load(Relaxed) {
            // getVelocityAffectingPos

            let slipperiness = f64::from(
                self.entity
                    .get_block_with_y_offset(0.500_001)
                    .1
                    .slipperiness,
            );

            let speed =
                effective_speed * 0.216_000_02 / (slipperiness * slipperiness * slipperiness);

            (speed, slipperiness * 0.91)
        } else {
            let speed = caller
                .get_player()
                .map_or(0.02, super::player::Player::get_off_ground_speed);

            (speed, 0.91)
        };

        self.entity
            .update_velocity_from_input(self.movement_input.load(), speed);

        self.apply_climbing_speed(caller);

        self.make_move(caller);

        let mut velo = self.entity.velocity.load();

        let can_powder_snow_climb = if self.entity.was_in_powder_snow.load(Relaxed) {
            crate::block::blocks::powder_snow::can_entity_walk_on_powder_snow(caller)
        } else {
            false
        };

        if (self.entity.horizontal_collision.load(SeqCst) || self.jumping.load(SeqCst))
            && (caller.on_climbable() || can_powder_snow_climb)
        {
            velo.y = 0.2;
        }

        let levitation = self.get_effect(&StatusEffect::LEVITATION);

        if let Some(lev) = levitation {
            velo.y += 0.05f64.mul_add(f64::from(lev.amplifier + 1), -velo.y) * 0.2;
        } else {
            velo.y -= self.get_effective_gravity(caller);

            // TODO: If world is not loaded: replace effective gravity with:

            // if below world's bottom y then -0.1, else 0.0
        }

        // If entity has no drag: store velo and return

        velo.x *= friction;

        velo.z *= friction;

        velo.y *= caller.get_y_velocity_drag().unwrap_or_else(|| {
            if caller.is_flutterer() {
                friction
            } else {
                0.98
            }
        });

        self.entity.velocity.store(velo);
    }

    fn travel_fall_flying(&self, caller: &dyn EntityBase) {
        if caller.on_climbable() || self.entity.on_ground.load(Relaxed) {
            self.travel_in_air(caller);
            self.entity.set_fall_flying(false);
            return;
        }

        let last_movement = self.entity.velocity.load();
        let last_speed =
            (last_movement.x * last_movement.x + last_movement.z * last_movement.z).sqrt();

        let new_movement = self.update_fall_flying_movement(last_movement, caller);
        self.entity.velocity.store(new_movement);
        self.make_move(caller);

        let current_movement = self.entity.velocity.load();
        let new_speed = (current_movement.x * current_movement.x
            + current_movement.z * current_movement.z)
            .sqrt();
        self.handle_fall_flying_collisions(caller, last_speed, new_speed);

        if self.entity.on_ground.load(Relaxed) {
            self.entity.set_fall_flying(false);
        }
    }

    fn update_fall_flying_movement(
        &self,
        movement: Vector3<f64>,
        caller: &dyn EntityBase,
    ) -> Vector3<f64> {
        let gravity = self.get_effective_gravity(caller);
        combat::calculate_fall_flying_movement_step(
            movement,
            self.entity.pitch.load(),
            self.entity.yaw.load(),
            gravity,
        )
    }

    pub(crate) fn handle_fall_flying_collisions(
        &self,
        caller: &dyn EntityBase,
        move_hor_length: f64,
        new_move_hor_length: f64,
    ) {
        if self.entity.horizontal_collision.load(SeqCst) {
            let diff = move_hor_length - new_move_hor_length;
            let dmg = combat::calculate_fly_into_wall_damage(diff);
            if dmg > 0.0 {
                let sound = combat::get_fly_into_wall_sound(dmg);
                self.entity.world.load().play_sound_fine(
                    sound,
                    SoundCategory::Players,
                    &self.entity.pos.load(),
                    1.0,
                    1.0,
                );
                caller.damage(caller, dmg, DamageType::FLY_INTO_WALL);
            }
        }
    }

    fn travel_in_fluid(&self, caller: &dyn EntityBase, water: bool) {
        let movement_input = self.movement_input.load();

        let falling = self.entity.velocity.load().y <= 0.0;
        let gravity = self.get_effective_gravity(caller);
        let effective_speed = self.get_attribute_value(&Attributes::MOVEMENT_SPEED);

        if water {
            if caller.get_player().is_some() && self.entity.is_swimming() {
                let pos = self.entity.pos.load();
                let head_pos = pumpkin_util::math::position::BlockPos::floored(pos.x, pos.y + 0.9, pos.z);
                let world = self.entity.world.load();
                let (fluid, _) = world.get_fluid_and_fluid_state(&head_pos);
                let head_submerged = fluid.id != pumpkin_data::fluid::Fluid::EMPTY.id;
                let look_angle_y = -f64::from(self.entity.pitch.load().to_radians().sin());
                let (impulse, should_apply) = crate::entity::combat::calculate_swimming_vertical_impulse(
                    look_angle_y,
                    self.entity.velocity.load().y,
                    self.jumping.load(SeqCst),
                    head_submerged,
                );
                if should_apply {
                    let mut velo = self.entity.velocity.load();
                    velo.y += impulse;
                    self.entity.velocity.store(velo);
                }
            }

            let mut friction = if self.entity.sprinting.load(Relaxed) {
                0.9
            } else {
                f64::from(self.water_movement_speed_multiplier)
            };

            let mut speed = 0.02;

            // Apply water movement efficiency attribute
            let mut water_movement_efficiency =
                self.get_attribute_value(&Attributes::WATER_MOVEMENT_EFFICIENCY);

            if water_movement_efficiency > 0.0 {
                if !self.entity.on_ground.load(SeqCst) {
                    water_movement_efficiency *= 0.5;
                }

                friction += (0.546_000_06 - friction) * water_movement_efficiency;
                speed += (effective_speed - speed) * water_movement_efficiency;
            }

            if self.has_effect(&StatusEffect::DOLPHINS_GRACE) {
                friction = 0.96;
            }

            self.entity
                .update_velocity_from_input(movement_input, speed);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();
            if self.entity.horizontal_collision.load(SeqCst) && caller.on_climbable() {
                velo.y = 0.2;
            }

            velo = velo.multiply(friction, 0.8, friction);

            self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            self.entity.velocity.store(velo);
        } else {
            self.entity.update_velocity_from_input(movement_input, 0.02);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();

            if self.entity.lava_height.load() <= self.get_swim_height() {
                velo.x *= 0.5;
                velo.z *= 0.5;
                velo.y *= 0.8;

                self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            } else {
                velo = velo * 0.5;
            }

            if gravity != 0.0 {
                velo.y -= gravity / 4.0; // Negative gravity = buoyancy
            }

            self.entity.velocity.store(velo);
        }

        let mut velo = self.entity.velocity.load();

        if self.entity.horizontal_collision.load(SeqCst)
            && !self
                .entity
                .world
                .load()
                .check_fluid_collision(self.entity.bounding_box.load().shift(velo))
        {
            velo.y = 0.3;

            self.entity.velocity.store(velo);
        }
    }

    fn apply_fluid_moving_speed(&self, dy: &mut f64, gravity: f64, falling: bool) {
        if gravity != 0.0 && !self.entity.sprinting.load(Relaxed) {
            if falling && (*dy - 0.005).abs() >= 0.003 && (*dy - gravity / 16.0).abs() < 0.003 {
                *dy = -0.003;
            } else {
                *dy -= gravity / 16.0;
            }
        }
    }

    fn make_move(&self, caller: &dyn EntityBase) {
        self.entity.move_entity(caller, self.entity.velocity.load());

        self.check_climbing();
    }

    fn check_climbing(&self) {
        // If spectator: return false

        // TODO
        // let mut pos = self.entity.block_pos.load();

        // let world = self.entity.world.read().await;

        // let (block, state) = world.get_block_and_state(&pos);

        // let name = block.properties(state.id).map(|props| props.name());

        // if let Some(name) = name {
        //     if name == "LadderLikeProperties"
        //         || name == "ScaffoldingLikeProperties"
        //         || name == "CaveVinesLikeProperties"
        //         || name == "CaveVinesPlantLikeProperties"
        //     {
        //         self.climbing.store(true, Relaxed);

        //         self.climbing_pos.store(Some(pos));

        //         return;
        //     }

        //     if name == "OakTrapdoorLikeProperties" {
        //         let trapdoor = OakTrapdoorLikeProperties::from_state_id(state.id);

        //         pos.0.y -= 1;

        //         let (down_block, down_state) = world.get_block_and_state(&pos);

        //         let is_ladder = down_block
        //             .properties(down_state.id)
        //             .is_some_and(|down_props| down_props.name() == "LadderLikeProperties");

        //         if is_ladder {
        //             let ladder = LadderLikeProperties::from_state_id(down_state.id);

        //             if trapdoor.r#facing == ladder.r#facing {
        //                 self.climbing.store(true, Relaxed);

        //                 self.climbing_pos.store(Some(pos));

        //                 return;
        //             }
        //         }
        //     }
        // }

        self.climbing.store(false, Relaxed);

        if self.entity.on_ground.load(SeqCst) {
            self.climbing_pos.store(None);
        }
    }

    fn apply_climbing_speed(&self, caller: &dyn EntityBase) {
        if caller.on_climbable() {
            self.fall_distance.store(0.0);

            let mut velo = self.entity.velocity.load();

            let pos = 0.15;

            let neg = -0.15;

            if velo.x < neg {
                velo.x = neg;
            } else if velo.x > pos {
                velo.x = pos;
            }

            if velo.z < neg {
                velo.z = neg;
            } else if velo.z > pos {
                velo.z = pos;
            }

            velo.y = velo.y.max(neg);

            // TODO
            // if velo.y < 0.0
            //     && self.entity.entity_type == &EntityType::PLAYER
            //     && self.entity.sneaking.load(Relaxed)
            // {
            //     let block = self
            //         .entity
            //         .world
            //         .read()
            //         .await
            //         .get_block(&self.entity.block_pos.load())
            //         .await;

            //     if let Some(props) = block.properties(block.default_state.id) {
            //         if props.name() == "ScaffoldingLikeProperties" {
            //             velo.y = 0.0;
            //         }
            //     }
            // }

            self.entity.velocity.store(velo);
        }
    }

    pub fn get_swim_height(&self) -> f64 {
        let eye_height = self.entity.get_eye_height();

        if self.entity.entity_type == &EntityType::BREEZE {
            eye_height
        } else if eye_height < 0.4 {
            0.0
        } else {
            0.4
        }
    }

    fn jump(&self) {
        let jump = self.get_jump_velocity(1.0);

        if jump <= 1.0e-5 {
            return;
        }

        let mut velo = self.entity.velocity.load();

        velo.y = jump.max(velo.y);

        if self.entity.sprinting.load(Relaxed) {
            let yaw = f64::from(self.entity.yaw.load()).to_radians();

            velo.x -= yaw.sin() * 0.2;
            velo.z += yaw.cos() * 0.2;
        }

        self.entity.velocity.store(velo);

        self.entity.velocity_dirty.store(true, SeqCst);
    }

    fn get_jump_velocity(&self, mut strength: f64) -> f64 {
        strength *= self.get_attribute_value(&Attributes::JUMP_STRENGTH);
        strength *= f64::from(self.entity.get_jump_velocity_multiplier());
        if let Some(effect) = self.get_effect(&StatusEffect::JUMP_BOOST) {
            strength += 0.1 * f64::from(effect.amplifier + 1);
        }
        strength
    }

    pub fn fall(
        &self,
        caller: &dyn EntityBase,
        height_difference: f64,
        ground: bool,
        dont_damage: bool,
    ) {
        // Vanilla does this in `LivingEntity.updateFallFlying`; the movement delta stands in for velocity.
        if self.entity.is_fall_flying() {
            self.check_fall_distance_accumulation(height_difference);
        }
        if ground {
            let fall_distance = self.fall_distance.swap(0.0);
            if fall_distance > 0.0 {
                self.on_changed_block(caller, self.entity.block_pos.load());
                if caller.get_player().is_some() {
                    let world = self.entity.world.load();
                    let pos = self.entity.get_pos_with_y_offset(0.2).0;
                    let block = world.get_block(&pos);
                    world
                        .block_registry
                        .update_entity_movement_after_fall_on(block, caller);
                }
            }
            if fall_distance <= 0.0
                || dont_damage
                || self.should_prevent_fall_damage()
                || self.should_prevent_fall_damage_in_area()
                || self.is_immune_to_fall_damage()
            {
                return;
            }
            let world = self.entity.world.load();
            let landing_pos = self.entity.get_pos_with_y_offset(0.2).0;
            let block = world.get_block(&landing_pos);
            let pumpkin_block = world.block_registry.get_pumpkin_block(block.id);
            if let Some(pumpkin_block) = pumpkin_block {
                pumpkin_block.on_landed_upon(OnLandedUponArgs {
                    world: &world,
                    position: &landing_pos,
                    fall_distance,
                    entity: caller,
                });
            } else {
                self.handle_fall_damage(caller, fall_distance, 1.0);
            }
            world.emit_game_event(
                GameEvent::HitGround.name(),
                self.entity.pos.load(),
            );
        } else if height_difference < 0.0 {
            let new_fall_distance = if !self.should_prevent_fall_damage()
                && !self.should_prevent_fall_damage_in_area()
            {
                let distance = self.fall_distance.load();
                distance - (height_difference as f32)
            } else {
                0f32
            };
            self.fall_distance.store(new_fall_distance);
        }
    }

    /// Mirrors vanilla `Entity.checkFallDistanceAccumulation`.
    fn check_fall_distance_accumulation(&self, velocity_y: f64) {
        if velocity_y > -0.5 && self.fall_distance.load() > 1.0 {
            self.fall_distance.store(1.0);
        }
    }

    pub fn handle_fall_damage(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f32,
        damage_per_distance: f32,
    ) {
        self.handle_fall_damage_from(caller, fall_distance, damage_per_distance, DamageType::FALL);
    }

    /// Like [`Self::handle_fall_damage`], but with the damage type to deal (e.g. stalagmites).
    pub fn handle_fall_damage_from(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f32,
        damage_per_distance: f32,
        damage_type: DamageType,
    ) {
        let may_fly = caller.get_player().is_some_and(|player| {
            player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .allow_flying
        });
        if may_fly || self.is_immune_to_fall_damage() {
            return;
        }

        // Vanilla parity: the fall_damage gamerule only affects players.
        if caller.get_player().is_some()
            && !self
                .entity
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .fall_damage
        {
            return;
        }

        if fall_distance >= 2.0
            && let Some(player) = caller.get_player()
        {
            player.increment_stat(
                StatisticCategory::Custom,
                CustomStatistic::FallOneCm as i32,
                (fall_distance * 100.0).round() as i32,
            );
        }

        let damage = caller.get_mob().map_or_else(
            || self.calculate_fall_damage(f64::from(fall_distance), damage_per_distance),
            |mob| mob.calculate_fall_damage(f64::from(fall_distance), damage_per_distance),
        ) as f32;

        if damage > 0.0 {
            let check_damage = self.damage(caller, damage, damage_type);
            if check_damage {
                self.entity
                    .play_sound(Self::get_fall_sound(damage as i32));
            }
        }
    }

    /// Calculates fall power based on distance and safe fall distance attribute,
    /// mirroring vanilla `LivingEntity.calculateFallPower`.
    pub fn calculate_fall_power(&self, fall_distance: f64) -> f64 {
        let base_safe = self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE);
        let jump_amp = self
            .get_effect(&StatusEffect::JUMP_BOOST)
            .map(|e| e.amplifier as u32);
        let safe_distance =
            crate::entity::combat::calculate_safe_fall_distance(base_safe, jump_amp);
        crate::entity::combat::calculate_fall_power(fall_distance, safe_distance)
    }

    /// Calculates fall damage based on distance, damage modifier, and fall damage multiplier attribute,
    /// mirroring vanilla `LivingEntity.calculateFallDamage`.
    pub fn calculate_fall_damage(&self, fall_distance: f64, damage_modifier: f32) -> i32 {
        if self.is_immune_to_fall_damage()
            || self.has_effect(&StatusEffect::SLOW_FALLING)
            || self.has_effect(&StatusEffect::LEVITATION)
        {
            return 0;
        }

        let fall_power = self.calculate_fall_power(fall_distance);
        let multiplier = self.get_attribute_value(&Attributes::FALL_DAMAGE_MULTIPLIER) as f32;
        crate::entity::combat::calculate_fall_damage_raw(fall_power, damage_modifier, multiplier)
    }

    const fn get_fall_sound(damage: i32) -> Sound {
        if damage > 4 {
            Sound::EntityGenericBigFall
        } else {
            Sound::EntityGenericSmallFall
        }
    }

    /// Calculates decreased air supply taking `Attributes::OXYGEN_BONUS` and helmet Respiration enchantment into account,
    /// mirroring vanilla `LivingEntity.decreaseAirSupply` and `Enchantments.java:287-295`.
    #[must_use]
    pub fn decrease_air_supply(&self, current_supply: i32) -> i32 {
        let mut oxygen_bonus = self.get_attribute_value(&Attributes::OXYGEN_BONUS);
        let respiration_level = {
            let equipment = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment
                .get(&EquipmentSlot::HEAD)
                .get_enchantment_level(&pumpkin_data::Enchantment::RESPIRATION)
        };
        if respiration_level > 0 {
            oxygen_bonus += respiration_level as f64;
        }

        if oxygen_bonus > 0.0 && rand::random::<f64>() >= 1.0 / (oxygen_bonus + 1.0) {
            current_supply
        } else {
            current_supply - 1
        }
    }

    /// Calculates increased air supply, mirroring vanilla `LivingEntity.increaseAirSupply`.
    #[must_use]
    pub fn increase_air_supply(&self, current_supply: i32, max_air: i32) -> i32 {
        (current_supply + 4).min(max_air)
    }

    /// Whether this entity should take drowning damage, mirroring vanilla `LivingEntity.shouldTakeDrowningDamage`.
    #[must_use]
    pub fn should_take_drowning_damage(&self, current_supply: i32) -> bool {
        current_supply <= -20
    }

    /// Calculates extra knockback caused by this entity's attacks,
    /// mirroring vanilla `LivingEntity.getKnockback`.
    #[must_use]
    pub fn get_knockback(&self) -> f32 {
        let knockback = self.get_attribute_value(&Attributes::ATTACK_KNOCKBACK) as f32;
        knockback / 2.0
    }

    /// Whether this entity has an item equipped in its head slot,
    /// mirroring vanilla `!this.getItemBySlot(EquipmentSlot.HEAD).isEmpty()`.
    #[must_use]
    pub fn has_helmet(&self) -> bool {
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !equipment.get(&EquipmentSlot::HEAD).is_empty()
    }

    /// Returns a clone of the item equipped in the specified equipment slot.
    #[must_use]
    pub fn get_equipment_item(&self, slot: &EquipmentSlot) -> ItemStack {
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment.get(slot)
    }

    /// Vanilla parity: Thorns reflection from victim's armor.
    /// In vanilla `EnchantmentHelper.doPostAttackEffects`, when damaged by an attacker,
    /// each equipped armor item with `Thorns` has a `0.15 * level` chance to deal
    /// `1 + rand(4)` (or `level - 10` if level > 10) thorns damage to the attacker
    /// and consume 2 durability from the armor piece.
    pub fn apply_thorns_reflection(
        &self,
        caller: &dyn EntityBase,
        attacker: &dyn EntityBase,
    ) {
        use pumpkin_util::random::{RandomGenerator, RandomImpl, get_seed, xoroshiro128::Xoroshiro};

        let mut equipment_updates = Vec::new();
        let mut thorns_attacks = Vec::new();
        let mut rng = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));

        {
            let mut equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ] {
                if let Some(stack) = equipment_lock.equipment.get_mut(&slot)
                    && !stack.is_empty()
                {
                    let thorns_level = stack.get_enchantment_level(&Enchantment::THORNS);
                    if thorns_level > 0 {
                        let chance = 0.15 * (thorns_level as f32);
                        if rng.next_f32() < chance {
                            let thorns_damage = if thorns_level > 10 {
                                (thorns_level - 10) as f32
                            } else {
                                1.0 + rng.next_bounded_i32(4) as f32
                            };
                            thorns_attacks.push(thorns_damage);

                            let item_id = stack.item.id;
                            let slot_result = stack.damage_item(2);
                            if slot_result != pumpkin_data::item_stack::DamageResult::Untouched {
                                if slot_result == pumpkin_data::item_stack::DamageResult::Broken {
                                    if let Some(player) = caller.get_player() {
                                        player.increment_stat(
                                            pumpkin_data::statistic::StatisticCategory::Broken,
                                            item_id as i32,
                                            1,
                                        );
                                    }
                                    let world = self.entity.world.load();
                                    world.send_entity_status(
                                        &self.entity,
                                        super::equipment_break_status(&slot),
                                        None,
                                    );
                                }
                                equipment_updates.push((slot.clone(), stack.clone()));
                            }
                        }
                    }
                }
            }
        }

        for thorns_damage in thorns_attacks {
            attacker.damage_with_context(
                attacker,
                thorns_damage,
                DamageType::THORNS,
                None,
                Some(caller),
                Some(caller),
            );
        }

        if !equipment_updates.is_empty() {
            self.send_equipment_changes(&equipment_updates);
        }
    }

    /// Vanilla `LivingEntity.onEnterCombat`
    pub fn on_enter_combat(&self, caller: &dyn EntityBase) {
        if let Some(player) = caller.get_player() {
            player.on_enter_combat();
        }
    }

    /// Vanilla `LivingEntity.onLeaveCombat`
    pub fn on_leave_combat(&self, caller: &dyn EntityBase, duration_ticks: i64) {
        if let Some(player) = caller.get_player() {
            player.on_leave_combat(duration_ticks);
        }
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    /// Builds the chat death message for this entity: picks the
    /// `death.attack.<id>` (or the `.player` variant when a killer entity is
    /// known, or the kill-credit name when the killer is offline/removed)
    /// translation and fills in the victim and killer display names.
    pub fn get_death_message(
        dyn_self: &dyn EntityBase,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> TextComponent {
        let kill_credit = dyn_self.get_living_entity().and_then(Self::get_kill_credit);
        let kill_credit_name = kill_credit.as_ref().map(|c| c.get_display_name());

        if let Some(living) = dyn_self.get_living_entity() {
            let tracker = living
                .combat_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            return tracker.get_death_message(dyn_self.get_display_name(), kill_credit_name);
        }

        if let Some(cause) = cause
            && source.is_some()
        {
            TextComponent::translate_cross(
                format!("death.attack.{}", damage_type.message_id),
                format!("death.attack.{}", damage_type.message_id),
                [dyn_self.get_display_name(), cause.get_display_name()],
            )
        } else if let Some(killer) = cause
            .or(source)
            .map(|c| c.get_display_name())
            .or(kill_credit_name)
        {
            TextComponent::translate_cross(
                format!("death.attack.{}.player", damage_type.message_id),
                format!("death.attack.{}.player", damage_type.message_id),
                [dyn_self.get_display_name(), killer],
            )
        } else {
            TextComponent::translate_cross(
                format!("death.attack.{}", damage_type.message_id),
                format!("death.attack.{}", damage_type.message_id),
                [dyn_self.get_display_name()],
            )
        }
    }

    /// Marks the entity as dead exactly once and runs the server-side death
    /// flow: stop movement input, attribute the kill, drop loot, broadcast the
    /// `Death` (3) entity event, and hand out XP. Safe to call on every lethal
    /// damage event; only the first call has an effect.
    #[allow(clippy::too_many_lines)]
    pub fn on_death(
        &self,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) {
        let world = self.entity.world.load();
        let Some(dyn_self) = world.get_entity_by_id(self.entity.entity_id) else {
            return;
        };
        if self
            .dead
            .compare_exchange(false, true, Relaxed, Relaxed)
            .is_ok()
        {
            self.movement_input.store(Vector3::default());
            self.jumping.store(false, Relaxed);

            // Vanilla LivingEntity.java:1466-1470 & Player.java:534-542:
            // if (this.isSleeping()) { this.stopSleeping(); }
            // this.stopUsingItem();
            // this.setDeltaMovement(...);
            if let Some(player) = dyn_self.get_player() {
                if player.is_sleeping() {
                    player.wake_up();
                }
                player.stop_using_item();
                let death_velocity = crate::entity::combat::calculate_player_death_velocity(
                    self.entity.yaw.load(),
                    0.0,
                    source.is_some() || cause.is_some(),
                );
                self.entity.set_velocity(death_velocity);
            } else {
                self.clear_active_hand();
            }

            self.set_arrow_count(0);
            self.set_stinger_count(0);
            self.set_auto_spin_attack(false);
            self.entity.set_fall_flying(false);
            self.fall_fly_ticks.store(0, Relaxed);

            let kill_credit = self.get_kill_credit();
            let killer = cause.or(source).or(kill_credit.as_deref());

            self.update_death_stats(&*dyn_self, killer);

            // Plays the death sound
            world.play_sound_fine(
                self.death_sound(&*dyn_self),
                self.sound_category(dyn_self.get_player().is_some()),
                &self.entity.pos.load(),
                1.0,
                self.get_pitch(),
            );
            world.send_entity_status(&self.entity, EntityStatus::Death, Some(ActorEventID::Death));
            let looting_level;
            let tool = if let Some(cause_ent) = cause {
                if let Some(player) = cause_ent
                    .cast_any()
                    .downcast_ref::<crate::entity::player::Player>()
                {
                    let hand_stack = player
                        .inventory()
                        .get_stack_in_hand(pumpkin_util::Hand::Right);
                    looting_level = hand_stack
                        .get_enchantment_level(&Enchantment::LOOTING)
                        .max(0) as u32;
                    (!hand_stack.is_empty()).then(|| hand_stack.clone())
                } else {
                    looting_level = 0;
                    None
                }
            } else {
                looting_level = 0;
                None
            };

            let is_raining = world.is_raining();
            let is_thundering = world.is_thundering();

            let has_player_kill =
                killer.is_some_and(|c| c.get_entity().entity_type == &EntityType::PLAYER)
                    || self.get_kill_credit().is_some_and(|k| k.get_player().is_some())
                    || {
                        let tracker = self
                            .combat_tracker
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        tracker.has_player_attacker()
                    };

            let mut params = LootContextParameters {
                killed_by_player: Some(has_player_kill),
                this_entity: Some(self.entity.entity_type),
                killer_entity: killer.map(|c| c.get_entity().entity_type),
                direct_killer_entity: source.map(|s| s.get_entity().entity_type),
                position: Some(self.entity.pos.load()),
                world_time: world.level_info.load().day_time as u64,
                damage_type: Some(damage_type),
                tool,
                is_raining: Some(is_raining),
                is_thundering: Some(is_thundering),
                is_on_fire: Some(
                    self.entity
                        .fire_ticks
                        .load(std::sync::atomic::Ordering::Relaxed)
                        > 0,
                ),
                direct_attacker_weapon: source.and_then(|direct| {
                    direct
                        .get_living_entity()
                        .map(|living| living.held_item(direct))
                }),
                ..Default::default()
            };
            if let Some(mob) = dyn_self.get_mob() {
                mob.populate_loot_context(&mut params);
            }

            // Drop loot
            self.drop_loot(&params);

            // Check for nearby Sculk Catalyst (vanilla radius = 8 blocks)
            let death_pos = self.entity.pos.load().to_block_pos();
            let mut absorbed_by_catalyst = false;
            'catalyst_search: for dy in -8..=8 {
                for dx in -8..=8 {
                    for dz in -8..=8 {
                        if dx * dx + dy * dy + dz * dz <= 64 {
                            let check_pos = BlockPos::new(
                                death_pos.0.x + dx,
                                death_pos.0.y + dy,
                                death_pos.0.z + dz,
                            );
                            let block = world.get_block(&check_pos);
                            if block.id == pumpkin_data::BlockId::SCULK_CATALYST {
                                crate::block::blocks::sculk::sculk_catalyst::SculkCatalystBlock::bloom(&world, &check_pos);
                                if let Some(player) = killer.and_then(|k| k.get_player()) {
                                    player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::KillMobNearSculkCatalyst);
                                }
                                absorbed_by_catalyst = true;
                                break 'catalyst_search;
                            }
                        }
                    }
                }
            }

            // Check for nearby Axolotl supporting effect if killed by player
            if let Some(player) = killer.and_then(|k| k.get_player()) {
                let p_pos = player.position();
                let min = [p_pos.x - 20.0, p_pos.y - 20.0, p_pos.z - 20.0];
                let max = [p_pos.x + 20.0, p_pos.y + 20.0, p_pos.z + 20.0];
                let search_box = pumpkin_util::math::boundingbox::BoundingBox::new_array(min, max);
                for entity in world.get_entities_at_box(&search_box) {
                    if entity.get_entity().entity_type == &pumpkin_data::entity::EntityType::AXOLOTL
                    {
                        if let Some(axolotl) = entity
                            .cast_any()
                            .downcast_ref::<crate::entity::passive::axolotl::AxolotlEntity>(
                        ) {
                            axolotl.apply_supporting_effects(&player);
                            break;
                        }
                    }
                }
            }

            // Award experience (Vanilla LivingEntity.java:1529-1536)
            let is_always_dropper = dyn_self.get_player().is_some();
            let should_drop = dyn_self.get_mob().is_none_or(|m| m.should_drop_experience());
            let mob_drops = world.level_info.load().game_rules.mob_drops;
            let was_killed_by_player = params.killed_by_player.unwrap_or(false);

            if should_entity_drop_experience(
                is_always_dropper,
                was_killed_by_player,
                should_drop,
                mob_drops,
                absorbed_by_catalyst,
            ) {
                let amount = dyn_self.get_experience_reward(killer);
                if amount > 0 {
                    ExperienceOrbEntity::spawn(&world, self.entity.pos.load(), amount);
                }
            }
            self.entity.pose.store(EntityPose::Dying);

            if dyn_self.get_player().is_none() {
                self.drop_equipment(looting_level);
            }

            // Vanilla LivingEntity.java:1480:
            // this.createWitherRose(killer);
            self.create_wither_rose(killer);

            // Broadcast death message if it's a player and the gamerule is enabled
            self.broadcast_death_message(&*dyn_self, damage_type, source, cause);

            // Trigger on_mob_death for active status effects
            let active_effects_vec: Vec<_> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects
                    .values()
                    .map(|e| (e.effect_type, e.amplifier))
                    .collect()
            };
            for (effect_type, amplifier) in active_effects_vec {
                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect_type) {
                    mob_effect.on_mob_death(self, amplifier, &damage_type);
                }
            }

            self.reset_effects_and_attributes();
        }
    }

    fn drop_equipment(&self, looting_level: u32) {
        let world = self.entity.world.load();
        let block_pos = self.entity.block_pos.load();

        let drop_chances = self
            .equipment_drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let slots_to_drop: Vec<EquipmentSlot> = {
            let mut slots: Vec<_> = self.equipment_slots.values().cloned().collect();
            slots.push(EquipmentSlot::MAIN_HAND);
            slots
        };

        for slot in &slots_to_drop {
            let mut chance = drop_chances
                .get(slot)
                .copied()
                .unwrap_or(DEFAULT_EQUIPMENT_DROP_CHANCE);
            // A chance above 1.0 marks a guaranteed, undamaged drop.
            let preserved = chance > 1.0;
            // Vanilla approximation: EnchantmentHelper.processEquipmentDropChance
            // adds lootingLevel * 0.01 to the per-slot equipment drop chance.
            chance += looting_level as f32 * 0.01;
            chance = chance.min(1.0);
            if !preserved && rand::random::<f32>() >= chance {
                continue;
            }
            let mut item = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .equipment
                .remove(slot)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            if item.is_empty() {
                continue;
            }
            // Vanilla Mob.java:899-905: Curse of Vanishing prevents equipment from dropping on death.
            if has_vanishing_curse(&item) {
                continue;
            }
            // Vanilla approximation: Mob.dropCustomDeathLoot applies random
            // damage to dropped equipment using two chained random calls:
            // setDamageValue(maxDamage - random.nextInt(1 + random.nextInt(max(maxDamage - 3, 1))))
            if !preserved && let Some(max_damage) = item.get_max_damage() {
                let mut rng = rand::rng();
                let inner = rng.random_range(0..(max_damage - 3).max(1));
                let outer = rng.random_range(0..=inner);
                item.set_damage((max_damage - outer).max(0));
            }
            world.drop_stack(&block_pos, item);
        }
    }

    /// Creates a Wither Rose when an entity is killed by a Wither boss, mirroring vanilla
    /// `LivingEntity.createWitherRose` (LivingEntity.java:1494-1512).
    pub fn create_wither_rose(&self, killer: Option<&dyn EntityBase>) {
        let killer_is_wither = killer.is_some_and(|k| {
            k.get_entity().entity_type == &pumpkin_data::entity::EntityType::WITHER
        });
        if !killer_is_wither {
            return;
        }

        let world = self.entity.world.load();
        let pos = self.entity.pos.load().to_block_pos();
        let below_pos = BlockPos::new(pos.0.x, pos.0.y - 1, pos.0.z);

        let target_is_air = world.get_block(&pos).is_air();
        let below_supports = world
            .get_block(&below_pos)
            .has_tag(&tag::Block::MINECRAFT_SUPPORTS_WITHER_ROSE);

        let can_place = can_place_wither_rose(target_is_air, below_supports);
        let mob_griefing = world.level_info.load().game_rules.mob_griefing;

        match determine_wither_rose_action(true, mob_griefing, can_place) {
            WitherRosePlacementAction::PlaceBlock => {
                world.set_block_state(
                    &pos,
                    pumpkin_data::Block::WITHER_ROSE.default_state.id,
                    crate::world::BlockFlags::NOTIFY_ALL,
                );
            }
            WitherRosePlacementAction::DropItem => {
                world.drop_stack_at(
                    self.entity.pos.load(),
                    pumpkin_data::item_stack::ItemStack::new(
                        1,
                        &pumpkin_data::item::Item::WITHER_ROSE,
                    ),
                );
            }
            WitherRosePlacementAction::None => {}
        }
    }

    fn broadcast_death_message(
        &self,
        dyn_self: &dyn EntityBase,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) {
        let world = self.entity.world.load();
        let show_death_messages = { world.level_info.load().game_rules.show_death_messages };
        if self.entity.entity_type == &EntityType::PLAYER {
            let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);

            // Vanilla `ServerPlayer.die`: recheck combat status after death message is retrieved
            let left_combat_duration = {
                let mut tracker = self
                    .combat_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                tracker.recheck_status(self.entity.age.load(Relaxed) as i64, false)
            };
            if let Some(duration) = left_combat_duration {
                self.on_leave_combat(dyn_self, duration);
            }

            let mut final_death_message = death_message;
            if let Some(player) = dyn_self.get_player() {
                if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                    && let Some(server) = world.server.upgrade()
                {
                    let mut event =
                        crate::plugin::api::events::entity::entity_death::PlayerDeathEvent::new(
                            player_arc,
                            final_death_message.clone(),
                            0,
                        );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        return;
                    }
                    final_death_message = event.death_message;
                }

                // Vanilla ServerPlayer.die: if showDeathMessages is false, send empty message in combat kill packet
                let empty_message = pumpkin_util::text::TextComponent::empty();
                let client_death_message = if show_death_messages {
                    &final_death_message
                } else {
                    &empty_message
                };
                player.handle_killed(client_death_message);
            }

            if show_death_messages && let Some(server) = world.server.upgrade() {
                for player in server.get_all_players() {
                    player.send_system_message(&final_death_message);
                }
            }
        } else if self.entity.custom_name.load().is_some() {
            let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
            tracing::info!(
                "Named entity {} died: {}",
                dyn_self.get_display_name().to_pretty_console(),
                death_message.to_pretty_console()
            );
        }
    }

    fn update_death_stats(&self, dyn_self: &dyn EntityBase, cause: Option<&dyn EntityBase>) {
        if let Some(victim_player) = dyn_self.get_player() {
            victim_player.increment_custom_stat(CustomStatistic::Deaths, 1);
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceDeath as i32,
                0,
            );
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceRest as i32,
                0,
            );
            if let Some(killer_entity) = cause.map(EntityBase::get_entity) {
                victim_player.increment_stat(
                    StatisticCategory::KilledBy,
                    killer_entity.entity_type.id as i32,
                    1,
                );
            }
        }

        if let Some(killer_player) = cause.and_then(|c| c.get_player()) {
            killer_player.increment_stat(
                StatisticCategory::Killed,
                self.entity.entity_type.id as i32,
                1,
            );
            if dyn_self.get_player().is_some() {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::PlayerKills as i32,
                    1,
                );
            } else {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::MobKills as i32,
                    1,
                );

                let resource_name = self.entity.entity_type.resource_name;
                let criterion_key = format!("minecraft:{resource_name}");
                killer_player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::PlayerKilledEntity {
                        entity_type_resource: criterion_key,
                    },
                );

                if resource_name == "skeleton" {
                    let distance_sq = killer_player
                        .position()
                        .squared_distance_to_vec(&self.entity.pos.load());
                    if distance_sq >= 2500.0 {
                        killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::SniperDuel);
                    }
                }

                if resource_name == "phantom" {
                    killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::TwoBirdsOneArrow);
                }

                let held_item = killer_player.inventory().held_item();
                let is_crossbow = held_item.item.registry_key == "crossbow";
                if is_crossbow {
                    killer_player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::Arbalistic,
                    );
                    if resource_name == "pillager" {
                        killer_player.trigger_advancement(
                            crate::entity::player::advancement::trigger::AdvancementTrigger::WhosThePillagerNow,
                        );
                    }
                }
            }
        }
    }

    fn drop_loot(&self, params: &LootContextParameters) {
        let resource_name = self.get_entity().entity_type.resource_name;
        let key = format!("minecraft:entities/{resource_name}");
        let world = self.entity.world.load();
        if let Some(loot_table) = world.get_loot_table(&key) {
            let seed: i64 = rand::random();
            let pos = self.entity.block_pos.load();
            for stack in crate::world::loot::generate_loot_from_handle(&loot_table, seed, params) {
                world.drop_stack(&pos, stack);
            }
        }
    }

    fn tick_effects(&self) {
        let mut effects_to_remove = Vec::new();
        let mut effects_to_apply = Vec::new();
        let mut effects_to_refresh = Vec::new();

        {
            let Ok(mut effects) = self.active_effects.try_lock() else {
                return;
            };
            let entity_age = self.entity.age.load(Relaxed);
            for effect in effects.values_mut() {
                if effect.duration == 0 {
                    effects_to_remove.push(effect.effect_type);
                    continue;
                }

                let tick_duration = if effect.duration == -1 {
                    entity_age
                } else {
                    effect.duration
                };

                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect.effect_type)
                    && mob_effect.should_apply_effect_tick(tick_duration, effect.amplifier)
                {
                    effects_to_apply.push((mob_effect, effect.amplifier));
                }

                if effect.duration != -1 {
                    effect.duration -= 1;
                    // Vanilla `LivingEntity.tickEffects` re-sends the effect every 600
                    // ticks via `onEffectUpdated(effect, false, null)`; the `false`
                    // means the attribute modifiers are not reapplied.
                    if effect.duration > 0 && effect.duration % 600 == 0 {
                        effects_to_refresh.push(effect.clone());
                    }
                }
            }
        }

        // Call the central removal function for each expired effect
        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }

        for (mob_effect, amplifier) in effects_to_apply {
            mob_effect.apply_effect_tick(self, amplifier);
        }

        for effect in effects_to_refresh {
            self.broadcast_effect(&effect);
        }
    }

    pub fn clear_status_effects(&self) {
        let effect_types: Vec<&'static StatusEffect> = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .copied()
            .collect();
        for effect_type in effect_types {
            self.remove_effect(effect_type);
        }
    }

    /// Tries to use a totem of undying from the entity's hands. If successful, applies the totem effects and returns true.
    /// Matches vanilla Minecraft 26.3 `LivingEntity.checkTotemDeathProtection(source)`.
    pub fn check_totem_death_protection(
        &self,
        caller: &dyn EntityBase,
        damage_type: DamageType,
    ) -> bool {
        // Vanilla LivingEntity.java:1394: checkTotemDeathProtection
        // if (damageSource.is(DamageTypeTags.BYPASSES_INVULNERABILITY)) return false;
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY) {
            return false;
        }

        for hand in Hand::all() {
            let mut stack = self.get_stack_in_hand(caller, hand);

            if stack.item.id == Item::TOTEM_OF_UNDYING.id
                || stack.get_data_component::<DeathProtectionImpl>().is_some()
            {
                let mut resurrect_event =
                    crate::plugin::api::events::entity::entity_resurrect::EntityResurrectEvent::new(
                        self.entity.entity_id,
                    );
                if let Some(server) = self.entity.world.load().server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut resurrect_event);
                }
                if resurrect_event.cancelled {
                    return false;
                }

                stack.item_count = stack.item_count.saturating_sub(1);
                if stack.item_count == 0 {
                    stack = ItemStack::EMPTY.clone();
                }
                let slot = EquipmentSlot::from_hand(hand);
                if let Some(player) = caller.get_player() {
                    player.inventory.set_stack_in_hand(hand, stack.clone());
                    player
                        .inventory
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot.clone(), stack.clone());
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::UsedTotem,
                    );
                    player.increment_stat(
                        StatisticCategory::Used,
                        Item::TOTEM_OF_UNDYING.id as i32,
                        1,
                    );
                } else {
                    self.entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot.clone(), stack.clone());
                }
                self.send_equipment_changes(&[(slot, stack)]);

                self.set_health(1.0);
                self.clear_status_effects();

                let world = self.entity.world.load();
                world.send_entity_status(
                    &self.entity,
                    EntityStatus::ProtectedFromDeath,
                    Some(ActorEventID::TalismanActivate),
                );
                world.play_sound(
                    Sound::ItemTotemUse,
                    SoundCategory::Players,
                    &self.entity.pos.load(),
                );
                world.emit_game_event(
                    GameEvent::ItemInteractFinish.name(),
                    self.entity.pos.load(),
                );

                // Set Absorption II (100 ticks), Regeneration II (900 ticks), Fire Resistance I (800 ticks)
                self.add_effect(Effect {
                    effect_type: &StatusEffect::ABSORPTION,
                    duration: 100,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::REGENERATION,
                    duration: 900,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::FIRE_RESISTANCE,
                    duration: 800,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });

                return true;
            }
        }

        false
    }

    fn damage_armor_items(
        &self,
        caller: &dyn EntityBase,
        damage_amount: f32,
        damage_type: &DamageType,
    ) {
        if damage_amount <= 0.0 || damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ARMOR) {
            return;
        }

        let is_helmet_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_DAMAGES_HELMET);

        // Formula: armor loses floor(incoming_damage / 4) durability, minimum 1.
        let armor_damage = (damage_amount / 4.0).floor().max(1.0) as i32;
        let mut equipment_updates = Vec::new();

        let armor_slots: Vec<(usize, ItemStack, EquipmentSlot)> = {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.equipment_slots
                .iter()
                .filter(|(_, slot)| {
                    if is_helmet_damage {
                        matches!(slot, EquipmentSlot::Head(_))
                    } else {
                        slot.is_armor_slot()
                    }
                })
                .filter_map(|(index, slot)| {
                    equipment_lock
                        .equipment
                        .get(slot)
                        .cloned()
                        .map(|stack| (*index, stack, slot.clone()))
                })
                .collect()
        };

        for (slot_index, mut stack, slot) in armor_slots {
            if stack.is_empty() {
                continue;
            }

            // DamageResistant component check (e.g. netherite vs fire).
            if let Some(res) = stack.get_data_component::<DamageResistantImpl>()
                && damage_type.is_tagged_with(res.res_type.as_str()) == Some(true)
            {
                continue;
            }

            let takes_damage = stack
                .get_data_component::<EquippableImpl>()
                .is_none_or(|equippable| equippable.damage_on_hurt);

            if takes_damage {
                let item_id = stack.item.id;
                let slot_result = stack.damage_item(armor_damage);
                if slot_result != pumpkin_data::item_stack::DamageResult::Untouched {
                    if slot_result == pumpkin_data::item_stack::DamageResult::Broken {
                        if let Some(player) = caller.get_player() {
                            player.increment_stat(
                                pumpkin_data::statistic::StatisticCategory::Broken,
                                item_id as i32,
                                1,
                            );
                        }
                        let world = self.entity.world.load();
                        world.send_entity_status(
                            &self.entity,
                            super::equipment_break_status(&slot),
                            None,
                        );
                    }
                    equipment_updates.push((slot.clone(), stack.clone()));
                    if let Some(player) = caller.get_player() {
                        player.enqueue_slot_set_packet(&CSetPlayerInventory::new(
                            (slot_index as i32).into(),
                            &ItemStackSerializer::from(stack),
                        ));
                    }
                }
            }
        }

        if !equipment_updates.is_empty() {
            let mut equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (slot, stack) in &equipment_updates {
                equipment_lock.equipment.insert(slot.clone(), stack.clone());
            }
            drop(equipment_lock);

            self.send_equipment_changes(&equipment_updates);
        }
    }

    pub fn held_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.held_item();
        }
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(&EquipmentSlot::MAIN_HAND)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    pub fn get_stack_in_hand(&self, caller: &dyn EntityBase, hand: Hand) -> ItemStack {
        match hand {
            Hand::Left => self.off_hand_item(caller),
            Hand::Right => self.held_item(caller),
        }
    }

    /// getOffHandStack in source
    pub fn off_hand_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.off_hand_item();
        }
        let Some(slot) = self.equipment_slots.get(&PlayerInventory::OFF_HAND_SLOT) else {
            return ItemStack::EMPTY.clone();
        };
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// Forgotten after 40 ticks.
    pub fn get_last_damage_type(&self) -> Option<DamageType> {
        let stamp = self.last_damage_stamp.load(Ordering::Relaxed);
        let mut last = self
            .last_damage_type
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.entity.world.load().get_world_age() - stamp > 40 {
            *last = None;
        }
        *last
    }

    pub fn can_take_damage(&self) -> bool {
        !self.entity.invulnerable.load(Ordering::Relaxed) && self.is_part_of_game()
    }

    #[must_use]
    pub fn can_breathe_under_water(&self) -> bool {
        self.entity
            .entity_type
            .has_tag(&tag::EntityType::MINECRAFT_CAN_BREATHE_UNDER_WATER)
    }

    #[must_use]
    pub fn can_drown(&self) -> bool {
        !self.can_breathe_under_water()
    }

    /// Vanilla `LivingEntity.canBeSeenByAnyone`, whose `isAlive` is health aware, unlike
    /// `Entity::is_alive` which only means "not removed".
    pub fn is_part_of_game(&self) -> bool {
        !self.is_spectator() && self.is_alive()
    }

    pub fn is_alive(&self) -> bool {
        self.entity.is_alive() && self.health.load() > 0.0
    }

    /// Returns true if the entity is dead or dying, mirroring vanilla `LivingEntity.isDeadOrDying`.
    pub fn is_dead_or_dying(&self) -> bool {
        self.health.load() <= 0.0
    }

    pub fn can_attack(&self, target: &dyn EntityBase) -> bool {
        if target.get_player().is_some()
            && self.entity.world.load().level_info.load().difficulty == Difficulty::Peaceful
        {
            return false;
        }
        can_be_seen_as_enemy(target)
    }

    pub fn reset_state(&self) {
        self.entity.reset_state();

        // Restore to maximum health for this entity type
        let max_health = self.get_max_health();
        self.set_health(max_health);
        // Clear any absorption
        self.absorption.store(0.0);
        // Send health metadata
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, max_health);

        self.reset_effects_and_attributes();

        // Give a short grace period of invulnerability after respawn
        self.hurt_cooldown.store(20, Relaxed);
        self.last_damage_taken.store(0f32);

        self.entity.portal_cooldown.store(0, Relaxed);
        *self
            .entity
            .portal_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;

        // Clear fall/fire state
        self.fall_distance.store(0f32);
        self.death_time.store(0, Relaxed);
        self.entity.extinguish();
        self.entity.fire_ticks.store(0, Relaxed);

        // Clear velocity and movement input to remove persisted momentum
        self.entity.velocity.store(Vector3::default());
        self.entity.velocity_dirty.store(true, SeqCst);
        self.movement_input.store(Vector3::default());
        self.jumping.store(false, Relaxed);

        // If this LivingEntity corresponds to a Player, reset their hunger manager
        let world = self.entity.world.load();
        if let Some(player) = world.get_player_by_id(self.entity.entity_id) {
            player.hunger_manager.restart();
        }

        self.dead.store(false, Relaxed);
    }

    pub fn is_player(&self) -> bool {
        let world = self.entity.world.load();
        world.get_player_by_id(self.entity.entity_id).is_some()
    }

    pub fn get_movement(&self) -> Vector3<f64> {
        self.entity.movement.load()
    }

    fn death_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.death_sound()
        {
            return audio;
        }

        Self::death_sound_for_entity(self.entity.entity_type)
    }

    fn hurt_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.hurt_sound()
        {
            return audio;
        }

        Self::hurt_sound_for_entity(self.entity.entity_type)
    }
}

impl LivingEntity {
    /// Writes living-entity tags.
    pub fn write_living_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put("Health", NbtTag::Float(self.health.load()));
        // Avoid persisting a lethal fall distance when the entity is dead to prevent death loops
        let fall_distance = if self.dead.load(Relaxed) {
            0.0
        } else {
            self.fall_distance.load()
        };
        // Persist current absorption amount
        nbt.put("AbsorptionAmount", NbtTag::Float(self.absorption.load()));
        let attributes = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::entity::attributes::write_attributes_nbt(nbt, &attributes);
        drop(attributes);
        nbt.put("FallDistance", NbtTag::Float(fall_distance));
        nbt.put_short("HurtTime", self.hurt_cooldown.load(Relaxed).max(0) as i16);
        nbt.put_short("DeathTime", i16::from(self.death_time.load(Relaxed)));
        nbt.put_bool("FallFlying", self.entity.is_fall_flying());
        self.breath.write_nbt(nbt);
        {
            let effects_vec: Vec<pumpkin_data::potion::Effect> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects.values().cloned().collect()
            };
            if !effects_vec.is_empty() {
                // Iterate effects and create Box<[NbtTag]>
                let mut effects_list = Vec::with_capacity(effects_vec.len());
                for effect in effects_vec {
                    let mut effect_nbt = pumpkin_nbt::compound::NbtCompound::new();
                    effect.write_nbt(&mut effect_nbt);
                    effects_list.push(NbtTag::Compound(effect_nbt));
                }
                nbt.put("active_effects", NbtTag::List(effects_list));
            }
        }
        let equipment = {
            let guard = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut compound = NbtCompound::new();
            for (slot, stack) in &guard.equipment {
                if !stack.is_empty() {
                    let mut item_nbt = NbtCompound::new();
                    stack.write_item_stack(&mut item_nbt);
                    compound.put(slot.to_name(), NbtTag::Compound(item_nbt));
                }
            }
            compound
        };
        if !equipment.child_tags.is_empty() {
            nbt.put("equipment", NbtTag::Compound(equipment));
        }
    }

    pub fn read_living_nbt_non_mut(&self, nbt: &NbtCompound) {
        // Restore saved attributes (base values and permanent modifiers) first so
        // the health default and absorption clamp below use the saved values,
        // mirroring vanilla `LivingEntity.readAdditionalSaveData`.
        let mut attributes = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::entity::attributes::read_attributes_nbt(nbt, &mut attributes);
        drop(attributes);

        // Vanilla LivingEntity.readAdditionalSaveData defaults to the mob's own max health,
        // not a flat 20; a hoglin (40 max) or iron golem (100 max) with no saved Health would
        // otherwise be silently reset to 20 here.
        self.health.store(
            nbt.get_float("Health")
                .unwrap_or_else(|| self.get_max_health()),
        );

        if let Some(equipment) = nbt.get_compound("equipment") {
            let mut guard = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (name, tag) in &equipment.child_tags {
                if let Some(slot) = EquipmentSlot::get_from_name(name)
                    && let Some(compound) = tag.extract_compound()
                    && let Some(stack) = ItemStack::read_item_stack(compound)
                {
                    guard.put(slot, stack);
                }
            }
        }

        // Clamp any persisted absorption to the entity's configured max
        let raw_abs = nbt.get_float("AbsorptionAmount").unwrap_or(0.0);
        let max_abs = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
        let clamped_abs = raw_abs.max(0.0).min(max_abs);
        self.absorption.store(clamped_abs);

        // Load fall distance, but if this entity is currently marked dead ensure we don't restore
        // a lethal fall distance that would immediately re-kill on spawn.
        let fd = nbt
            .get_float("FallDistance")
            .or_else(|| nbt.get_float("fall_distance"))
            .unwrap_or(0.0);
        if self.dead.load(Relaxed) {
            self.fall_distance.store(0.0);
        } else {
            self.fall_distance.store(fd);
        }
        if let Some(hurt_time) = nbt.get_short("HurtTime") {
            self.hurt_cooldown.store(i32::from(hurt_time), Relaxed);
        }
        if let Some(death_time) = nbt.get_short("DeathTime") {
            self.death_time.store(death_time as u8, Relaxed);
        }
        self.entity
            .fall_flying
            .store(nbt.get_bool("FallFlying").unwrap_or(false), Relaxed);
        {
            let nbt_effects = nbt.get_list("active_effects");
            if let Some(nbt_effects) = nbt_effects {
                let mut read_effects = Vec::new();
                for effect in nbt_effects {
                    if let NbtTag::Compound(effect_nbt) = effect {
                        if let Some(mut effect) = Effect::create_from_nbt(&mut effect_nbt.clone()) {
                            effect.blend = true; // TODO: change, is taken from effect give command
                            read_effects.push(effect);
                        } else {
                            warn!("Unable to read effect from nbt");
                        }
                    }
                }
                if !read_effects.is_empty() {
                    let mut active_effects = self
                        .active_effects
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for effect in read_effects {
                        active_effects.insert(effect.effect_type, effect);
                    }
                }
            }
        }
        // todo more...
    }

    /// Returns the entity's total armor value, mirroring vanilla `LivingEntity.getArmorValue()`.
    pub fn get_armor_value(&self) -> i32 {
        self.get_attribute_value(&Attributes::ARMOR).floor() as i32
    }

    /// Calculates damage after armor reduction, mirroring vanilla `LivingEntity.getDamageAfterArmorAbsorb`.
    pub fn get_damage_after_armor_absorb(
        &self,
        damage: f32,
        damage_type: &DamageType,
        attacker: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ARMOR) {
            return damage;
        }

        // If items are present in entity_equipment but attribute modifiers were not yet synchronized
        // (e.g. direct slot placement or equipment added without packet updates), sync them now.
        let equipment_unapplied = {
            let map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.is_empty()
        };
        if equipment_unapplied {
            let has_equipment = {
                let eq = self
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                !eq.equipment.is_empty()
            };
            if has_equipment {
                self.apply_current_equipment_attribute_modifiers();
            }
        }

        let total_armor = self.get_armor_value() as f32;
        let armor_toughness = self.get_attribute_value(&Attributes::ARMOR_TOUGHNESS) as f32;

        let breach_level = attacker
            .and_then(|att| {
                let hand_stack = if let Some(player) = att.get_player() {
                    player
                        .inventory()
                        .get_stack_in_hand(pumpkin_util::Hand::Right)
                } else if let Some(living) = att.get_living_entity() {
                    living.get_stack_in_hand(att, pumpkin_util::Hand::Right)
                } else {
                    return None;
                };
                let level = hand_stack.get_enchantment_level(&Enchantment::BREACH);
                (level > 0).then_some(level as u32)
            })
            .unwrap_or(0);

        CombatRules::get_damage_after_absorb(damage, total_armor, armor_toughness, breach_level)
    }

    /// Calculates damage after magic/resistance/enchantment reduction, mirroring vanilla `LivingEntity.getDamageAfterMagicAbsorb`.
    pub fn get_damage_after_magic_absorb(
        &self,
        mut damage: f32,
        damage_type: &DamageType,
        caller: &dyn EntityBase,
        cause: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_EFFECTS) {
            return damage;
        }

        // 1. Resistance Effect (evaluated before enchantments)
        if !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_RESISTANCE)
            && let Some(effect) = self.get_effect(&StatusEffect::RESISTANCE)
        {
            let absorb_value = (effect.amplifier + 1) * 5;
            let absorb = 25 - absorb_value;
            let v = damage * absorb as f32;
            let old_damage = damage;
            damage = (v / 25.0).max(0.0);
            let damage_resisted = old_damage - damage;
            if damage_resisted > 0.0 {
                let stat_amount = (damage_resisted * 10.0).round() as i32;
                if let Some(victim_player) = caller.get_player() {
                    victim_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageResisted as i32,
                        stat_amount,
                    );
                }
                if let Some(attacker_player) = cause.and_then(|c| c.get_player()) {
                    attacker_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageDealtResisted as i32,
                        stat_amount,
                    );
                }
            }
        }

        if damage <= 0.0 {
            return 0.0;
        }

        // 2. Enchantment Protection
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ENCHANTMENTS) {
            return damage;
        }

        let mut epf = 0.0f32;
        {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
                EquipmentSlot::BODY,
            ] {
                if let Some(stack) = equipment_lock.equipment.get(&slot)
                    && !stack.is_empty()
                    && let Some(enchantments) = stack.get_data_component::<EnchantmentsImpl>()
                {
                    for (enchantment, level) in enchantments.enchantment.iter() {
                        enchantment.modify_damage_protection_against(*level, damage_type, &mut epf);
                    }
                }
            }
        }

        if epf > 0.0 {
            damage = CombatRules::get_damage_after_magic_absorb(damage, epf);
        }

        damage
    }

    #[allow(clippy::too_many_lines)]
    pub fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        let mut amount = amount;

        // Check invulnerability before applying damage
        if self.entity.is_invulnerable_to(&damage_type, cause) {
            return false;
        }

        if self.health.load() <= 0.0 || self.dead.load(Relaxed) {
            return false; // Dying or dead
        }

        if amount < 0.0 {
            return false;
        }

        if let Some(mob) = caller.get_mob() {
            if !mob.pre_damage(damage_type, source.or(cause)) {
                return false;
            }
            amount = mob.adjust_damage_amount(amount, &damage_type);
        }

        let mut damage_event =
            crate::plugin::api::events::entity::entity_damage::EntityDamageEvent::new(
                self.entity.entity_id,
                damage_type,
                amount,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut damage_event);
        }
        if damage_event.cancelled {
            return false;
        }
        amount = damage_event.damage;

        if let Some(damager) = source.or(cause) {
            let mut by_entity_event =
                crate::plugin::api::events::entity::entity_damage_by_entity::EntityDamageByEntityEvent {
                    entity_id: self.entity.entity_id,
                    damager_id: damager.get_entity().entity_id,
                    damage: amount,
                    cause: format!("{damage_type:?}"),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_entity_event);
            }
            if by_entity_event.cancelled {
                return false;
            }
            amount = by_entity_event.damage;
        } else if position.is_some()
            || matches!(
                damage_type,
                DamageType::CACTUS
                    | DamageType::SWEET_BERRY_BUSH
                    | DamageType::CAMPFIRE
                    | DamageType::HOT_FLOOR
                    | DamageType::STALAGMITE
            )
        {
            let damager_pos = position.map(|p| {
                BlockPos(Vector3::new(
                    p.x.floor() as i32,
                    p.y.floor() as i32,
                    p.z.floor() as i32,
                ))
            });
            let mut by_block_event =
                crate::plugin::api::events::entity::entity_damage_by_block::EntityDamageByBlockEvent {
                    entity_id: self.entity.entity_id,
                    damager_pos,
                    damage: amount,
                    cause: format!("{damage_type:?}"),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_block_event);
            }
            if by_block_event.cancelled {
                return false;
            }
            amount = by_block_event.damage;
        }

        let world = self.entity.world.load();
        let is_fire_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FIRE);

        // Fire damage can be prevented by either game rules or fire resistance
        if is_fire_damage {
            // Check game rule for fire damage (only for players)
            if self.entity.entity_type == &EntityType::PLAYER
                && !world.level_info.load().game_rules.fire_damage
            {
                return false;
            }

            // Check for fire resistance effect
            if self.has_effect(&StatusEffect::FIRE_RESISTANCE)
                && !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_EFFECTS)
            {
                return false;
            }
        }

        if let Some(player) = caller.get_player()
            && player.sleeping_since.load().is_some()
        {
            player.wake_up();
        }

        // Vanilla parity: entities in FREEZE_HURTS_EXTRA_TYPES take 5x freezing damage.
        if damage_type == DamageType::FREEZE
            && self
                .entity
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_FREEZE_HURTS_EXTRA_TYPES)
        {
            amount *= 5.0;
        }

        // Vanilla parity: DAMAGES_HELMET (falling anvil/block/stalactite) damages helmet and reduces damage by 25%.
        let is_damages_helmet = damage_type.has_tag(&tag::DamageType::MINECRAFT_DAMAGES_HELMET);
        if is_damages_helmet && self.has_helmet() {
            self.damage_armor_items(caller, amount, &damage_type);
            amount *= 0.75;
        }

        // Check for shield blocking before armor/magic/cooldown
        let damage_pos = position.or_else(|| source.map(|s| s.get_entity().pos.load()));
        let is_piercing_arrow = source
            .and_then(|s| s.cast_any().downcast_ref::<crate::entity::projectile::arrow::ArrowEntity>())
            .is_some_and(|arrow| arrow.pierce_level.load(Ordering::Relaxed) > 0);

        if self.is_blocking()
            && !is_piercing_arrow
            && !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_SHIELD)
            && crate::entity::combat::can_block_damage_source(
                self.entity.pos.load(),
                self.entity.head_yaw.load(),
                damage_pos,
            )
        {
            let player_pos = self.entity.pos.load();
            let disable_seconds = if let Some(attacker) = source.or(cause) {
                attacker.get_seconds_to_disable_blocking()
            } else {
                0.0
            };
            let disable_blocking = disable_seconds > 0.0;

            if disable_blocking {
                world.play_sound(Sound::ItemShieldBreak, SoundCategory::Players, &player_pos);
                let status_packet = CEntityStatus::new(self.entity.entity_id, 30);
                world.send_to_tracking_players_and_self(&self.entity, &status_packet);
                if let Some(player) = caller.get_player() {
                    let cooldown_ticks = (disable_seconds * 20.0).round() as i32;
                    player.set_item_cooldown("minecraft:shield", cooldown_ticks);
                }
                self.clear_active_hand();
            } else {
                world.play_sound(Sound::ItemShieldBlock, SoundCategory::Players, &player_pos);
            }

            if let Some(player) = caller.get_player() {
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageBlockedByShield as i32,
                    (amount * 10.0).round() as i32,
                );
                if damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_PROJECTILE) {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::DeflectedDamage,
                    );
                }
            }

            let active_hand = self
                .active_hand
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(hand) = *active_hand {
                let slot = EquipmentSlot::from_hand(hand);

                let durability_damage =
                    crate::entity::combat::calculate_shield_durability_damage(amount);
                if durability_damage > 0 {
                    if let Some(player) = caller.get_player() {
                        let broke = player.damage_item_in_slot(&slot, durability_damage);
                        let empty = player
                            .inventory
                            .get_stack_in_hand(match &slot {
                                EquipmentSlot::OffHand(_) => Hand::Left,
                                _ => Hand::Right,
                            })
                            .is_empty();
                        if broke && empty {
                            self.clear_active_hand();
                        }
                    } else {
                        let mut equipment_guard = self
                            .entity_equipment
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if let Some(stack) = equipment_guard.equipment.get_mut(&slot)
                            && stack.damage_item(durability_damage) == DamageResult::Broken
                        {
                            world.send_entity_status(
                                &self.entity,
                                crate::entity::equipment_break_status(&slot),
                                None,
                            );
                            *stack = ItemStack::EMPTY.clone();
                            let broken_stack = stack.clone();
                            drop(equipment_guard);

                            self.send_equipment_changes(&[(slot, broken_stack)]);
                            self.clear_active_hand();
                        }
                    }
                }
            }

            // Vanilla parity: blockUsingItem -> attacker.blockedByItem callback
            if !damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_PROJECTILE) {
                if let Some(direct_attacker) = source.or(cause) {
                    direct_attacker.on_blocked_by_shield(caller, !disable_blocking);
                }
            }

            return false;
        }

        // Vanilla parity: Damage cooldown check (damageCooldownTime > 10.0F && !bypasses_cooldown)
        let bypasses_cooldown_protection =
            damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_COOLDOWN);
        let last_damage = self.last_damage_taken.load();
        let hurt_cd = self.hurt_cooldown.load(Relaxed);

        let Some((damage_to_apply, took_full_damage)) = crate::entity::combat::calculate_cooldown_damage(
            amount,
            last_damage,
            hurt_cd,
            bypasses_cooldown_protection,
        ) else {
            return false;
        };

        self.last_damage_taken.store(amount);
        if took_full_damage {
            self.hurt_cooldown.store(20, Relaxed);
        }

        // Vanilla parity: actuallyHurt
        // 1. Armor damage & absorb on damage_to_apply
        if !is_damages_helmet {
            self.damage_armor_items(caller, damage_to_apply, &damage_type);
        }
        let damage_after_armor =
            self.get_damage_after_armor_absorb(damage_to_apply, &damage_type, cause.or(source));

        let effective_amount = self.get_damage_after_magic_absorb(
            damage_after_armor,
            &damage_type,
            caller,
            cause.or(source),
        );

        let damage_amount = effective_amount.max(0.0);

        // Record the source once the hit is confirmed.
        *self
            .last_damage_type
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(damage_type);
        self.last_damage_stamp.store(world.get_world_age(), Relaxed);

        if took_full_damage {
            // Vanilla LivingEntity.dealDefaultKnockback: calculate horizontal knockback/hurt direction (xd, zd)
            let target_pos = self.entity.pos.load();
            let (is_projectile, projectile_velocity, source_pos) = if let Some(source) = source {
                let ent = source.get_entity();
                let is_proj = crate::entity::projectile::is_projectile(ent.entity_type);
                let vel = ent.velocity.load();
                let pos = ent.pos.load();
                (is_proj, Some(vel), Some(pos))
            } else {
                (false, None, None)
            };
            let (xd, zd) = crate::entity::combat::calculate_knockback_direction(
                target_pos,
                source_pos,
                is_projectile,
                projectile_velocity,
                position,
            );

            world.broadcast_damage_event(
                &self.entity,
                i32::from(damage_type.id),
                source.map(|e| e.get_entity().entity_id),
                cause.map(|e| e.get_entity().entity_id),
                position,
            );

            // Vanilla LivingEntity.java:1245:
            // if (!source.is(DamageTypeTags.NO_IMPACT) && (!blocked || damage > 0.0F)) { this.markHurt(); }
            if !damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_IMPACT) {
                self.entity.mark_hurt();
            }

            // Vanilla LivingEntity.java:1250 & dealDefaultKnockback:
            // boolean fullyBlocked = blocked && damage <= 0.0F;
            // if (!source.is(DamageTypeTags.NO_KNOCKBACK) && !fullyBlocked) {
            //     this.dealDefaultKnockback(source, damage, blocked);
            //     if (!blocked) { this.indicateDamage(xd, zd); }
            // }
            if !damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK) {
                if let Some(server) = world.server.upgrade() {
                    let config = &server.advanced_config.pvp;

                    if config.hurt_animation {
                        let entity_id = self.entity.entity_id;
                        let hurt_yaw = crate::entity::combat::calculate_hurt_direction_yaw(
                            xd,
                            zd,
                            self.entity.yaw.load(),
                        );
                        let hurt_event = SActorEvent {
                            target_runtime_id: VarULong(entity_id as u64),
                            event_id: ActorEventID::Hurt,
                            data: VarInt(0),
                            fire_at_position: None,
                        };
                        let hurt_animation = CHurtAnimation::new(entity_id.into(), hurt_yaw);
                        world.send_to_tracking_players_and_self_editioned(
                            &self.entity,
                            &hurt_animation,
                            &hurt_event,
                        );
                    }
                }

                let resistance = self.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE);
                let attacker_living = source
                    .and_then(|s| s.get_living_entity())
                    .or_else(|| cause.and_then(|c| c.get_living_entity()));
                let extra_kb = attacker_living.map_or(0.0, LivingEntity::get_knockback);
                let total_kb = 0.4 + extra_kb;
                self.entity.apply_knockback(
                    knockback_after_resistance(total_kb as f64, resistance),
                    xd,
                    zd,
                    source.map(|s| s.get_entity()),
                );
            }

            self.entity.hurt_marked.store(true, Ordering::SeqCst);
        }

        let play_sound = took_full_damage;

        // Vanilla parity: actuallyHurt
        let original_damage = damage_amount;
        let current_abs = self.absorption.load();
        let dmg_to_health = (original_damage - current_abs).max(0.0);
        let absorbed_damage = original_damage - dmg_to_health;

        if absorbed_damage > 0.0 {
            let new_abs = (current_abs - absorbed_damage).max(0.0);
            self.set_absorption(new_abs);

            if let Some(player) = caller.get_player() {
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealtAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }

            if let Some(attacker) = cause.or(source) {
                self.last_attacker_id
                    .store(attacker.get_entity().entity_id, Relaxed);
                self.last_attacked_time
                    .store(self.entity.age.load(Relaxed), Relaxed);
            }
        }

        let max_h = self.get_max_health();
        let new_health = (self.health.load() - dmg_to_health).clamp(0.0, max_h);

        if dmg_to_health > 0.0 {
            if let Some(player) = caller.get_player() {
                if damage_type.exhaustion > 0.0 {
                    player.add_exhaustion(damage_type.exhaustion);
                }
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageTaken as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            self.set_health(new_health);

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealt as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            if let Some(attacker) = cause.or(source) {
                let attacker_id = attacker.get_entity().entity_id;
                self.last_attacker_id.store(attacker_id, Relaxed);
                self.last_attacked_time
                    .store(self.entity.age.load(Relaxed), Relaxed);

                let current_tick = self.entity.age.load(Relaxed) as i64;

                // 1. Vanilla LivingEntity.java:1365-1376 (resolvePlayerResponsibleForDamage)
                if attacker.get_player().is_some() {
                    self.last_hurt_by_player_id.store(attacker_id, Relaxed);
                    self.last_hurt_by_player_time.store(current_tick, Relaxed);
                } else if let Some(mob) = attacker.get_mob()
                    && let Some(tamable) = mob.as_tamable()
                    && tamable.is_tame()
                    && let Some(owner_uuid) = tamable.get_owner()
                    && let Some(owner_player) = world.get_player_by_uuid(owner_uuid)
                {
                    // If damage was caused by a tame wolf/animal, credit the owner as last_hurt_by_player
                    let owner_id = owner_player.get_entity().entity_id;
                    self.last_hurt_by_player_id.store(owner_id, Relaxed);
                    self.last_hurt_by_player_time.store(current_tick, Relaxed);
                }

                // 2. Vanilla LivingEntity.java:1357-1363 (resolveMobResponsibleForDamage)
                // Sets last_hurt_by_mob for all living attackers (including players), unless
                // exempt by #minecraft:no_anger or wind_charge with #minecraft:no_anger_from_wind_charge.
                let attacker_is_living = attacker.get_living_entity().is_some();
                let has_no_anger_tag = damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_ANGER);
                let is_wind_charge = damage_type == pumpkin_data::damage::DamageType::WIND_CHARGE;
                let victim_immune_wind_anger = self
                    .entity
                    .entity_type
                    .has_tag(&tag::EntityType::MINECRAFT_NO_ANGER_FROM_WIND_CHARGE);

                if can_cause_mob_anger(
                    attacker_is_living,
                    has_no_anger_tag,
                    is_wind_charge,
                    victim_immune_wind_anger,
                ) {
                    self.last_hurt_by_mob_id.store(attacker_id, Relaxed);
                    self.last_hurt_by_mob_time.store(current_tick, Relaxed);
                }
            }
        }

        if dmg_to_health > 0.0 || absorbed_damage > 0.0 {
            if let Some(attacker) = cause.or(source) {
                // Vanilla parity: Thorns reflection (doPostAttack with EnchantmentTarget.VICTIM)
                if damage_type != DamageType::THORNS
                    && !damage_type.has_tag(&tag::DamageType::MINECRAFT_AVOIDS_GUARDIAN_THORNS)
                {
                    self.apply_thorns_reflection(caller, attacker);
                }
            }
            let current_tick = self.entity.age.load(Relaxed) as i64;
            let fall_location = FallLocation::get_current_fall_location(self, &world);
            let fall_distance = self.fall_distance.load();

            let (left_combat_duration, entered_combat) = {
                let mut tracker = self
                    .combat_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                tracker.record_damage(
                    current_tick,
                    self.health.load() > 0.0 && !self.dead.load(Relaxed),
                    fall_distance,
                    fall_location,
                    damage_type,
                    effective_amount,
                    source,
                    cause,
                )
            };

            if let Some(duration) = left_combat_duration {
                self.on_leave_combat(caller, duration);
            }
            if entered_combat {
                self.on_enter_combat(caller);
            }
        }

        // Vanilla parity: LivingEntity.java:1255-1267:
        // Check totem death protection before playing death sound or triggering on_death.
        if new_health <= 0.0 {
            if self.check_totem_death_protection(caller, damage_type) {
                return true;
            }

            if play_sound {
                let sound = Self::death_sound_for_entity(self.entity.entity_type);
                world.play_sound_fine(
                    sound,
                    self.sound_category(caller.get_player().is_some()),
                    &self.entity.pos.load(),
                    1.0,
                    self.get_pitch(),
                );

                if damage_type == DamageType::THORNS {
                    let category = if caller.get_player().is_some() {
                        SoundCategory::Players
                    } else {
                        SoundCategory::Hostile
                    };
                    world.play_sound_fine(
                        Sound::EnchantThornsHit,
                        category,
                        &self.entity.pos.load(),
                        1.0,
                        self.get_pitch(),
                    );
                }
            }

            let mut death_event =
                crate::plugin::api::events::entity::entity_death::EntityDeathEvent::new(
                    self.entity.entity_id,
                    0,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut death_event);
            }
            self.on_death(damage_type, source, cause);
        } else if play_sound {
            let sound = self.hurt_sound(caller);
            world.play_sound_fine(
                sound,
                self.sound_category(caller.get_player().is_some()),
                &self.entity.pos.load(),
                1.0,
                self.get_pitch(),
            );

            // Vanilla parity: playSecondaryHurtSound for thorns hit
            if damage_type == DamageType::THORNS {
                let category = if caller.get_player().is_some() {
                    SoundCategory::Players
                } else {
                    SoundCategory::Hostile
                };
                world.play_sound_fine(
                    Sound::EnchantThornsHit,
                    category,
                    &self.entity.pos.load(),
                    1.0,
                    self.get_pitch(),
                );
            }

            if let Some(mob) = caller.get_mob() {
                let interval = mob.get_ambient_sound_interval();
                mob.get_mob_entity()
                    .ambient_sound_time
                    .store(-interval, Relaxed);
            }
        }

        if let Some(mob) = caller.get_mob() {
            mob.on_damage(damage_type, source.or(cause));
        }

        true
    }

    pub fn damage(&self, caller: &dyn EntityBase, amount: f32, damage_type: DamageType) -> bool {
        self.damage_with_context(caller, amount, damage_type, None, None, None)
    }
}

impl EntityBase for LivingEntity {
    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, position, source, cause)
    }

    fn tick_in_void(&self, dyn_self: &dyn EntityBase) {
        dyn_self.damage(dyn_self, 4.0, DamageType::OUT_OF_WORLD);
    }

    fn get_gravity(&self) -> f64 {
        self.get_attribute_value(&Attributes::GRAVITY)
    }

    /// Advances the living entity by one tick: base entity tick, movement and
    /// physics while alive (still applied during the 20-tick death animation so
    /// knockback lands), velocity coalescing, status effects, void damage, and
    /// death-animation completion.
    #[allow(clippy::too_many_lines)]
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        self.entity.tick(caller, server);

        // Only tick movement if the entity is alive. This prevents a dead "corpse"
        // from continuing to be simulated (accumulating fall_distance/velocity).
        // We allow movement during death animation (20 ticks) so knockback is applied.
        let is_alive = !self.dead.load(Relaxed) && self.health.load() > 0.0;
        let in_death_animation = self.health.load() <= 0.0 && self.death_time.load(Relaxed) < 20;
        let is_player = self.entity.entity_type == &EntityType::PLAYER;

        // Vanilla `LivingEntity.tick` (LivingEntity.java:2755-2778):
        // Auto-decrement stuck arrows and bee stingers timers
        let (new_arrows, new_arrow_timer) = crate::entity::combat::tick_stuck_projectile_counter(
            self.get_arrow_count(),
            self.remove_arrow_time.load(Ordering::Relaxed),
        );
        if new_arrows != self.get_arrow_count() {
            self.set_arrow_count(new_arrows);
        }
        self.remove_arrow_time.store(new_arrow_timer, Ordering::Relaxed);

        let (new_stingers, new_stinger_timer) = crate::entity::combat::tick_stuck_projectile_counter(
            self.get_stinger_count(),
            self.remove_stinger_time.load(Ordering::Relaxed),
        );
        if new_stingers != self.get_stinger_count() {
            self.set_stinger_count(new_stingers);
        }
        self.remove_stinger_time.store(new_stinger_timer, Ordering::Relaxed);

        // Vanilla `LivingEntity.tick` (LivingEntity.java:2780): every 20 ticks (1s), recheck combat status
        let tick_count = self.entity.age.load(Relaxed);
        if tick_count % 20 == 0 {
            let left_combat_duration = {
                let mut tracker = self
                    .combat_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                tracker.recheck_status(tick_count as i64, is_alive)
            };
            if let Some(duration) = left_combat_duration {
                self.on_leave_combat(caller, duration);
            }
        }

        // Vanilla `ServerEntity.sendChanges` runs before the player's `aiStep`.
        if is_player {
            self.entity.flush_velocity(caller.get_player());
        }
        if (is_alive || in_death_animation) && !is_player {
            self.tick_movement(caller);
            // Vanilla-like order: freeze logic runs after movement/collisions.
            self.entity.tick_frozen(caller);
        } else if is_alive {
            // Client-authoritative players skip `travel`, so decay pushed velocity like
            // vanilla to prevent it accumulating and launching the player.
            self.apply_travel_friction();

            let suffocating = self.entity.tick_block_collisions(caller);
            if suffocating
                && !caller.get_player().is_some_and(|p| p.is_sleeping())
                && self.entity.pose.load() != pumpkin_data::entity::EntityPose::Sleeping
            {
                caller.damage(caller, 1.0, DamageType::IN_WALL);
            }

            // Players push other entities like any living entity.
            self.push_entities(caller);

            self.entity.tick_frozen(caller);
        }
        if !is_player {
            self.entity.flush_velocity(None);
        }

        if is_alive {
            self.breath.tick(self, caller);
        }

        // Fetch supporting blocks for players or other entities
        let supporting_pos = caller.get_player().map_or_else(
            || self.entity.get_supporting_block_pos(),
            super::player::Player::get_supporting_block_pos,
        );

        // Notify the block under the entity each tick if a supporting block position is found
        if self.entity.is_affected_by_blocks()
            && let Some(supporting) = supporting_pos
        {
            let world = self.entity.world.load_full();
            let (block, state) = world.get_block_and_state(&supporting);

            world
                .block_registry
                .on_entity_step(block, &world, caller, &supporting, state, false);

            // Check slightly below supporting_pos for additional supporting blocks (blocks under carpets and the like)
            if !block.is_solid() {
                let below_supporting = supporting.down();
                let (below_block, below_state) = world.get_block_and_state(&below_supporting);

                // If block is not air, notify it as well
                world.block_registry.on_entity_step(
                    below_block,
                    &world,
                    caller,
                    &below_supporting,
                    below_state,
                    true, // below supporting block
                );
            }
        }

        let current_block_pos = self.entity.block_pos.load();
        if is_alive && self.last_block_pos.load() != Some(current_block_pos) {
            self.last_block_pos.store(Some(current_block_pos));
            self.on_changed_block(caller, current_block_pos);
        }

        self.tick_effects();

        let remaining_use_ticks = self.item_use_time.load(Ordering::Relaxed);
        if remaining_use_ticks > 0 {
            let item_in_use = self
                .item_in_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(item) = item_in_use.as_ref() {
                if let Some(player) = caller.get_player() {
                    server
                        .item_registry
                        .on_use_tick(item, player, remaining_use_ticks);
                }

                // Parity Gap 143: Consumable item use ticking effects (Consumable.java:119-124)
                if let Some(consumable) = item.get_data_component::<ConsumableImpl>() {
                    let consume_ticks = consumable.consume_ticks();
                    if crate::entity::combat::should_emit_consumable_use_effects(
                        consume_ticks,
                        remaining_use_ticks,
                    ) {
                        self.emit_consumable_use_effects(item, consumable, 5);
                    }
                }
            }
        }

        // Current active item
        if self.item_use_time.load(Ordering::Relaxed) > 0
            && self.item_use_time.fetch_sub(1, Ordering::Relaxed) <= 1
        {
            let item_in_use = self
                .item_in_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(item) = item_in_use.as_ref() {
                if let Some(food) = item.get_data_component::<FoodImpl>()
                    && let Some(player) = caller.get_player()
                {
                    player
                        .hunger_manager
                        .eat(player, food.nutrition as u8, food.saturation);
                    self.entity.world.load().play_bedrock_level_sound(
                        "burp",
                        &self.entity.pos.load(),
                        -1,
                    );
                }

                // Parity Gap 143: Emit final consumption sound and 16 particles (Consumable.java:79)
                if let Some(consumable) = item.get_data_component::<ConsumableImpl>() {
                    self.emit_consumable_use_effects(item, consumable, 16);
                }

                self.apply_consumable_effects(caller, item);

                // Handle potion consumption
                if item
                    .get_data_component::<pumpkin_data::data_component_impl::PotionContentsImpl>()
                    .is_some()
                {
                    let effects = crate::item::potion::PotionContents::read_potion_effects(item);
                    crate::item::potion::PotionContents::apply_effects_to(
                        self,
                        effects,
                        1.0,
                        crate::item::potion::PotionApplicationSource::Normal,
                    );
                }

                if let Some(player) = caller.get_player() {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::ConsumeItem {
                            item_id: item.item.resource_location().into_owned(),
                        },
                    );

                    // Vanilla `ItemStack.finishUsingItem`: consume one, then apply `use_remainder`.
                    let finish_using = |mut stack: ItemStack| {
                        let stack_count_before_using = stack.item_count;
                        let gamemode = player.gamemode.load();
                        stack.decrement_unless_creative(gamemode, 1);
                        match item.get_data_component::<UseRemainderImpl>() {
                            Some(use_remainder) => use_remainder.convert_into_remainder(
                                stack,
                                stack_count_before_using,
                                gamemode == GameMode::Creative,
                                |mut extra| {
                                    if !player.inventory.insert_stack_anywhere(&mut extra)
                                        && !extra.is_empty()
                                    {
                                        player.drop_item(extra);
                                    }
                                },
                            ),
                            None => stack,
                        }
                    };

                    // Prefer modifying the exact stack that matches the consumed item:
                    // 1) selected hotbar (held_item)
                    // 2) off-hand
                    // 3) fallback to active_hand if the above didn't match
                    let mut handled = false;

                    // Check main hand (hotbar selected)
                    let held = player.inventory.held_item();
                    if held.are_items_and_components_equal(item) {
                        player.inventory.set_held_item(finish_using(held));
                        handled = true;
                    }

                    if !handled {
                        // Check off-hand
                        let off_hand = player.inventory.off_hand_item();
                        if off_hand.are_items_and_components_equal(item) {
                            player
                                .inventory
                                .set_stack_in_hand(Hand::Left, finish_using(off_hand));
                            handled = true;
                        }
                    }

                    if !handled {
                        // Use stored active_hand (as a fallback)
                        let active_hand = *self
                            .active_hand
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let hand_to_modify = active_hand.unwrap_or(Hand::Right);
                        let item_stack = self.get_stack_in_hand(caller, hand_to_modify);
                        player
                            .inventory
                            .set_stack_in_hand(hand_to_modify, finish_using(item_stack));
                    }

                    if let Some(cooldown) = item.get_use_cooldown() {
                        let group = cooldown
                            .cooldown_group
                            .clone()
                            .unwrap_or_else(|| item.item.registry_key.to_string());
                        player.start_cooldown(group, (cooldown.seconds * 20.0) as i32);
                    }
                }

                self.clear_active_hand();
            }
        }

        if self.hurt_cooldown.load(Relaxed) > 0 {
            self.hurt_cooldown.fetch_sub(1, Relaxed);
        }

        // Parity Gap 144: Auto spin attack (Riptide) timer countdown
        // Vanilla LivingEntity.java:3155-3158, 3261-3265
        if self.auto_spin_attack_ticks.load(Relaxed) > 0 {
            let remaining = self.auto_spin_attack_ticks.fetch_sub(1, Relaxed) - 1;
            if remaining <= 0 {
                self.set_living_flag(Self::USING_RIPTIDE_FLAG, false);
            }
        }

        // Parity Gap 145: Elytra gliding decay and auto-exit
        // Vanilla LivingEntity.java:3181-3200
        if self.entity.is_fall_flying() {
            if !self.can_glide() || self.entity.on_ground.load(Relaxed) {
                self.entity.set_fall_flying(false);
                self.fall_fly_ticks.store(0, Relaxed);
            } else {
                let check_ticks = self.fall_fly_ticks.fetch_add(1, Relaxed) + 1;
                if crate::entity::combat::should_damage_elytra(check_ticks) {
                    if let Some(player) = caller.get_player() {
                        player.damage_item_in_slot(&EquipmentSlot::CHEST, 1);
                        if !self.can_glide() {
                            self.entity.set_fall_flying(false);
                            self.fall_fly_ticks.store(0, Relaxed);
                        }
                    }
                }
            }
        } else if self.fall_fly_ticks.load(Relaxed) > 0 {
            self.fall_fly_ticks.store(0, Relaxed);
        }
        if self.health.load() <= 0.0 {
            let time = self
                .death_time
                .fetch_update(Relaxed, Relaxed, |time| Some(time.saturating_add(1)))
                .unwrap_or_else(|time| time)
                .saturating_add(1);
            if self.entity.entity_type == &EntityType::PLAYER {
                // Bedrock keeps a dead remote player actor in its death pose.
                // Remove Java players after the animation so respawn can
                // recreate a live, interactable actor with the same identity.
                if time == 10 {
                    self.entity
                        .world
                        .load()
                        .despawn_dead_java_player_for_bedrock(&self.entity);
                }
                // Players remain part of the world until their client requests a
                // respawn. Removing one here breaks reconnecting while dead.
                return;
            }
            // Vanilla `LivingEntity.tickDeath` sends the POOF (60) particle event
            // once the death animation finished; the death event (3) was already
            // broadcast in `on_death`. Sending it again here would restart the
            // client-side death animation.
            if time >= 20 && !self.entity.removed.swap(true, Ordering::Relaxed) {
                self.entity
                    .world
                    .load()
                    .send_entity_status(&self.entity, EntityStatus::Poof, None);
                self.entity.remove();
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(self)
    }

    fn is_pushable(&self) -> bool {
        self.health.load() > 0.0 && !self.dead.load(Relaxed) && self.collides.load(Relaxed)
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub const SPEED_MODIFIER_SPRINTING_ID: &str = "minecraft:sprinting";
pub const SPEED_MODIFIER_SPRINTING_AMOUNT: f64 = 0.300_000_011_920_928_96;

impl LivingEntity {
    pub fn set_sprinting(&self, is_sprinting: bool) {
        self.entity.set_sprinting(is_sprinting);
        self.update_attribute(&Attributes::MOVEMENT_SPEED, |speed| {
            speed.remove_modifier(SPEED_MODIFIER_SPRINTING_ID);
            if is_sprinting {
                speed.add_or_replace_modifier(Modifier {
                    id: SPEED_MODIFIER_SPRINTING_ID.to_string(),
                    amount: SPEED_MODIFIER_SPRINTING_AMOUNT,
                    operation: ModifierOperation::MultiplyTotal,
                    permanent: false,
                });
            }
        });
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MOVEMENT_SPEED],
        );
    }

    #[must_use]
    pub fn get_block_speed_factor(&self) -> f32 {
        let efficiency = self.get_attribute_value(&Attributes::MOVEMENT_EFFICIENCY) as f32;
        let super_factor = self.entity.get_block_speed_factor();
        super_factor + efficiency * (1.0 - super_factor)
    }

    /// Applies data-driven `apply_effects` consume effects after an item completes use.
    /// Vanilla: `Consumable.onConsume` invokes every configured effect server-side.
    fn apply_consumable_effects(&self, caller: &dyn EntityBase, item: &ItemStack) {
        let Some(consumable) = item.get_data_component::<ConsumableImpl>() else {
            return;
        };

        for consume_effect in consumable.effects.iter() {
            match consume_effect {
                ConsumeEffect::ApplyEffects((effects, probability)) => {
                    if !consume_effect_probability_applies(*probability, rand::random()) {
                        continue;
                    }

                    for effect in effects.iter() {
                        let Some(effect_type) =
                            StatusEffect::from_minecraft_name(&effect.effect_id)
                        else {
                            continue;
                        };
                        let Ok(amplifier) = u8::try_from(effect.amplifier) else {
                            continue;
                        };

                        self.add_effect(Effect {
                            effect_type,
                            duration: effect.duration,
                            amplifier,
                            ambient: effect.ambient,
                            show_particles: effect.show_particles,
                            show_icon: effect.show_icon,
                            blend: false,
                        });
                    }
                }
                ConsumeEffect::ClearAllEffects => {
                    self.reset_effects_and_attributes();
                }
                ConsumeEffect::RemoveEffects(idset) => {
                    if let pumpkin_data::data_component_impl::IDSet::IDs(ids) = idset {
                        for effect_type in ids.iter() {
                            self.remove_effect(effect_type);
                        }
                    }
                }
                ConsumeEffect::TeleportRandomly(diameter) => {
                    // Java Edition dismounts the consumer before random teleport attempts.
                    let vehicle = caller
                        .get_entity()
                        .vehicle
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if let Some(vehicle) = vehicle {
                        vehicle
                            .get_entity()
                            .remove_passenger_sync(caller.get_entity().entity_id);
                        if caller.get_entity().has_vehicle() {
                            continue;
                        }
                    }

                    let center = self.entity.pos.load();
                    let Some(pos) = self.find_random_teleport_target(*diameter) else {
                        continue;
                    };
                    let (yaw, pitch) = (self.entity.yaw.load(), self.entity.pitch.load());
                    let world = self.entity.world.load_full();
                    caller.teleport(pos, Some(yaw), Some(pitch), world.clone());

                    let destination = self.entity.pos.load();
                    if destination != center {
                        self.fall_distance.store(0.0);
                        // Vanilla broadcasts entity event 46 (teleport particles) on success.
                        world.send_entity_status(&self.entity, EntityStatus::Teleport, None);
                        world.emit_game_event("teleport", center);
                        world.play_sound(
                            Sound::ItemChorusFruitTeleport,
                            SoundCategory::Players,
                            &destination,
                        );

                        let origin_pos = center.to_block_pos();
                        let target_pos = destination.to_block_pos();
                        let x_diff = (target_pos.0.x - origin_pos.0.x).clamp(-127, 127);
                        let y_diff = (target_pos.0.y - origin_pos.0.y).clamp(-127, 127);
                        let z_diff = (target_pos.0.z - origin_pos.0.z).clamp(-127, 127);
                        let packed_data = (((x_diff + 127) as u32 & 0xFF) << 16)
                            | (((y_diff + 127) as u32 & 0xFF) << 8)
                            | ((z_diff + 127) as u32 & 0xFF);
                        world.sync_world_event(
                            pumpkin_data::world::WorldEvent::ParticlesConsumeEffectTeleport,
                            origin_pos,
                            packed_data as i32,
                        );
                    }
                }
                ConsumeEffect::PlaySound(sound_event) => {
                    let sound_category = if self.entity.entity_type == &EntityType::PLAYER {
                        SoundCategory::Players
                    } else {
                        SoundCategory::Neutral
                    };
                    let pos = self.entity.pos.load();
                    self.entity.world.load().play_sound_event_fine(
                        sound_event,
                        sound_category,
                        &pos,
                        1.0,
                        1.0,
                    );
                }
            }
        }
    }

    /// Emits consumable eating/drinking sounds and item particles.
    ///
    /// Mirrors vanilla `Consumable.emitParticlesAndSounds`:
    /// ```java
    /// if (this.hasConsumeParticles) {
    ///     user.spawnItemParticles(itemStack, particleCount);
    /// }
    /// user.playSound(consumeSound, consumableVolume, consumablePitch);
    /// ```
    pub fn emit_consumable_use_effects(
        &self,
        item: &ItemStack,
        consumable: &pumpkin_data::data_component_impl::ConsumableImpl,
        particle_count: u32,
    ) {
        let is_drink = consumable.animation
            == pumpkin_data::data_component_impl::ConsumeAnimation::Drink;
        let mut rng = rand::rng();
        let (volume, pitch) = crate::entity::combat::calculate_consumable_sound_volume_and_pitch(
            is_drink,
            rng.random_bool(0.5),
            rng.random::<f32>(),
            rng.random::<f32>(),
        );

        let sound_category = if self.entity.entity_type == &EntityType::PLAYER {
            SoundCategory::Players
        } else {
            SoundCategory::Neutral
        };

        let pos = self.entity.pos.load();
        self.entity.world.load().play_sound_event_fine(
            &consumable.sound_event,
            sound_category,
            &pos,
            volume,
            pitch,
        );

        if consumable.consume_particles && particle_count > 0 {
            // Vanilla LivingEntity.java:3550-3566:
            // Particles spawn slightly in front of and around eye level
            let eye_y = pos.y + self.entity.get_eye_height();
            let particle_pos = Vector3::new(pos.x, eye_y - 0.2, pos.z);
            self.entity.world.load().spawn_particles(
                pumpkin_data::particle::Particle::Item,
                particle_pos,
                particle_count,
                Vector3::new(0.18, 0.18, 0.18),
                0.05,
            );
        }
        let _ = item;
    }

    fn find_random_teleport_target(&self, diameter: f32) -> Option<Vector3<f64>> {
        let center = self.entity.pos.load();
        let world = self.entity.world.load();
        let bottom_y = world.get_bottom_y();
        let top_y = world.get_top_y();
        let dimensions = self.entity.entity_dimension.load();
        let mut rng = rand::rng();

        'attempts: for _ in 0..Self::RANDOM_TELEPORT_ATTEMPTS {
            let target_x = random_teleport_coordinate(center.x, diameter, rng.random());
            let target_z = random_teleport_coordinate(center.z, diameter, rng.random());
            let sampled_y = random_teleport_coordinate(center.y, diameter, rng.random())
                .clamp(f64::from(bottom_y + 1), f64::from(top_y));
            let mut block_y = sampled_y.floor() as i32;
            let block_x = target_x.floor() as i32;
            let block_z = target_z.floor() as i32;

            loop {
                if block_y <= bottom_y {
                    continue 'attempts;
                }

                let below = BlockPos::new(block_x, block_y - 1, block_z);
                let Some(below_state) = world.get_block_state_if_loaded(&below) else {
                    continue 'attempts;
                };
                if below_state.is_solid() {
                    break;
                }
                block_y -= 1;
            }

            let target = Vector3::new(target_x, f64::from(block_y), target_z);
            let bounding_box = BoundingBox::new_from_pos(target.x, target.y, target.z, &dimensions);

            for block_pos in
                BlockPos::iterate(bounding_box.min_block_pos(), bounding_box.max_block_pos())
            {
                if world.get_block_state_if_loaded(&block_pos).is_none()
                    || world.get_fluid(&block_pos).id != Fluid::EMPTY.id
                {
                    continue 'attempts;
                }
            }

            if world.is_space_empty(bounding_box) {
                return Some(target);
            }
        }

        None
    }
}

fn random_teleport_coordinate(center: f64, diameter: f32, random: f64) -> f64 {
    center + (random - 0.5) * f64::from(diameter)
}

fn attributes_by_id(id: u8) -> Option<&'static Attributes> {
    Attributes::ALL.iter().find(|attr| attr.id == id)
}

fn push_unique_attribute(touched: &mut Vec<Attributes>, attr: &Attributes) {
    if !touched.iter().any(|existing| existing.id == attr.id) {
        touched.push(attr.clone());
    }
}

const fn attribute_modifier_slot_matches(
    modifier_slot: &AttributeModifierSlot,
    equipment_slot: &EquipmentSlot,
) -> bool {
    match modifier_slot {
        AttributeModifierSlot::Any => true,
        AttributeModifierSlot::MainHand => matches!(equipment_slot, EquipmentSlot::MainHand(_)),
        AttributeModifierSlot::OffHand => matches!(equipment_slot, EquipmentSlot::OffHand(_)),
        AttributeModifierSlot::Hand => {
            matches!(
                equipment_slot,
                EquipmentSlot::MainHand(_) | EquipmentSlot::OffHand(_)
            )
        }
        AttributeModifierSlot::Feet => matches!(equipment_slot, EquipmentSlot::Feet(_)),
        AttributeModifierSlot::Legs => matches!(equipment_slot, EquipmentSlot::Legs(_)),
        AttributeModifierSlot::Chest => matches!(equipment_slot, EquipmentSlot::Chest(_)),
        AttributeModifierSlot::Head => matches!(equipment_slot, EquipmentSlot::Head(_)),
        AttributeModifierSlot::Armor => matches!(
            equipment_slot,
            EquipmentSlot::Feet(_)
                | EquipmentSlot::Legs(_)
                | EquipmentSlot::Chest(_)
                | EquipmentSlot::Head(_)
        ),
        AttributeModifierSlot::Body => matches!(equipment_slot, EquipmentSlot::Body(_)),
        AttributeModifierSlot::Saddle => matches!(equipment_slot, EquipmentSlot::Saddle(_)),
    }
}

/// Mirrors vanilla's strict `random < probability` consume-effect gate.
const fn consume_effect_probability_applies(probability: f32, random: f32) -> bool {
    random < probability
}

#[cfg(test)]
mod consumable_effect_tests {
    use super::{consume_effect_probability_applies, random_teleport_coordinate};

    #[test]
    fn consumable_effect_probability_matches_vanilla_strict_threshold() {
        assert!(!consume_effect_probability_applies(0.0, 0.0));
        assert!(consume_effect_probability_applies(1.0, 0.999));
        assert!(consume_effect_probability_applies(0.5, 0.499));
        assert!(!consume_effect_probability_applies(0.5, 0.5));
    }

    #[test]
    fn random_teleport_coordinate_uses_full_diameter() {
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.0), 2.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.5), 10.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 1.0), 18.0);
    }
}
/// Returns `true` if `damage_type` is in `#minecraft:bypasses_armor` (1.21.11).
/// These sources bypass armor entirely (fall, drown, freeze, etc.).
#[allow(dead_code)]
pub(crate) const fn bypasses_armor_durability(damage_type: &DamageType) -> bool {
    // Bitmask lookup: O(1) with two instructions (shift + AND), no array scan.
    // DamageType IDs can exceed 31; use u64 for sufficient range.
    // TODO: Make data-driven once the data pack system can handle it without performance regressions.
    // Compile-time assertions: ensure all bypassing types fit in u64 bitmask.
    const _: () = assert!(
        DamageType::FALL.id < 64
            && DamageType::FLY_INTO_WALL.id < 64
            && DamageType::ON_FIRE.id < 64
            && DamageType::IN_WALL.id < 64
            && DamageType::CRAMMING.id < 64
            && DamageType::DROWN.id < 64
            && DamageType::GENERIC.id < 64
            && DamageType::WITHER.id < 64
            && DamageType::DRAGON_BREATH.id < 64
            && DamageType::STARVE.id < 64
            && DamageType::ENDER_PEARL.id < 64
            && DamageType::FREEZE.id < 64
            && DamageType::STALAGMITE.id < 64
            && DamageType::MAGIC.id < 64
            && DamageType::INDIRECT_MAGIC.id < 64
            && DamageType::OUT_OF_WORLD.id < 64
            && DamageType::GENERIC_KILL.id < 64
            && DamageType::SONIC_BOOM.id < 64
            && DamageType::OUTSIDE_BORDER.id < 64,
        "One or more bypass DamageType IDs exceed u64 bitmask width (>= 64)"
    );
    const BYPASS_MASK: u64 = (1u64 << DamageType::FALL.id)
        | (1u64 << DamageType::FLY_INTO_WALL.id)
        | (1u64 << DamageType::ON_FIRE.id)
        | (1u64 << DamageType::IN_WALL.id)
        | (1u64 << DamageType::CRAMMING.id)
        | (1u64 << DamageType::DROWN.id)
        | (1u64 << DamageType::GENERIC.id)
        | (1u64 << DamageType::WITHER.id)
        | (1u64 << DamageType::DRAGON_BREATH.id)
        | (1u64 << DamageType::STARVE.id)
        | (1u64 << DamageType::ENDER_PEARL.id)
        | (1u64 << DamageType::FREEZE.id)
        | (1u64 << DamageType::STALAGMITE.id)
        | (1u64 << DamageType::MAGIC.id)
        | (1u64 << DamageType::INDIRECT_MAGIC.id)
        | (1u64 << DamageType::OUT_OF_WORLD.id)
        | (1u64 << DamageType::GENERIC_KILL.id)
        | (1u64 << DamageType::SONIC_BOOM.id)
        | (1u64 << DamageType::OUTSIDE_BORDER.id);
    (damage_type.id < 64) && ((BYPASS_MASK >> damage_type.id) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bypasses_armor_durability ─────────────────────────────────────

    /// Every member of `minecraft:bypasses_armor` (1.21.11) must return `true`.
    #[test]
    fn bypasses_armor_durability_returns_true_for_tag_members() {
        // Exact contents of the minecraft:bypasses_armor tag in 1.21.11.
        let bypassing: &[DamageType] = &[
            DamageType::ON_FIRE,
            DamageType::IN_WALL,
            DamageType::CRAMMING,
            DamageType::DROWN,
            DamageType::FLY_INTO_WALL,
            DamageType::GENERIC,
            DamageType::WITHER,
            DamageType::DRAGON_BREATH,
            DamageType::STARVE,
            DamageType::FALL,
            DamageType::ENDER_PEARL,
            DamageType::FREEZE,
            DamageType::STALAGMITE,
            DamageType::MAGIC,
            DamageType::INDIRECT_MAGIC,
            DamageType::OUT_OF_WORLD,
            DamageType::GENERIC_KILL,
            DamageType::SONIC_BOOM,
            DamageType::OUTSIDE_BORDER,
        ];
        for dt in bypassing {
            assert!(
                bypasses_armor_durability(dt),
                "{} should bypass armor durability",
                dt.message_id
            );
        }
    }

    /// Physical/combat damage types must NOT bypass armor durability.
    #[test]
    fn bypasses_armor_durability_returns_false_for_physical_sources() {
        let physical: &[DamageType] = &[
            DamageType::MOB_ATTACK,
            DamageType::PLAYER_ATTACK,
            DamageType::ARROW,
            DamageType::CACTUS,
            DamageType::SWEET_BERRY_BUSH,
            DamageType::LAVA,
            DamageType::EXPLOSION,
            DamageType::PLAYER_EXPLOSION,
            DamageType::LIGHTNING_BOLT,
            DamageType::FIREBALL,
            DamageType::THORNS,
            DamageType::TRIDENT,
        ];
        for dt in physical {
            assert!(
                !bypasses_armor_durability(dt),
                "{} should NOT bypass armor durability",
                dt.message_id
            );
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_zombie_family_sounds() {
        let cases = [
            (&EntityType::ZOMBIE, Sound::EntityZombieHurt),
            (&EntityType::DROWNED, Sound::EntityDrownedHurt),
            (&EntityType::HUSK, Sound::EntityHuskHurt),
            (
                &EntityType::ZOMBIE_VILLAGER,
                Sound::EntityZombieVillagerHurt,
            ),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_enderman_hurt_sound() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ENDERMAN),
            Sound::EntityEndermanHurt
        );
    }

    #[test]
    fn hurt_sound_for_entity_uses_skeleton_family_sounds() {
        let cases = [
            (&EntityType::SKELETON, Sound::EntitySkeletonHurt),
            (&EntityType::BOGGED, Sound::EntityBoggedHurt),
            (&EntityType::PARCHED, Sound::EntityParchedHurt),
            (
                &EntityType::WITHER_SKELETON,
                Sound::EntityWitherSkeletonHurt,
            ),
            (&EntityType::STRAY, Sound::EntityStrayHurt),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_defaults_to_generic_hurt() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ITEM),
            Sound::EntityGenericHurt
        );
    }

    #[test]
    fn regeneration_particle_metadata_uses_vanilla_argb_color() {
        let effect = Effect {
            effect_type: &StatusEffect::REGENERATION,
            duration: 200,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        };
        let metadata = pumpkin_protocol::java::client::play::Metadata::new(
            tracked_data::living_entity::EFFECT_PARTICLES,
            EffectParticles(vec![EffectParticle::from_effect(&effect)]),
        );
        let mut bytes = Vec::new();

        metadata
            .write(
                &mut bytes,
                &pumpkin_util::version::JavaMinecraftVersion::V_26_3,
            )
            .unwrap();

        assert_eq!(bytes, [10, 17, 1, 28, 0xff, 0xcd, 0x5c, 0xab]);
    }
}
