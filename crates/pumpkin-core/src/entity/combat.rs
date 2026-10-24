use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::entity::EntityBase;
use crate::entity::decoration::armor_stand::ArmorStandEntity;
use crate::entity::living::LivingEntity;
use pumpkin_data::data_component_impl::EnchantmentsImpl;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{
    attributes::Attributes,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::vector3::Vector3;

use crate::{
    entity::{Entity, player::Player},
    world::World,
};

use crate::net::ClientPlatform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackType {
    Knockback,
    Critical,
    Sweeping,
    Strong,
    Weak,
}

impl AttackType {
    pub fn new(player: &Player, victim: &dyn EntityBase, attack_cooldown_progress: f32) -> Self {
        let entity = &player.get_entity();

        let sprinting = entity.is_sprinting();
        let on_ground = entity.on_ground.load(Ordering::Relaxed);
        let held_item = player.inventory().held_item();

        let sword = held_item.is_sword();
        let is_bedrock = matches!(player.client.as_ref(), ClientPlatform::Bedrock(_));

        let is_strong = attack_cooldown_progress > 0.9;
        if sprinting && is_strong {
            return Self::Knockback;
        }

        if can_critical_attack(player, victim, attack_cooldown_progress) {
            return Self::Critical;
        }

        let speed = player
            .living_entity
            .get_attribute_value(&Attributes::MOVEMENT_SPEED);
        let vel = entity.velocity.load();
        let horizontal_speed_sq = vel.x * vel.x + vel.z * vel.z;

        // Vanilla Player.isSweepAttack: fullStrengthAttack && !criticalAttack && !knockbackAttack && this.onGround()
        if can_sweep_attack_raw(
            sword,
            is_strong,
            false,
            false,
            on_ground,
            horizontal_speed_sq,
            speed,
            is_bedrock,
        ) {
            return Self::Sweeping;
        }

        if is_strong { Self::Strong } else { Self::Weak }
    }
}

/// Checks critical attack eligibility based on raw physics and status flags,
/// mirroring vanilla `Player.canCriticalAttack(Entity)`.
#[must_use]
pub fn can_critical_attack_raw(
    sprinting: bool,
    on_ground: bool,
    fall_distance: f32,
    on_climbable: bool,
    is_in_water: bool,
    is_blind: bool,
    is_passenger: bool,
    is_victim_living: bool,
    attack_cooldown_progress: f32,
) -> bool {
    let is_strong = attack_cooldown_progress > 0.9;
    is_strong
        && fall_distance > 0.0
        && !on_ground
        && !on_climbable
        && !is_in_water
        && !is_blind
        && !is_passenger
        && is_victim_living
        && !sprinting
}

/// Vanilla `Player.canCriticalAttack(Entity)`.
#[must_use]
pub fn can_critical_attack(
    player: &Player,
    victim: &dyn EntityBase,
    attack_cooldown_progress: f32,
) -> bool {
    let entity = &player.get_entity();
    can_critical_attack_raw(
        entity.is_sprinting(),
        entity.on_ground.load(Ordering::Relaxed),
        player.living_entity.fall_distance.load(),
        player.on_climbable(),
        entity.is_in_water(),
        player.living_entity.has_effect(&pumpkin_data::effect::StatusEffect::BLINDNESS),
        player.is_passenger(),
        victim.get_living_entity().is_some(),
        attack_cooldown_progress,
    )
}

/// Checks sweep attack eligibility based on raw physics and status flags,
/// mirroring vanilla `Player.isSweepAttack(boolean, boolean, boolean)`.
#[must_use]
pub fn can_sweep_attack_raw(
    is_sword: bool,
    is_strong: bool,
    is_critical: bool,
    is_knockback: bool,
    on_ground: bool,
    horizontal_speed_sq: f64,
    movement_speed: f64,
    is_bedrock: bool,
) -> bool {
    if is_bedrock {
        return false;
    }
    let max_speed = movement_speed * 2.5;
    is_sword
        && is_strong
        && !is_critical
        && !is_knockback
        && on_ground
        && horizontal_speed_sq < max_speed * max_speed
}

/// Checks smash eligibility, rejecting stale fall distance after a ground-only packet.
pub fn can_smash_attack(attacker: &LivingEntity) -> bool {
    attacker.fall_distance.load() > 1.5
        && !attacker.entity.on_ground.load(Ordering::Relaxed)
        && !attacker.entity.is_fall_flying()
}

/// Vanilla `MaceItem.getAttackDamageBonus`, before the enchantment part.
pub fn mace_smash_damage_bonus(fall_distance: f64) -> f64 {
    if fall_distance <= 3.0 {
        4.0 * fall_distance
    } else if fall_distance <= 8.0 {
        2.0f64.mul_add(fall_distance - 3.0, 12.0)
    } else {
        22.0 + fall_distance - 8.0
    }
}

/// Vanilla `MaceItem.knockback`: pushes everything near the victim away from it.
pub fn mace_smash_knockback(world: &World, attacker: &dyn EntityBase, victim: &dyn EntityBase) {
    let victim_entity = victim.get_entity();
    world.sync_world_event(
        WorldEvent::ParticlesSmashAttack,
        victim_entity.get_on_pos(),
        750,
    );

    let victim_pos = victim_entity.pos.load();
    let heavy = attacker
        .get_living_entity()
        .is_some_and(|living| living.fall_distance.load() > 5.0);
    let search_box = victim_entity.bounding_box.load().expand(3.5, 3.5, 3.5);
    let nearby_entities = world.get_entities_at_box(&search_box).into_iter().chain(
        world
            .get_players_at_box(&search_box)
            .into_iter()
            .map(|player| player as Arc<dyn EntityBase>),
    );

    for nearby in nearby_entities {
        let Some(nearby_living) = nearby.get_living_entity() else {
            continue;
        };
        if !mace_smash_can_knockback(attacker, victim, nearby.as_ref()) {
            continue;
        }
        let direction = nearby.get_entity().pos.load() - victim_pos;
        let resistance = nearby_living.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE);
        let power = (3.5 - direction.length())
            * f64::from(0.7f32)
            * if heavy { 2.0 } else { 1.0 }
            * (1.0 - resistance);
        if power > 0.0 {
            let push = direction.normalize() * power;
            let velocity = Vector3::new(push.x, f64::from(0.7f32), push.z);
            let entity = nearby.get_entity();
            if let Some(player) = nearby.get_player() {
                player.set_velocity(entity.velocity.load() + velocity);
            } else {
                entity.add_velocity(velocity);
            }
        }
    }
}

fn mace_smash_can_knockback(
    attacker: &dyn EntityBase,
    victim: &dyn EntityBase,
    nearby: &dyn EntityBase,
) -> bool {
    let nearby_entity = nearby.get_entity();
    let victim_entity = victim.get_entity();
    if nearby.is_spectator()
        || nearby_entity.entity_id == attacker.get_entity().entity_id
        || nearby_entity.entity_id == victim_entity.entity_id
        || attacker.is_allied_to(nearby)
    {
        return false;
    }
    // Vanilla checks the owner against the victim here, not the attacker.
    if let Some(mob) = nearby.get_mob()
        && mob.is_tamed()
        && victim.get_living_entity().is_some()
        && mob.get_owner_uuid() == Some(victim_entity.entity_uuid)
    {
        return false;
    }
    if nearby
        .cast_any()
        .downcast_ref::<ArmorStandEntity>()
        .is_some_and(ArmorStandEntity::is_marker)
    {
        return false;
    }
    if victim_entity
        .pos
        .load()
        .squared_distance_to_vec(&nearby_entity.pos.load())
        > 3.5 * 3.5
    {
        return false;
    }
    !nearby.get_player().is_some_and(|player| {
        player.is_creative()
            && player
                .abilities
                .lock()
                .is_ok_and(|abilities| abilities.flying)
    })
}

/// Scales a knockback `strength` by a living entity's knockback resistance,
/// mirroring vanilla `LivingEntity.knockback`: `strength *= 1.0 - resistance`.
/// A resistance of 1.0 (iron golem, warden, ...) cancels the knockback entirely.
pub fn knockback_after_resistance(strength: f64, resistance: f64) -> f64 {
    strength * (1.0 - resistance)
}

/// Calculates the horizontal knockback and hurt animation direction (xd, zd)
/// mirroring Vanilla `LivingEntity.dealDefaultKnockback`.
#[must_use]
pub fn calculate_knockback_direction(
    target_pos: Vector3<f64>,
    source_pos: Option<Vector3<f64>>,
    is_projectile: bool,
    projectile_velocity: Option<Vector3<f64>>,
    damage_position: Option<Vector3<f64>>,
) -> (f64, f64) {
    if is_projectile {
        if let Some(vel) = projectile_velocity {
            if vel.x.mul_add(vel.x, vel.z * vel.z) > 1e-6 {
                return (-vel.x, -vel.z);
            }
        }
    }
    if let Some(pos) = source_pos {
        (pos.x - target_pos.x, pos.z - target_pos.z)
    } else if let Some(pos) = damage_position {
        (pos.x - target_pos.x, pos.z - target_pos.z)
    } else {
        (0.0, 0.0)
    }
}

/// Calculates the hurt animation yaw relative to entity yaw
/// mirroring Vanilla `ServerPlayer.indicateDamage`:
/// `(float)(Mth.atan2(zd, xd) * 180.0F / (float)Math.PI - this.getYRot())`.
#[must_use]
pub fn calculate_hurt_direction_yaw(xd: f64, zd: f64, entity_yaw: f32) -> f32 {
    if xd == 0.0 && zd == 0.0 {
        -entity_yaw
    } else {
        (zd.atan2(xd).to_degrees() as f32) - entity_yaw
    }
}

/// Calculates the attack strength scale, mirroring Vanilla `Player.getAttackStrengthScale`:
/// `Mth.clamp((this.attackStrengthTicker + a) / this.getCurrentItemAttackStrengthDelay(), 0.0F, 1.0F)`.
#[must_use]
pub fn calculate_attack_cooldown_progress(
    attack_ticks: u32,
    base_time: f64,
    tps: f64,
    attack_speed: f64,
) -> f64 {
    let x = f64::from(attack_ticks) + base_time;
    let progress_per_tick = tps / attack_speed;
    (x / progress_per_tick).clamp(0.0, 1.0)
}

/// Calculates the base damage scale factor from attack strength scale,
/// mirroring Vanilla `Player.baseDamageScaleFactor()`:
/// `0.2F + attackStrengthScale * attackStrengthScale * 0.8F`.
#[must_use]
pub fn calculate_base_damage_scale_factor(attack_strength_scale: f64) -> f64 {
    attack_strength_scale.powi(2).mul_add(0.8, 0.2)
}

/// Calculates total melee attack damage mirroring Vanilla `Player.attack`:
/// `baseDamage *= baseDamageScaleFactor();`
/// `if (criticalAttack) { baseDamage *= 1.5F; }`
/// `totalDamage = baseDamage + magicBoost;`
///
/// In vanilla Minecraft, enchantment damage (magicBoost) is NOT multiplied
/// by the 1.5x critical hit multiplier. Only weapon base damage crits.
#[must_use]
pub fn calculate_player_attack_damage(
    base_weapon_damage: f64,
    attack_cooldown_progress: f64,
    bonus_item_damage: f64,
    extra_enchantment_damage: f64,
    is_critical: bool,
) -> f64 {
    let damage_multiplier = if attack_cooldown_progress < 1.0 {
        calculate_base_damage_scale_factor(attack_cooldown_progress)
    } else {
        1.0
    };
    let mut base_damage = (base_weapon_damage * damage_multiplier).max(0.0);
    base_damage += bonus_item_damage;
    if is_critical {
        base_damage *= 1.5;
    }
    let magic_boost = extra_enchantment_damage * attack_cooldown_progress;
    (base_damage + magic_boost).max(0.0)
}

pub fn handle_knockback(attacker: &Entity, victim: &dyn EntityBase, strength: f64) {
    let resistance = victim.get_living_entity().map_or(0.0, |living| {
        living.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE)
    });
    let strength = knockback_after_resistance(strength * 0.5, resistance);

    if strength > 0.0 {
        let yaw = attacker.yaw.load();
        victim.get_entity().apply_knockback(
            strength,
            f64::from((yaw.to_radians()).sin()),
            f64::from(-(yaw.to_radians()).cos()),
            Some(attacker),
        );
    }

    let velocity = attacker.velocity.load();
    attacker.velocity.store(velocity.multiply(0.6, 1.0, 0.6));
}

/// Vanilla `Player.doSweepAttack` damage calculation:
/// `1.0F + (float)this.getAttributeValue(Attributes.SWEEPING_DAMAGE_RATIO) * baseDamage`.
#[must_use]
pub fn calculate_sweeping_damage(base_damage: f32, sweeping_ratio: f32) -> f32 {
    1.0 + sweeping_ratio * base_damage
}

pub fn spawn_sweep_particle(attacker_entity: &Entity, world: &World) {
    let yaw = attacker_entity.yaw.load();
    let dx = -f64::from((yaw.to_radians()).sin());
    let dz = f64::from((yaw.to_radians()).cos());
    let attacker_pos = attacker_entity.pos.load();

    let scale = 0.5;
    let body_y = f64::from(attacker_entity.height()).mul_add(scale, attacker_pos.y);

    world.spawn_particle(
        Vector3::new(attacker_pos.x + dx, body_y, attacker_pos.z + dz),
        Vector3::new(dx as f32, 0.0, dz as f32),
        0.0,
        0,
        Particle::SweepAttack,
    );
}

/// Vanilla `Player.deflectProjectile`: checks whether an entity is a redirectable projectile
/// (`#minecraft:redirectable_projectile`).
#[must_use]
pub fn is_redirectable_projectile(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    entity_type.has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_REDIRECTABLE_PROJECTILE)
}

/// Vanilla `Player.damageStatsAndHearts`:
/// When actual damage dealt to an entity exceeds 2.0 (more than 1 heart),
/// server spawns `(actual_damage * 0.5) as i32` cracked heart indicator particles.
#[must_use]
pub fn damage_indicator_particle_count(actual_damage: f32) -> Option<i32> {
    if actual_damage > 2.0 {
        Some((actual_damage * 0.5) as i32)
    } else {
        None
    }
}

/// Vanilla `LivingEntity.hurtServer` damage cooldown logic:
/// If within hurt immunity cooldown (`damageCooldownTime > 10.0F` and not bypassing cooldown):
/// - Incoming damage <= last_damage: rejected (`None`).
/// - Incoming damage > last_damage: difference `(damage - last_damage)` dealt, without replaying
///   hurt animations, sounds, or knockback (`Some((diff, false))`).
/// Outside cooldown: full damage applied with full animations, sound, and knockback (`Some((damage, true))`).
#[must_use]
pub fn calculate_cooldown_damage(
    raw_damage: f32,
    last_damage: f32,
    hurt_cooldown: i32,
    bypasses_cooldown: bool,
) -> Option<(f32, bool)> {
    if hurt_cooldown > 10 && !bypasses_cooldown {
        if raw_damage <= last_damage {
            None
        } else {
            Some((raw_damage - last_damage, false))
        }
    } else {
        Some((raw_damage, true))
    }
}

/// Vanilla `LivingEntity.applyItemBlocking`:
/// Determines if an incoming attack from `damage_pos` can be blocked by the defender's shield.
/// Horizontal blocking angle is 90 degrees (`PI / 2`).
/// The vector to the damage source is projected onto the horizontal plane (`y = 0.0`)
/// and normalized, then dotted with the defender's look vector (`calculateViewVector(0.0, head_yaw)`).
/// If `dot >= 0.0` (i.e. angle <= 90 degrees), the hit is within the shield arc.
#[must_use]
pub fn can_block_damage_source(
    entity_pos: Vector3<f64>,
    head_yaw: f32,
    damage_pos: Option<Vector3<f64>>,
) -> bool {
    let Some(source_pos) = damage_pos else {
        return false;
    };
    let dx = source_pos.x - entity_pos.x;
    let dz = source_pos.z - entity_pos.z;
    let horizontal_dist_sq = dx * dx + dz * dz;
    if horizontal_dist_sq <= 1e-12 {
        // Source directly at or above/below the defender
        return true;
    }
    let inv_dist = 1.0 / horizontal_dist_sq.sqrt();
    let norm_dx = dx * inv_dist;
    let norm_dz = dz * inv_dist;

    // Vanilla calculateViewVector(0.0F, getYHeadRot()):
    // realYRot = -yRot * PI / 180.0
    // vx = sin(-yRot) = -sin(yRot)
    // vz = cos(-yRot) = cos(yRot)
    let rad = f64::from(head_yaw).to_radians();
    let view_x = -rad.sin();
    let view_z = rad.cos();

    // dot product between horizontal direction to source and defender view vector
    let dot = norm_dx * view_x + norm_dz * view_z;
    dot >= 0.0
}

/// Vanilla `BlocksAttacks.ItemDamageFunction(3.0F, 1.0F, 1.0F)`:
/// Shield takes 0 durability damage for attacks below 3.0 damage (< 1.5 hearts).
/// For attacks of 3.0 or greater, shield durability damage is `floor(1.0 + dealt_damage)`.
#[must_use]
pub fn calculate_shield_durability_damage(dealt_damage: f32) -> i32 {
    if dealt_damage < 3.0 {
        0
    } else {
        (1.0 + dealt_damage).floor() as i32
    }
}

pub fn player_attack_sound(pos: &Vector3<f64>, world: &World, attack_type: AttackType) {
    match attack_type {
        AttackType::Knockback => {
            world.play_sound(
                Sound::EntityPlayerAttackKnockback,
                SoundCategory::Players,
                pos,
            );
            world.play_sound(Sound::EntityPlayerAttackStrong, SoundCategory::Players, pos);
        }
        AttackType::Critical => {
            world.play_sound(Sound::EntityPlayerAttackCrit, SoundCategory::Players, pos);
        }
        AttackType::Sweeping => {
            world.play_sound(Sound::EntityPlayerAttackSweep, SoundCategory::Players, pos);
        }
        AttackType::Strong => {
            world.play_sound(Sound::EntityPlayerAttackStrong, SoundCategory::Players, pos);
        }
        AttackType::Weak => {
            world.play_sound(Sound::EntityPlayerAttackWeak, SoundCategory::Players, pos);
        }
    }
}

/// Vanilla `Entity.java:1063`: `playEntityOnFireExtinguishedSound`
/// Pitch formula: `1.6F + (random.nextFloat() - random.nextFloat()) * 0.4F`
#[must_use]
pub fn calculate_extinguish_sound_pitch(rand1: f32, rand2: f32) -> f32 {
    1.6 + (rand1 - rand2) * 0.4
}

/// Vanilla `Entity.java:1002-1008` & `WaterFluid.java:88`:
/// If an entity is burning (fire_ticks > 0 or is_on_fire flag) and enters water or is in rain,
/// the fire is extinguished.
#[must_use]
pub fn should_extinguish_fire(
    fire_ticks: i32,
    is_on_fire: bool,
    in_water: bool,
    in_rain: bool,
) -> bool {
    (fire_ticks > 0 || is_on_fire) && (in_water || in_rain)
}

/// Vanilla `ApplyMobEffect.java:41-47`:
/// `int ticks = Math.round(Mth.randomBetween(random, minDuration, maxDuration) * 20.0F);`
#[must_use]
pub fn calculate_mob_effect_duration_ticks(
    min_duration_sec: f32,
    max_duration_sec: f32,
    rand_factor: f32,
) -> i32 {
    let clamped_rand = rand_factor.clamp(0.0, 1.0);
    let sec = min_duration_sec + (max_duration_sec - min_duration_sec) * clamped_rand;
    (sec * 20.0).round() as i32
}

/// Vanilla `ApplyMobEffect.java:44-46`:
/// `int amplifier = Math.max(0, Math.round(Mth.randomBetween(random, minAmplifier, maxAmplifier)));`
#[must_use]
pub fn calculate_mob_effect_amplifier(
    min_amplifier: f32,
    max_amplifier: f32,
    rand_factor: f32,
) -> u8 {
    let clamped_rand = rand_factor.clamp(0.0, 1.0);
    let amp = min_amplifier + (max_amplifier - min_amplifier) * clamped_rand;
    amp.round().max(0.0) as u8
}

/// Vanilla `Mob.java:1384`: Calculates final attack damage for a mob including enchantment bonus.
#[must_use]
pub fn calculate_mob_attack_damage(base_attack_damage: f32, extra_enchantment_damage: f32) -> f32 {
    (base_attack_damage + extra_enchantment_damage).max(0.0)
}

/// Vanilla `LivingEntity.java:206`: `getKnockback`
/// Formula: `(attributeValue(Attributes.ATTACK_KNOCKBACK) + enchantment_knockback) / 2.0F`
#[must_use]
pub fn calculate_mob_extra_knockback(attack_knockback_attr: f32, enchantment_knockback: f32) -> f32 {
    ((attack_knockback_attr + enchantment_knockback) / 2.0).max(0.0)
}

/// Vanilla `Player.java:1078-1094` & `EnchantmentHelper.java:226-261`:
/// Applies the attacker's weapon post-attack effects (e.g. Fire Aspect, Bane of Arthropods)
/// to the victim.
pub fn apply_weapon_post_attack_effects(item_stack: &ItemStack, victim: &dyn EntityBase) {
    let victim_entity = victim.get_entity();
    if let Some(enchantments) = item_stack.get_data_component::<EnchantmentsImpl>() {
        for (enchantment, level) in enchantments.enchantment.iter() {
            for post_effect in enchantment.get_post_attack_effects() {
                if post_effect.affected == Some(pumpkin_data::enchantment::EnchantmentTarget::Victim) {
                    match &post_effect.effect {
                        pumpkin_data::enchantment::EnchantmentEntityEffect::Ignite { duration } => {
                            let duration_seconds = duration.calculate(*level);
                            if victim_entity.is_in_water() {
                                victim_entity.play_entity_on_fire_extinguished_sound();
                            } else {
                                victim_entity
                                    .set_on_fire_for_ticks((duration_seconds * 20.0) as u32);
                            }
                        }
                        pumpkin_data::enchantment::EnchantmentEntityEffect::ApplyMobEffect {
                            to_apply,
                            min_duration,
                            max_duration,
                            min_amplifier,
                            max_amplifier,
                        } => {
                            if victim_entity
                                .entity_type
                                .has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS)
                                && let Some(living_victim) = victim.get_living_entity()
                            {
                                let rand_val = rand::random::<f32>();
                                let min_dur = min_duration.calculate(*level);
                                let max_dur = max_duration.calculate(*level);
                                let min_amp = min_amplifier.calculate(*level);
                                let max_amp = max_amplifier.calculate(*level);
                                let ticks = calculate_mob_effect_duration_ticks(
                                    min_dur, max_dur, rand_val,
                                );
                                let amplifier = calculate_mob_effect_amplifier(
                                    min_amp, max_amp, rand_val,
                                );
                                for effect_type in *to_apply {
                                    living_victim.add_effect(pumpkin_data::potion::Effect {
                                        effect_type,
                                        duration: ticks,
                                        amplifier,
                                        ambient: false,
                                        show_particles: true,
                                        show_icon: true,
                                        blend: false,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Vanilla `Player.java:553-568` (`destroyVanishingCursedItems`), `Mob.java:899-905` (`dropEquipment`):
/// Checks whether an item stack has the Curse of Vanishing (`minecraft:vanishing_curse`),
/// which prevents equipment from dropping on death.
#[must_use]
pub fn has_vanishing_curse(item_stack: &ItemStack) -> bool {
    item_stack.get_enchantment_level(&pumpkin_data::Enchantment::VANISHING_CURSE) > 0
}

/// Combat calculation rules mirroring vanilla `net.minecraft.world.damagesource.CombatRules`.
pub struct CombatRules;

impl CombatRules {
    pub const MAX_ARMOR: f32 = 20.0;
    pub const ARMOR_PROTECTION_DIVIDER: f32 = 25.0;
    pub const BASE_ARMOR_TOUGHNESS: f32 = 2.0;
    pub const MIN_ARMOR_RATIO: f32 = 0.2;

    /// Calculates damage after armor reduction, mirroring vanilla `CombatRules.getDamageAfterAbsorb`.
    #[must_use]
    pub fn get_damage_after_absorb(
        damage: f32,
        total_armor: f32,
        armor_toughness: f32,
        breach_level: u32,
    ) -> f32 {
        let toughness = Self::BASE_ARMOR_TOUGHNESS + armor_toughness / 4.0;
        let real_armor = (total_armor - damage / toughness)
            .clamp(total_armor * Self::MIN_ARMOR_RATIO, Self::MAX_ARMOR);
        let mut armor_fraction = real_armor / Self::ARMOR_PROTECTION_DIVIDER;

        if breach_level > 0 {
            let mut effectiveness = 1.0f32;
            pumpkin_data::Enchantment::BREACH
                .modify_armor_effectiveness(breach_level as i32, &mut effectiveness);
            armor_fraction = (armor_fraction * effectiveness.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        }

        let damage_multiplier = 1.0 - armor_fraction;
        damage * damage_multiplier
    }

    /// Calculates damage after magic/enchantment armor reduction, mirroring vanilla `CombatRules.getDamageAfterMagicAbsorb`.
    #[must_use]
    pub fn get_damage_after_magic_absorb(damage: f32, total_magic_armor: f32) -> f32 {
        let real_armor = total_magic_armor.clamp(0.0, Self::MAX_ARMOR);
        damage * (1.0 - real_armor / Self::ARMOR_PROTECTION_DIVIDER)
    }
}

pub const RESET_DAMAGE_STATUS_TIME: i64 = 100;
pub const RESET_COMBAT_STATUS_TIME: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallLocation {
    Generic,
    Ladder,
    Vines,
    WeepingVines,
    TwistingVines,
    Scaffolding,
    OtherClimbable,
    Water,
}

impl FallLocation {
    #[must_use]
    pub const fn language_key(self) -> &'static str {
        match self {
            Self::Generic => "death.fell.accident.generic",
            Self::Ladder => "death.fell.accident.ladder",
            Self::Vines => "death.fell.accident.vines",
            Self::WeepingVines => "death.fell.accident.weeping_vines",
            Self::TwistingVines => "death.fell.accident.twisting_vines",
            Self::Scaffolding => "death.fell.accident.scaffolding",
            Self::OtherClimbable => "death.fell.accident.other_climbable",
            Self::Water => "death.fell.accident.water",
        }
    }

    pub fn get_current_fall_location(
        living: &crate::entity::living::LivingEntity,
        world: &World,
    ) -> Self {
        let pos = living
            .climbing_pos
            .load()
            .unwrap_or_else(|| living.entity.block_pos.load());
        let block = world.get_block(&pos);
        if block == &pumpkin_data::Block::LADDER
            || block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_TRAPDOORS)
        {
            Self::Ladder
        } else if block == &pumpkin_data::Block::VINE {
            Self::Vines
        } else if block == &pumpkin_data::Block::WEEPING_VINES
            || block == &pumpkin_data::Block::WEEPING_VINES_PLANT
        {
            Self::WeepingVines
        } else if block == &pumpkin_data::Block::TWISTING_VINES
            || block == &pumpkin_data::Block::TWISTING_VINES_PLANT
        {
            Self::TwistingVines
        } else if block == &pumpkin_data::Block::SCAFFOLDING {
            Self::Scaffolding
        } else if living.entity.is_in_water() {
            Self::Water
        } else if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_CLIMBABLE) {
            Self::OtherClimbable
        } else {
            Self::Generic
        }
    }
}

#[derive(Clone)]
pub struct CombatEntry {
    pub damage_type: pumpkin_data::damage::DamageType,
    pub damage: f32,
    pub fall_location: Option<FallLocation>,
    pub fall_distance: f32,
    pub timestamp: i64,
    pub source_id: Option<i32>,
    pub attacker_id: Option<i32>,
    pub attacker_name: Option<pumpkin_util::text::TextComponent>,
    pub attacker_item_name: Option<pumpkin_util::text::TextComponent>,
    pub attacker_is_living: bool,
    pub attacker_is_player: bool,
}

#[derive(Default)]
pub struct CombatTracker {
    entries: Vec<CombatEntry>,
    last_damage_time: i64,
    combat_start_time: i64,
    combat_end_time: i64,
    in_combat: bool,
    taking_damage: bool,
}

impl CombatTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_damage(
        &mut self,
        current_tick: i64,
        is_alive: bool,
        fall_distance: f32,
        fall_location: FallLocation,
        damage_type: pumpkin_data::damage::DamageType,
        damage: f32,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> (Option<i64>, bool) {
        let left_combat_duration = self.recheck_status(current_tick, is_alive);

        let attacker = cause.or(source);
        let attacker_id = attacker.map(|e| e.get_entity().entity_id);
        let attacker_name = attacker.map(super::EntityBase::get_display_name);
        let attacker_is_living = attacker.is_some_and(|e| e.get_living_entity().is_some());
        let attacker_is_player = attacker.is_some_and(|e| {
            e.get_entity().entity_type == &pumpkin_data::entity::EntityType::PLAYER
        });

        // Check for custom named item in attacker's main hand
        let attacker_item_name =
            attacker.and_then(|e| {
                if let Some(player) = e.get_player() {
                    let hand_stack = player
                        .inventory()
                        .get_stack_in_hand(pumpkin_util::Hand::Right);
                    if !hand_stack.is_empty()
                    && let Some(custom_name) = hand_stack
                        .get_data_component::<pumpkin_data::data_component_impl::CustomNameImpl>()
                {
                    return Some(custom_name.name.clone());
                }
                } else if let Some(living) = e.get_living_entity() {
                    let equip_guard = living
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(stack) = equip_guard
                    .equipment
                    .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
                    && !stack.is_empty()
                    && let Some(custom_name) = stack
                        .get_data_component::<pumpkin_data::data_component_impl::CustomNameImpl>()
                {
                    return Some(custom_name.name.clone());
                }
                }
                None
            });

        let entry = CombatEntry {
            damage_type,
            damage,
            fall_location: Some(fall_location),
            fall_distance,
            timestamp: current_tick,
            source_id: source.map(|e| e.get_entity().entity_id),
            attacker_id,
            attacker_name,
            attacker_item_name,
            attacker_is_living,
            attacker_is_player,
        };

        self.entries.push(entry);
        self.last_damage_time = current_tick;
        self.taking_damage = true;

        let entered_combat = if !self.in_combat && is_alive && attacker_is_living {
            self.in_combat = true;
            self.combat_start_time = current_tick;
            self.combat_end_time = self.combat_start_time;
            true
        } else {
            false
        };

        (left_combat_duration, entered_combat)
    }

    /// Rechecks combat status based on timeout (100 ticks for damage, 300 ticks for combat)
    /// or death. Returns `Some(combat_duration)` if the entity left combat, `None` otherwise.
    pub fn recheck_status(&mut self, current_tick: i64, is_alive: bool) -> Option<i64> {
        let reset = if self.in_combat {
            RESET_COMBAT_STATUS_TIME
        } else {
            RESET_DAMAGE_STATUS_TIME
        };
        if self.taking_damage && (!is_alive || current_tick - self.last_damage_time > reset) {
            let was_in_combat = self.in_combat;
            let duration = self.get_combat_duration(current_tick);
            self.taking_damage = false;
            self.in_combat = false;
            self.combat_end_time = current_tick;
            self.entries.clear();
            if was_in_combat {
                return Some(duration);
            }
        }
        None
    }

    #[must_use]
    pub const fn get_combat_duration(&self, current_tick: i64) -> i64 {
        if self.in_combat {
            current_tick - self.combat_start_time
        } else {
            self.combat_end_time - self.combat_start_time
        }
    }

    #[must_use]
    pub const fn is_in_combat(&self) -> bool {
        self.in_combat
    }

    #[must_use]
    pub fn has_player_attacker(&self) -> bool {
        self.entries.iter().any(|e| e.attacker_is_player)
    }

    #[must_use]
    pub fn get_killer_entry(&self) -> Option<&CombatEntry> {
        let mut best_living: Option<&CombatEntry> = None;
        let mut best_player: Option<&CombatEntry> = None;
        let mut max_living_dmg = 0.0f32;
        let mut max_player_dmg = 0.0f32;

        for entry in &self.entries {
            if entry.attacker_is_player && (best_player.is_none() || entry.damage > max_player_dmg)
            {
                max_player_dmg = entry.damage;
                best_player = Some(entry);
            }
            if entry.attacker_is_living && (best_living.is_none() || entry.damage > max_living_dmg)
            {
                max_living_dmg = entry.damage;
                best_living = Some(entry);
            }
        }

        if best_player.is_some() && max_player_dmg >= max_living_dmg / 3.0 {
            best_player
        } else {
            best_living
        }
    }

    #[must_use]
    pub fn get_death_message(
        &self,
        victim_name: pumpkin_util::text::TextComponent,
        kill_credit_name: Option<pumpkin_util::text::TextComponent>,
    ) -> pumpkin_util::text::TextComponent {
        if self.entries.is_empty() {
            return pumpkin_util::text::TextComponent::translate_cross(
                "death.attack.generic",
                "death.attack.generic",
                [victim_name],
            );
        }

        let killing_blow = &self.entries[self.entries.len() - 1];
        let killing_damage_type = killing_blow.damage_type;
        let knock_off_entry = self.get_most_significant_fall();

        match killing_damage_type.death_message_type {
            pumpkin_data::damage::DeathMessageType::FallVariants => {
                if let Some(knock_off) = knock_off_entry {
                    Self::get_fall_message(victim_name, knock_off, killing_blow)
                } else {
                    let loc = killing_blow.fall_location.unwrap_or(FallLocation::Generic);
                    pumpkin_util::text::TextComponent::translate_cross(
                        loc.language_key(),
                        loc.language_key(),
                        [victim_name],
                    )
                }
            }
            pumpkin_data::damage::DeathMessageType::IntentionalGameDesign => {
                let death_msg = format!("death.attack.{}", killing_damage_type.message_id);
                let link = pumpkin_util::text::TextComponent::text("[")
                    .add_child(pumpkin_util::text::TextComponent::translate_cross(
                        format!("{death_msg}.link"),
                        format!("{death_msg}.link"),
                        [],
                    ))
                    .add_child(pumpkin_util::text::TextComponent::text("]"))
                    .click_event(pumpkin_util::text::click::ClickEvent::OpenUrl {
                        url: "https://bugs.mojang.com/browse/MCPE-28723".into(),
                    })
                    .hover_event(pumpkin_util::text::hover::HoverEvent::show_text(
                        pumpkin_util::text::TextComponent::text("MCPE-28723"),
                    ));

                pumpkin_util::text::TextComponent::translate_cross(
                    format!("{death_msg}.message"),
                    format!("{death_msg}.message"),
                    [victim_name, link],
                )
            }
            pumpkin_data::damage::DeathMessageType::Default => {
                self.get_default_death_message(victim_name, killing_blow, kill_credit_name)
            }
        }
    }

    fn get_most_significant_fall(&self) -> Option<&CombatEntry> {
        let mut result = None;
        let mut alternative = None;
        let mut alt_damage = 0.0f32;
        let mut best_fall = 0.0f32;

        for i in 0..self.entries.len() {
            let entry = &self.entries[i];
            let previous = (i > 0).then(|| &self.entries[i - 1]);
            let damage_type = entry.damage_type;
            let is_fake_fall = damage_type
                .has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_ALWAYS_MOST_SIGNIFICANT_FALL);
            let fall_distance = if is_fake_fall {
                f32::MAX
            } else {
                entry.fall_distance
            };

            if (damage_type.has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_IS_FALL)
                || is_fake_fall)
                && fall_distance > 0.0
                && (result.is_none() || fall_distance > best_fall)
            {
                if i > 0 {
                    result = previous;
                } else {
                    result = Some(entry);
                }
                best_fall = fall_distance;
            }

            if entry.fall_location.is_some() && (alternative.is_none() || entry.damage > alt_damage)
            {
                alternative = Some(entry);
                alt_damage = entry.damage;
            }
        }

        if best_fall > 5.0 && result.is_some() {
            result
        } else if alt_damage > 5.0 && alternative.is_some() {
            alternative
        } else {
            None
        }
    }

    fn get_fall_message(
        victim_name: pumpkin_util::text::TextComponent,
        knock_off_entry: &CombatEntry,
        killing_blow: &CombatEntry,
    ) -> pumpkin_util::text::TextComponent {
        let knock_off_type = knock_off_entry.damage_type;
        if !knock_off_type.has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_IS_FALL)
            && !knock_off_type
                .has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_ALWAYS_MOST_SIGNIFICANT_FALL)
        {
            let killer_name = killing_blow.attacker_name.as_ref();
            let attacker_name = knock_off_entry.attacker_name.as_ref();
            let attacker_item = knock_off_entry.attacker_item_name.clone();

            if let Some(attacker_name) = attacker_name
                && (killer_name.is_none() || killer_name != Some(attacker_name))
            {
                Self::get_message_for_assisted_fall(
                    victim_name,
                    attacker_name.clone(),
                    attacker_item,
                    "death.fell.assist.item",
                    "death.fell.assist",
                )
            } else if let Some(killer_name) = killer_name {
                Self::get_message_for_assisted_fall(
                    victim_name,
                    killer_name.clone(),
                    killing_blow.attacker_item_name.clone(),
                    "death.fell.finish.item",
                    "death.fell.finish",
                )
            } else {
                pumpkin_util::text::TextComponent::translate_cross(
                    "death.fell.killer",
                    "death.fell.killer",
                    [victim_name],
                )
            }
        } else {
            let loc = knock_off_entry
                .fall_location
                .unwrap_or(FallLocation::Generic);
            pumpkin_util::text::TextComponent::translate_cross(
                loc.language_key(),
                loc.language_key(),
                [victim_name],
            )
        }
    }

    fn get_message_for_assisted_fall(
        victim_name: pumpkin_util::text::TextComponent,
        attacker_name: pumpkin_util::text::TextComponent,
        attacker_item: Option<pumpkin_util::text::TextComponent>,
        message_with_item: &'static str,
        message_without_item: &'static str,
    ) -> pumpkin_util::text::TextComponent {
        if let Some(item_name) = attacker_item {
            pumpkin_util::text::TextComponent::translate_cross(
                message_with_item,
                message_with_item,
                [victim_name, attacker_name, item_name],
            )
        } else {
            pumpkin_util::text::TextComponent::translate_cross(
                message_without_item,
                message_without_item,
                [victim_name, attacker_name],
            )
        }
    }

    fn get_default_death_message(
        &self,
        victim_name: pumpkin_util::text::TextComponent,
        killing_blow: &CombatEntry,
        kill_credit_name: Option<pumpkin_util::text::TextComponent>,
    ) -> pumpkin_util::text::TextComponent {
        let damage_type = killing_blow.damage_type;
        let msg_id = damage_type.message_id;

        if let Some(attacker_name) = &killing_blow.attacker_name {
            if let Some(item_name) = &killing_blow.attacker_item_name {
                pumpkin_util::text::TextComponent::translate_cross(
                    format!("death.attack.{msg_id}.item"),
                    format!("death.attack.{msg_id}.item"),
                    [victim_name, attacker_name.clone(), item_name.clone()],
                )
            } else {
                pumpkin_util::text::TextComponent::translate_cross(
                    format!("death.attack.{msg_id}"),
                    format!("death.attack.{msg_id}"),
                    [victim_name, attacker_name.clone()],
                )
            }
        } else if let Some(killer_name) = kill_credit_name.or_else(|| {
            self.get_killer_entry()
                .and_then(|k| k.attacker_name.clone())
        }) {
            pumpkin_util::text::TextComponent::translate_cross(
                format!("death.attack.{msg_id}.player"),
                format!("death.attack.{msg_id}.player"),
                [victim_name, killer_name],
            )
        } else {
            pumpkin_util::text::TextComponent::translate_cross(
                format!("death.attack.{msg_id}"),
                format!("death.attack.{msg_id}"),
                [victim_name],
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_resistance_keeps_full_strength() {
        assert_eq!(knockback_after_resistance(0.4, 0.0), 0.4);
    }

    #[test]
    fn full_resistance_cancels_knockback() {
        // Iron golem / warden have KNOCKBACK_RESISTANCE == 1.0.
        assert_eq!(knockback_after_resistance(0.4, 1.0), 0.0);
    }

    #[test]
    fn partial_resistance_scales_strength() {
        // Ravager has KNOCKBACK_RESISTANCE == 0.75.
        assert!((knockback_after_resistance(0.4, 0.75) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn over_full_resistance_is_negative_so_callers_skip_it() {
        // Stacked armour modifiers can push resistance above 1.0; the result is
        // negative and callers guard on `strength > 0.0`.
        assert!(knockback_after_resistance(0.4, 1.2) < 0.0);
    }

    #[test]
    fn test_sweeping_damage_calculation_parity() {
        // Without Sweeping Edge enchantment (ratio = 0.0), sweep damage is always 1.0 (0.5 hearts)
        assert_eq!(calculate_sweeping_damage(9.0, 0.0), 1.0);
        assert_eq!(calculate_sweeping_damage(15.0, 0.0), 1.0);

        // Sweeping Edge I (ratio = 0.5): 1.0 + 0.5 * 9.0 = 5.5
        assert_eq!(calculate_sweeping_damage(9.0, 0.5), 5.5);

        // Sweeping Edge II (ratio = 2.0 / 3.0): 1.0 + (2.0/3.0) * 9.0 = 7.0
        let se2_ratio = 2.0 / 3.0;
        assert!((calculate_sweeping_damage(9.0, se2_ratio) - 7.0).abs() < 1e-6);

        // Sweeping Edge III (ratio = 0.75): 1.0 + 0.75 * 9.0 = 7.75
        assert_eq!(calculate_sweeping_damage(9.0, 0.75), 7.75);
    }

    #[test]
    fn test_critical_attack_conditions_parity() {
        // Standard valid critical attack condition:
        // not sprinting, airborne (!on_ground), fall_distance > 0, not on climbable,
        // not in water, not blind, not passenger, living victim, cooldown > 0.9.
        assert!(can_critical_attack_raw(
            false, // sprinting
            false, // on_ground
            1.0,   // fall_distance
            false, // on_climbable
            false, // is_in_water
            false, // is_blind
            false, // is_passenger
            true,  // is_victim_living
            1.0,   // attack_cooldown_progress
        ));

        // 1. On ground cannot critical attack
        assert!(!can_critical_attack_raw(false, true, 1.0, false, false, false, false, true, 1.0));

        // 2. Sprinting cannot critical attack (produces knockback attack instead)
        assert!(!can_critical_attack_raw(true, false, 1.0, false, false, false, false, true, 1.0));

        // 3. Fall distance <= 0 cannot critical attack (moving upward)
        assert!(!can_critical_attack_raw(false, false, 0.0, false, false, false, false, true, 1.0));

        // 4. On climbable (ladder / vines) cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, true, false, false, false, true, 1.0));

        // 5. In water cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, false, true, false, false, true, 1.0));

        // 6. Blindness effect (mobility restricted) cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, false, false, true, false, true, 1.0));

        // 7. Passenger (riding horse/boat/minecart) cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, false, false, false, true, true, 1.0));

        // 8. Non-living victim (ArmorStand, boat, minecart) cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, false, false, false, false, false, 1.0));

        // 9. Attack cooldown not ready (progress <= 0.9) cannot critical attack
        assert!(!can_critical_attack_raw(false, false, 1.0, false, false, false, false, true, 0.9));
        assert!(!can_critical_attack_raw(false, false, 1.0, false, false, false, false, true, 0.5));
    }

    #[test]
    fn test_sweep_attack_conditions_parity() {
        // Standard valid sweep attack condition (Player.isSweepAttack):
        // sword, strong (> 0.9 cooldown), not critical, not knockback (not sprinting),
        // on ground, horizontal speed < speed * 2.5, Java edition (!is_bedrock).
        let normal_speed = 0.1; // default movement speed
        let max_speed = normal_speed * 2.5;
        let valid_vel_sq = (max_speed * 0.5) * (max_speed * 0.5);

        assert!(can_sweep_attack_raw(
            true,         // is_sword
            true,         // is_strong
            false,        // is_critical
            false,        // is_knockback
            true,         // on_ground
            valid_vel_sq, // horizontal_speed_sq
            normal_speed, // movement_speed
            false,        // is_bedrock
        ));

        // 1. Not a sword cannot sweep
        assert!(!can_sweep_attack_raw(false, true, false, false, true, valid_vel_sq, normal_speed, false));

        // 2. Attack cooldown not ready (progress <= 0.9) cannot sweep
        assert!(!can_sweep_attack_raw(true, false, false, false, true, valid_vel_sq, normal_speed, false));

        // 3. Critical attack takes precedence, cannot sweep
        assert!(!can_sweep_attack_raw(true, true, true, false, true, valid_vel_sq, normal_speed, false));

        // 4. Knockback attack (sprinting) takes precedence, cannot sweep
        assert!(!can_sweep_attack_raw(true, true, false, true, true, valid_vel_sq, normal_speed, false));

        // 5. Airborne (!on_ground) cannot sweep
        assert!(!can_sweep_attack_raw(true, true, false, false, false, valid_vel_sq, normal_speed, false));

        // 6. Moving too fast (horizontal_speed_sq >= (speed * 2.5)^2) cannot sweep
        let too_fast_vel_sq = (max_speed + 0.01) * (max_speed + 0.01);
        assert!(!can_sweep_attack_raw(true, true, false, false, true, too_fast_vel_sq, normal_speed, false));

        // 7. Bedrock edition clients do not have sweeping attacks
        assert!(!can_sweep_attack_raw(true, true, false, false, true, valid_vel_sq, normal_speed, true));
    }

    #[test]
    fn test_damage_indicator_and_redirectable_projectile_parity() {
        use pumpkin_data::entity::EntityType;

        // Damage indicator particles: only when actual damage > 2.0 (more than 1 heart)
        assert_eq!(damage_indicator_particle_count(0.0), None);
        assert_eq!(damage_indicator_particle_count(1.0), None);
        assert_eq!(damage_indicator_particle_count(2.0), None);
        assert_eq!(damage_indicator_particle_count(2.1), Some(1));
        assert_eq!(damage_indicator_particle_count(4.0), Some(2));
        assert_eq!(damage_indicator_particle_count(6.5), Some(3));
        assert_eq!(damage_indicator_particle_count(10.0), Some(5));

        // Redirectable projectiles tag (#minecraft:redirectable_projectile):
        // Only fireball, wind_charge, and breeze_wind_charge can be deflected by player attack
        assert!(is_redirectable_projectile(&EntityType::FIREBALL));
        assert!(is_redirectable_projectile(&EntityType::WIND_CHARGE));
        assert!(is_redirectable_projectile(&EntityType::BREEZE_WIND_CHARGE));

        // Non-redirectable projectiles/entities take normal attack damage instead
        assert!(!is_redirectable_projectile(&EntityType::ARROW));
        assert!(!is_redirectable_projectile(&EntityType::DRAGON_FIREBALL));
        assert!(!is_redirectable_projectile(&EntityType::SMALL_FIREBALL));
        assert!(!is_redirectable_projectile(&EntityType::WITHER_SKULL));
        assert!(!is_redirectable_projectile(&EntityType::SNOWBALL));
        assert!(!is_redirectable_projectile(&EntityType::ZOMBIE));
    }

    #[test]
    fn test_damage_cooldown_and_partial_hit_parity() {
        // Vanilla LivingEntity.hurtServer:
        // When outside cooldown (hurt_cooldown <= 10): full hit applied
        assert_eq!(
            calculate_cooldown_damage(10.0, 5.0, 10, false),
            Some((10.0, true))
        );
        assert_eq!(
            calculate_cooldown_damage(4.0, 8.0, 0, false),
            Some((4.0, true))
        );

        // When damage type bypasses cooldown (#minecraft:bypasses_cooldown): full hit applied regardless of cooldown
        assert_eq!(
            calculate_cooldown_damage(5.0, 10.0, 20, true),
            Some((5.0, true))
        );

        // When in cooldown (hurt_cooldown > 10) and does not bypass cooldown:
        // 1. Weaker hit: completely discarded (None)
        assert_eq!(
            calculate_cooldown_damage(4.0, 8.0, 15, false),
            None
        );
        // 2. Equal hit: completely discarded (None)
        assert_eq!(
            calculate_cooldown_damage(8.0, 8.0, 20, false),
            None
        );
        // 3. Stronger hit: only the difference is applied, and took_full_damage is false
        // (no knockback, no hurt animation, no damage sound replay)
        assert_eq!(
            calculate_cooldown_damage(12.0, 8.0, 18, false),
            Some((4.0, false))
        );
    }

    #[test]
    fn test_combat_tracker_lifecycle_and_death_message_parity() {
        use pumpkin_data::damage::DamageType;
        use pumpkin_util::text::TextComponent;

        let mut tracker = CombatTracker::new();
        let victim_name = TextComponent::text("Player1");

        // 1. Empty tracker gives generic death message
        let msg = tracker.get_death_message(victim_name.clone(), None);
        assert_eq!(msg.to_pretty_console(), "Player1 died");

        // 2. Non-living damage (e.g. fire/drowning without attacker)
        // Taking damage outside combat: resets after 100 ticks
        let (left_combat, entered_combat) = tracker.record_damage(
            100,
            true,
            0.0,
            FallLocation::Generic,
            DamageType::DROWN,
            2.0,
            None,
            None,
        );
        assert_eq!(left_combat, None);
        assert!(!entered_combat);
        assert!(!tracker.is_in_combat());
        assert_eq!(tracker.entries.len(), 1);

        // Before 100 ticks: still taking damage, entries kept
        assert_eq!(tracker.recheck_status(150, true), None);
        assert_eq!(tracker.entries.len(), 1);

        // After 100 ticks (e.g. tick 201): resets taking_damage and clears entries
        assert_eq!(tracker.recheck_status(201, true), None);
        assert_eq!(tracker.entries.len(), 0);

        // 3. Combat entry and timeout (300 ticks)
        // Simulate an entry directly with a living attacker
        let combat_entry = CombatEntry {
            damage_type: DamageType::GENERIC,
            damage: 4.0,
            fall_location: None,
            fall_distance: 0.0,
            timestamp: 300,
            source_id: Some(10),
            attacker_id: Some(10),
            attacker_name: Some(TextComponent::text("Zombie")),
            attacker_item_name: None,
            attacker_is_living: true,
            attacker_is_player: false,
        };
        tracker.entries.push(combat_entry);
        tracker.in_combat = true;
        tracker.combat_start_time = 300;
        tracker.combat_end_time = 300;
        tracker.last_damage_time = 300;
        tracker.taking_damage = true;

        assert!(tracker.is_in_combat());
        assert_eq!(tracker.get_combat_duration(400), 100);

        // Before 300 ticks in combat: no leave
        assert_eq!(tracker.recheck_status(599, true), None);
        assert!(tracker.is_in_combat());

        // At tick 601 (301 ticks since last damage): leaves combat, returns duration (301 ticks)
        let duration = tracker.recheck_status(601, true);
        assert_eq!(duration, Some(301));
        assert!(!tracker.is_in_combat());
        assert_eq!(tracker.entries.len(), 0);

        // 4. Death (is_alive == false) immediately ends combat and returns duration
        tracker.in_combat = true;
        tracker.combat_start_time = 700;
        tracker.combat_end_time = 700;
        tracker.last_damage_time = 700;
        tracker.taking_damage = true;
        let death_duration = tracker.recheck_status(750, false);
        assert_eq!(death_duration, Some(50));
        assert!(!tracker.is_in_combat());
        assert_eq!(tracker.entries.len(), 0);
    }

    #[test]
    fn test_shield_blocking_angle_and_durability_parity() {
        // 1. Durability function parity (BlocksAttacks.ItemDamageFunction(3.0F, 1.0F, 1.0F)):
        // Hits < 3.0 take 0 damage
        assert_eq!(calculate_shield_durability_damage(0.0), 0);
        assert_eq!(calculate_shield_durability_damage(1.0), 0);
        assert_eq!(calculate_shield_durability_damage(2.9), 0);

        // Hits >= 3.0 take floor(1.0 + damage)
        assert_eq!(calculate_shield_durability_damage(3.0), 4);
        assert_eq!(calculate_shield_durability_damage(4.5), 5);
        assert_eq!(calculate_shield_durability_damage(5.0), 6);
        assert_eq!(calculate_shield_durability_damage(10.0), 11);

        // 2. Shield blocking angle parity (horizontal angle <= 90 deg, dot >= 0):
        let defender_pos = Vector3::new(0.0, 64.0, 0.0);
        // Yaw 0 faces south (towards +Z in Minecraft coordinate system)
        // With head_yaw = 0.0, view_vector is (0, 0, 1)

        // None damage pos cannot be blocked
        assert!(!can_block_damage_source(defender_pos, 0.0, None));

        // Directly in front (+Z) is blocked
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 64.0, 5.0))));

        // Directly behind (-Z) is NOT blocked
        assert!(!can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 64.0, -5.0))));

        // 45 degrees to the right (+X, +Z) is blocked
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(5.0, 64.0, 5.0))));

        // Exactly 90 degrees (+X, 0) is blocked (dot == 0.0)
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(5.0, 64.0, 0.0))));

        // Behind 90 degrees (+X, -Z) is NOT blocked
        assert!(!can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(5.0, 64.0, -1.0))));

        // Elevation difference does NOT compromise horizontal angle check:
        // Attacker is 10 blocks above, but in front -> blocked
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 74.0, 5.0))));
        // Attacker is 10 blocks below, but in front -> blocked
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 54.0, 5.0))));
        // Attacker is 10 blocks above, but behind -> NOT blocked
        assert!(!can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 74.0, -5.0))));

        // Directly above (dx = 0, dz = 0) -> blocked
        assert!(can_block_damage_source(defender_pos, 0.0, Some(Vector3::new(0.0, 70.0, 0.0))));
    }

    #[test]
    fn test_knockback_direction_and_hurt_indication_parity() {
        use pumpkin_data::damage::DamageType;
        use pumpkin_data::tag;

        let target_pos = Vector3::new(10.0, 64.0, 20.0);

        // 1. Projectile knockback direction: uses negated velocity (-vel.x, -vel.z)
        let (p_xd, p_zd) = calculate_knockback_direction(
            target_pos,
            Some(Vector3::new(100.0, 64.0, 100.0)),
            true,
            Some(Vector3::new(2.5, 0.0, -1.5)),
            None,
        );
        assert_eq!((p_xd, p_zd), (-2.5, 1.5));

        // 2. Direct attacker position: (source_pos.x - target_pos.x, source_pos.z - target_pos.z)
        let (s_xd, s_zd) = calculate_knockback_direction(
            target_pos,
            Some(Vector3::new(15.0, 64.0, 32.0)),
            false,
            None,
            None,
        );
        assert_eq!((s_xd, s_zd), (5.0, 12.0));

        // 3. Positional damage (explosion, block) without entity source:
        let (b_xd, b_zd) = calculate_knockback_direction(
            target_pos,
            None,
            false,
            None,
            Some(Vector3::new(6.0, 64.0, 10.0)),
        );
        assert_eq!((b_xd, b_zd), (-4.0, -10.0));

        // 4. Environmental damage without source and without position:
        let (n_xd, n_zd) = calculate_knockback_direction(target_pos, None, false, None, None);
        assert_eq!((n_xd, n_zd), (0.0, 0.0));

        // 5. Hurt indication yaw (Vanilla ServerPlayer.indicateDamage):
        // When (xd, zd) == (0, 0), returns -yaw
        assert_eq!(calculate_hurt_direction_yaw(0.0, 0.0, 45.0), -45.0);

        // Attacker directly in front along +Z (xd = 0.0, zd = 10.0), entity facing south (yaw = 0.0)
        // atan2(10, 0) = 90 deg -> 90 - 0 = 90 deg
        let yaw_front = calculate_hurt_direction_yaw(0.0, 10.0, 0.0);
        assert!((yaw_front - 90.0).abs() < 1e-4);

        // Attacker to the right along +X (xd = 10.0, zd = 0.0), entity facing south (yaw = 0.0)
        // atan2(0, 10) = 0 deg -> 0 - 0 = 0 deg
        let yaw_right = calculate_hurt_direction_yaw(10.0, 0.0, 0.0);
        assert!((yaw_right - 0.0).abs() < 1e-4);

        // 6. Tag parity for NO_KNOCKBACK and NO_IMPACT:
        assert!(DamageType::IN_FIRE.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(DamageType::ON_FIRE.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(DamageType::DROWN.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(DamageType::STARVE.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(DamageType::FALL.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(!DamageType::PLAYER_ATTACK.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));
        assert!(!DamageType::MOB_ATTACK.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK));

        assert!(DamageType::DROWN.has_tag(&tag::DamageType::MINECRAFT_NO_IMPACT));
        assert!(!DamageType::PLAYER_ATTACK.has_tag(&tag::DamageType::MINECRAFT_NO_IMPACT));
    }

    #[test]
    fn test_attack_cooldown_progress_and_base_damage_scale_parity() {
        // Vanilla Player.java:1799-1811:
        // delay = 1.0 / attackSpeed * 20.0 (progress_per_tick = 20.0 / attack_speed)
        // scale = clamp((ticker + 0.5) / delay, 0.0, 1.0)
        // baseDamageScaleFactor = 0.2 + scale * scale * 0.8

        // 1. Bare hand (attack_speed = 4.0): delay is 5.0 ticks
        // At tick 0 (just reset/swapped/swung): (0 + 0.5) / 5.0 = 0.1
        let p0 = calculate_attack_cooldown_progress(0, 0.5, 20.0, 4.0);
        assert!((p0 - 0.1).abs() < 1e-4);
        let dmg0 = calculate_base_damage_scale_factor(p0);
        assert!((dmg0 - (0.2 + 0.01 * 0.8)).abs() < 1e-4);

        // At tick 2: (2 + 0.5) / 5.0 = 0.5 (half-charged)
        let p2 = calculate_attack_cooldown_progress(2, 0.5, 20.0, 4.0);
        assert!((p2 - 0.5).abs() < 1e-4);
        let dmg2 = calculate_base_damage_scale_factor(p2);
        assert!((dmg2 - (0.2 + 0.25 * 0.8)).abs() < 1e-4); // 0.4 (40% damage)

        // At tick 5: (5 + 0.5) / 5.0 = 1.1 -> clamped to 1.0 (fully charged)
        let p5 = calculate_attack_cooldown_progress(5, 0.5, 20.0, 4.0);
        assert_eq!(p5, 1.0);
        let dmg5 = calculate_base_damage_scale_factor(p5);
        assert_eq!(dmg5, 1.0);

        // 2. Iron Axe (attack_speed = 0.9): delay is 22.22 ticks
        let p_axe_0 = calculate_attack_cooldown_progress(0, 0.5, 20.0, 0.9);
        assert!((p_axe_0 - 0.0225).abs() < 1e-4);

        // At tick 23: fully charged (1.0)
        let p_axe_full = calculate_attack_cooldown_progress(23, 0.5, 20.0, 0.9);
        assert_eq!(p_axe_full, 1.0);
    }

    #[test]
    fn test_player_attack_damage_critical_and_magic_boost_parity() {
        // Vanilla Player.java:946-966:
        // baseDamage *= baseDamageScaleFactor();
        // baseDamage += bonus_item_damage;
        // if (criticalAttack) { baseDamage *= 1.5F; }
        // totalDamage = baseDamage + magicBoost;

        // 1. Diamond sword (base 7.0), fully charged, no enchants, non-crit
        let dmg_normal = calculate_player_attack_damage(7.0, 1.0, 0.0, 0.0, false);
        assert_eq!(dmg_normal, 7.0);

        // 2. Diamond sword (base 7.0), fully charged, no enchants, CRIT (1.5x on base)
        let dmg_crit = calculate_player_attack_damage(7.0, 1.0, 0.0, 0.0, true);
        assert_eq!(dmg_crit, 10.5);

        // 3. Diamond sword (base 7.0) with Sharpness V (3.0 extra magic damage):
        // Normal hit: base (7.0) + magic (3.0) = 10.0
        let dmg_sharp = calculate_player_attack_damage(7.0, 1.0, 0.0, 3.0, false);
        assert_eq!(dmg_sharp, 10.0);

        // Critical hit: base crits (7.0 * 1.5 = 10.5) + magic (3.0, NOT multiplied) = 13.5
        // (Vanilla parity: Sharpness does NOT crit!)
        let dmg_sharp_crit = calculate_player_attack_damage(7.0, 1.0, 0.0, 3.0, true);
        assert_eq!(dmg_sharp_crit, 13.5);

        // 4. Mace with bonus damage: bonus damage IS added before crit multiplier
        let dmg_mace_crit = calculate_player_attack_damage(6.0, 1.0, 8.0, 0.0, true);
        assert_eq!(dmg_mace_crit, (6.0 + 8.0) * 1.5); // 21.0

        // 5. Cooldown reduction: at progress 0.5 (damage multiplier 0.4):
        // Base damage = 7.0 * 0.4 = 2.8
        // Magic boost = 3.0 * 0.5 = 1.5
        // Total = 4.3
        let dmg_cooldown = calculate_player_attack_damage(7.0, 0.5, 0.0, 3.0, false);
        assert!((dmg_cooldown - 4.3).abs() < 1e-4);
    }

    #[test]
    fn test_fire_extinguish_and_mob_effect_post_attack_parity() {
        use pumpkin_data::entity::EntityType;

        // 1. Extinguish sound pitch formula parity (Vanilla Entity.java:1063):
        // 1.6 + (rand1 - rand2) * 0.4
        let base_pitch = calculate_extinguish_sound_pitch(0.0, 0.0);
        assert_eq!(base_pitch, 1.6);
        let max_pitch = calculate_extinguish_sound_pitch(1.0, 0.0);
        assert_eq!(max_pitch, 2.0);
        let min_pitch = calculate_extinguish_sound_pitch(0.0, 1.0);
        assert_eq!(min_pitch, 1.2);

        // 2. Extinguish condition parity (Vanilla Entity.java:1002-1008 & WaterFluid.java:88):
        // Must extinguish when on fire and touching water or in rain
        assert!(should_extinguish_fire(100, true, true, false));
        assert!(should_extinguish_fire(100, false, true, false));
        assert!(should_extinguish_fire(0, true, false, true));
        assert!(!should_extinguish_fire(100, true, false, false));
        assert!(!should_extinguish_fire(0, false, true, false));

        // 3. Mob effect duration calculation parity (Vanilla ApplyMobEffect.java:41-47):
        // Bane of Arthropods V: min 1.5s, max 3.5s
        let dur_min = calculate_mob_effect_duration_ticks(1.5, 3.5, 0.0);
        assert_eq!(dur_min, 30); // 1.5s * 20 = 30 ticks
        let dur_max = calculate_mob_effect_duration_ticks(1.5, 3.5, 1.0);
        assert_eq!(dur_max, 70); // 3.5s * 20 = 70 ticks
        let dur_mid = calculate_mob_effect_duration_ticks(1.5, 3.5, 0.5);
        assert_eq!(dur_mid, 50); // 2.5s * 20 = 50 ticks

        // 4. Mob effect amplifier calculation parity (Vanilla ApplyMobEffect.java:44-46):
        // Bane of Arthropods: amplifier 3 (Slowness IV)
        let amp = calculate_mob_effect_amplifier(3.0, 3.0, 0.5);
        assert_eq!(amp, 3);

        // 5. Sensitive to Bane of Arthropods tag parity:
        assert!(EntityType::SPIDER.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(EntityType::CAVE_SPIDER.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(EntityType::SILVERFISH.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(EntityType::ENDERMITE.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(EntityType::BEE.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(!EntityType::ZOMBIE.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
        assert!(!EntityType::PLAYER.has_tag(&tag::EntityType::MINECRAFT_SENSITIVE_TO_BANE_OF_ARTHROPODS));
    }

    #[test]
    fn test_mob_attack_damage_and_knockback_parity() {
        // Vanilla Mob.java:1381-1389 & LivingEntity.java:206
        // 1. Zombie bare hand (base 3.0, no enchants):
        let dmg = calculate_mob_attack_damage(3.0, 0.0);
        assert_eq!(dmg, 3.0);

        // 2. Zombie with Iron Sword Sharpness II (extra 1.5):
        let dmg_sharp = calculate_mob_attack_damage(3.0, 1.5);
        assert_eq!(dmg_sharp, 4.5);

        // 3. Mob extra knockback:
        // No knockback:
        assert_eq!(calculate_mob_extra_knockback(0.0, 0.0), 0.0);
        // Knockback I on sword (1.0): (0.0 + 1.0) / 2.0 = 0.5
        assert_eq!(calculate_mob_extra_knockback(0.0, 1.0), 0.5);
        // Warden / Ravager with base ATTACK_KNOCKBACK = 1.5: 1.5 / 2.0 = 0.75
        assert_eq!(calculate_mob_extra_knockback(1.5, 0.0), 0.75);
    }

    #[test]
    fn test_vanishing_curse_and_totem_bypasses_invulnerability_parity() {
        use pumpkin_data::damage::DamageType;
        use pumpkin_data::item::Item;

        // 1. Vanishing curse check parity
        let mut item = ItemStack::new(1, &Item::DIAMOND_SWORD);
        assert!(!has_vanishing_curse(&item));

        // Add vanishing curse
        item.add_enchantment(&pumpkin_data::Enchantment::VANISHING_CURSE, 1);
        assert!(has_vanishing_curse(&item));

        // 2. Totem death protection bypasses invulnerability tag parity:
        // Vanilla LivingEntity.java:1394: checkTotemDeathProtection
        // if (damageSource.is(DamageTypeTags.BYPASSES_INVULNERABILITY)) return false;
        assert!(DamageType::OUT_OF_WORLD.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY));
        assert!(DamageType::GENERIC_KILL.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY));
        assert!(!DamageType::GENERIC.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY));
        assert!(!DamageType::PLAYER_ATTACK.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY));
        assert!(!DamageType::FALL.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY));
    }
}




