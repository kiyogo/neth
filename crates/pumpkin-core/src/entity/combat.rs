use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::entity::EntityBase;
use crate::entity::decoration::armor_stand::ArmorStandEntity;
use crate::entity::living::LivingEntity;
use pumpkin_data::data_component_impl::EnchantmentsImpl;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_data::entity::EntityType;
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

/// Vanilla `Player.java:1086-1098` & `ItemStack.java:539-558`:
/// Determines whether an attack against an entity should consume weapon durability
/// and award the `ITEM_USED` statistic.
/// In Vanilla, weapon durability damage and `Stats.ITEM_USED` are applied ONLY when
/// the target is a living entity and the weapon has the `Weapon` component.
#[must_use]
pub fn should_weapon_damage_on_attack(is_victim_living: bool, has_weapon_component: bool) -> bool {
    is_victim_living && has_weapon_component
}

/// Vanilla `LivingEntity.java:1862` & `MobEffects.java:51`:
/// Calculates the effective safe fall distance, accounting for the base attribute
/// and the Jump Boost effect modifier (+1.0 block per amplifier level).
#[must_use]
pub fn calculate_safe_fall_distance(
    base_safe_fall: f64,
    jump_boost_amplifier: Option<u32>,
) -> f64 {
    let mut safe = base_safe_fall;
    if let Some(amp) = jump_boost_amplifier {
        safe += f64::from(amp + 1);
    }
    safe
}

/// Vanilla `LivingEntity.java:1862`:
/// Calculates the fall power: `fall_distance + 1.0E-6 - safe_fall_distance`.
#[must_use]
pub fn calculate_fall_power(fall_distance: f64, safe_fall_distance: f64) -> f64 {
    fall_distance + 1.0E-6 - safe_fall_distance
}

/// Vanilla `LivingEntity.java:1858`:
/// Calculates the integer fall damage from fall power, damage modifier, and multiplier.
#[must_use]
pub fn calculate_fall_damage_raw(
    fall_power: f64,
    damage_modifier: f32,
    fall_damage_multiplier: f32,
) -> i32 {
    if fall_power <= 0.0 {
        return 0;
    }
    (fall_power as f32 * damage_modifier * fall_damage_multiplier)
        .floor()
        .max(0.0) as i32
}

/// Vanilla `LivingEntity.java:1368, 1445`:
/// Determines whether a damage credit (player or mob) is still active within the 100-tick window (5 seconds).
#[must_use]
pub fn is_kill_credit_active(attacker_id: i32, recorded_tick: i64, current_tick: i64) -> bool {
    attacker_id != 0 && (current_tick - recorded_tick).abs() <= 100
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
            // Vanilla `CombatRules.java:23-28`: modifyArmorEffectiveness applies additive
            // reduction (-0.15 * level) directly to armorFraction.
            pumpkin_data::Enchantment::BREACH
                .modify_armor_effectiveness(breach_level as i32, &mut armor_fraction);
            armor_fraction = armor_fraction.clamp(0.0, 1.0);
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

/// Determines if an entity should drop experience orbs on death, mirroring vanilla
/// `LivingEntity.dropExperience`, `Player.isAlwaysExperienceDropper`, and `Mob.shouldDropExperience`.
///
/// In vanilla:
/// - If absorbed by a sculk catalyst (`wasExperienceConsumed`), no experience is dropped.
/// - Players are always-droppers (`Player.isAlwaysExperienceDropper() == true`), bypassing the
///   player-kill requirement and `mobDrops` gamerule.
/// - Other living entities (mobs) require being killed by a player within 100 ticks (`lastHurtByPlayerMemoryTime > 0`),
///   passing `shouldDropExperience()` (e.g. non-baby for animals), and having the `mobDrops` gamerule enabled.
#[must_use]
pub const fn should_entity_drop_experience(
    is_always_dropper: bool,
    was_killed_by_player: bool,
    should_drop: bool,
    mob_drops_gamerule: bool,
    absorbed_by_catalyst: bool,
) -> bool {
    if absorbed_by_catalyst {
        return false;
    }
    if is_always_dropper {
        return true;
    }
    was_killed_by_player && should_drop && mob_drops_gamerule
}

/// Calculates the experience points dropped by a player on death, mirroring vanilla
/// `Player.getBaseExperienceReward` (Player.java:1610-1612).
///
/// In vanilla:
/// `!keepInventory && !isSpectator ? Math.min(this.experienceLevel * 7, 100) : 0`
#[must_use]
pub const fn calculate_player_death_experience_reward(
    experience_level: i32,
    keep_inventory: bool,
    is_spectator: bool,
) -> u32 {
    if keep_inventory || is_spectator || experience_level <= 0 {
        0
    } else {
        let reward = experience_level * 7;
        if reward > 100 { 100 } else { reward as u32 }
    }
}

/// Determines if an attacker and damage type should cause mob anger / set last_hurt_by_mob,
/// mirroring vanilla `LivingEntity.resolveMobResponsibleForDamage` (LivingEntity.java:1357-1363).
///
/// In vanilla:
/// - Attacker must be a LivingEntity (including Player).
/// - Damage type must NOT have `#minecraft:no_anger`.
/// - If damage type is `wind_charge`, the victim must NOT have `#minecraft:no_anger_from_wind_charge`.
#[must_use]
pub const fn can_cause_mob_anger(
    attacker_is_living: bool,
    has_no_anger_tag: bool,
    is_wind_charge: bool,
    victim_no_anger_from_wind_charge: bool,
) -> bool {
    if !attacker_is_living {
        return false;
    }
    if has_no_anger_tag {
        return false;
    }
    if is_wind_charge && victim_no_anger_from_wind_charge {
        return false;
    }
    true
}

/// Determines if directional hurt animation and knockback should be dealt upon taking full damage,
/// mirroring vanilla `LivingEntity.hurtServer` (LivingEntity.java:1250) and `dealDefaultKnockback`.
///
/// In vanilla:
/// `if (!source.is(DamageTypeTags.NO_KNOCKBACK) && !fullyBlocked) { this.dealDefaultKnockback(source, damage, blocked); }`
/// And `dealDefaultKnockback` only invokes `indicateDamage` (sending `ClientboundHurtAnimationPacket`) if `!blocked`.
#[must_use]
pub const fn should_deal_default_knockback_and_hurt_anim(
    has_no_knockback_tag: bool,
    fully_blocked: bool,
) -> bool {
    !has_no_knockback_tag && !fully_blocked
}

/// Action to perform for Wither Rose creation when an entity is killed by a Wither boss,
/// mirroring vanilla `LivingEntity.createWitherRose` (LivingEntity.java:1494-1512).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitherRosePlacementAction {
    /// Place the Wither Rose block in the world.
    PlaceBlock,
    /// Drop the Wither Rose as an item entity.
    DropItem,
    /// Not killed by a Wither boss, no action.
    None,
}

/// Determines if a Wither Rose can be placed at a block position, mirroring vanilla
/// `WitherRoseBlock.mayPlaceOn` and `LivingEntity.createWitherRose`.
///
/// In vanilla:
/// - Target block must be air.
/// - Supporting block directly below must have `#minecraft:supports_wither_rose`.
#[must_use]
pub const fn can_place_wither_rose(
    target_is_air: bool,
    below_supports_wither_rose: bool,
) -> bool {
    target_is_air && below_supports_wither_rose
}

/// Determines the action for Wither Rose generation upon entity death, mirroring vanilla
/// `LivingEntity.createWitherRose` (LivingEntity.java:1494-1512).
#[must_use]
pub const fn determine_wither_rose_action(
    killer_is_wither: bool,
    mob_griefing: bool,
    can_place: bool,
) -> WitherRosePlacementAction {
    if !killer_is_wither {
        return WitherRosePlacementAction::None;
    }
    if mob_griefing && can_place {
        WitherRosePlacementAction::PlaceBlock
    } else {
        WitherRosePlacementAction::DropItem
    }
}

/// Calculates the player's delta movement upon death, mirroring Vanilla `Player.java:534-542`:
/// ```java
/// if (source != null) {
///    this.setDeltaMovement(
///       -Mth.cos((this.getHurtDir() + this.getYRot()) * (float) (Math.PI / 180.0)) * 0.1F,
///       0.1F,
///       -Mth.sin((this.getHurtDir() + this.getYRot()) * (float) (Math.PI / 180.0)) * 0.1F
///    );
/// } else {
///    this.setDeltaMovement(0.0, 0.1, 0.0);
/// }
/// ```
#[must_use]
pub fn calculate_player_death_velocity(
    yaw: f32,
    hurt_dir: f32,
    has_source: bool,
) -> Vector3<f64> {
    if has_source {
        let angle_rad = (hurt_dir + yaw) * (std::f32::consts::PI / 180.0);
        Vector3::new(
            f64::from(-angle_rad.cos() * 0.1),
            0.1,
            f64::from(-angle_rad.sin() * 0.1),
        )
    } else {
        Vector3::new(0.0, 0.1, 0.0)
    }
}

/// Calculates the removal interval in ticks for stuck arrows/stingers,
/// mirroring Vanilla `LivingEntity.java:2758, 2770`:
/// `20 * (30 - count)`.
#[must_use]
pub fn calculate_stuck_projectile_removal_time(count: i32) -> i32 {
    20 * (30 - count)
}

/// Updates the stuck projectile counter and removal timer for one tick,
/// mirroring Vanilla `LivingEntity.java:2755-2778`.
/// Returns `(new_count, new_timer)`.
#[must_use]
pub fn tick_stuck_projectile_counter(count: i32, current_timer: i32) -> (i32, i32) {
    if count <= 0 {
        return (0, 0);
    }
    let timer = if current_timer <= 0 {
        calculate_stuck_projectile_removal_time(count)
    } else {
        current_timer
    };
    let next_timer = timer - 1;
    if next_timer <= 0 {
        ((count - 1).max(0), 0)
    } else {
        (count, next_timer)
    }
}

/// Determines whether an equipment update represents a hand swap between main hand and off hand,
/// mirroring Vanilla `LivingEntity.java:2989-3002`:
/// ```java
/// ItemStack.matches(currentMainHand, this.lastEquipmentItems.get(EquipmentSlot.OFFHAND))
/// && ItemStack.matches(currentOffHand, this.lastEquipmentItems.get(EquipmentSlot.MAINHAND))
/// ```
#[must_use]
pub fn is_equipment_hand_swap(
    prev_main_item_id: u16,
    prev_off_item_id: u16,
    new_main_item_id: u16,
    new_off_item_id: u16,
) -> bool {
    new_main_item_id == prev_off_item_id && new_off_item_id == prev_main_item_id
}

/// Default consume duration in seconds in Vanilla MC (`Consumable.DEFAULT_CONSUME_SECONDS = 1.6F`).
pub const DEFAULT_CONSUME_SECONDS: f32 = 1.6;
/// Interval in ticks between consumable sound/particle emissions (`Consumable.CONSUME_EFFECTS_INTERVAL = 4`).
pub const CONSUME_EFFECTS_INTERVAL: i32 = 4;
/// Fractional progress required before emitting consumable use effects (`Consumable.CONSUME_EFFECTS_START_FRACTION = 0.21875F`).
pub const CONSUME_EFFECTS_START_FRACTION: f32 = 0.21875;

/// Computes whether eating/drinking particles and sound effects should be emitted
/// on the current tick of item usage.
///
/// Mirrors vanilla `Consumable.shouldEmitParticlesAndSounds(useItemRemainingTicks)`:
/// ```java
/// int itemUsedForTicks = this.consumeTicks() - useItemRemainingTicks;
/// int waitTicksBeforeUseEffects = (int)(this.consumeTicks() * 0.21875F);
/// boolean isValidTime = itemUsedForTicks > waitTicksBeforeUseEffects;
/// return isValidTime && useItemRemainingTicks % 4 == 0;
/// ```
#[must_use]
pub const fn should_emit_consumable_use_effects(
    consume_ticks: i32,
    use_item_remaining_ticks: i32,
) -> bool {
    let item_used_for_ticks = consume_ticks - use_item_remaining_ticks;
    let wait_ticks = (consume_ticks as f32 * CONSUME_EFFECTS_START_FRACTION) as i32;
    let is_valid_time = item_used_for_ticks > wait_ticks;
    is_valid_time && use_item_remaining_ticks % CONSUME_EFFECTS_INTERVAL == 0
}

/// Calculates volume and pitch for consumable eating/drinking sounds.
///
/// Mirrors vanilla `Consumable.emitParticlesAndSounds`:
/// - Drink: volume 0.5, pitch in [0.9, 1.0] (`Mth.randomBetween(random, 0.9F, 1.0F)`)
/// - Eat: volume 0.5 (50%) or 1.0 (50%), pitch triangle around 1.0 with deviation 0.2 (`random.triangle(1.0F, 0.2F)`)
#[must_use]
pub fn calculate_consumable_sound_volume_and_pitch(
    is_drink: bool,
    rand_bool: bool,
    rand_val1: f32,
    rand_val2: f32,
) -> (f32, f32) {
    if is_drink {
        let pitch = 0.9 + (rand_val1.clamp(0.0, 1.0) * 0.1);
        (0.5, pitch)
    } else {
        let volume = if rand_bool { 0.5 } else { 1.0 };
        let pitch = 1.0 + (rand_val1.clamp(0.0, 1.0) - rand_val2.clamp(0.0, 1.0)) * 0.2;
        (volume, pitch)
    }
}

/// Calculates the Riptide spin attack strength based on enchantment level.
///
/// Mirrors vanilla `riptide.json`: `base: 1.5, per_level_above_first: 0.75`:
/// - Level 1: 1.5
/// - Level 2: 2.25
/// - Level 3: 3.0
#[must_use]
pub fn calculate_riptide_strength(level: u32) -> f32 {
    if level == 0 {
        0.0
    } else {
        1.5 + (level - 1) as f32 * 0.75
    }
}

/// Calculates the 3D velocity push vector for a Riptide launch given yaw, pitch, and riptide strength.
///
/// Mirrors vanilla `TridentItem.java:93-102`:
/// ```java
/// float xd = -Mth.sin(yRot * (float) (Math.PI / 180.0)) * Mth.cos(xRot * (float) (Math.PI / 180.0));
/// float yd = -Mth.sin(xRot * (float) (Math.PI / 180.0));
/// float zd = Mth.cos(yRot * (float) (Math.PI / 180.0)) * Mth.cos(xRot * (float) (Math.PI / 180.0));
/// float dist = Mth.sqrt(xd * xd + yd * yd + zd * zd);
/// xd *= riptideStrength / dist;
/// yd *= riptideStrength / dist;
/// zd *= riptideStrength / dist;
/// ```
#[must_use]
pub fn calculate_riptide_velocity(yaw: f32, pitch: f32, riptide_strength: f32) -> Vector3<f64> {
    let f_yaw = yaw.to_radians();
    let f_pitch = pitch.to_radians();
    let xd = f64::from(-f_yaw.sin() * f_pitch.cos());
    let yd = f64::from(-f_pitch.sin());
    let zd = f64::from(f_yaw.cos() * f_pitch.cos());
    let dist = (xd * xd + yd * yd + zd * zd).sqrt();
    if dist > 0.0 {
        let factor = f64::from(riptide_strength) / dist;
        Vector3::new(xd * factor, yd * factor, zd * factor)
    } else {
        Vector3::new(0.0, 0.0, 0.0)
    }
}

/// Selects the appropriate Riptide sound for a given enchantment level.
///
/// Mirrors vanilla `riptide.json`:
/// - Level 1: `Sound::ItemTridentRiptide1`
/// - Level 2: `Sound::ItemTridentRiptide2`
/// - Level >= 3: `Sound::ItemTridentRiptide3`
#[must_use]
pub fn get_riptide_sound(level: u32) -> Sound {
    match level {
        1 => Sound::ItemTridentRiptide1,
        2 => Sound::ItemTridentRiptide2,
        _ => Sound::ItemTridentRiptide3,
    }
}

/// Determines whether an entity can perform a Riptide launch.
///
/// Mirrors vanilla `TridentItem.java:70, 129`:
/// `riptideStrength > 0.0F && player.isInWaterOrRain() && !player.isPassenger()`
#[must_use]
pub fn can_perform_riptide(riptide_level: u32, is_in_water_or_rain: bool, is_passenger: bool) -> bool {
    riptide_level > 0 && is_in_water_or_rain && !is_passenger
}

/// Determines whether an Elytra glider should receive durability damage on this tick.
///
/// Mirrors vanilla `LivingEntity.java:3188-3195`:
/// ```java
/// int checkFallFlyTicks = this.fallFlyTicks + 1;
/// if (checkFallFlyTicks % 10 == 0) {
///     int freeFallInterval = checkFallFlyTicks / 10;
///     if (freeFallInterval % 2 == 0) {
///         this.getItemBySlot(slotToDamage).hurtAndBreak(1, this, slotToDamage);
///     }
/// }
/// ```
#[must_use]
pub const fn should_damage_elytra(check_fall_fly_ticks: i32) -> bool {
    check_fall_fly_ticks > 0 && check_fall_fly_ticks % 20 == 0
}

/// Determines whether an Elytra glider game event should be emitted on this tick (every 10 ticks).
///
/// Mirrors vanilla `LivingEntity.java:3188-3198`:
/// ```java
/// if (checkFallFlyTicks % 10 == 0) {
///     this.gameEvent(GameEvent.ELYTRA_GLIDE);
/// }
/// ```
#[must_use]
pub const fn should_emit_elytra_game_event(check_fall_fly_ticks: i32) -> bool {
    check_fall_fly_ticks > 0 && check_fall_fly_ticks % 10 == 0
}

/// Calculates firework lifetime in ticks from flight duration and random bounds.
///
/// Mirrors vanilla `FireworkRocketEntity.java:62-70`:
/// ```java
/// int flightCount = 1;
/// Fireworks fireworks = sourceItemStack.get(DataComponents.FIREWORKS);
/// if (fireworks != null) {
///    flightCount += fireworks.flightDuration();
/// }
/// this.lifetime = 10 * flightCount + this.random.nextInt(6) + this.random.nextInt(7);
/// ```
#[must_use]
pub fn calculate_firework_lifetime(flight_duration: Option<u8>, rand_6: i32, rand_7: i32) -> u32 {
    let mut flight_count = 1;
    if let Some(duration) = flight_duration {
        flight_count += duration as i32;
    }
    (10 * flight_count + rand_6 + rand_7).max(0) as u32
}

/// Calculates the new 3D velocity vector applied to an Elytra-gliding entity boosted by a firework rocket.
///
/// Mirrors vanilla `FireworkRocketEntity.java:128-139`:
/// ```java
/// Vec3 lookAngle = this.attachedToEntity.getLookAngle();
/// double power = 1.5;
/// double powerAdd = 0.1;
/// Vec3 movement = this.attachedToEntity.getDeltaMovement();
/// this.attachedToEntity.setDeltaMovement(
///     movement.add(
///         lookAngle.x * 0.1 + (lookAngle.x * 1.5 - movement.x) * 0.5,
///         lookAngle.y * 0.1 + (lookAngle.y * 1.5 - movement.y) * 0.5,
///         lookAngle.z * 0.1 + (lookAngle.z * 1.5 - movement.z) * 0.5
///     )
/// );
/// ```
#[must_use]
pub fn calculate_elytra_firework_boost_velocity(
    current_velocity: Vector3<f64>,
    look_vector: Vector3<f64>,
) -> Vector3<f64> {
    Vector3::new(
        current_velocity.x + look_vector.x * 0.1 + (look_vector.x * 1.5 - current_velocity.x) * 0.5,
        current_velocity.y + look_vector.y * 0.1 + (look_vector.y * 1.5 - current_velocity.y) * 0.5,
        current_velocity.z + look_vector.z * 0.1 + (look_vector.z * 1.5 - current_velocity.z) * 0.5,
    )
}

/// Calculates direct firework explosion damage (e.g. to the attached flyer).
///
/// Mirrors vanilla `FireworkRocketEntity.java:226, 231`:
/// `damageAmount = 5.0F + explosions.size() * 2;`
#[must_use]
pub fn calculate_firework_direct_damage(explosions_count: usize) -> f32 {
    if explosions_count == 0 {
        0.0
    } else {
        5.0 + (explosions_count as f32) * 2.0
    }
}

/// Calculates radial firework explosion damage based on distance from the detonation center.
///
/// Mirrors vanilla `FireworkRocketEntity.java:234-253`:
/// `float damage = damageAmount * (float)Math.sqrt((5.0 - this.distanceTo(target)) / 5.0);`
#[must_use]
pub fn calculate_firework_explosion_damage(explosions_count: usize, distance: f64) -> f32 {
    if explosions_count == 0 || distance > 5.0 {
        0.0
    } else {
        let damage_amount = 5.0 + (explosions_count as f32) * 2.0;
        damage_amount * (((5.0 - distance) / 5.0).max(0.0) as f32).sqrt()
    }
}

/// Returns the vanilla dimensions (width, height, eye_height) for a Player in a given pose.
///
/// Mirrors vanilla `Avatar.java:21-37` and `LivingEntity.java:196`:
/// - `Standing`: (0.6, 1.8, 1.62)
/// - `Crouching`: (0.6, 1.5, 1.27)
/// - `Swimming`: (0.6, 0.6, 0.4)
/// - `FallFlying`: (0.6, 0.6, 0.4)
/// - `SpinAttack`: (0.6, 0.6, 0.4)
/// - `Sleeping`: (0.2, 0.2, 0.2)
/// - `Dying`: (0.2, 0.2, 1.62)
#[must_use]
pub const fn get_player_pose_dimensions(pose: pumpkin_data::entity::EntityPose) -> (f32, f32, f32) {
    use pumpkin_data::entity::EntityPose;
    match pose {
        EntityPose::Standing => (0.6, 1.8, 1.62),
        EntityPose::Crouching => (0.6, 1.5, 1.27),
        EntityPose::Swimming | EntityPose::FallFlying | EntityPose::SpinAttack => (0.6, 0.6, 0.4),
        EntityPose::Sleeping => (0.2, 0.2, 0.2),
        EntityPose::Dying => (0.2, 0.2, 1.62),
        _ => (0.6, 1.8, 1.62),
    }
}

/// Determines the actual pose of a player based on desired pose and collision clearance.
///
/// Mirrors vanilla `Player.java:345-358`:
/// ```java
/// if (this.canPlayerFitWithinBlocksAndEntitiesWhen(Pose.SWIMMING)) {
///    Pose desiredPose = this.getDesiredPose();
///    Pose actualPose;
///    if (this.isSpectator() || this.isPassenger() || this.canPlayerFitWithinBlocksAndEntitiesWhen(desiredPose)) {
///       actualPose = desiredPose;
///    } else if (this.canPlayerFitWithinBlocksAndEntitiesWhen(Pose.CROUCHING)) {
///       actualPose = Pose.CROUCHING;
///    } else {
///       actualPose = Pose.SWIMMING;
///    }
///    this.setPose(actualPose);
/// }
/// ```
#[must_use]
pub const fn determine_player_actual_pose(
    desired_pose: pumpkin_data::entity::EntityPose,
    is_spectator: bool,
    is_passenger: bool,
    can_fit_swimming: bool,
    can_fit_desired: bool,
    can_fit_crouching: bool,
    current_pose: pumpkin_data::entity::EntityPose,
) -> pumpkin_data::entity::EntityPose {
    use pumpkin_data::entity::EntityPose;
    if !can_fit_swimming {
        current_pose
    } else if is_spectator || is_passenger || can_fit_desired {
        desired_pose
    } else if can_fit_crouching {
        EntityPose::Crouching
    } else {
        EntityPose::Swimming
    }
}

/// Calculates the swimming vertical acceleration impulse and condition based on look angle and head submersion.
///
/// Mirrors vanilla `Player.java:1392-1402`:
/// ```java
/// double lookAngleY = this.getLookAngle().y;
/// double multiplier = lookAngleY < -0.2 ? 0.085 : 0.06;
/// if (lookAngleY <= 0.0
///    || this.jumping
///    || !this.level().getFluidState(BlockPos.containing(this.getX(), this.getY() + 1.0 - 0.1, this.getZ())).isEmpty()) {
///    Vec3 movement = this.getDeltaMovement();
///    this.setDeltaMovement(movement.add(0.0, (lookAngleY - movement.y) * multiplier, 0.0));
/// }
/// ```
#[must_use]
pub fn calculate_swimming_vertical_impulse(
    look_angle_y: f64,
    current_vel_y: f64,
    is_jumping: bool,
    head_submerged: bool,
) -> (f64, bool) {
    let should_apply = look_angle_y <= 0.0 || is_jumping || head_submerged;
    if !should_apply {
        return (0.0, false);
    }
    let multiplier = if look_angle_y < -0.2 { 0.085 } else { 0.06 };
    let impulse = (look_angle_y - current_vel_y) * multiplier;
    (impulse, true)
}

/// Calculates the total oxygen bonus combining base attribute and helmet Respiration enchantment.
///
/// Mirrors vanilla `Enchantments.java:287-295` (Respiration adds 1.0 * level to Attributes.OXYGEN_BONUS).
#[must_use]
pub fn calculate_respiration_oxygen_bonus(helmet_respiration_level: i32, base_oxygen_bonus: f64) -> f64 {
    base_oxygen_bonus + (helmet_respiration_level.max(0) as f64)
}

/// Determines whether an entity's air supply should decrease this tick based on oxygen bonus.
///
/// Mirrors vanilla `LivingEntity.java:591-601`:
/// `oxygenBonus > 0.0 && this.random.nextDouble() >= 1.0 / (oxygenBonus + 1.0) ? currentSupply : currentSupply - 1;`
#[must_use]
pub fn should_decrease_air_supply(oxygen_bonus: f64, random_sample: f64) -> bool {
    if oxygen_bonus <= 0.0 {
        true
    } else {
        random_sample < 1.0 / (oxygen_bonus + 1.0)
    }
}

/// Calculates increased air supply when breathing or surfaced.
///
/// Mirrors vanilla `LivingEntity.java:603-605`:
/// `Math.min(currentSupply + 4, this.getMaxAirSupply());`
#[must_use]
pub fn calculate_increased_air_supply(current_air: i32, max_air: i32) -> i32 {
    (current_air + 4).min(max_air)
}

/// Determines whether an entity should take drowning damage.
///
/// Mirrors vanilla `LivingEntity.java:509-511`:
/// `return this.getAirSupply() <= -20;`
#[must_use]
pub fn should_take_drowning_damage(current_air: i32) -> bool {
    current_air <= -20
}

/// Checks if any water breathing effect is active.
///
/// Mirrors vanilla `MobEffectUtil.java:43-45`:
/// `mob.hasEffect(MobEffects.WATER_BREATHING) || mob.hasEffect(MobEffects.CONDUIT_POWER) || mob.hasEffect(MobEffects.BREATH_OF_THE_NAUTILUS);`
#[must_use]
pub fn has_water_breathing_effects(
    has_water_breathing: bool,
    has_conduit_power: bool,
    has_nautilus_breath: bool,
) -> bool {
    has_water_breathing || has_conduit_power || has_nautilus_breath
}

/// Checks if active status effects allow refilling air supply underwater.
///
/// Mirrors vanilla `MobEffectUtil.java:47-49`:
/// `!mob.hasEffect(MobEffects.BREATH_OF_THE_NAUTILUS) || mob.hasEffect(MobEffects.WATER_BREATHING) || mob.hasEffect(MobEffects.CONDUIT_POWER);`
#[must_use]
pub fn should_effects_refill_air_supply(
    has_water_breathing: bool,
    has_conduit_power: bool,
    has_nautilus_breath: bool,
) -> bool {
    !has_nautilus_breath || has_water_breathing || has_conduit_power
}

/// Calculates submerged mining speed taking base attribute and helmet Aqua Affinity enchantment into account.
///
/// Mirrors vanilla `Enchantments.java:311-320`:
/// Aqua Affinity applies `ADD_MULTIPLIED_TOTAL` with `4.0 * level` to `Attributes.SUBMERGED_MINING_SPEED` (base 0.2),
/// turning `0.2 * (1 + 4.0) = 1.0`.
#[must_use]
pub fn calculate_submerged_mining_speed(base_submerged_speed: f32, aqua_affinity_level: i32) -> f32 {
    let multiplier = 1.0 + 4.0 * (aqua_affinity_level.max(0) as f32);
    base_submerged_speed * multiplier
}

/// Calculates Mining Fatigue speed penalty multiplier.
///
/// Mirrors vanilla `Player.java:595`:
/// `float scale = (float)Math.pow(0.3, amplifier + 1);`
#[must_use]
pub fn calculate_mining_fatigue_scale(amplifier: u8) -> f32 {
    0.3_f32.powi(i32::from(amplifier) + 1)
}

/// Calculates total player destroy speed for a block matching vanilla formula order.
///
/// Mirrors vanilla `Player.java:583-609`:
/// 1. `heldItem.getDestroySpeed(state)`
/// 2. If speed > 1.0: `speed += MINING_EFFICIENCY`
/// 3. If Haste/Conduit: `speed *= 1.0 + (haste_amp + 1) * 0.2`
/// 4. If Mining Fatigue: `speed *= 0.3^(fatigue_amp + 1)`
/// 5. `speed *= BLOCK_BREAK_SPEED`
/// 6. If `is_eye_in_water`: `speed *= SUBMERGED_MINING_SPEED`
/// 7. If `!on_ground`: `speed /= 5.0`
#[must_use]
pub fn calculate_player_destroy_speed(
    tool_speed: f32,
    efficiency_bonus: f32,
    haste_amplifier: Option<u32>,
    fatigue_amplifier: Option<u8>,
    block_break_speed: f32,
    is_eye_in_water: bool,
    submerged_speed: f32,
    on_ground: bool,
) -> f32 {
    let mut speed = tool_speed;
    if speed > 1.0 {
        speed += efficiency_bonus;
    }

    if let Some(amp) = haste_amplifier {
        speed *= 1.0 + ((amp + 1) as f32) * 0.2;
    }

    if let Some(amp) = fatigue_amplifier {
        speed *= calculate_mining_fatigue_scale(amp);
    }

    speed *= block_break_speed;

    if is_eye_in_water {
        speed *= submerged_speed;
    }

    if !on_ground {
        speed /= 5.0;
    }

    speed
}

/// Calculates per-tick destroy progress for a block.
///
/// Mirrors vanilla `BlockBehaviour.java:345-352`:
/// `player.getDestroySpeed(state) / destroySpeed / (canHarvest ? 30 : 100)`
#[must_use]
pub fn calculate_block_destroy_progress(destroy_speed: f32, block_hardness: f32, can_harvest: bool) -> f32 {
    if block_hardness == -1.0 {
        return 0.0;
    }
    let modifier = if can_harvest { 30.0 } else { 100.0 };
    destroy_speed / block_hardness / modifier
}

/// Calculates Wind Charge throw sound pitch.
///
/// Mirrors vanilla `WindChargeItem.java:49`:
/// `0.4F / (random.nextFloat() * 0.4F + 0.8F)`
#[must_use]
pub fn calculate_wind_charge_throw_pitch(random_sample: f32) -> f32 {
    0.4 / (random_sample * 0.4 + 0.8)
}

/// Calculates Wind Charge projectile spawn position from player eye level.
///
/// Mirrors vanilla `WindChargeItem.java:31`:
/// `new WindCharge(player, level, player.position().x(), player.getEyePosition().y(), player.position().z())`
#[must_use]
pub fn calculate_wind_charge_spawn_position(player_pos: Vector3<f64>, eye_y: f64) -> Vector3<f64> {
    Vector3::new(player_pos.x, eye_y, player_pos.z)
}

/// Calculates explosion position on block collision with 0.25 normal offset.
///
/// Mirrors vanilla `AbstractWindCharge.java:107-109`:
/// `Vec3 scaledNormal = Vec3.atLowerCornerOf(collisionNormal).multiply(0.25, 0.25, 0.25);`
/// `Vec3 explosionPos = hitResult.getLocation().add(scaledNormal);`
#[must_use]
pub fn calculate_wind_charge_block_explosion_pos(
    hit_pos: Vector3<f64>,
    face_offset: Vector3<f64>,
) -> Vector3<f64> {
    hit_pos + face_offset * 0.25
}

/// Returns the vanilla cooldown ticks for wind charge throw (0.5 seconds = 10 ticks).
///
/// Mirrors vanilla `Items.java:2233`:
/// `new Item.Properties().useCooldown(0.5F)`
#[must_use]
pub const fn calculate_wind_charge_cooldown_ticks() -> i32 {
    10
}

/// Calculates updated position and velocity for Thrown Trident Loyalty return step.
///
/// Mirrors vanilla `ThrownTrident.java:83-87`:
/// ```java
/// this.setNoPhysics(true);
/// Vec3 vec = currentOwner.getEyePosition().subtract(this.position());
/// this.setPosRaw(this.getX(), this.getY() + vec.y * 0.015 * loyalty, this.getZ());
/// double accel = 0.05 * loyalty;
/// this.setDeltaMovement(this.getDeltaMovement().scale(0.95).add(vec.normalize().scale(accel)));
/// ```
#[must_use]
pub fn calculate_trident_loyalty_step(
    pos: Vector3<f64>,
    velocity: Vector3<f64>,
    owner_eye_pos: Vector3<f64>,
    loyalty: u8,
) -> (Vector3<f64>, Vector3<f64>) {
    let delta = owner_eye_pos - pos;
    let dist = delta.length();
    let norm = if dist > 1e-4 {
        delta * (1.0 / dist)
    } else {
        Vector3::new(0.0, 0.0, 0.0)
    };
    let accel = 0.05 * f64::from(loyalty);
    let new_vel = velocity * 0.95 + norm * accel;
    let y_offset = delta.y * 0.015 * f64::from(loyalty);
    let new_pos = Vector3::new(
        pos.x + new_vel.x,
        pos.y + y_offset + new_vel.y,
        pos.z + new_vel.z,
    );
    (new_pos, new_vel)
}

/// Determines if trident should return via Loyalty enchantment.
///
/// Mirrors vanilla `ThrownTrident.java:70`:
/// `if (loyalty > 0 && (this.dealtDamage || this.isNoPhysics()) && currentOwner != null)`
#[must_use]
pub const fn should_trident_return_loyalty(
    loyalty: u8,
    dealt_damage: bool,
    is_no_physics: bool,
    has_owner: bool,
) -> bool {
    loyalty > 0 && (dealt_damage || is_no_physics) && has_owner
}

/// Determines if trident should start returning after being stuck in ground.
///
/// Mirrors vanilla `ThrownTrident.java:64-66`:
/// `if (this.inGroundTime > 4) { this.dealtDamage = true; }`
#[must_use]
pub const fn should_trident_trigger_return_after_in_ground(in_ground_time: u32) -> bool {
    in_ground_time > 4
}

/// Checks whether owner is acceptable to receive returned trident.
///
/// Mirrors vanilla `ThrownTrident.java:100-102`:
/// `return currentOwner == null || !currentOwner.isAlive() ? false : !(currentOwner instanceof ServerPlayer) || !currentOwner.isSpectator();`
#[must_use]
pub const fn is_acceptable_trident_return_owner(is_owner_alive: bool, is_spectator: bool) -> bool {
    is_owner_alive && !is_spectator
}

/// Checks if trident has fallen below world void threshold and should auto-recall to owner.
///
/// Under vanilla `Entity.java:605` (`getY() < min_y - 64`), entities enter void.
/// Tridents with Loyalty auto-recall to prevent item loss in void.
#[must_use]
pub const fn should_trident_auto_recall_from_void(
    pos_y: f64,
    loyalty: u8,
    has_owner: bool,
    min_y: f64,
) -> bool {
    pos_y < (min_y - 64.0) && loyalty > 0 && has_owner
}

/// Checks if Channeling enchantment should strike lightning on entity hit.
///
/// Mirrors vanilla `Enchantments.java:1070-1074`:
/// `WeatherCheck.weather().setThundering(true)` && `canSeeSky(true)`
#[must_use]
pub const fn should_channeling_strike_entity(
    has_channeling: bool,
    is_thundering: bool,
    can_see_sky: bool,
) -> bool {
    has_channeling && is_thundering && can_see_sky
}

/// Checks if Channeling enchantment should strike lightning on block hit.
///
/// Mirrors vanilla `Enchantments.java:1086-1095`:
/// `WeatherCheck.weather().setThundering(true)` && `canSeeSky(true)` && `is_lightning_rod`
#[must_use]
pub const fn should_channeling_strike_block(
    has_channeling: bool,
    is_thundering: bool,
    can_see_sky: bool,
    is_lightning_rod: bool,
) -> bool {
    has_channeling && is_thundering && can_see_sky && is_lightning_rod
}

/// Calculates kinetic damage from horizontal collision while fall flying (flying into wall).
///
/// Mirrors vanilla `LivingEntity.java:2616-2617`:
/// ```java
/// double diff = moveHorLength - newMoveHorLength;
/// float dmg = (float)(diff * 10.0 - 3.0);
/// ```
#[must_use]
pub fn calculate_fly_into_wall_damage(horizontal_speed_diff: f64) -> f32 {
    let dmg = (horizontal_speed_diff.mul_add(10.0, -3.0)) as f32;
    if dmg > 0.0 { dmg } else { 0.0 }
}

/// Returns the sound played when receiving fly-into-wall kinetic damage.
///
/// Mirrors vanilla `LivingEntity.java:2619`:
/// `this.playSound(this.getFallDamageSound((int)dmg), 1.0F, 1.0F);`
/// where `getFallDamageSound(dmg)` returns big fall sound if dmg > 4, else small fall sound.
#[must_use]
pub fn get_fly_into_wall_sound(damage: f32) -> pumpkin_data::sound::Sound {
    if damage > 4.0 {
        pumpkin_data::sound::Sound::EntityGenericBigFall
    } else {
        pumpkin_data::sound::Sound::EntityGenericSmallFall
    }
}

/// Determines whether an entity can glide using Elytra.
///
/// Mirrors vanilla `LivingEntity.java:3202-3214`:
/// `return !this.onGround() && !this.isPassenger() && !this.hasEffect(MobEffects.LEVITATION) && hasGliderItem;`
#[must_use]
pub const fn can_entity_glide(
    on_ground: bool,
    is_passenger: bool,
    has_levitation: bool,
    has_glider_equipped: bool,
) -> bool {
    !on_ground && !is_passenger && !has_levitation && has_glider_equipped
}

/// Determines whether fall flying should be stopped when touching a climbable block (ladder, vine).
///
/// Mirrors vanilla `LivingEntity.java:2567-2570`:
/// ```java
/// if (this.onClimbable()) {
///    this.travelInAir(input);
///    this.stopFallFlying();
/// }
/// ```
#[must_use]
pub const fn should_stop_fall_flying_on_climbable(on_climbable: bool) -> bool {
    on_climbable
}

/// Calculates updated fall-flying movement per tick matching vanilla aerodynamics.
///
/// Mirrors vanilla `LivingEntity.java:2587-2612`:
/// - Pitch lean lift force = cos(lean_angle)^2
/// - Gravity modification: gravity * (-1.0 + lift_force * 0.75)
/// - Dive pull down into horizontal speed conversion: movement.y * -0.1 * lift_force
/// - Pitch up lift: horizontal_speed * -sin(lean_angle) * 0.04 (up to 3.2 y multiplier)
/// - Direction alignment dampening with 0.1 factor
/// - Drag multipliers: (0.99, 0.98, 0.99)
#[must_use]
pub fn calculate_fall_flying_movement_step(
    mut movement: Vector3<f64>,
    pitch_deg: f32,
    yaw_deg: f32,
    gravity: f64,
) -> Vector3<f64> {
    let pitch_rad = (pitch_deg as f64).to_radians();
    let yaw_rad = (yaw_deg as f64).to_radians();

    let look_x = -pitch_rad.cos() * yaw_rad.sin();
    let look_y = -pitch_rad.sin();
    let look_z = pitch_rad.cos() * yaw_rad.cos();
    let look_angle = Vector3::new(look_x, look_y, look_z);

    let lean_angle = pitch_rad;
    let look_hor_length = (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
    let move_hor_length = (movement.x * movement.x + movement.z * movement.z).sqrt();
    let cos_lean = lean_angle.cos();
    let lift_force = cos_lean * cos_lean;

    movement.y += gravity * (-1.0 + lift_force * 0.75);

    if movement.y < 0.0 && look_hor_length > 0.0 {
        let convert = movement.y * -0.1 * lift_force;
        movement.x += look_angle.x * convert / look_hor_length;
        movement.y += convert;
        movement.z += look_angle.z * convert / look_hor_length;
    }

    if lean_angle < 0.0 && look_hor_length > 0.0 {
        let convert = move_hor_length * -lean_angle.sin() * 0.04;
        movement.x += -look_angle.x * convert / look_hor_length;
        movement.y += convert * 3.2;
        movement.z += -look_angle.z * convert / look_hor_length;
    }

    if look_hor_length > 0.0 {
        movement.x += (look_angle.x / look_hor_length * move_hor_length - movement.x) * 0.1;
        movement.z += (look_angle.z / look_hor_length * move_hor_length - movement.z) * 0.1;
    }

    movement.x *= 0.99;
    movement.y *= 0.98;
    movement.z *= 0.99;

    movement
}

/// Calculates splash potion effect scale based on squared distance from impact center.
///
/// Mirrors vanilla `ThrownSplashPotion.java:54-56`:
/// ```java
/// double dist = potionAabb.distanceToSqr(entity.getBoundingBox().inflate(margin));
/// if (dist < 16.0) {
///    double scale = 1.0 - Math.sqrt(dist) / 4.0;
/// }
/// ```
#[must_use]
pub fn calculate_splash_potion_scale(distance_sq: f64) -> f32 {
    if (0.0..16.0).contains(&distance_sq) {
        (1.0 - distance_sq.sqrt() / 4.0) as f32
    } else {
        0.0
    }
}

/// Calculates scaled effect duration for splash potion effects.
///
/// Mirrors vanilla `ThrownSplashPotion.java:63`:
/// `int duration = effectInstance.mapDuration(d -> (int)(scale * d * durationScale + 0.5));`
#[must_use]
pub fn calculate_scaled_effect_duration(base_duration: i32, scale: f32, duration_scale: f32) -> i32 {
    ((scale * (base_duration as f32) * duration_scale) + 0.5) as i32
}

/// Checks whether a scaled effect should be applied to an entity.
///
/// Mirrors vanilla `ThrownSplashPotion.java:67`:
/// `if (!newEffect.endsWithin(20)) { entity.addEffect(newEffect, effectSource); }`
#[must_use]
pub const fn should_apply_scaled_effect(duration: i32) -> bool {
    duration > 20
}

/// Returns the sound level event ID for potion break.
///
/// Mirrors vanilla `AbstractThrownPotion.java:80-90`:
/// `level.levelEvent(hasInstantEffects ? 1054 : 1053, blockPosition, 0)`
#[must_use]
pub const fn get_splash_potion_break_sound_event(has_instant_effects: bool) -> i32 {
    if has_instant_effects {
        1054
    } else {
        1053
    }
}

/// Returns the particle level event ID for potion break.
///
/// Mirrors vanilla `AbstractThrownPotion.java:81, 86`:
/// `level.levelEvent(hasInstantEffects ? 2007 : 2002, blockPosition, color)`
#[must_use]
pub const fn get_splash_potion_particle_event(has_instant_effects: bool) -> i32 {
    if has_instant_effects {
        2007
    } else {
        2002
    }
}

/// Checks whether a water potion splash should extinguish an entity.
///
/// Mirrors vanilla `AbstractThrownPotion.java:100, 109`:
/// `if (extinguishesEntities && entity.isOnFire() && entity.isAlive()) { entity.extinguishFire(); }`
#[must_use]
pub const fn should_water_potion_extinguish_entity(
    is_water: bool,
    is_on_fire: bool,
    is_alive: bool,
) -> bool {
    is_water && is_on_fire && is_alive
}

/// Returns the vanilla default duration (in ticks) for lingering potion clouds (30 seconds = 600 ticks).
///
/// Mirrors vanilla `AreaEffectCloud.java:33`:
/// `private int duration = 600;`
#[must_use]
pub const fn get_lingering_potion_initial_duration() -> i32 {
    600
}

/// Returns the vanilla initial radius (in blocks) for lingering potion clouds (3.0 blocks).
///
/// Mirrors vanilla `AreaEffectCloud.java:29`:
/// `public static final float DEFAULT_RADIUS = 3.0F;`
#[must_use]
pub const fn get_lingering_potion_initial_radius() -> f32 {
    3.0
}

/// Returns the vanilla wait time (in ticks) before lingering potion cloud activates (0.5 seconds = 10 ticks).
///
/// Mirrors vanilla `ThrownLingeringPotion.java:47`:
/// `areaEffectCloud.setWaitTime(10);`
#[must_use]
pub const fn get_lingering_potion_wait_time() -> i32 {
    10
}

/// Returns the vanilla radius shrinkage on use when an entity is affected (-0.5 blocks).
///
/// Mirrors vanilla `ThrownLingeringPotion.java:46`:
/// `areaEffectCloud.setRadiusOnUse(-0.5F);`
#[must_use]
pub const fn get_lingering_potion_radius_on_use() -> f32 {
    -0.5
}

/// Returns the vanilla duration change on use when an entity is affected (0 ticks).
///
/// Mirrors vanilla `AreaEffectCloud.java:36` (not modified by `ThrownLingeringPotion`):
/// `private int durationOnUse = 0;`
#[must_use]
pub const fn get_lingering_potion_duration_on_use() -> i32 {
    0
}

/// Returns the vanilla reapplication delay between reapplying effects to the same entity (20 ticks = 1 second).
///
/// Mirrors vanilla `AreaEffectCloud.java:35`:
/// `private int reapplicationDelay = 20;`
#[must_use]
pub const fn get_lingering_potion_reapplication_delay() -> i32 {
    20
}

/// Calculates radius shrinkage per tick for lingering potion clouds.
///
/// Mirrors vanilla `ThrownLingeringPotion.java:48`:
/// `areaEffectCloud.setRadiusPerTick(-areaEffectCloud.getRadius() / (float)areaEffectCloud.getDuration());`
#[must_use]
pub fn calculate_lingering_potion_radius_per_tick(radius: f32, duration: i32) -> f32 {
    -radius / (duration as f32).max(1.0)
}

/// Checks whether an area effect cloud has expired by duration or shrunk below minimum radius (0.5 blocks).
///
/// Mirrors vanilla `AreaEffectCloud.java:191-192, 204-206, 239-241`:
/// ```java
/// if (this.duration != -1 && this.tickCount - this.waitTime >= this.duration) { this.discard(); }
/// if (radius < 0.5F) { this.discard(); }
/// ```
#[must_use]
pub const fn should_area_effect_cloud_discard(
    age: i32,
    wait_time: i32,
    duration: i32,
    radius: f32,
) -> bool {
    if duration != -1 && (age - wait_time) >= duration {
        return true;
    }
    if radius < 0.5 {
        return true;
    }
    false
}

/// Checks whether an entity is within horizontal radius of an area effect cloud.
///
/// Mirrors vanilla `AreaEffectCloud.java:223-226`:
/// ```java
/// double xd = entity.getX() - this.getX();
/// double zd = entity.getZ() - this.getZ();
/// double dist = xd * xd + zd * zd;
/// if (dist <= radius * radius)
/// ```
#[must_use]
pub fn is_entity_in_area_effect_cloud_radius(
    cloud_x: f64,
    cloud_z: f64,
    entity_x: f64,
    entity_z: f64,
    radius: f32,
) -> bool {
    let dx = entity_x - cloud_x;
    let dz = entity_z - cloud_z;
    let dist_sq = dx * dx + dz * dz;
    dist_sq <= ((radius * radius) as f64)
}

/// Calculates arrow damage based on impact velocity, base damage, and critical status.
///
/// Mirrors vanilla `AbstractArrow.java:434, 452-455`:
/// ```java
/// int damage = Mth.ceil(Mth.clamp(pow * arrowDamage, 0.0, 2.147483647E9));
/// if (this.isCritArrow()) {
///    long dmgIncrease = this.random.nextInt(damage / 2 + 2);
///    damage = (int)Math.min(dmgIncrease + damage, 2147483647L);
/// }
/// ```
#[must_use]
pub fn calculate_arrow_damage(
    speed: f64,
    base_damage: f64,
    is_critical: bool,
    crit_rng: u32,
) -> i32 {
    let raw = (speed * base_damage).clamp(0.0, i32::MAX as f64).ceil() as i32;
    if is_critical {
        let max_bonus = (raw / 2 + 2).max(1) as u32;
        let bonus = (crit_rng % max_bonus) as i32;
        raw.saturating_add(bonus)
    } else {
        raw
    }
}

/// Calculates punch enchantment knockback impulse scaled by victim's knockback resistance.
///
/// Mirrors vanilla `AbstractArrow.java:517-518`:
/// ```java
/// double knockbackResistance = Math.max(0.0, 1.0 - mob.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE));
/// Vec3 movement = this.getDeltaMovement().multiply(1.0, 0.0, 1.0).normalize().scale(knockback * 0.6 * knockbackResistance);
/// ```
#[must_use]
pub fn calculate_arrow_punch_knockback(punch_level: f64, victim_knockback_resistance: f64) -> f64 {
    let resistance_factor = (1.0 - victim_knockback_resistance).max(0.0);
    punch_level * 0.6 * resistance_factor
}

/// Determines whether a stuck arrow should be added to the victim mob's arrow count.
///
/// Mirrors vanilla `AbstractArrow.java:468-470`:
/// `if (!this.level().isClientSide() && this.getPierceLevel() <= 0) { mob.setArrowCount(mob.getArrowCount() + 1); }`
#[must_use]
pub const fn should_arrow_stick_in_entity(pierce_level: u8) -> bool {
    pierce_level == 0
}

/// Calculates sound pitch for arrow impact sound.
///
/// Mirrors vanilla `AbstractArrow.java:495, 539`:
/// `1.2F / (this.random.nextFloat() * 0.2F + 0.9F)`
#[must_use]
pub fn calculate_arrow_hit_sound_pitch(rng_val: f32) -> f32 {
    1.2 / (rng_val * 0.2 + 0.9)
}

/// Returns the vanilla duration of glowing effect from a spectral arrow (10 seconds = 200 ticks).
///
/// Mirrors vanilla `SpectralArrow.java:18`:
/// `private static final int DEFAULT_DURATION = 200;`
#[must_use]
pub const fn get_spectral_arrow_duration() -> i32 {
    200
}

/// Calculates radius change per tick for Dragon Fireball area effect cloud (+4.0 / 600.0).
///
/// Mirrors vanilla `DragonFireball.java:43`:
/// `cloud.setRadiusPerTick((7.0F - cloud.getRadius()) / (float)cloud.getDuration());`
#[must_use]
pub fn calculate_dragon_fireball_radius_per_tick(
    initial_radius: f32,
    target_radius: f32,
    duration: i32,
) -> f32 {
    (target_radius - initial_radius) / (duration as f32).max(1.0)
}

/// Checks whether a small fireball is allowed to place fire on block hit.
///
/// Mirrors vanilla `SmallFireball.java:55`:
/// `if (!(owner instanceof Mob) || serverLevel.getGameRules().get(GameRules.MOB_GRIEFING))`
#[must_use]
pub const fn can_small_fireball_place_fire(is_mob_owner: bool, mob_griefing: bool) -> bool {
    !is_mob_owner || mob_griefing
}

/// Returns the vanilla initial radius for dragon fireball breath cloud (3.0 blocks).
///
/// Mirrors vanilla `DragonFireball.java:41`:
/// `cloud.setRadius(3.0F);`
#[must_use]
pub const fn get_dragon_fireball_initial_radius() -> f32 {
    3.0
}

/// Returns the vanilla max radius for dragon fireball breath cloud expansion (7.0 blocks).
///
/// Mirrors vanilla `DragonFireball.java:43`:
/// `7.0F`
#[must_use]
pub const fn get_dragon_fireball_max_radius() -> f32 {
    7.0
}

/// Returns the vanilla duration for dragon fireball breath cloud (600 ticks = 30 seconds).
///
/// Mirrors vanilla `DragonFireball.java:42`:
/// `cloud.setDuration(600);`
#[must_use]
pub const fn get_dragon_fireball_duration() -> i32 {
    600
}

/// Returns the vanilla level event ID for dragon breath explosion (2006).
///
/// Mirrors vanilla `DragonFireball.java:56`:
/// `this.level().levelEvent(2006, this.blockPosition(), this.isSilent() ? -1 : 1);`
#[must_use]
pub const fn get_dragon_breath_level_event() -> i32 {
    2006
}

/// Returns inertia multiplier for Wither Skull projectile based on dangerous flag.
///
/// Mirrors vanilla `WitherSkull.java:42-44`:
/// `return this.isDangerous() ? 0.73F : 0.95F;`
#[must_use]
pub const fn get_wither_skull_inertia(is_dangerous: bool) -> f64 {
    if is_dangerous {
        0.73
    } else {
        0.95
    }
}

/// Returns wither effect duration (in ticks) inflicted by Wither Skull based on world difficulty.
///
/// Mirrors vanilla `WitherSkull.java:79-88`:
/// - Normal difficulty: 10 seconds (200 ticks)
/// - Hard difficulty: 40 seconds (800 ticks)
/// - Easy/Peaceful: 0 seconds (no wither effect)
#[must_use]
pub const fn get_wither_skull_effect_duration(difficulty: pumpkin_util::Difficulty) -> i32 {
    match difficulty {
        pumpkin_util::Difficulty::Hard => 800,
        pumpkin_util::Difficulty::Normal => 200,
        pumpkin_util::Difficulty::Easy | pumpkin_util::Difficulty::Peaceful => 0,
    }
}

/// Calculates heal amount granted to Wither boss owner upon killing a victim entity (5.0 HP).
///
/// Mirrors vanilla `WitherSkull.java:71`:
/// `livingOwner.heal(5.0F);`
#[must_use]
pub const fn get_wither_skull_kill_heal_amount() -> f32 {
    5.0
}

/// Calculates experience orb amount dropped by thrown experience bottle.
///
/// Mirrors vanilla `ThrownExperienceBottle.java:48`:
/// `int xpCount = 3 + this.random.nextInt(5) + this.random.nextInt(5);`
#[must_use]
pub const fn calculate_experience_bottle_xp(rand1: i32, rand2: i32) -> i32 {
    3 + rand1 + rand2
}

/// Returns default gravity acceleration for thrown experience bottle (0.07).
///
/// Mirrors vanilla `ThrownExperienceBottle.java:36`:
/// `return 0.07;`
#[must_use]
pub const fn get_experience_bottle_gravity() -> f64 {
    0.07
}

/// Returns potion break splash color int for thrown experience bottle (-13083194).
///
/// Mirrors vanilla `ThrownExperienceBottle.java:43`:
/// `level.levelEvent(2002, this.blockPosition(), -13083194);`
#[must_use]
pub const fn get_experience_bottle_splash_color() -> i32 {
    -13_083_194
}

/// Returns base damage dealt by Shulker bullet projectile (4.0 HP).
///
/// Mirrors vanilla `ShulkerBullet.java:285`:
/// `boolean wasHurt = target.hurtOrSimulate(damageSource, 4.0F);`
#[must_use]
pub const fn get_shulker_bullet_damage() -> f32 {
    4.0
}

/// Returns levitation effect duration applied by Shulker bullet (200 ticks = 10 seconds).
///
/// Mirrors vanilla `ShulkerBullet.java:292`:
/// `new MobEffectInstance(MobEffects.LEVITATION, 200)`
#[must_use]
pub const fn get_shulker_bullet_levitation_duration() -> i32 {
    200
}

/// Checks whether a Shulker bullet should instantly despawn due to Peaceful difficulty.
///
/// Mirrors vanilla `ShulkerBullet.java:179-181`:
/// `if (this.level().getDifficulty() == Difficulty.PEACEFUL) { this.discard(); }`
#[must_use]
pub const fn should_shulker_bullet_despawn_in_peaceful(difficulty: pumpkin_util::Difficulty) -> bool {
    matches!(difficulty, pumpkin_util::Difficulty::Peaceful)
}

/// Returns the particle count when a Shulker bullet is deflected / hit by an attack (15 crit particles).
///
/// Mirrors vanilla `ShulkerBullet.java:333`:
/// `level.sendParticles(ParticleTypes.CRIT, this.getX(), this.getY(), this.getZ(), 15, 0.2, 0.2, 0.2, 0.0);`
#[must_use]
pub const fn get_shulker_bullet_deflect_particle_count() -> i32 {
    15
}

/// Returns base damage dealt by Llama spit projectile (1.0 HP).
///
/// Mirrors vanilla `LlamaSpit.java:75`:
/// `target.hurtServer(serverLevel, damageSource, 1.0F)`
#[must_use]
pub const fn get_llama_spit_damage() -> f32 {
    1.0
}

/// Returns default gravity acceleration for Llama spit (0.06).
///
/// Mirrors vanilla `LlamaSpit.java:39`:
/// `return 0.06;`
#[must_use]
pub const fn get_llama_spit_gravity() -> f64 {
    0.06
}

/// Returns air drag factor for Llama spit (0.99).
///
/// Mirrors vanilla `LlamaSpit.java:66`:
/// `return 0.99F;`
#[must_use]
pub const fn get_llama_spit_air_drag() -> f32 {
    0.99
}

/// Calculates chicken hatch count from thrown egg impact based on vanilla probabilities.
///
/// Mirrors vanilla `ThrownEgg.java:70-74`:
/// ```java
/// if (this.random.nextInt(8) == 0) {
///    int count = 1;
///    if (this.random.nextInt(32) == 0) {
///       count = 4;
///    }
/// }
/// ```
#[must_use]
pub const fn calculate_egg_hatch_count(rand_8: i32, rand_32: i32) -> u32 {
    if rand_8 == 0 {
        if rand_32 == 0 {
            4
        } else {
            1
        }
    } else {
        0
    }
}

/// Returns damage dealt by snowball projectile (3.0 for Blaze, 0.0 for all others).
///
/// Mirrors vanilla `Snowball.java:58-59`:
/// `int damage = entity instanceof Blaze ? 3 : 0;`
#[must_use]
pub const fn get_snowball_damage(is_blaze: bool) -> f32 {
    if is_blaze {
        3.0
    } else {
        0.0
    }
}

/// Checks whether an Ender Pearl impact should spawn an Endermite.
///
/// Mirrors vanilla `ThrownEnderpearl.java:107`:
/// `this.random.nextFloat() < 0.05F && level.isSpawningMonsters() && level.getLevelData().getDifficulty() != Difficulty.PEACEFUL`
#[must_use]
pub const fn should_ender_pearl_spawn_endermite(
    chance_sample: f32,
    is_spawning_monsters: bool,
    difficulty: pumpkin_util::Difficulty,
) -> bool {
    chance_sample < 0.05 && is_spawning_monsters && !matches!(difficulty, pumpkin_util::Difficulty::Peaceful)
}

/// Returns self damage dealt to owner upon Ender Pearl teleportation (5.0 HP).
///
/// Mirrors vanilla `ThrownEnderpearl.java:125`:
/// `newOwner.hurtServer(player.level(), this.damageSources().enderPearl(), 5.0F);`
#[must_use]
pub const fn get_ender_pearl_self_damage() -> f32 {
    5.0
}

/// Returns base damage dealt by Evoker Fangs attack (6.0 HP).
///
/// Mirrors vanilla `EvokerFangs.java:110, 117`:
/// `6.0F`
#[must_use]
pub const fn get_evoker_fangs_damage() -> f32 {
    6.0
}

/// Checks whether the Evoker Fangs should deal attack damage on this tick.
///
/// Mirrors vanilla `EvokerFangs.java:89`:
/// `if (this.warmupDelayTicks == -8)`
#[must_use]
pub const fn is_evoker_fangs_attack_tick(warmup_ticks: i32) -> bool {
    warmup_ticks == -8
}

/// Calculates sound pitch for Evoker Fangs attack sound (0.85 .. 1.05).
///
/// Mirrors vanilla `EvokerFangs.java:138`:
/// `this.random.nextFloat() * 0.2F + 0.85F`
#[must_use]
pub fn calculate_evoker_fangs_sound_pitch(pitch_rand: f32) -> f32 {
    pitch_rand * 0.2 + 0.85
}

/// Returns direct hit damage dealt by a Large Fireball projectile (6.0 HP).
///
/// Mirrors vanilla `LargeFireball.java:48`:
/// `var6.hurtServer(serverLevel, damageSource, 6.0F);`
#[must_use]
pub const fn get_large_fireball_damage() -> f32 {
    6.0
}

/// Checks whether a Large Fireball should cause griefing/fire based on gamerule mob_griefing.
///
/// Mirrors vanilla `LargeFireball.java:35`:
/// `boolean grief = serverLevel.getGameRules().get(GameRules.MOB_GRIEFING);`
#[must_use]
pub const fn can_large_fireball_grief(mob_griefing: bool) -> bool {
    mob_griefing
}

/// Determines whether an Eye of Ender survives after death (drops item instead of shattering).
///
/// Mirrors vanilla `EyeOfEnder.java:84`:
/// `this.surviveAfterDeath = this.random.nextInt(5) > 0;`
#[must_use]
pub const fn should_eye_of_ender_survive(rand_5: i32) -> bool {
    rand_5 > 0
}

/// Returns maximum flight ticks for an Eye of Ender before expiry (80 ticks = 4 seconds).
///
/// Mirrors vanilla `EyeOfEnder.java:103`:
/// `if (this.life > 80)`
#[must_use]
pub const fn get_eye_of_ender_max_life() -> u32 {
    80
}

/// Returns level event ID for Eye of Ender break particles (2003).
///
/// Mirrors vanilla `EyeOfEnder.java:109`:
/// `this.level().levelEvent(2003, this.blockPosition(), 0);`
#[must_use]
pub const fn get_eye_of_ender_break_level_event() -> i32 {
    2003
}

/// Calculates food saturation gained from nutrition and saturation modifier.
///
/// Mirrors vanilla `FoodConstants.java:30-32`:
/// `return nutrition * modifier * 2.0F;`
#[must_use]
pub fn calculate_food_saturation_from_modifier(nutrition: i32, modifier: f32) -> f32 {
    (nutrition as f32) * modifier * 2.0
}

/// Checks whether a player has enough food level to sprint (> 6 food, or can fly).
///
/// Mirrors vanilla `Player.java:1579-1581` & `FoodData.java:93`:
/// `return this.getFoodData().hasEnoughFood() || this.getAbilities().mayfly;`
/// where `hasEnoughFood()` is `this.getFoodLevel() > 6.0F`.
#[must_use]
pub const fn can_player_sprint_with_food(food_level: u8, may_fly: bool) -> bool {
    food_level > 6 || may_fly
}

/// Determines whether a starving player should take damage on a starvation tick.
///
/// Mirrors vanilla `FoodData.java:63-65`:
/// `if (player.getHealth() > 10.0F || difficulty == Difficulty.HARD || player.getHealth() > 1.0F && difficulty == Difficulty.NORMAL)`
#[must_use]
pub const fn should_starve_take_damage(health: f32, difficulty: pumpkin_util::Difficulty) -> bool {
    health > 10.0
        || matches!(difficulty, pumpkin_util::Difficulty::Hard)
        || (health > 1.0 && matches!(difficulty, pumpkin_util::Difficulty::Normal))
}

/// Calculates heal amount and saturation cost for fast health regeneration (saturation >= 20, food == 20).
///
/// Mirrors vanilla `FoodData.java:48-50`:
/// ```java
/// float saturationSpent = Math.min(this.saturationLevel, 6.0F);
/// player.heal(saturationSpent / 6.0F);
/// this.addExhaustion(saturationSpent);
/// ```
#[must_use]
pub fn calculate_fast_regen_heal_and_cost(saturation: f32) -> (f32, f32) {
    let cost = saturation.min(6.0);
    let heal = cost / 6.0;
    (heal, cost)
}

/// Checks whether a player needs food (foodLevel < 20).
///
/// Mirrors vanilla `FoodData.java:97`:
/// `return this.foodLevel < 20;`
#[must_use]
pub const fn is_food_needs_eating(food_level: u8) -> bool {
    food_level < 20
}

/// Determines whether Totem of Undying can prevent death from this damage source.
///
/// Mirrors vanilla `LivingEntity.java:1393-1396`:
/// ```java
/// private boolean checkTotemDeathProtection(final DamageSource killingDamage) {
///    if (killingDamage.is(DamageTypeTags.BYPASSES_INVULNERABILITY)) {
///       return false;
///    }
///    ...
/// ```
#[must_use]
pub const fn should_totem_prevent_death(bypasses_invulnerability: bool, has_totem_item: bool) -> bool {
    !bypasses_invulnerability && has_totem_item
}

/// Determines whether an entity death sound should be played upon taking fatal damage.
///
/// Mirrors vanilla `LivingEntity.java:1255-1267`:
/// ```java
/// if (this.isDeadOrDying()) {
///    if (!this.checkTotemDeathProtection(source)) {
///       if (tookFullDamage) {
///          this.makeSound(this.getDeathSound());
///          this.playSecondaryHurtSound(source);
///       }
///       this.die(source);
///    }
/// } else if (tookFullDamage) {
///    this.playHurtSound(source);
///    this.playSecondaryHurtSound(source);
/// }
/// ```
#[must_use]
pub const fn should_play_death_sound(is_dying: bool, totem_prevented_death: bool, took_full_damage: bool) -> bool {
    is_dying && !totem_prevented_death && took_full_damage
}

/// Determines whether an entity hurt sound should be played upon taking damage.
///
/// Mirrors vanilla `LivingEntity.java:1264-1267`.
#[must_use]
pub const fn should_play_hurt_sound(is_dying: bool, took_full_damage: bool) -> bool {
    !is_dying && took_full_damage
}

/// Determines if an entity type is sensitive to water and rain.
///
/// Mirrors vanilla `LivingEntity.java:3162-3174` and entity overrides:
/// - `SnowGolem.java:86`: `return true;`
/// - `Blaze.java:114`: `return true;`
/// - `Enderman.java:242`: `return true;`
/// - `Strider.java:374`: `return true;`
#[must_use]
pub fn is_entity_sensitive_to_water(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    entity_type == &pumpkin_data::entity::EntityType::SNOW_GOLEM
        || entity_type == &pumpkin_data::entity::EntityType::BLAZE
        || entity_type == &pumpkin_data::entity::EntityType::ENDERMAN
        || entity_type == &pumpkin_data::entity::EntityType::STRIDER
}

/// Determines whether an entity should receive cramming damage under maxEntityCramming game rule.
///
/// Mirrors vanilla `LivingEntity.java:3223-3235`:
/// ```java
/// int maxCramming = serverLevel.getGameRules().get(GameRules.MAX_ENTITY_CRAMMING);
/// if (maxCramming > 0 && pushableEntities.size() > maxCramming - 1 && this.random.nextInt(4) == 0) {
///     int count = 0;
///     for (Entity entity : pushableEntities) {
///         if (!entity.isPassenger()) {
///             count++;
///         }
///     }
///     if (count > maxCramming - 1) {
///         this.hurtServer(serverLevel, this.damageSources().cramming(), 6.0F);
///     }
/// }
/// ```
#[must_use]
pub const fn should_take_cramming_damage(max_cramming: i32, non_passenger_pushable_count: i32, random_4: i32) -> bool {
    max_cramming > 0 && non_passenger_pushable_count > (max_cramming - 1) && random_4 == 0
}

/// Vanilla cramming damage amount (6.0F = 3 hearts).
pub const CRAMMING_DAMAGE: f32 = 6.0;

/// Calculates out-of-world-border damage for players outside safe zone.
///
/// Mirrors vanilla `LivingEntity.java:432-437`:
/// ```java
/// double dist = level.getWorldBorder().getDistanceToBorder(this) + level.getWorldBorder().getSafeZone();
/// if (dist < 0.0) {
///     double damagePerBlock = level.getWorldBorder().getDamagePerBlock();
///     if (damagePerBlock > 0.0) {
///         this.hurtServer(level, this.damageSources().outOfBorder(), Math.max(1, Mth.floor(-dist * damagePerBlock)));
///     }
/// }
/// ```
#[must_use]
pub fn calculate_out_of_border_damage(distance_to_border: f64, safe_zone: f64, damage_per_block: f64) -> Option<f32> {
    let dist = distance_to_border + safe_zone;
    if dist < 0.0 && damage_per_block > 0.0 {
        let raw = (-dist * damage_per_block).floor() as f32;
        Some(raw.max(1.0))
    } else {
        None
    }
}

/// Determines if an entity in a suffocating wall should receive suffocation damage.
///
/// Mirrors vanilla `LivingEntity.java:429-430`, `LivingEntity.java:3877-3878`:
/// ```java
/// if (this.isInWall()) {
///     this.hurtServer(level, this.damageSources().inWall(), 1.0F);
/// }
/// ...
/// public boolean isInWall() {
///     return !this.isSleeping() && super.isInWall();
/// }
/// ```
#[must_use]
pub const fn should_take_in_wall_damage(is_sleeping: bool, in_wall: bool) -> bool {
    !is_sleeping && in_wall
}

/// Vanilla in-wall suffocation damage amount (1.0F = 0.5 heart).
pub const IN_WALL_DAMAGE: f32 = 1.0;

/// Returns the water drag/inertia multiplier for thrown tridents (0.99F in vanilla).
///
/// Mirrors vanilla `ThrownTrident.java:34`, `ThrownTrident.java:220-222`:
/// ```java
/// private static final float WATER_INERTIA = 0.99F;
/// @Override
/// protected float getWaterInertia() {
///    return 0.99F;
/// }
/// ```
#[must_use]
pub const fn calculate_trident_water_inertia() -> f64 {
    0.99
}

/// Determines whether a trident stuck in ground should despawn.
///
/// Mirrors vanilla `ThrownTrident.java:212-217`:
/// ```java
/// @Override
/// public void tickDespawn() {
///    int loyalty = this.entityData.get(ID_LOYALTY);
///    if (this.pickup != AbstractArrow.Pickup.ALLOWED || loyalty <= 0) {
///       super.tickDespawn();
///    }
/// }
/// ```
#[must_use]
pub const fn should_trident_despawn(
    life: u32,
    max_despawn_ticks: u32,
    loyalty: u8,
    is_pickup_allowed: bool,
) -> bool {
    if is_pickup_allowed && loyalty > 0 {
        false
    } else {
        life >= max_despawn_ticks
    }
}

/// Calculates fishing tick speed based on raining weather and sky exposure.
///
/// Mirrors vanilla `FishingHook.java:298-306`:
/// ```java
/// int fishingSpeed = 1;
/// BlockPos above = blockPos.above();
/// if (this.random.nextFloat() < 0.25F && this.level().isRainingAt(above)) {
///    fishingSpeed++;
/// }
/// if (this.random.nextFloat() < 0.5F && !this.level().canSeeSky(above)) {
///    fishingSpeed--;
/// }
/// ```
#[must_use]
pub fn calculate_fishing_tick_speed(
    is_raining_at_hook: bool,
    can_see_sky: bool,
    rand_rain: f32,
    rand_sky: f32,
) -> i32 {
    let mut speed = 1;
    if rand_rain < 0.25 && is_raining_at_hook {
        speed += 1;
    }
    if rand_sky < 0.5 && !can_see_sky {
        speed -= 1;
    }
    speed
}

/// Determines if a block is doused by splash water bottles (fire and campfires).
///
/// Mirrors vanilla `AbstractThrownPotion.java:124-135`.
#[must_use]
pub fn is_douseable_fire_or_campfire(is_fire: bool, is_lit_campfire: bool, is_lit_candle: bool) -> bool {
    is_fire || is_lit_campfire || is_lit_candle
}

/// Target entity type hooked by a fishing bobber.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HookedTargetType {
    Item,
    OtherEntity,
}

/// Calculates durability damage dealt to a fishing rod upon retrieving the bobber.
///
/// Mirrors vanilla `FishingHook.java:441-480`:
/// ```java
/// int dmg = 0;
/// if (this.hookedIn != null) {
///    this.pullEntity(this.hookedIn);
///    dmg = this.hookedIn instanceof ItemEntity ? 3 : 5;
/// } else if (this.nibble > 0) {
///    ...
///    dmg = 1;
/// }
/// if (this.onGround()) {
///    dmg = 2;
/// }
/// return dmg;
/// ```
#[must_use]
pub const fn calculate_fishing_rod_damage(
    hooked_target: Option<HookedTargetType>,
    is_nibbling: bool,
    on_ground: bool,
) -> u32 {
    if let Some(target) = hooked_target {
        match target {
            HookedTargetType::Item => 3,
            HookedTargetType::OtherEntity => 5,
        }
    } else if is_nibbling {
        1
    } else if on_ground {
        2
    } else {
        0
    }
}

/// Calculates pull velocity applied to a hooked entity when reeling in.
///
/// Mirrors vanilla `FishingHook.java:500-506`:
/// ```java
/// Vec3 delta = new Vec3(owner.getX() - this.getX(), owner.getY() - this.getY(), owner.getZ() - this.getZ()).scale(0.1);
/// entity.setDeltaMovement(entity.getDeltaMovement().add(delta));
/// ```
#[must_use]
pub fn calculate_fishing_hook_pull_motion(
    owner_pos: Vector3<f64>,
    hook_pos: Vector3<f64>,
) -> Vector3<f64> {
    (owner_pos - hook_pos) * 0.1
}

/// Calculates velocity vector for item loot caught when fishing.
///
/// Mirrors vanilla `FishingHook.java:460-464`:
/// ```java
/// double xa = owner.getX() - this.getX();
/// double ya = owner.getY() - this.getY();
/// double za = owner.getZ() - this.getZ();
/// double speed = 0.1;
/// entity.setDeltaMovement(xa * 0.1, ya * 0.1 + Math.sqrt(Math.sqrt(xa * xa + ya * ya + za * za)) * 0.08, za * 0.1);
/// ```
#[must_use]
pub fn calculate_caught_item_velocity(
    owner_pos: Vector3<f64>,
    hook_pos: Vector3<f64>,
) -> Vector3<f64> {
    let delta = owner_pos - hook_pos;
    let dist_sq = delta.length_squared();
    Vector3::new(
        delta.x * 0.1,
        delta.y.mul_add(0.1, dist_sq.sqrt().sqrt() * 0.08),
        delta.z * 0.1,
    )
}

/// Calculates pitch for fishing bobber throw and retrieve sounds.
///
/// Mirrors vanilla `FishingRodItem.java:38, 50`:
/// `0.4F / (level.getRandom().nextFloat() * 0.4F + 0.8F)`
#[must_use]
pub fn calculate_bobber_sound_pitch(rand_val: f32) -> f32 {
    0.4 / (rand_val * 0.4 + 0.8)
}

/// Calculates durability loss for firing a crossbow.
///
/// Mirrors vanilla `ProjectileWeaponItem.java:71` & `CrossbowItem.java:167-169`:
/// Firework rocket costs 3 durability per shot, arrow costs 1 durability per shot.
/// Under Multishot, durability is consumed for each projectile launched.
#[must_use]
pub const fn calculate_crossbow_durability_use(is_firework: bool, projectile_count: usize) -> u32 {
    let per_shot: u32 = if is_firework { 3 } else { 1 };
    per_shot * (projectile_count as u32)
}

/// Calculates projectile spread angle (in degrees) for multishot projectile weapons.
///
/// Mirrors vanilla `ProjectileWeaponItem.java:54-63`:
/// ```java
/// float angleStep = projectiles.size() == 1 ? 0.0F : 2.0F * maxAngle / (projectiles.size() - 1);
/// float angleOffset = (projectiles.size() - 1) % 2 * angleStep / 2.0F;
/// float angle = angleOffset + direction * ((i + 1) / 2) * angleStep;
/// ```
#[must_use]
pub fn calculate_multishot_spread_angle(index: usize, count: usize, max_spread: f32) -> f32 {
    if count <= 1 {
        return 0.0;
    }
    let angle_step = 2.0 * max_spread / (count as f32 - 1.0);
    let angle_offset = ((count - 1) % 2) as f32 * angle_step / 2.0;
    let direction = if index % 2 == 1 { -1.0 } else { 1.0 };
    angle_offset + direction * (index.div_ceil(2) as f32) * angle_step
}

/// Calculates crossbow shot sound pitch.
///
/// Mirrors vanilla `CrossbowItem.java:193-200`:
/// ```java
/// private static float getShotPitch(final RandomSource random, final int index) {
///    return index == 0 ? 1.0F : getRandomShotPitch((index & 1) == 1, random);
/// }
/// private static float getRandomShotPitch(final boolean highPitch, final RandomSource random) {
///    float rangeDecider = highPitch ? 0.63F : 0.43F;
///    return 1.0F / (random.nextFloat() * 0.5F + 1.8F) + rangeDecider;
/// }
/// ```
#[must_use]
pub fn calculate_crossbow_shot_pitch(index: usize, rand_val: f32) -> f32 {
    if index == 0 {
        1.0
    } else {
        let range_decider = if (index & 1) == 1 { 0.63 } else { 0.43 };
        1.0 / (rand_val * 0.5 + 1.8) + range_decider
    }
}

/// Calculates pitch for crossbow load end sound.
///
/// Mirrors vanilla `CrossbowItem.java:234`:
/// `1.0F / (level.getRandom().nextFloat() * 0.5F + 1.0F) + 0.2F`
#[must_use]
pub fn calculate_crossbow_loading_end_pitch(rand_val: f32) -> f32 {
    1.0 / (rand_val * 0.5 + 1.0) + 0.2
}

/// Calculates bow charge power based on draw duration in ticks.
///
/// Mirrors vanilla `BowItem.java:74-82`:
/// ```java
/// public static float getPowerForTime(final int timeHeld) {
///    float pow = timeHeld / 20.0F;
///    pow = (pow * pow + pow * 2.0F) / 3.0F;
///    if (pow > 1.0F) {
///       pow = 1.0F;
///    }
///    return pow;
/// }
/// ```
#[must_use]
pub fn calculate_bow_power_for_time(time_held_ticks: i32) -> f32 {
    let mut pow = (time_held_ticks as f32) / 20.0;
    pow = (pow * pow + pow * 2.0) / 3.0;
    if pow > 1.0 {
        1.0
    } else {
        pow
    }
}

/// Checks whether a bow has been drawn enough to release an arrow.
///
/// Mirrors vanilla `BowItem.java:35`:
/// `if (pow < 0.1) return false;`
#[must_use]
pub const fn can_bow_release(power: f32) -> bool {
    power >= 0.1
}

/// Calculates pitch for bow shoot sound.
///
/// Mirrors vanilla `BowItem.java:52`:
/// `1.0F / (level.getRandom().nextFloat() * 0.4F + 1.2F) + pow * 0.5F`
#[must_use]
pub fn calculate_bow_sound_pitch(power: f32, rand_val: f32) -> f32 {
    1.0 / (rand_val * 0.4 + 1.2) + power * 0.5
}

/// Calculates horizontal acceleration factor for firework rockets.
///
/// Mirrors vanilla `FireworkRocketEntity.java:152`:
/// `double horizontalAcceleration = this.horizontalCollision ? 1.0 : 1.15;`
#[must_use]
pub const fn calculate_firework_horizontal_acceleration(has_horizontal_collision: bool) -> f64 {
    if has_horizontal_collision {
        1.0
    } else {
        1.15
    }
}

/// Calculates durability repaired and remaining XP for Mending enchantment.
///
/// Mirrors vanilla `ExperienceOrb.java:296-308`:
/// ```java
/// int toRepairFromXpAmount = amount * 2;
/// int repair = Math.min(toRepairFromXpAmount, itemStack.getDamageValue());
/// int remaining = amount - repair * amount / toRepairFromXpAmount;
/// ```
#[must_use]
pub const fn calculate_mending_repair(xp_amount: i32, current_damage: i32) -> (i32, i32) {
    if xp_amount <= 0 || current_damage <= 0 {
        return (0, xp_amount);
    }
    let to_repair = xp_amount * 2;
    let repair = if current_damage < to_repair {
        current_damage
    } else {
        to_repair
    };
    let remaining = xp_amount - (repair * xp_amount) / to_repair;
    (repair, remaining)
}

/// Calculates Thorns damage and durability cost upon taking damage.
///
/// Mirrors vanilla `Enchantments.java:540-565`:
/// - Activation chance: `level * 0.15F`
/// - Damage: 1.0..=4.0 HP (or `level - 10` if level > 10)
/// - Extra durability loss: 2
#[must_use]
pub fn calculate_thorns_damage(
    level: u32,
    rand_chance: f32,
    rand_damage: f32,
) -> Option<(f32, u32)> {
    if level == 0 {
        return None;
    }
    let chance = (level as f32) * 0.15;
    if rand_chance < chance {
        let dmg = if level > 10 {
            (level - 10) as f32
        } else {
            1.0 + (rand_damage * 4.0).floor().min(3.0)
        };
        Some((dmg, 2))
    } else {
        None
    }
}

/// Calculates pitch for lightning thunder sound (distant rumble).
///
/// Mirrors vanilla `LightningBolt.java:89`:
/// `0.8F + this.random.nextFloat() * 0.2F`
#[must_use]
pub fn calculate_lightning_thunder_pitch(rand_val: f32) -> f32 {
    0.8 + rand_val * 0.2
}

/// Calculates pitch for lightning impact sound (close crack).
///
/// Mirrors vanilla `LightningBolt.java:100`:
/// `0.5F + this.random.nextFloat() * 0.2F`
#[must_use]
pub fn calculate_lightning_impact_pitch(rand_val: f32) -> f32 {
    0.5 + rand_val * 0.2
}

/// Checks whether lightning strikes should spawn fire based on difficulty.
///
/// Mirrors vanilla `LightningBolt.java:105-108`:
/// `if (difficulty == Difficulty.NORMAL || difficulty == Difficulty.HARD) { this.spawnFire(4); }`
#[must_use]
pub const fn can_lightning_spawn_fire(difficulty: pumpkin_util::Difficulty) -> bool {
    matches!(difficulty, pumpkin_util::Difficulty::Normal | pumpkin_util::Difficulty::Hard)
}

/// Calculates random short fuse for primed TNT chain reactions.
///
/// Mirrors vanilla `PrimedTnt.java:183-185`:
/// `random.nextInt(Math.max(1, fuse / 4)) + fuse / 8;`
#[must_use]
pub fn calculate_tnt_short_fuse(fuse: u32, rand_sample: u32) -> u32 {
    let bound = (fuse / 4).max(1);
    (rand_sample % bound) + fuse / 8
}

/// Calculates Y-coordinate for TNT explosion origin.
///
/// Mirrors vanilla `PrimedTnt.java:135`:
/// `this.getY(0.0625)` -> `pos.y + height * 0.0625`
#[must_use]
pub fn calculate_tnt_explosion_pos_y(pos_y: f64, height: f32) -> f64 {
    f64::from(height).mul_add(0.0625, pos_y)
}

/// Checks whether primed TNT takes damage from outside damage sources.
///
/// Mirrors vanilla `PrimedTnt.java:210-212`:
/// ```java
/// @Override
/// public final boolean hurtServer(final ServerLevel level, final DamageSource source, final float damage) {
///    return false;
/// }
/// ```
#[must_use]
pub const fn should_tnt_take_damage() -> bool {
    false
}

/// Calculates vertical velocity change for entities on top of a bubble column.
///
/// Mirrors vanilla `Entity.java:2945-2954` (`handleOnAboveBubbleColumn`):
/// ```java
/// if (dragDown) {
///    yd = Math.max(-0.9, movement.y - 0.03);
/// } else {
///    yd = Math.min(1.8, movement.y + 0.1);
/// }
/// ```
#[must_use]
pub fn calculate_entity_bubble_column_above_y(current_y: f64, drag_down: bool) -> f64 {
    if drag_down {
        (current_y - 0.03).max(-0.9)
    } else {
        (current_y + 0.1).min(1.8)
    }
}

/// Calculates vertical velocity change for entities submerged inside a bubble column.
///
/// Mirrors vanilla `Entity.java:2980-2991` (`handleOnInsideBubbleColumn`):
/// ```java
/// if (dragDown) {
///    yd = Math.max(-0.3, movement.y - 0.03);
/// } else {
///    yd = Math.min(0.7, movement.y + 0.06);
/// }
/// ```
#[must_use]
pub fn calculate_entity_bubble_column_inside_y(current_y: f64, drag_down: bool) -> f64 {
    if drag_down {
        (current_y - 0.03).max(-0.3)
    } else {
        (current_y + 0.06).min(0.7)
    }
}

/// Calculates vertical velocity impulse for boats on bubble columns when the timer reaches 0.
///
/// Mirrors vanilla `AbstractBoat.java:346-354` (`handleBubbleColumnEffect`):
/// ```java
/// if (dragDown) {
///    this.setDeltaMovement(movement.add(0.0, -0.7, 0.0));
/// } else {
///    this.setDeltaMovement(movement.x, 0.6, movement.z);
/// }
/// ```
#[must_use]
pub fn calculate_boat_bubble_column_effect_y(current_y: f64, drag_down: bool) -> f64 {
    if drag_down {
        current_y - 0.7
    } else {
        0.6
    }
}

/// Damage inflicted on a turtle when struck by lightning.
///
/// Mirrors vanilla `Turtle.java:283-285` (`thunderHit`):
/// `this.hurtServer(level, this.damageSources().lightningBolt(), Float.MAX_VALUE);`
#[must_use]
pub const fn get_turtle_thunder_hit_damage() -> f32 {
    f32::MAX
}

/// Checks whether pig / villager converts to zombified piglin / witch upon lightning strike.
///
/// Mirrors vanilla `Pig.java:201` and `Villager.java:745`:
/// `if (level.getDifficulty() != Difficulty.PEACEFUL)`
#[must_use]
pub const fn can_lightning_convert_mob(difficulty: pumpkin_util::Difficulty) -> bool {
    !matches!(difficulty, pumpkin_util::Difficulty::Peaceful)
}

// ============================================================================
// Zombie Family Parity Constants (Zombie, Husk, Drowned, Zombie Villager)
// ============================================================================

/// Base max health for a Zombie / Husk / Drowned / Zombie Villager (20.0 HP / 10 hearts).
/// Mirrors vanilla `Zombie.java:93` (`Attributes.MAX_HEALTH, 20.0`).
pub const ZOMBIE_MAX_HEALTH: f32 = 20.0;

/// Base follow range for a Zombie (35.0 blocks).
/// Mirrors vanilla `Zombie.java:94` (`Attributes.FOLLOW_RANGE, 35.0`).
pub const ZOMBIE_FOLLOW_RANGE: f64 = 35.0;

/// Base movement speed for a Zombie / Husk / Drowned (0.23).
/// Mirrors vanilla `Zombie.java:95` (`Attributes.MOVEMENT_SPEED, 0.23F`).
pub const ZOMBIE_BASE_MOVEMENT_SPEED: f64 = 0.23;

/// Base attack damage for a Zombie / Husk / Drowned (3.0 HP).
/// Mirrors vanilla `Zombie.java:96` (`Attributes.ATTACK_DAMAGE, 3.0`).
pub const ZOMBIE_BASE_ATTACK_DAMAGE: f32 = 3.0;

/// Base natural armor for a Zombie (2.0 points = 1 armor icon).
/// Mirrors vanilla `Zombie.java:97` (`Attributes.ARMOR, 2.0`).
pub const ZOMBIE_BASE_ARMOR: f64 = 2.0;

/// Base experience reward dropped by a Zombie / Husk / Drowned / Zombie Villager on death (5 XP).
/// Mirrors vanilla `Monster.java:55` (`this.xpReward = 5`).
pub const ZOMBIE_XP_REWARD: u32 = 5;

/// Baby zombie movement speed modifier identifier ("minecraft:baby").
/// Mirrors vanilla `Zombie.java:55` (`SPEED_MODIFIER_BABY_ID`).
pub const ZOMBIE_BABY_SPEED_MODIFIER_ID: &str = "minecraft:baby";

/// Baby zombie movement speed multiplier (+0.5 = +50% base speed).
/// Mirrors vanilla `Zombie.java:56-58` (`SPEED_MODIFIER_BABY = ... 0.5 ... ADD_MULTIPLIED_BASE`).
pub const ZOMBIE_BABY_SPEED_MODIFIER_AMOUNT: f64 = 0.5;

/// Submerged duration before a Zombie begins shaking conversion to Drowned (600 ticks / 30 seconds).
/// Mirrors vanilla `Zombie.java:101-105` (`drowningTracker: "InWaterTime", 600`).
pub const ZOMBIE_DROWN_CONVERSION_SUBMERGED_TICKS: i32 = 600;

/// Shaking conversion duration from Zombie to Drowned or Husk to Zombie (300 ticks / 15 seconds).
/// Mirrors vanilla `Zombie.java:103` (`drowningTracker: "DrownedConversionTime", 300`).
pub const ZOMBIE_DROWN_CONVERSION_SHAKING_TICKS: i32 = 300;

/// Zombie bare-hand attack fire ignition chance factor (0.3).
/// Mirrors vanilla `Zombie.java:319`:
/// `if (this.getMainHandItem().isEmpty() && this.isOnFire() && this.random.nextFloat() < difficulty * 0.3F)`
pub const ZOMBIE_FIRE_ATTACK_CHANCE_FACTOR: f32 = 0.3;

/// Zombie bare-hand attack fire ignition duration multiplier in seconds (2).
/// Mirrors vanilla `Zombie.java:320`:
/// `target.igniteForSeconds(2 * (int)difficulty);`
pub const ZOMBIE_FIRE_ATTACK_DURATION_MULTIPLIER: u32 = 2;

/// Zombie leader spawn chance (5% scaled by regional difficulty).
/// Mirrors vanilla `Zombie.java:87`:
/// `public static final float ZOMBIE_LEADER_CHANCE = 0.05F;`
pub const ZOMBIE_LEADER_CHANCE: f32 = 0.05;

/// Zombie reinforcement spawn attempts (50).
/// Mirrors vanilla `Zombie.java:88`:
/// `public static final int REINFORCEMENT_ATTEMPTS = 50;`
pub const ZOMBIE_REINFORCEMENT_ATTEMPTS: i32 = 50;

/// Zombie reinforcement max spawn range (40 blocks).
/// Mirrors vanilla `Zombie.java:89`:
/// `public static final int REINFORCEMENT_RANGE_MAX = 40;`
pub const ZOMBIE_REINFORCEMENT_RANGE_MAX: i32 = 40;

/// Zombie reinforcement min spawn range (7 blocks).
/// Mirrors vanilla `Zombie.java:90`:
/// `public static final int REINFORCEMENT_RANGE_MIN = 7;`
pub const ZOMBIE_REINFORCEMENT_RANGE_MIN: i32 = 7;

/// Zombie break door chance on Hard difficulty (10%).
/// Mirrors vanilla `Zombie.java:94`:
/// `private static final float BREAK_DOOR_CHANCE = 0.1F;`
pub const ZOMBIE_BREAK_DOOR_CHANCE: f32 = 0.1;

/// Zombie reinforcement caller charge deduction (-0.05).
/// Mirrors vanilla `Zombie.java:302`:
/// `existingAmount - 0.05`
pub const ZOMBIE_REINFORCEMENT_CALLER_CHARGE: f64 = -0.05;

/// Zombie reinforcement callee charge penalty (-0.05).
/// Mirrors vanilla `Zombie.java:80`:
/// `new AttributeModifier(..., -0.05F, AttributeModifier.Operation.ADD_VALUE)`
pub const ZOMBIE_REINFORCEMENT_CALLEE_CHARGE: f64 = -0.05;

/// Zombie converting villager to zombie villager sound/level event ID (1026).
/// Mirrors vanilla `Zombie.java:247`:
/// `level.levelEvent(null, 1026, this.blockPosition(), 0);`
pub const ZOMBIE_TO_ZOMBIE_VILLAGER_CONVERSION_SOUND_EVENT_ID: i32 = 1026;

/// Zombie converting to drowned sound/level event ID (1040).
/// Mirrors vanilla `Zombie.java:347`:
/// `return 1040;`
pub const ZOMBIE_TO_DROWNED_CONVERSION_SOUND_EVENT_ID: i32 = 1040;

/// Zombie Villager cure minimum wait time in ticks (3600 ticks = 3 minutes).
/// Mirrors vanilla `ZombieVillager.java:69`:
/// `private static final int VILLAGER_CONVERSION_WAIT_MIN = 3600;`
pub const ZOMBIE_VILLAGER_CONVERSION_WAIT_MIN: i32 = 3600;

/// Zombie Villager cure maximum wait time in ticks (6000 ticks = 5 minutes).
/// Mirrors vanilla `ZombieVillager.java:70`:
/// `private static final int VILLAGER_CONVERSION_WAIT_MAX = 6000;`
pub const ZOMBIE_VILLAGER_CONVERSION_WAIT_MAX: i32 = 6000;

/// Zombie Villager cure random spread range in ticks (2401 ticks).
/// Mirrors vanilla `ZombieVillager.java:170`:
/// `this.random.nextInt(2401) + 3600`
pub const ZOMBIE_VILLAGER_CONVERSION_SPREAD: i32 = 2401;

/// Zombie Villager max nearby special blocks considered for curing speedup (14).
/// Mirrors vanilla `ZombieVillager.java:71`:
/// `private static final int MAX_SPECIAL_BLOCKS_COUNT = 14;`
pub const ZOMBIE_VILLAGER_MAX_SPECIAL_BLOCKS: i32 = 14;

/// Zombie Villager special block search radius (4 blocks in X, Y, Z).
/// Mirrors vanilla `ZombieVillager.java:72`:
/// `private static final int SPECIAL_BLOCK_RADIUS = 4;`
pub const ZOMBIE_VILLAGER_SPECIAL_BLOCK_RADIUS: i32 = 4;

/// Zombie Villager special block check probability per tick (1%).
/// Mirrors vanilla `ZombieVillager.java:281`:
/// `if (this.random.nextFloat() < 0.01F)`
pub const ZOMBIE_VILLAGER_SPECIAL_BLOCK_CHECK_CHANCE: f32 = 0.01;

/// Zombie Villager special block speedup probability per block (30%).
/// Mirrors vanilla `ZombieVillager.java:290`:
/// `if (this.random.nextFloat() < 0.3F) amount++;`
pub const ZOMBIE_VILLAGER_SPECIAL_BLOCK_BOOST_CHANCE: f32 = 0.3;

/// Zombie Villager cured nausea duration in ticks (200 ticks = 10 seconds).
/// Mirrors vanilla `ZombieVillager.java:266`:
/// `villager.addEffect(new MobEffectInstance(MobEffects.NAUSEA, 200, 0));`
pub const ZOMBIE_VILLAGER_NAUSEA_DURATION_TICKS: i32 = 200;

/// Zombie Villager curing sound entity status event ID (16).
/// Mirrors vanilla `ZombieVillager.java:202, 212`:
/// `this.level().broadcastEntityEvent(this, (byte)16);`
pub const ZOMBIE_VILLAGER_CURE_SOUND_EVENT_ID: u8 = 16;

/// Zombie Villager cured world/level event ID (1027).
/// Mirrors vanilla `ZombieVillager.java:268`:
/// `level.levelEvent(null, 1027, this.blockPosition(), 0);`
pub const ZOMBIE_VILLAGER_CURED_WORLD_EVENT_ID: i32 = 1027;

/// Calculates whether a zombie bare-hand melee attack on fire ignites the target and the duration in seconds.
///
/// Mirrors vanilla `Zombie.java:318-321`:
/// ```text
/// float difficulty = level.getCurrentDifficultyAt(this.blockPosition()).getEffectiveDifficulty();
/// if (this.getMainHandItem().isEmpty() && this.isOnFire() && this.random.nextFloat() < difficulty * 0.3F) {
///     target.igniteForSeconds(2 * (int)difficulty);
/// }
/// ```
#[must_use]
pub fn calculate_zombie_fire_attack_seconds(
    is_on_fire: bool,
    main_hand_empty: bool,
    difficulty: f32,
    rand_val: f32,
) -> Option<u32> {
    if !is_on_fire || !main_hand_empty || difficulty <= 0.0 {
        return None;
    }
    if rand_val < difficulty * ZOMBIE_FIRE_ATTACK_CHANCE_FACTOR {
        Some(ZOMBIE_FIRE_ATTACK_DURATION_MULTIPLIER * (difficulty as u32))
    } else {
        None
    }
}

/// Calculates initial conversion time in ticks for curing a Zombie Villager.
///
/// Mirrors vanilla `ZombieVillager.java:170`:
/// `this.random.nextInt(2401) + 3600`
#[must_use]
pub const fn calculate_zombie_villager_cure_time(rand_spread: i32) -> i32 {
    ZOMBIE_VILLAGER_CONVERSION_WAIT_MIN + rand_spread
}

/// Base duration for Hunger inflicted by unarmed Husk hit (140 ticks / 7 seconds per difficulty level).
/// Mirrors vanilla `Husk.java:59`: `140 * (int)difficulty`.
pub const HUSK_UNARMED_HUNGER_BASE_TICKS: i32 = 140;

/// Projectile throw velocity for a Drowned throwing a Trident (1.6 blocks/sec scale).
/// Mirrors vanilla `Drowned.java:319` (`projectile.shoot(..., 1.6F, uncertainty)`).
pub const DROWNED_TRIDENT_SHOOT_VELOCITY: f64 = 1.6;

/// Base step height for a Drowned (1.0 block).
/// Mirrors vanilla `Drowned.java:82` (`Attributes.STEP_HEIGHT, 1.0`).
pub const DROWNED_STEP_HEIGHT: f64 = 1.0;

/// Drowned nautilus shell offhand chance (3%).
/// Mirrors vanilla `Drowned.java:69, 113`:
/// `public static final float NAUTILUS_SHELL_CHANCE = 0.03F;`
pub const DROWNED_NAUTILUS_SHELL_CHANCE: f32 = 0.03;

/// Drowned weapon hold chance (10%).
/// Mirrors vanilla `Drowned.java:200`:
/// `if (random.nextFloat() > 0.9)`
pub const DROWNED_WEAPON_CHANCE: f32 = 0.10;

/// Drowned trident weapon roll threshold (10 out of 16 = 62.5%).
/// Mirrors vanilla `Drowned.java:201-203`:
/// `int rand = random.nextInt(16); if (rand < 10) ... TRIDENT ... else ... FISHING_ROD`
pub const DROWNED_TRIDENT_ROLL_THRESHOLD: u32 = 10;
pub const DROWNED_WEAPON_ROLL_MAX: u32 = 16;

/// Drowned zombie nautilus jockey chance when holding trident (50%).
/// Mirrors vanilla `Drowned.java:70, 120`:
/// `private static final float ZOMBIE_NAUTILUS_JOCKEY_CHANCE = 0.5F;`
pub const DROWNED_ZOMBIE_NAUTILUS_JOCKEY_CHANCE: f32 = 0.5;

/// Drowned trident attack interval (40 ticks = 2 seconds).
/// Mirrors vanilla `Drowned.java:95`:
/// `new Drowned.DrownedTridentAttackGoal(this, 1.0, 40, 10.0F)`
pub const DROWNED_TRIDENT_ATTACK_INTERVAL_TICKS: i32 = 40;

/// Drowned trident attack range (10.0 blocks).
/// Mirrors vanilla `Drowned.java:95`:
pub const DROWNED_TRIDENT_ATTACK_RANGE: f32 = 10.0;

/// Husk conversion sound event ID (1041).
/// Mirrors vanilla `Husk.java:84`:
/// `LevelEvent.SOUND_HUSK_TO_ZOMBIE_CONVERT = 1041;`
pub const HUSK_TO_ZOMBIE_CONVERSION_SOUND_EVENT_ID: i32 = 1041;

/// Calculates the projectile inaccuracy (spread angle in degrees) when a Drowned throws a Trident.
///
/// Mirrors vanilla `Drowned.java:323-327` and `RangedAttackMob.java:23-28`:
/// - Easy / Peaceful: 14.0
/// - Normal: 10.0
/// - Hard: 6.0
#[must_use]
pub fn calculate_drowned_trident_uncertainty(difficulty: pumpkin_util::Difficulty) -> f64 {
    match difficulty {
        pumpkin_util::Difficulty::Peaceful | pumpkin_util::Difficulty::Easy => 14.0,
        pumpkin_util::Difficulty::Normal => 10.0,
        pumpkin_util::Difficulty::Hard => 6.0,
    }
}

/// Calculates Hunger duration in ticks inflicted by an unarmed Husk attack based on difficulty.
///
/// Mirrors vanilla `Husk.java:59-60`: `140 * (int)difficulty`
/// - Peaceful: 0 ticks (no hunger)
/// - Easy (1): 140 ticks (7s)
/// - Normal (2): 280 ticks (14s)
/// - Hard (3): 420 ticks (21s)
#[must_use]
pub fn calculate_husk_hunger_duration(difficulty: pumpkin_util::Difficulty) -> i32 {
    let multiplier = match difficulty {
        pumpkin_util::Difficulty::Peaceful => 0,
        pumpkin_util::Difficulty::Easy => 1,
        pumpkin_util::Difficulty::Normal => 2,
        pumpkin_util::Difficulty::Hard => 3,
    };
    HUSK_UNARMED_HUNGER_BASE_TICKS * multiplier
}

// ============================================================================
// Golem Family Parity Constants (Iron Golem & Snow Golem)
// ============================================================================

/// Base max health for an Iron Golem (100.0 HP / 50 hearts).
/// Mirrors vanilla `IronGolem.java:99` (`Attributes.MAX_HEALTH, 100.0`).
pub const IRON_GOLEM_MAX_HEALTH: f32 = 100.0;

/// Base movement speed for an Iron Golem.
/// Mirrors vanilla `IronGolem.java:100` (`Attributes.MOVEMENT_SPEED, 0.25`).
pub const IRON_GOLEM_BASE_MOVEMENT_SPEED: f64 = 0.25;

/// Knockback resistance for an Iron Golem (1.0 = 100% immune to normal knockback).
/// Mirrors vanilla `IronGolem.java:101` (`Attributes.KNOCKBACK_RESISTANCE, 1.0`).
pub const IRON_GOLEM_KNOCKBACK_RESISTANCE: f32 = 1.0;

/// Base attack damage for an Iron Golem before random spread.
/// Mirrors vanilla `IronGolem.java:102` (`Attributes.ATTACK_DAMAGE, 15.0`).
pub const IRON_GOLEM_BASE_ATTACK_DAMAGE: f32 = 15.0;

/// Step height for an Iron Golem.
/// Mirrors vanilla `IronGolem.java:103` (`Attributes.STEP_HEIGHT, 1.0`).
pub const IRON_GOLEM_STEP_HEIGHT: f32 = 1.0;

/// Health restored when repairing an Iron Golem with an Iron Ingot (25.0 HP).
/// Mirrors vanilla `IronGolem.java:48` (`IRON_INGOT_HEAL_AMOUNT = 25`).
pub const IRON_GOLEM_IRON_INGOT_HEAL_AMOUNT: f32 = 25.0;

/// Attack animation duration in ticks (10 ticks = 0.5s).
/// Mirrors vanilla `IronGolem.java:189,228` (`attackAnimationTick = 10`).
pub const IRON_GOLEM_ATTACK_ANIMATION_TICKS: i32 = 10;

/// Offer flower duration in ticks (400 ticks = 20s).
/// Mirrors vanilla `IronGolem.java:230,240` (`offerFlowerTick = 400`).
pub const IRON_GOLEM_OFFER_FLOWER_TICKS: i32 = 400;

/// Upward velocity multiplier added on target hit before knockback resistance scaling.
/// Mirrors vanilla `IronGolem.java:199` (`0.4F * scale`).
pub const IRON_GOLEM_FLING_UPWARD_VELOCITY: f64 = 0.4;

/// Base experience reward dropped by Iron Golem on death (0 XP).
/// Mirrors vanilla `AbstractGolem.java` / `Mob.java` (default `xpReward = 0`).
pub const IRON_GOLEM_XP_REWARD: u32 = 0;

/// Base max health for a Snow Golem (4.0 HP / 2 hearts).
/// Mirrors vanilla `SnowGolem.java:62` (`Attributes.MAX_HEALTH, 4.0`).
pub const SNOW_GOLEM_MAX_HEALTH: f32 = 4.0;

/// Base movement speed for a Snow Golem.
/// Mirrors vanilla `SnowGolem.java:62` (`Attributes.MOVEMENT_SPEED, 0.2F`).
pub const SNOW_GOLEM_BASE_MOVEMENT_SPEED: f64 = 0.2;

/// Snowball projectile velocity shot by Snow Golem (1.6 blocks/sec scale).
/// Mirrors vanilla `SnowGolem.java:130` (`projectile.shoot(..., 1.6F, 12.0F)`).
pub const SNOW_GOLEM_SHOOT_VELOCITY: f64 = 1.6;

/// Snowball projectile inaccuracy (spread angle in degrees: 12.0).
/// Mirrors vanilla `SnowGolem.java:137` (`rangedAttackUncertainty = 12.0F`).
pub const SNOW_GOLEM_SHOOT_INACCURACY: f64 = 12.0;

/// Damage taken by Snow Golem when melting in warm biomes or touching water/rain (1.0 HP).
/// Mirrors vanilla `SnowGolem.java:93,98`.
pub const SNOW_GOLEM_MELT_DAMAGE: f32 = 1.0;

/// Base experience reward dropped by Snow Golem on death (0 XP).
/// Mirrors vanilla `AbstractGolem.java` / `Mob.java` (default `xpReward = 0`).
pub const SNOW_GOLEM_XP_REWARD: u32 = 0;

/// Calculates attack damage dealt by an Iron Golem with random spread.
///
/// Mirrors vanilla `IronGolem.java:193`:
/// `float damage = (int)attackDamage > 0 ? attackDamage / 2.0F + this.random.nextInt((int)attackDamage) : attackDamage;`
#[must_use]
pub fn calculate_iron_golem_attack_damage(base_attack: f32, rand_sample: u32) -> f32 {
    let int_atk = base_attack as i32;
    if int_atk > 0 {
        base_attack / 2.0 + (rand_sample % (int_atk as u32)) as f32
    } else {
        base_attack
    }
}

/// Calculates upward vertical velocity added to an Iron Golem's victim scaled by knockback resistance.
///
/// Mirrors vanilla `IronGolem.java:198-199`:
/// ```java
/// double scale = Math.max(0.0, 1.0 - knockbackResistance);
/// target.setDeltaMovement(target.getDeltaMovement().add(0.0, 0.4F * scale, 0.0));
/// ```
#[must_use]
pub fn calculate_iron_golem_fling_velocity_y(current_y: f64, knockback_resistance: f64) -> f64 {
    let scale = (1.0 - knockback_resistance).max(0.0);
    current_y + 0.4 * scale
}

/// Calculates pitch for Iron Golem repair sound when fed an Iron Ingot.
///
/// Mirrors vanilla `IronGolem.java:273`:
/// `1.0F + (this.random.nextFloat() - this.random.nextFloat()) * 0.2F;`
#[must_use]
pub fn calculate_iron_golem_repair_pitch(rand1: f32, rand2: f32) -> f32 {
    (rand1 - rand2).mul_add(0.2, 1.0)
}

/// Calculates Crackiness level of an Iron Golem from health ratio.
///
/// Mirrors vanilla `IronGolem.java:219` and `Crackiness.java`:
/// - 0: None (> 0.75)
/// - 1: Low (0.50..=0.75)
/// - 2: Medium (0.25..=0.50)
/// - 3: High (<= 0.25)
#[must_use]
pub fn calculate_iron_golem_crackiness(health: f32, max_health: f32) -> u8 {
    if max_health <= 0.0 {
        return 0;
    }
    let fraction = health / max_health;
    if fraction > 0.75 {
        0
    } else if fraction > 0.50 {
        1
    } else if fraction > 0.25 {
        2
    } else {
        3
    }
}

/// Calculates sound pitch when a Snow Golem throws a snowball.
///
/// Mirrors vanilla `SnowGolem.java:133`:
/// `0.4F / (this.getRandom().nextFloat() * 0.4F + 0.8F)`
#[must_use]
pub fn calculate_snow_golem_shoot_pitch(rand: f32) -> f32 {
    0.4 / rand.mul_add(0.4, 0.8)
}

/// Determines whether an Enderman should evade damage via teleportation and whether the damage should be cancelled.
///
/// Mirrors vanilla `EnderMan.java:374-388`:
/// - If projectile damage and not a passenger: cancel damage (`true`), repeatedly teleport (`true`).
/// - If damage source is NOT from a living entity (e.g. water/rain/fire/environmental): damage is applied (`false`),
///   teleports with 90% chance (`rand_int_10 != 0`).
/// - Otherwise: damage is applied (`false`), do not auto-teleport (`false`).
#[must_use]
pub fn should_enderman_evade_damage(
    is_projectile: bool,
    is_passenger: bool,
    has_living_attacker: bool,
    rand_int_10: u32,
) -> (bool, bool) {
    if is_projectile && !is_passenger {
        (true, true)
    } else if !has_living_attacker {
        (false, rand_int_10 % 10 != 0)
    } else {
        (false, false)
    }
}

/// Shulker max health (30.0 HP).
///
/// Mirrors vanilla `Shulker.java:148`:
/// `return Mob.createMobAttributes().add(Attributes.MAX_HEALTH, 30.0);`
pub const SHULKER_MAX_HEALTH: f32 = 30.0;

/// Shulker covered armor bonus (+20.0 Armor).
///
/// Mirrors vanilla `Shulker.java:69`:
/// `COVERED_ARMOR_MODIFIER = new AttributeModifier(COVERED_ARMOR_MODIFIER_ID, 20.0, AttributeModifier.Operation.ADD_VALUE)`
pub const SHULKER_COVERED_ARMOR_BONUS: f32 = 20.0;

/// Shulker covered armor modifier ID (`minecraft:covered`).
///
/// Mirrors vanilla `Shulker.java:68`:
/// `COVERED_ARMOR_MODIFIER_ID = Identifier.withDefaultNamespace("covered")`
pub const SHULKER_COVERED_ARMOR_MODIFIER_ID: &str = "minecraft:covered";

/// Shulker experience reward (5 XP).
///
/// Mirrors vanilla `Shulker.java:92`:
/// `this.xpReward = 5;`
pub const SHULKER_XP_REWARD: u32 = 5;

/// Shulker lid opening rate per tick (0.05).
///
/// Mirrors vanilla `Shulker.java:79`:
/// `PEEK_PER_TICK = 0.05F;`
pub const SHULKER_PEEK_PER_TICK: f32 = 0.05;

/// Shulker maximum teleport distance (8 blocks).
///
/// Mirrors vanilla `Shulker.java:76`:
/// `MAX_TELEPORT_DISTANCE = 8;`
pub const SHULKER_MAX_TELEPORT_DISTANCE: i32 = 8;

/// Shulker maximum attack distance squared (400.0 = 20 blocks).
///
/// Mirrors vanilla `Shulker.java:632`:
/// `if (distance < 400.0)`
pub const SHULKER_ATTACK_RANGE_SQ: f64 = 400.0;

/// Shulker bullet flight speed (0.15).
///
/// Mirrors vanilla `ShulkerBullet.java:38`:
/// `SPEED = 0.15;`
pub const SHULKER_BULLET_SPEED: f64 = 0.15;

/// Checks whether a Shulker is closed based on raw peek value.
///
/// Mirrors vanilla `Shulker.java:453-455`:
/// `return this.getRawPeekAmount() == 0;`
#[must_use]
pub const fn is_shulker_closed(raw_peek: u8) -> bool {
    raw_peek == 0
}

/// Determines whether a Shulker completely deflects an incoming projectile when closed.
///
/// Mirrors vanilla `Shulker.java:430-435`:
/// ```java
/// if (this.isClosed()) {
///    Entity directEntity = source.getDirectEntity();
///    if (directEntity instanceof AbstractArrow) {
///       return false;
///    }
/// }
/// ```
#[must_use]
pub const fn should_shulker_deflect_arrow(is_closed: bool, is_arrow: bool) -> bool {
    is_closed && is_arrow
}

/// Determines whether a Shulker attempts low-health teleport upon being hurt.
///
/// Mirrors vanilla `Shulker.java:441-442`:
/// `if (this.getHealth() < this.getMaxHealth() * 0.5 && this.random.nextInt(4) == 0)`
#[must_use]
pub fn should_shulker_teleport_on_low_health(
    health: f32,
    max_health: f32,
    rand_int_4: u32,
) -> bool {
    health < max_health * 0.5 && rand_int_4 % 4 == 0
}

/// Calculates failure chance of Shulker duplication when hit by a Shulker Bullet.
///
/// Mirrors vanilla `Shulker.java:461-463`:
/// ```java
/// int shulkerCount = this.level().getEntities(EntityTypes.SHULKER, oldAabb.inflate(8.0), Entity::isAlive).size();
/// float failureChance = (shulkerCount - 1) / 5.0F;
/// ```
#[must_use]
pub fn calculate_shulker_duplication_failure_chance(nearby_shulkers: usize) -> f32 {
    let count = nearby_shulkers.saturating_sub(1) as f32;
    (count / 5.0).clamp(0.0, 1.0)
}

/// Determines whether Shulker reproduces upon being hit by a Shulker Bullet.
///
/// Mirrors vanilla `Shulker.java:460-464`:
/// - Shulker must not be closed (`!is_closed`)
/// - Shulker must successfully teleport (`teleport_success`)
/// - Random float must be >= failureChance (`rand_float >= failure_chance`)
#[must_use]
pub fn can_shulker_duplicate(
    is_closed: bool,
    teleport_success: bool,
    failure_chance: f32,
    rand_float: f32,
) -> bool {
    !is_closed && teleport_success && rand_float >= failure_chance
}

/// Creeper base max health (20.0 HP / 10 hearts).
/// Mirrors vanilla `Creeper.java:78` (`Attributes.MAX_HEALTH, 20.0`).
pub const CREEPER_MAX_HEALTH: f32 = 20.0;

/// Creeper base movement speed (0.25).
/// Mirrors vanilla `Creeper.java:78` (`Attributes.MOVEMENT_SPEED, 0.25`).
pub const CREEPER_BASE_MOVEMENT_SPEED: f64 = 0.25;

/// Creeper default fuse duration in ticks (30 ticks = 1.5 seconds).
/// Mirrors vanilla `Creeper.java:51`:
/// `private static final short DEFAULT_MAX_SWELL = 30;`
pub const CREEPER_DEFAULT_FUSE: i32 = 30;

/// Creeper default explosion radius in blocks (3 blocks).
/// Mirrors vanilla `Creeper.java:52`:
/// `private static final byte DEFAULT_EXPLOSION_RADIUS = 3;`
pub const CREEPER_DEFAULT_EXPLOSION_RADIUS: i32 = 3;

/// Charged Creeper explosion radius multiplier (2.0x = 6 blocks).
/// Mirrors vanilla `Creeper.java:232`:
/// `float explosionMultiplier = this.isPowered() ? 2.0F : 1.0F;`
pub const CREEPER_POWERED_EXPLOSION_MULTIPLIER: f32 = 2.0;

/// Creeper experience reward on death (5 XP).
/// Mirrors vanilla `Monster.java:55` (`this.xpReward = 5`).
pub const CREEPER_XP_REWARD: u32 = 5;

/// Creeper avoid feline (Cat/Ocelot) distance in blocks (6.0 blocks).
/// Mirrors vanilla `Creeper.java:67-68`:
/// `new AvoidEntityGoal<>(this, Ocelot.class, 6.0F, 1.0, 1.2)`
pub const CREEPER_AVOID_FELINE_DISTANCE: f32 = 6.0;
pub const CREEPER_AVOID_FELINE_WALK_SPEED: f64 = 1.0;
pub const CREEPER_AVOID_FELINE_SPRINT_SPEED: f64 = 1.2;

/// Creeper lingering effect cloud parameters on explosion.
/// Mirrors vanilla `Creeper.java:241-258`.
pub const CREEPER_CLOUD_RADIUS: f32 = 2.5;
pub const CREEPER_CLOUD_RADIUS_ON_USE: f32 = -0.5;
pub const CREEPER_CLOUD_WAIT_TIME: i32 = 10;
pub const CREEPER_CLOUD_DURATION: i32 = 300;
pub const CREEPER_CLOUD_POTION_DURATION_SCALE: f32 = 0.25;

/// Silverfish wake up friends search radius horizontal and vertical.
/// Mirrors vanilla `Silverfish.java:209-211`:
/// `yOff <= 5 && yOff >= -5 ... xOff <= 10 ... zOff <= 10`
pub const SILVERFISH_WAKE_UP_SEARCH_XZ: i32 = 10;
pub const SILVERFISH_WAKE_UP_SEARCH_Y: i32 = 5;

/// Silverfish / Endermite step sound volume (0.15).
/// Mirrors vanilla `Silverfish.java:78` and `Endermite.java:77`:
/// `this.playSound(SoundEvents...._STEP, 0.15F, 1.0F);`
pub const SILVERFISH_STEP_SOUND_VOLUME: f32 = 0.15;
pub const ENDERMITE_STEP_SOUND_VOLUME: f32 = 0.15;

/// Determines whether an item can be used to ignite a Creeper.
///
/// Mirrors vanilla `ItemTags.CREEPER_IGNITERS` (`Creeper.java:212`):
/// - `minecraft:flint_and_steel`
/// - `minecraft:fire_charge`
#[must_use]
pub fn is_creeper_igniter(item_id: u16) -> bool {
    item_id == pumpkin_data::item::Item::FLINT_AND_STEEL.id
        || item_id == pumpkin_data::item::Item::FIRE_CHARGE.id
}

/// Calculates sound pitch when a Creeper is ignited with flint and steel or fire charge.
///
/// Mirrors vanilla `Creeper.java:214`:
/// `this.random.nextFloat() * 0.4F + 0.8F`
#[must_use]
pub fn calculate_creeper_ignite_sound_pitch(rand_val: f32) -> f32 {
    rand_val.mul_add(0.4, 0.8)
}

/// Calculates explosion radius of a Creeper scaled by charged state.
///
/// Mirrors vanilla `Creeper.java:232`:
/// `float explosionMultiplier = this.isPowered() ? 2.0F : 1.0F;`
#[must_use]
pub fn calculate_creeper_explosion_radius(base_radius: f32, is_charged: bool) -> f32 {
    if is_charged {
        base_radius * 2.0
    } else {
        base_radius
    }
}

/// Calculates swell accumulated by a Creeper from fall damage.
///
/// Mirrors vanilla `Creeper.java:89-92`:
/// ```java
/// this.swell += (int)(fallDistance * 1.5);
/// if (this.swell > this.maxSwell - 5) {
///    this.swell = this.maxSwell - 5;
/// }
/// ```
#[must_use]
pub fn calculate_creeper_fall_swell(
    current_swell: i32,
    max_swell: i32,
    fall_distance: f64,
) -> i32 {
    let added = (fall_distance * 1.5) as i32;
    (current_swell + added).min(max_swell - 5).max(current_swell)
}

/// Checks whether a Creeper is allowed to target an entity.
///
/// Mirrors vanilla `Creeper.java:154-158`:
/// `if (!(target instanceof Goat)) { super.setTarget(target); }`
#[must_use]
pub fn can_creeper_target_entity(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    entity_type != &pumpkin_data::entity::EntityType::GOAT
}

/// Checks whether Blaze should take water/rain drown damage.
///
/// Mirrors vanilla `LivingEntity.java:3162-3174` & `Blaze.java:114`:
/// `this.isSensitiveToWater() && this.isInWaterOrRain()`
#[must_use]
pub const fn should_blaze_take_water_damage(touching_water: bool, raining: bool) -> bool {
    touching_water || raining
}

/// Determines whether a Guardian / Elder Guardian inflicts thorns spike damage upon taking damage.
///
/// Mirrors vanilla `Guardian.java:311-317`:
/// ```java
/// if (!this.isMoving()
///    && !source.is(DamageTypeTags.AVOIDS_GUARDIAN_THORNS)
///    && !source.is(DamageTypes.THORNS)
///    && source.getDirectEntity() instanceof LivingEntity cause) {
///    cause.hurtServer(level, this.damageSources().thorns(this), 2.0F);
/// }
/// ```
#[must_use]
pub fn should_guardian_retaliate_thorns(
    is_moving: bool,
    damage_type: &pumpkin_data::damage::DamageType,
    direct_attacker_is_living: bool,
) -> bool {
    use pumpkin_data::tag::Taggable;
    if is_moving {
        return false;
    }
    if damage_type == &pumpkin_data::damage::DamageType::THORNS {
        return false;
    }
    if damage_type.has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_AVOIDS_GUARDIAN_THORNS) {
        return false;
    }
    direct_attacker_is_living
}

// ============================================================================
// Guardian Family Parity Constants (Guardian & Elder Guardian)
// ============================================================================

/// Vanilla Guardian thorns damage amount (2.0F).
/// Mirrors vanilla Guardian.java:315.
pub const GUARDIAN_THORNS_DAMAGE: f32 = 2.0;

/// Base max health for a Guardian (30.0 HP / 15 hearts).
/// Mirrors vanilla Guardian.java:94 (Attributes.MAX_HEALTH, 30.0).
pub const GUARDIAN_MAX_HEALTH: f32 = 30.0;

/// Base movement speed for a Guardian.
/// Mirrors vanilla Guardian.java:94 (Attributes.MOVEMENT_SPEED, 0.5).
pub const GUARDIAN_BASE_MOVEMENT_SPEED: f64 = 0.5;

/// Base attack damage for a Guardian.
/// Mirrors vanilla Guardian.java:94 (Attributes.ATTACK_DAMAGE, 6.0).
pub const GUARDIAN_BASE_ATTACK_DAMAGE: f32 = 6.0;

/// Experience reward dropped by a Guardian on death (10 XP).
/// Mirrors vanilla Guardian.java:55 (	his.xpReward = 10).
pub const GUARDIAN_XP_REWARD: u32 = 10;

/// Laser warmup duration before beam starts (10 ticks / 0.5s, from -10 to 0).
/// Mirrors vanilla Guardian.java:374 (	his.attackTime = -10).
pub const GUARDIAN_LASER_WARMUP_TICKS: i32 = 10;

/// Laser beam charging duration for a normal Guardian (80 ticks / 4s).
/// Mirrors vanilla Guardian.java:49,125 (getAttackDuration() { return 80; }).
pub const GUARDIAN_ATTACK_DURATION_TICKS: i32 = 80;

/// Base max health for an Elder Guardian (80.0 HP / 40 hearts).
/// Mirrors vanilla ElderGuardian.java:36 (Attributes.MAX_HEALTH, 80.0).
pub const ELDER_GUARDIAN_MAX_HEALTH: f32 = 80.0;

/// Base movement speed for an Elder Guardian.
/// Mirrors vanilla ElderGuardian.java:36 (Attributes.MOVEMENT_SPEED, 0.3F).
pub const ELDER_GUARDIAN_BASE_MOVEMENT_SPEED: f64 = 0.3;

/// Base attack damage for an Elder Guardian.
/// Mirrors vanilla ElderGuardian.java:36 (Attributes.ATTACK_DAMAGE, 8.0).
pub const ELDER_GUARDIAN_BASE_ATTACK_DAMAGE: f32 = 8.0;

/// Experience reward dropped by an Elder Guardian on death (10 XP).
/// Mirrors vanilla ElderGuardian.java / Guardian.java:55 (	his.xpReward = 10).
pub const ELDER_GUARDIAN_XP_REWARD: u32 = 10;

/// Laser beam charging duration for an Elder Guardian (60 ticks / 3s).
/// Mirrors vanilla ElderGuardian.java:41 (getAttackDuration() { return 60; }).
pub const ELDER_GUARDIAN_ATTACK_DURATION_TICKS: i32 = 60;

/// Interval between Elder Guardian ghost curse pulses (1200 ticks / 60s).
/// Mirrors vanilla ElderGuardian.java:23 (EFFECT_INTERVAL = 1200).
pub const ELDER_GUARDIAN_CURSE_INTERVAL_TICKS: i32 = 1200;

/// Radius of Elder Guardian ghost curse pulse (50.0 blocks).
/// Mirrors vanilla ElderGuardian.java:24 (EFFECT_RADIUS = 50).
pub const ELDER_GUARDIAN_CURSE_RADIUS: f64 = 50.0;

/// Duration of Mining Fatigue applied by Elder Guardian curse (6000 ticks / 5 minutes).
/// Mirrors vanilla ElderGuardian.java:25 (EFFECT_DURATION = 6000).
pub const ELDER_GUARDIAN_CURSE_DURATION_TICKS: i32 = 6000;

/// Amplifier of Mining Fatigue applied by Elder Guardian curse (amplifier 2 = Level III).
/// Mirrors vanilla ElderGuardian.java:26 (EFFECT_AMPLIFIER = 2).
pub const ELDER_GUARDIAN_CURSE_AMPLIFIER: u8 = 2;

/// Elder Guardian mining fatigue display/re-apply limit in ticks (1200 ticks = 1 minute).
/// Mirrors vanilla `ElderGuardian.java:25`:
/// `private static final int EFFECT_DISPLAY_LIMIT = 1200;`
pub const ELDER_GUARDIAN_CURSE_DISPLAY_LIMIT_TICKS: i32 = 1200;

/// Elder Guardian scale compared to normal Guardian (2.35x).
/// Mirrors vanilla `ElderGuardian.java:20`:
/// `EntityTypes.ELDER_GUARDIAN.getWidth() / EntityTypes.GUARDIAN.getWidth()`
pub const ELDER_GUARDIAN_SIZE_SCALE: f32 = 2.35;

/// Minimum squared distance required for normal Guardian to initiate or continue laser attack (9.0 = 3.0 blocks).
/// Mirrors vanilla `Guardian.java:360, 433`:
/// `this.guardian.distanceToSqr(this.guardian.getTarget()) > 9.0`
pub const GUARDIAN_MIN_ATTACK_DISTANCE_SQ: f64 = 9.0;

/// Guardian flop motion parameters when grounded on land.
/// Mirrors vanilla `Guardian.java:260-262`:
/// `(random.nextFloat() * 2.0F - 1.0F) * 0.4F, 0.5, (random.nextFloat() * 2.0F - 1.0F) * 0.4F`
pub const GUARDIAN_FLOP_VERTICAL_VELOCITY: f64 = 0.5;
pub const GUARDIAN_FLOP_HORIZONTAL_AMPLITUDE: f32 = 0.4;

/// Returns laser attack duration in ticks (60 for Elder Guardian, 80 for normal Guardian).
/// Mirrors vanilla Guardian.java:125 and ElderGuardian.java:41.
#[must_use]
pub const fn get_guardian_attack_duration(is_elder: bool) -> i32 {
    if is_elder {
        ELDER_GUARDIAN_ATTACK_DURATION_TICKS
    } else {
        GUARDIAN_ATTACK_DURATION_TICKS
    }
}

/// Creates the Mining Fatigue III potion effect instance applied by Elder Guardian curse.
/// Mirrors vanilla ElderGuardian.java:70:
/// 
/// `new MobEffectInstance(MobEffects.MINING_FATIGUE, 6000, 2)`
#[must_use]
pub fn create_elder_guardian_curse_effect() -> pumpkin_data::potion::Effect {
    pumpkin_data::potion::Effect {
        effect_type: &pumpkin_data::effect::StatusEffect::MINING_FATIGUE,
        duration: ELDER_GUARDIAN_CURSE_DURATION_TICKS,
        amplifier: ELDER_GUARDIAN_CURSE_AMPLIFIER,
        ambient: false,
        show_particles: true,
        show_icon: true,
        blend: false,
    }
}

/// Calculates magic damage dealt by Guardian or Elder Guardian laser beam.
///
/// Mirrors vanilla `Guardian.java:403-410`:
/// - Base: 1.0F
/// - Hard difficulty bonus: +2.0F
/// - Elder Guardian bonus: +2.0F
#[must_use]
pub fn calculate_guardian_laser_damage(
    difficulty: pumpkin_util::Difficulty,
    is_elder: bool,
) -> f32 {
    let mut damage = 1.0;
    if difficulty == pumpkin_util::Difficulty::Hard {
        damage += 2.0;
    }
    if is_elder {
        damage += 2.0;
    }
    damage
}

/// Calculates the sound pitch for a Witch drinking a potion.
///
/// Mirrors vanilla `Witch.java:165`: `0.8F + this.random.nextFloat() * 0.4F`
#[must_use]
pub fn calculate_witch_drink_pitch(rand_0_to_1: f32) -> f32 {
    0.8 + rand_0_to_1 * 0.4
}

/// Calculates the sound pitch for a Witch throwing a splash potion.
///
/// Mirrors vanilla `Witch.java:265`: `0.8F + this.random.nextFloat() * 0.4F`
#[must_use]
pub fn calculate_witch_throw_pitch(rand_0_to_1: f32) -> f32 {
    0.8 + rand_0_to_1 * 0.4
}

/// Determines whether a Witch should drink a Potion of Water Breathing.
///
/// Mirrors vanilla `Witch.java:136-137`:
/// `this.random.nextFloat() < 0.15F && this.isEyeInFluid(FluidTags.WATER) && !this.hasEffect(MobEffects.WATER_BREATHING)`
#[must_use]
pub const fn should_witch_drink_water_breathing(
    rand_0_to_1: f32,
    eye_in_water: bool,
    has_water_breathing: bool,
) -> bool {
    rand_0_to_1 < 0.15 && eye_in_water && !has_water_breathing
}

/// Determines whether a Witch should drink a Potion of Fire Resistance.
///
/// Mirrors vanilla `Witch.java:138-141`:
/// `this.random.nextFloat() < 0.15F && (this.isOnFire() || lastDamageSource.is(DamageTypeTags.IS_FIRE)) && !this.hasEffect(MobEffects.FIRE_RESISTANCE)`
#[must_use]
pub const fn should_witch_drink_fire_resistance(
    rand_0_to_1: f32,
    is_on_fire: bool,
    damage_is_fire: bool,
    has_fire_resistance: bool,
) -> bool {
    rand_0_to_1 < 0.15 && (is_on_fire || damage_is_fire) && !has_fire_resistance
}

/// Determines whether a Witch should drink a Potion of Healing.
///
/// Mirrors vanilla `Witch.java:142-143`:
/// `this.random.nextFloat() < 0.05F && this.getHealth() < this.getMaxHealth()`
#[must_use]
pub const fn should_witch_drink_healing(
    rand_0_to_1: f32,
    health: f32,
    max_health: f32,
) -> bool {
    rand_0_to_1 < 0.05 && health < max_health
}

/// Determines whether a Witch should drink a Potion of Swiftness.
///
/// Mirrors vanilla `Witch.java:144-149`:
/// `this.random.nextFloat() < 0.5F && this.getTarget() != null && !this.hasEffect(MobEffects.SPEED) && this.getTarget().distanceToSqr(this) > 121.0`
#[must_use]
pub const fn should_witch_drink_speed(
    rand_0_to_1: f32,
    has_target: bool,
    dist_sq: f64,
    has_speed: bool,
) -> bool {
    rand_0_to_1 < 0.5 && has_target && dist_sq > 121.0 && !has_speed
}

/// Witch thrown potion selection.
///
/// Mirrors vanilla `Witch.java:230-246`:
/// - When targeting a Raider ally:
///   - Health <= 4.0: Healing
///   - Health > 4.0: Regeneration
/// - When targeting an enemy:
///   - Distance >= 8.0 and not slowness: Slowness
///   - Target health >= 8.0 and not poison: Poison
///   - Distance <= 3.0 and not weakness and 25% chance: Weakness
///   - Default: Harming
#[must_use]
pub fn determine_witch_thrown_potion(
    is_raider: bool,
    target_health: f32,
    horizontal_dist: f64,
    has_slowness: bool,
    has_poison: bool,
    has_weakness: bool,
    weakness_rand: f32,
) -> &'static pumpkin_data::potion::Potion {
    use pumpkin_data::potion::Potion;
    if is_raider {
        if target_health <= 4.0 {
            &Potion::HEALING
        } else {
            &Potion::REGENERATION
        }
    } else if horizontal_dist >= 8.0 && !has_slowness {
        &Potion::SLOWNESS
    } else if target_health >= 8.0 && !has_poison {
        &Potion::POISON
    } else if horizontal_dist <= 3.0 && !has_weakness && weakness_rand < 0.25 {
        &Potion::WEAKNESS
    } else {
        &Potion::HARMING
    }
}

/// Calculates damage received by a Witch after magic absorption and self-immunity.
///
/// Mirrors vanilla `Witch.java:209-220`:
/// ```java
/// if (damageSource.getEntity() == this) {
///     damage = 0.0F;
/// }
/// if (damageSource.is(DamageTypeTags.WITCH_RESISTANT_TO)) {
///     damage *= 0.15F;
/// }
/// ```
#[must_use]
pub const fn calculate_witch_damage_received(
    raw_damage: f32,
    is_self_damage: bool,
    is_witch_resistant_to: bool,
) -> f32 {
    if is_self_damage {
        0.0
    } else if is_witch_resistant_to {
        raw_damage * 0.15
    } else {
        raw_damage
    }
}

/// Phantom max health (20.0 HP).
///
/// Mirrors vanilla `Phantom.java:44` / `Mob.java`:
pub const PHANTOM_MAX_HEALTH: f32 = 20.0;

/// Phantom base attack damage (6.0 HP).
///
/// Mirrors vanilla `Phantom.java:89`: `6 + this.getPhantomSize()`.
pub const PHANTOM_BASE_ATTACK_DAMAGE: f32 = 6.0;

/// Phantom experience reward (5 XP).
///
/// Mirrors vanilla `Phantom.java:54`: `this.xpReward = 5;`.
pub const PHANTOM_XP_REWARD: u32 = 5;

/// Phantom maximum size limit (64).
///
/// Mirrors vanilla `Phantom.java:84`: `Mth.clamp(size, 0, 64)`.
pub const PHANTOM_MAX_SIZE: i32 = 64;

/// Phantom ticks per wing flap cycle (25 ticks).
///
/// Mirrors vanilla `Phantom.java:46`: `TICKS_PER_FLAP = Mth.ceil(24.166098F)`.
pub const PHANTOM_TICKS_PER_FLAP: i32 = 25;

/// Phantom wing flap degrees per tick (~7.45 deg/tick).
///
/// Mirrors vanilla `Phantom.java:45`: `FLAP_DEGREES_PER_TICK = 7.448451F`.
pub const PHANTOM_FLAP_DEGREES_PER_TICK: f32 = 7.448451;

/// Phantom cat detection radius for fear response (16.0 blocks).
///
/// Mirrors vanilla `Phantom.java:463`: `Phantom.this.getBoundingBox().inflate(16.0)`.
pub const PHANTOM_CAT_CHECK_RADIUS: f64 = 16.0;

/// Phantom target acquisition horizontal search box inflation (16.0 blocks).
///
/// Mirrors vanilla `Phantom.java:222`: `Phantom.this.getBoundingBox().inflate(16.0, 64.0, 16.0)`.
pub const PHANTOM_TARGET_SEARCH_XZ: f64 = 16.0;

/// Phantom target acquisition vertical search box inflation (64.0 blocks).
///
/// Mirrors vanilla `Phantom.java:222`: `Phantom.this.getBoundingBox().inflate(16.0, 64.0, 16.0)`.
pub const PHANTOM_TARGET_SEARCH_Y: f64 = 64.0;

/// Checks if a Phantom is currently at its wing flap peak.
///
/// Mirrors vanilla `Phantom.java:60-62`:
/// `(this.getUniqueFlapTickOffset() + this.tickCount) % TICKS_PER_FLAP == 0`
#[must_use]
pub const fn is_phantom_flapping(entity_id: i32, tick_count: i32) -> bool {
    let offset = entity_id * 3;
    (offset + tick_count) % PHANTOM_TICKS_PER_FLAP == 0
}

/// Calculates Phantom attack damage scaling based on size (0..=64).
///
/// Mirrors vanilla `Phantom.java:89`: `6 + this.getPhantomSize()`
#[must_use]
pub fn calculate_phantom_attack_damage(phantom_size: i32) -> f32 {
    6.0 + phantom_size.clamp(0, 64) as f32
}

/// Calculates Phantom bounding box dimension scale based on size (0..=64).
///
/// Mirrors vanilla `Phantom.java:197`: `1.0F + 0.15F * size`
#[must_use]
pub fn calculate_phantom_dimension_scale(phantom_size: i32) -> f32 {
    1.0 + 0.15 * phantom_size.clamp(0, 64) as f32
}

/// Calculates Phantom swoop sound pitch.
///
/// Mirrors vanilla `Phantom.java:277`: `0.95F + this.random.nextFloat() * 0.1F`
#[must_use]
pub fn calculate_phantom_swoop_pitch(rand_0_to_1: f32) -> f32 {
    0.95 + rand_0_to_1 * 0.1
}

/// Calculates Phantom wing flap sound pitch.
///
/// Mirrors vanilla `Phantom.java:118`: `0.95F + this.random.nextFloat() * 0.05F`
#[must_use]
pub fn calculate_phantom_flap_pitch(rand_0_to_1: f32) -> f32 {
    0.95 + rand_0_to_1 * 0.05
}

/// Determines whether a Phantom should abort swoop attack due to collision or recent damage.
///
/// Mirrors vanilla `Phantom.java:493`:
/// `else if (Phantom.this.horizontalCollision || Phantom.this.wasHurtRecently())`
#[must_use]
pub const fn should_phantom_abort_swoop_on_collision_or_damage(
    horizontal_collision: bool,
    was_hurt_recently: bool,
) -> bool {
    horizontal_collision || was_hurt_recently
}

/// Ghast base max health (10.0 HP / 5 hearts).
/// Mirrors vanilla `Ghast.java:118` (`Attributes.MAX_HEALTH, 10.0`).
pub const GHAST_MAX_HEALTH: f32 = 10.0;

/// Ghast player follow range (100.0 blocks).
/// Mirrors vanilla `Ghast.java:119` (`Attributes.FOLLOW_RANGE, 100.0`).
pub const GHAST_FOLLOW_RANGE: f64 = 100.0;

/// Ghast camera distance for third-person rendering (8.0).
/// Mirrors vanilla `Ghast.java:120` (`Attributes.CAMERA_DISTANCE, 8.0`).
pub const GHAST_CAMERA_DISTANCE: f64 = 8.0;

/// Ghast flying speed attribute (0.06).
/// Mirrors vanilla `Ghast.java:121` (`Attributes.FLYING_SPEED, 0.06`).
pub const GHAST_FLYING_SPEED: f64 = 0.06;

/// Ghast experience reward on death (5 XP).
/// Mirrors vanilla `Ghast.java:51` (`this.xpReward = 5`).
pub const GHAST_XP_REWARD: u32 = 5;

/// Ghast ambient and hurt sound volume (5.0F).
/// Mirrors vanilla `Ghast.java:146` (`return 5.0F;`).
pub const GHAST_SOUND_VOLUME: f32 = 5.0;

/// Default explosion power of a Ghast LargeFireball (1).
/// Mirrors vanilla `Ghast.java:46` (`DEFAULT_EXPLOSION_POWER = 1`).
pub const GHAST_DEFAULT_EXPLOSION_POWER: u8 = 1;

/// Instakill damage dealt when a reflected LargeFireball hits a Ghast (1000.0F).
/// Mirrors vanilla `Ghast.java:103` (`super.hurtServer(level, source, 1000.0F)`).
pub const GHAST_REFLECTED_FIREBALL_DAMAGE: f32 = 1000.0;

/// Maximum squared distance for Ghast attack targeting (64 blocks / 4096.0 sq blocks).
/// Mirrors vanilla `Ghast.java:195, 358` (`target.distanceToSqr < 4096.0`).
pub const GHAST_ATTACK_RANGE_SQ: f64 = 4096.0;

/// Ticks of fireball charging before Ghast emits warning sound (10 ticks = 0.5s).
/// Mirrors vanilla `Ghast.java:361` (`this.chargeTime == 10`).
pub const GHAST_FIREBALL_WARN_TICKS: i32 = 10;

/// Ticks of fireball charging when Ghast launches LargeFireball (20 ticks = 1.0s).
/// Mirrors vanilla `Ghast.java:365` (`this.chargeTime == 20`).
pub const GHAST_FIREBALL_SHOOT_TICKS: i32 = 20;

/// Reset value of chargeTime following a fireball shot (-40 ticks = 2.0s cooldown).
/// Mirrors vanilla `Ghast.java:379` (`this.chargeTime = -40`).
pub const GHAST_FIREBALL_COOLDOWN_TICKS: i32 = -40;

/// Sound event ID broadcast when Ghast begins charging fireball (1015).
/// Mirrors vanilla `Ghast.java:362` (`level.levelEvent(null, 1015, ...)`).
pub const GHAST_WARN_SOUND_EVENT_ID: i32 = 1015;

/// Sound event ID broadcast when Ghast shoots fireball (1016).
/// Mirrors vanilla `Ghast.java:373` (`level.levelEvent(null, 1016, ...)`).
pub const GHAST_SHOOT_SOUND_EVENT_ID: i32 = 1016;

/// Flight acceleration scale when drifting towards wanted position (FLYING_SPEED * 5.0 / 3.0 = 0.1).
/// Mirrors vanilla `Ghast.java:253`.
pub const GHAST_FLOAT_MOVE_SCALE: f64 = 0.1;

/// Radius in blocks for random float position selection (16.0 blocks).
/// Mirrors vanilla `Ghast.java:479-481` (`(random.nextFloat() * 2.0F - 1.0F) * 16.0F`).
pub const GHAST_FLOAT_WANDER_RADIUS: f64 = 16.0;

/// Minimum squared distance to destination before re-rolling float target (1.0).
/// Mirrors vanilla `Ghast.java:416` (`dd < 1.0 || dd > 3600.0`).
pub const GHAST_FLOAT_MIN_DIST_SQ: f64 = 1.0;

/// Maximum squared distance to destination before cancelling float goal (3600.0 / 60 blocks).
/// Mirrors vanilla `Ghast.java:416`.
pub const GHAST_FLOAT_MAX_DIST_SQ: f64 = 3600.0;

/// Checks whether a damage source against a Ghast is a reflected LargeFireball by a Player.
///
/// Mirrors vanilla `Ghast.java:76-78`:
/// `source.getDirectEntity() instanceof LargeFireball && source.getEntity() instanceof Player`
#[must_use]
pub const fn is_ghast_reflected_fireball(
    direct_entity_is_fireball: bool,
    attacker_is_player: bool,
) -> bool {
    direct_entity_is_fireball && attacker_is_player
}

/// Calculates damage received by a Ghast. Reflected fireballs deal 1000.0F instakill damage.
///
/// Mirrors vanilla `Ghast.java:101-108`:
/// ```java
/// if (isReflectedFireball(source)) {
///     super.hurtServer(level, source, 1000.0F);
/// }
/// ```
#[must_use]
pub const fn calculate_ghast_damage_received(
    raw_damage: f32,
    is_reflected_fireball: bool,
) -> f32 {
    if is_reflected_fireball {
        1000.0
    } else {
        raw_damage
    }
}

/// Minimum valid size for any CubeMob (1).
/// Mirrors vanilla `AbstractCubeMob.java:44` (`MIN_SIZE = 1`).
pub const CUBE_MOB_MIN_SIZE: i32 = 1;

/// Maximum valid size for any CubeMob (127).
/// Mirrors vanilla `AbstractCubeMob.java:45` (`MAX_SIZE = 127`).
pub const CUBE_MOB_MAX_SIZE: i32 = 127;

/// Maximum natural spawn size scale for wild CubeMobs (4).
/// Mirrors vanilla `AbstractCubeMob.java:46` (`MAX_NATURAL_SIZE = 4`).
pub const CUBE_MOB_MAX_NATURAL_SIZE: i32 = 4;

/// Ticks a CubeMob attack goal can continue before growing tired (300 ticks = 15s).
/// Mirrors vanilla `AbstractCubeMob.java:352` (`reducedTickDelay(300)`).
pub const CUBE_MOB_ATTACK_TIRED_TICKS: i32 = 300;

/// Maximum height (Y) difference between CubeMob and player for target acquisition (4.0 blocks).
/// Mirrors vanilla `Slime.java:44` & `MagmaCube.java:45`:
/// `Math.abs(target.getY() - this.getY()) <= 4.0`
pub const CUBE_MOB_TARGET_PLAYER_MAX_Y_DIFF: f64 = 4.0;

/// Offset multiplier per size for passenger attachment height.
/// Mirrors vanilla `AbstractCubeMob.java:250`:
/// `dimensions.height() - 0.015625 * this.getSize() * scale`
pub const CUBE_MOB_PASSENGER_ATTACHMENT_Y_OFFSET_SCALE: f64 = 0.015625;

/// Jump delay multiplier for Magma Cubes relative to base CubeMob jump delay (4x).
/// Mirrors vanilla `MagmaCube.java:85` (`super.getJumpDelay() * 4`).
pub const MAGMA_CUBE_JUMP_DELAY_MULTIPLIER: i32 = 4;

/// Bonus attack damage added to Magma Cubes beyond base size attribute (+2.0F).
/// Mirrors vanilla `MagmaCube.java:119` (`super.getAttackDamage() + 2.0F`).
pub const MAGMA_CUBE_ATTACK_DAMAGE_BONUS: f32 = 2.0;

/// Armor points granted per size for Magma Cubes (3.0 per size).
/// Mirrors vanilla `MagmaCube.java:64` (`Attributes.ARMOR.setBaseValue(size * 3)`).
pub const MAGMA_CUBE_ARMOR_PER_SIZE: f32 = 3.0;

/// Additional jump boost per size when a Magma Cube jumps from ground (0.1F per size).
/// Mirrors vanilla `MagmaCube.java:96` (`this.getSize() * 0.1F`).
pub const MAGMA_CUBE_JUMP_BOOST_PER_SIZE: f64 = 0.1;

/// Base vertical velocity when a Magma Cube jumps in lava (0.22F).
/// Mirrors vanilla `MagmaCube.java:105` (`0.22F + this.getSize() * 0.05F`).
pub const MAGMA_CUBE_LAVA_JUMP_BASE: f64 = 0.22;

/// Extra vertical velocity per size when a Magma Cube jumps in lava (0.05F per size).
/// Mirrors vanilla `MagmaCube.java:105`.
pub const MAGMA_CUBE_LAVA_JUMP_PER_SIZE: f64 = 0.05;

/// Squish decay factor per tick for Slimes (0.6F).
/// Mirrors vanilla `AbstractCubeMob.java:150` (`this.targetSquish *= 0.6F`).
pub const SLIME_SQUISH_DECAY: f32 = 0.6;

/// Squish decay factor per tick for Magma Cubes (0.9F).
/// Mirrors vanilla `MagmaCube.java:90` (`this.targetSquish *= 0.9F`).
pub const MAGMA_CUBE_SQUISH_DECAY: f32 = 0.9;

/// Jump chance per tick when a CubeMob floats in liquid (80% / 0.8F).
/// Mirrors vanilla `AbstractCubeMob.java:405` (`random.nextFloat() < 0.8F`).
pub const CUBE_MOB_FLOAT_JUMP_CHANCE: f32 = 0.8;

/// Wanted speed modifier when a CubeMob floats in liquid (1.2).
/// Mirrors vanilla `AbstractCubeMob.java:410` (`setWantedMovement(1.2)`).
pub const CUBE_MOB_FLOAT_WANTED_MOVEMENT: f64 = 1.2;

/// Wanted speed modifier for CubeMob keep-jumping goal (1.0).
/// Mirrors vanilla `AbstractCubeMob.java:431` (`setWantedMovement(1.0)`).
pub const CUBE_MOB_KEEP_JUMPING_WANTED_MOVEMENT: f64 = 1.0;

/// Base randomize interval for CubeMob random direction goal (40 ticks).
/// Mirrors vanilla `AbstractCubeMob.java:509` (`40 + random.nextInt(60)`).
pub const CUBE_MOB_RANDOM_DIRECTION_MIN_INTERVAL: i32 = 40;

/// Random additional interval for CubeMob random direction goal (60 ticks).
/// Mirrors vanilla `AbstractCubeMob.java:509`.
pub const CUBE_MOB_RANDOM_DIRECTION_EXTRA_INTERVAL: i32 = 60;

/// Minimum child count spawned when a CubeMob splits on death (2).
/// Mirrors vanilla `AbstractCubeMob.java:216` (`2 + this.random.nextInt(3)`).
pub const CUBE_MOB_SPLIT_MIN_COUNT: i32 = 2;

/// Extra random child count spawned when a CubeMob splits on death (up to +2).
/// Mirrors vanilla `AbstractCubeMob.java:216`.
pub const CUBE_MOB_SPLIT_EXTRA_COUNT: i32 = 3;

/// Calculates passenger attachment Y position for CubeMob.
/// Mirrors vanilla `AbstractCubeMob.java:250`:
/// `dimensions.height() - 0.015625 * this.getSize() * scale`
#[must_use]
pub fn calculate_cube_mob_passenger_attachment_y(dimension_height: f64, size: i32, scale: f32) -> f64 {
    dimension_height - CUBE_MOB_PASSENGER_ATTACHMENT_Y_OFFSET_SCALE * f64::from(size.max(1)) * f64::from(scale)
}

/// Checks whether a player's vertical position is within targeting height difference range (<= 4.0 blocks).
/// Mirrors vanilla `Slime.java:44` & `MagmaCube.java:45`.
#[must_use]
pub fn is_target_player_within_cube_y_range(target_y: f64, cube_y: f64) -> bool {
    (target_y - cube_y).abs() <= CUBE_MOB_TARGET_PLAYER_MAX_Y_DIFF
}

/// Simulates natural CubeMob size calculation based on difficulty.
/// Mirrors vanilla `AbstractCubeMob.java:322-327`.
#[must_use]
pub fn calculate_cube_mob_natural_spawn_size(size_scale_roll: i32, difficulty_special_mult: f32, chance_roll: f32) -> i32 {
    let mut size_scale = size_scale_roll.clamp(0, 2);
    if size_scale < 2 && chance_roll < 0.5 * difficulty_special_mult {
        size_scale += 1;
    }
    1 << size_scale
}

/// Calculates CubeMob (Slime & MagmaCube) contact attack damage based on size and mob type.
///
/// Mirrors vanilla `Slime.java:122` & `MagmaCube.java:63, 118-120`:
/// - Slime: Tiny (size <= 1) deals 0.0 (cannot deal damage). Size >= 2 deals `size as f32`.
/// - MagmaCube: Deals `(size + 2) as f32` for all sizes.
#[must_use]
pub fn calculate_cube_mob_attack_damage(size: i32, is_magma_cube: bool) -> f32 {
    let actual_size = size.max(1);
    if is_magma_cube {
        (actual_size + 2) as f32
    } else if actual_size <= 1 {
        0.0
    } else {
        actual_size as f32
    }
}

/// Determines whether a CubeMob can deal contact damage to touched entities.
///
/// Mirrors vanilla `Slime.java` / `AbstractCubeMob.java:254` & `MagmaCube.java:113-115`:
/// - MagmaCube: `isEffectiveAi()` (can deal damage at all sizes)
/// - Slime: `!isTiny() && isEffectiveAi()` (size > 1 only)
#[must_use]
pub const fn should_cube_mob_deal_contact_damage(
    size: i32,
    is_magma_cube: bool,
    has_effective_ai: bool,
) -> bool {
    if !has_effective_ai {
        false
    } else if is_magma_cube {
        true
    } else {
        size > 1
    }
}

/// Calculates armor points for CubeMob based on size.
///
/// Mirrors vanilla `MagmaCube.java:64`:
/// `this.getAttribute(Attributes.ARMOR).setBaseValue(size * 3);`
#[must_use]
pub fn calculate_cube_mob_armor(size: i32, is_magma_cube: bool) -> f32 {
    if is_magma_cube {
        (size.max(1) * 3) as f32
    } else {
        0.0
    }
}

/// Calculates base experience reward dropped by a CubeMob.
///
/// Mirrors vanilla `Slime.java:123` & `MagmaCube.java:65`:
/// `this.xpReward = actualSize;`
#[must_use]
pub fn calculate_cube_mob_xp_reward(size: i32) -> u32 {
    size.max(1) as u32
}

/// Returns squish decay multiplier applied each tick.
///
/// Mirrors vanilla `AbstractCubeMob.java:150` & `MagmaCube.java:90`:
/// - Slime / Default: `0.6F`
/// - MagmaCube: `0.9F`
#[must_use]
pub const fn calculate_cube_mob_squish_decay(is_magma_cube: bool) -> f32 {
    if is_magma_cube {
        0.9
    } else {
        0.6
    }
}

/// Calculates Y-velocity boost applied when a CubeMob jumps from the ground.
///
/// Mirrors vanilla `AbstractCubeMob.java:268-270` & `MagmaCube.java:95-97`:
/// - Slime: `base_jump_power`
/// - MagmaCube: `base_jump_power + size * 0.1F`
#[must_use]
pub fn calculate_cube_mob_jump_velocity(
    base_jump_power: f64,
    size: i32,
    is_magma_cube: bool,
) -> f64 {
    if is_magma_cube {
        base_jump_power + f64::from(size.max(1)) * 0.1
    } else {
        base_jump_power
    }
}

/// Calculates Y-velocity applied when a MagmaCube jumps in lava.
///
/// Mirrors vanilla `MagmaCube.java:104-106`:
/// `0.22F + this.getSize() * 0.05F`
#[must_use]
pub fn calculate_magma_cube_lava_jump_velocity(size: i32) -> f64 {
    0.22 + f64::from(size.max(1)) * 0.05
}

/// Calculates sound volume for CubeMob based on size.
///
/// Mirrors vanilla `AbstractCubeMob.java:280`:
/// `0.4F * this.getSize()`
#[must_use]
pub fn calculate_cube_mob_sound_volume(size: i32) -> f32 {
    0.4 * size.max(1) as f32
}

/// Calculates general sound pitch for CubeMob (jump, ambient).
///
/// Mirrors vanilla `AbstractCubeMob.java:293-295`:
/// `((rand - rand) * 0.2F + 1.0F) * (isTiny ? 1.4F : 0.8F)`
#[must_use]
pub fn calculate_cube_mob_sound_pitch(is_tiny: bool, rand_diff: f32) -> f32 {
    let pitch_adjuster = if is_tiny { 1.4 } else { 0.8 };
    rand_diff.mul_add(0.2, 1.0) * pitch_adjuster
}

/// Calculates squish landing sound pitch for CubeMob.
///
/// Mirrors vanilla `AbstractCubeMob.java:139`:
/// `((rand - rand) * 0.2F + 1.0F) / 0.8F`
#[must_use]
pub fn calculate_cube_mob_squish_pitch(rand_diff: f32) -> f32 {
    rand_diff.mul_add(0.2, 1.0) / 0.8
}

/// Calculates attack sound pitch for CubeMob.
///
/// Mirrors vanilla `AbstractCubeMob.java:242`:
/// `(rand - rand) * 0.2F + 1.0F`
#[must_use]
pub fn calculate_cube_mob_attack_pitch(rand_diff: f32) -> f32 {
    rand_diff.mul_add(0.2, 1.0)
}

/// Determines whether damage to MagmaCube should be completely negated due to fire immunity and fall damage immunity.
///
/// Mirrors vanilla `EntityTypes.java:649` (`fireImmune()`), `MagmaCube.java:80`, and `EntityTypeTags.FALL_DAMAGE_IMMUNE`.
#[must_use]
pub fn is_magma_cube_damage_negated(damage_type: &pumpkin_data::damage::DamageType) -> bool {
    matches!(
        *damage_type,
        pumpkin_data::damage::DamageType::IN_FIRE
            | pumpkin_data::damage::DamageType::ON_FIRE
            | pumpkin_data::damage::DamageType::LAVA
            | pumpkin_data::damage::DamageType::HOT_FLOOR
            | pumpkin_data::damage::DamageType::CAMPFIRE
            | pumpkin_data::damage::DamageType::FALL
    )
}

// ============================================================================
// Ravager Parity Constants
// ============================================================================

/// Base max health for a Ravager (100.0 HP / 50 hearts).
/// Mirrors vanilla `Ravager.java:82` (`Attributes.MAX_HEALTH, 100.0`).
pub const RAVAGER_MAX_HEALTH: f32 = 100.0;

/// Base movement speed for a Ravager (0.3).
/// Mirrors vanilla `Ravager.java:83` (`Attributes.MOVEMENT_SPEED, 0.3`).
pub const RAVAGER_BASE_MOVEMENT_SPEED: f64 = 0.3;

/// Movement speed for a Ravager when pursuing a target (0.35).
/// Mirrors vanilla `Ravager.java:50,143` (`ATTACK_MOVEMENT_SPEED = 0.35`).
pub const RAVAGER_ATTACK_MOVEMENT_SPEED: f64 = 0.35;

/// Knockback resistance for a Ravager (0.75 = 75% resistance).
/// Mirrors vanilla `Ravager.java:84` (`Attributes.KNOCKBACK_RESISTANCE, 0.75`).
pub const RAVAGER_KNOCKBACK_RESISTANCE: f64 = 0.75;

/// Base melee attack damage for a Ravager (12.0 HP / 6 hearts).
/// Mirrors vanilla `Ravager.java:85` (`Attributes.ATTACK_DAMAGE, 12.0`).
pub const RAVAGER_ATTACK_DAMAGE: f32 = 12.0;

/// Attack knockback bonus for a Ravager (1.5).
/// Mirrors vanilla `Ravager.java:86` (`Attributes.ATTACK_KNOCKBACK, 1.5`).
pub const RAVAGER_ATTACK_KNOCKBACK: f64 = 1.5;

/// Follow range for a Ravager (32.0 blocks).
/// Mirrors vanilla `Ravager.java:87` (`Attributes.FOLLOW_RANGE, 32.0`).
pub const RAVAGER_FOLLOW_RANGE: f64 = 32.0;

/// Step height for a Ravager (1.0 block).
/// Mirrors vanilla `Ravager.java:88` (`Attributes.STEP_HEIGHT, 1.0`).
pub const RAVAGER_STEP_HEIGHT: f64 = 1.0;

/// Base experience reward dropped by a Ravager on death (20 XP).
/// Mirrors vanilla `Ravager.java:62` (`this.xpReward = 20`).
pub const RAVAGER_XP_REWARD: u32 = 20;

/// Attack animation duration in ticks (10 ticks / 0.5s).
/// Mirrors vanilla `Ravager.java:55` (`ATTACK_DURATION = 10`).
pub const RAVAGER_ATTACK_DURATION_TICKS: i32 = 10;

/// Stun duration in ticks when blocked by shield with 50% chance (40 ticks / 2s).
/// Mirrors vanilla `Ravager.java:56` (`STUN_DURATION = 40`).
pub const RAVAGER_STUN_DURATION_TICKS: i32 = 40;

/// Total roar countdown duration in ticks (20 ticks / 1s).
/// Mirrors vanilla `Ravager.java:166` (`this.roarTick = 20`).
pub const RAVAGER_ROAR_DURATION_TICKS: i32 = 20;

/// Tick at which roar attack wave bursts (10 ticks / 0.5s).
/// Mirrors vanilla `Ravager.java:157` (`if (this.roarTick == 10) { this.roar(); }`).
pub const RAVAGER_ROAR_BURST_TICK: i32 = 10;

/// Radius of Ravager roar attack box expansion (4.0 blocks).
/// Mirrors vanilla `Ravager.java:203` (`this.getBoundingBox().inflate(4.0)`).
pub const RAVAGER_ROAR_RADIUS: f64 = 4.0;

/// Roar mob attack damage dealt to non-illagers (6.0 HP / 3 hearts).
/// Mirrors vanilla `Ravager.java:205` (`entity.hurtServer(level, ..., 6.0F)`).
pub const RAVAGER_ROAR_DAMAGE: f32 = 6.0;

/// Probability of being stunned when blocked by shield (0.5 = 50%).
/// Mirrors vanilla `Ravager.java:186` (`if (this.random.nextDouble() < 0.5)`).
pub const RAVAGER_STUN_CHANCE: f64 = 0.5;

/// Calculates target movement speed for Ravager based on mobility state and target presence.
///
/// Mirrors vanilla `Ravager.java:139-145`:
/// - When immobile (stunned, attacking, roaring): `0.0`
/// - When targeting: `0.35`
/// - Default idle: `0.3`
#[must_use]
pub const fn calculate_ravager_target_speed(is_immobile: bool, has_target: bool) -> f64 {
    if is_immobile {
        0.0
    } else if has_target {
        0.35
    } else {
        0.3
    }
}

/// Calculates strong knockback impulse applied by Ravager.
///
/// Mirrors vanilla `Ravager.java:250-254`:
/// ```java
/// double xd = entity.getX() - this.getX();
/// double zd = entity.getZ() - this.getZ();
/// double dd = Math.max(xd * xd + zd * zd, 0.001);
/// entity.push(xd / dd * 4.0, 0.2, zd / dd * 4.0);
/// ```
#[must_use]
pub fn calculate_ravager_strong_knockback(
    ravager_pos_x: f64,
    ravager_pos_z: f64,
    target_pos_x: f64,
    target_pos_z: f64,
) -> (f64, f64, f64) {
    let xd = target_pos_x - ravager_pos_x;
    let zd = target_pos_z - ravager_pos_z;
    let dd = (xd.mul_add(xd, zd * zd)).max(0.001);
    let push_x = (xd / dd) * 4.0;
    let push_z = (zd / dd) * 4.0;
    (push_x, 0.2, push_z)
}

/// Determines whether an entity receives strong knockback from Ravager roar.
///
/// Mirrors vanilla `Ravager.java:233-235`:
/// `if (!(entity instanceof Player)) { this.strongKnockback(entity); }`
#[must_use]
pub const fn should_ravager_roar_knockback_target(is_player: bool) -> bool {
    !is_player
}

/// Checks whether an entity type is immune to Ravager roar damage (Illagers).
///
/// Mirrors vanilla `Ravager.java:229-231`:
/// `if (!(entity instanceof AbstractIllager)) { entity.hurtServer(level, ..., 6.0F); }`
#[must_use]
pub fn is_illager_immune_to_ravager_roar(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    entity_type == &pumpkin_data::entity::EntityType::PILLAGER
        || entity_type == &pumpkin_data::entity::EntityType::EVOKER
        || entity_type == &pumpkin_data::entity::EntityType::VINDICATOR
        || entity_type == &pumpkin_data::entity::EntityType::ILLUSIONER
        || entity_type == &pumpkin_data::entity::EntityType::WITCH
        || entity_type == &pumpkin_data::entity::EntityType::RAVAGER
}

/// Calculates base experience reward dropped by a Ravager (20 XP).
///
/// Mirrors vanilla `Ravager.java:70`:
/// `this.xpReward = 20;`
#[must_use]
pub const fn get_ravager_xp_reward() -> u32 {
    20
}

/// Determines whether a Vex is currently flapping its wings.
///
/// Mirrors vanilla `Vex.java:49, 64-66`:
/// `public static final int TICKS_PER_FLAP = Mth.ceil((float) (Math.PI * 5.0 / 4.0)); // 4`
#[must_use]
pub const fn is_vex_flapping(tick_count: u32) -> bool {
    tick_count % 4 == 0
}

/// Calculates base experience reward dropped by a Vex (3 XP).
///
/// Mirrors vanilla `Vex.java:61`:
/// `this.xpReward = 3;`
#[must_use]
pub const fn get_vex_xp_reward() -> u32 {
    3
}

/// Determines whether a Zoglin can target an entity.
///
/// Mirrors vanilla `Zoglin.java:139`:
/// `!target.is(EntityTypes.ZOGLIN) && !target.is(EntityTypes.CREEPER)`
#[must_use]
pub fn can_zoglin_target_entity(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    entity_type != &pumpkin_data::entity::EntityType::ZOGLIN
        && entity_type != &pumpkin_data::entity::EntityType::CREEPER
}

/// Calculates Zoglin attack damage based on baby status.
///
/// Mirrors vanilla `Zoglin.java:69-70, 252`:
/// - Adult: 6.0
/// - Baby: 0.5
#[must_use]
pub const fn calculate_zoglin_attack_damage(is_baby: bool) -> f64 {
    if is_baby {
        0.5
    } else {
        6.0
    }
}

/// Calculates sound pitch for Zoglin ambient/angry sound.
///
/// Mirrors vanilla `Zoglin.java:203-207`:
/// - Baby: `rand_diff * 0.2 + 1.5`
/// - Adult: `rand_diff * 0.2 + 1.0`
#[must_use]
pub fn calculate_zoglin_sound_pitch(is_baby: bool, rand_diff: f32) -> f32 {
    if is_baby {
        rand_diff.mul_add(0.2, 1.5)
    } else {
        rand_diff.mul_add(0.2, 1.0)
    }
}

/// Checks whether an entity type is immune to Poison status effect.
///
/// Spider max health (16.0 HP).
///
/// Mirrors vanilla `Spider.java:90`:
/// `return Monster.createMonsterAttributes().add(Attributes.MAX_HEALTH, 16.0).add(Attributes.MOVEMENT_SPEED, 0.3F);`
pub const SPIDER_MAX_HEALTH: f32 = 16.0;

/// Spider base attack damage (2.0 HP).
///
/// Mirrors vanilla `Monster.java` / `Spider.java`:
pub const SPIDER_BASE_ATTACK_DAMAGE: f32 = 2.0;

/// Spider base movement speed (0.3).
///
/// Mirrors vanilla `Spider.java:90`:
pub const SPIDER_BASE_MOVEMENT_SPEED: f64 = 0.3;

/// Spider experience reward (5 XP).
///
/// Mirrors vanilla `Spider.java` / `Monster.java`:
pub const SPIDER_XP_REWARD: u32 = 5;

/// Spider leap upward/forward velocity power (0.4).
///
/// Mirrors vanilla `Spider.java:60`:
/// `this.goalSelector.addGoal(3, new LeapAtTargetGoal(this, 0.4F));`
pub const SPIDER_LEAP_VELOCITY: f32 = 0.4;

/// Cave Spider max health (12.0 HP).
///
/// Mirrors vanilla `CaveSpider.java:26`:
/// `return Spider.createAttributes().add(Attributes.MAX_HEALTH, 12.0);`
pub const CAVE_SPIDER_MAX_HEALTH: f32 = 12.0;

/// Cave Spider base attack damage (2.0 HP).
///
/// Mirrors vanilla `Spider.java` / `CaveSpider.java`:
pub const CAVE_SPIDER_BASE_ATTACK_DAMAGE: f32 = 2.0;

/// Cave Spider base movement speed (0.3).
///
/// Mirrors vanilla `Spider.java` / `CaveSpider.java`:
pub const CAVE_SPIDER_BASE_MOVEMENT_SPEED: f64 = 0.3;

/// Cave Spider experience reward (5 XP).
///
/// Mirrors vanilla `CaveSpider.java` / `Monster.java`:
pub const CAVE_SPIDER_XP_REWARD: u32 = 5;

/// Duration in ticks of the Poison effect applied by Cave Spider on Normal difficulty (140 ticks = 7 seconds).
///
/// Mirrors vanilla `CaveSpider.java:35, 41`:
/// `poisonTime = 7; livingEntity.addEffect(new MobEffectInstance(MobEffects.POISON, poisonTime * 20, 0), this);`
pub const CAVE_SPIDER_POISON_NORMAL_TICKS: i32 = 140;

/// Duration in ticks of the Poison effect applied by Cave Spider on Hard difficulty (300 ticks = 15 seconds).
///
/// Mirrors vanilla `CaveSpider.java:37, 41`:
/// `poisonTime = 15; livingEntity.addEffect(new MobEffectInstance(MobEffects.POISON, poisonTime * 20, 0), this);`
pub const CAVE_SPIDER_POISON_HARD_TICKS: i32 = 300;

/// Calculates poison effect duration in ticks applied by Cave Spider based on world difficulty.
///
/// Mirrors vanilla `CaveSpider.java:33-40`:
/// - Easy or Peaceful: 0 ticks (no poison)
/// - Normal: 7 * 20 = 140 ticks
/// - Hard: 15 * 20 = 300 ticks
#[must_use]
pub const fn calculate_cave_spider_poison_duration(difficulty: pumpkin_util::Difficulty) -> i32 {
    match difficulty {
        pumpkin_util::Difficulty::Normal => CAVE_SPIDER_POISON_NORMAL_TICKS,
        pumpkin_util::Difficulty::Hard => CAVE_SPIDER_POISON_HARD_TICKS,
        _ => 0,
    }
}

/// Mirrors vanilla `Spider.java:126-128`, `CaveSpider.java`, `AbstractNautilus.java:462-464`:
/// Spiders, Cave Spiders, Nautiluses, and Zombie Nautiluses are completely immune to Poison.
#[must_use]
pub fn is_spider_or_nautilus_immune_to_poison(
    entity_type: &pumpkin_data::entity::EntityType,
    effect_type: &pumpkin_data::effect::StatusEffect,
) -> bool {
    (entity_type == &pumpkin_data::entity::EntityType::SPIDER
        || entity_type == &pumpkin_data::entity::EntityType::CAVE_SPIDER
        || entity_type == &pumpkin_data::entity::EntityType::NAUTILUS
        || entity_type == &pumpkin_data::entity::EntityType::ZOMBIE_NAUTILUS)
        && effect_type == &pumpkin_data::effect::StatusEffect::POISON
}

/// Checks whether Cave Spider can spawn skeleton jockeys or random hard difficulty effects.
///
/// Mirrors vanilla `CaveSpider.java:52`:
/// Unlike standard spiders, Cave Spiders NEVER spawn as skeleton jockeys and NEVER receive
/// random potion effects on hard difficulty.
#[must_use]
pub const fn should_cave_spider_spawn_jockey_or_effects() -> bool {
    false
}

/// Evaluates whether a dolphin can initiate swimming with a player.
///
/// Mirrors vanilla `Dolphin.java:482-484`:
/// - Player must be swimming
/// - Dolphin must not currently target the player
/// - Player must be within targeting range (distance sqr <= 100.0)
#[must_use]
pub fn should_dolphin_swim_with_player(
    is_player_swimming: bool,
    distance_sqr: f64,
    is_target: bool,
) -> bool {
    is_player_swimming && !is_target && distance_sqr <= 100.0
}

/// Evaluates whether a dolphin can continue swimming with a player.
///
/// Mirrors vanilla `Dolphin.java:487-489`:
/// - Player must still be swimming
/// - Distance sqr must be < 256.0 (16 blocks)
#[must_use]
pub fn should_dolphin_continue_swimming_with_player(
    is_player_swimming: bool,
    distance_sqr: f64,
) -> bool {
    is_player_swimming && distance_sqr < 256.0
}

/// Calculates duration in ticks for Dolphin's Grace effect.
///
/// Mirrors vanilla `Dolphin.java:493, 512`: 100 ticks (5 seconds).
#[must_use]
pub const fn calculate_dolphin_grace_duration() -> i32 {
    100
}

/// Calculates flee motion vector for Squid and GlowSquid when hurt by an attacker.
///
/// Mirrors vanilla `Squid.java:267-290`:
/// ```java
/// Vec3 fleeTo = new Vec3(Squid.this.getX() - lastHurtByMob.getX(), Squid.this.getY() - lastHurtByMob.getY(), Squid.this.getZ() - lastHurtByMob.getZ());
/// double length = fleeTo.length();
/// if (length > 0.0) {
///    fleeTo.normalize();
///    double avoidSpeed = 3.0;
///    if (length > 5.0) {
///       avoidSpeed -= (length - 5.0) / 5.0;
///    }
///    if (avoidSpeed > 0.0) {
///       fleeTo = fleeTo.scale(avoidSpeed);
///    }
/// }
/// if (blockState.isAir()) {
///    fleeTo = fleeTo.subtract(0.0, fleeTo.y, 0.0);
/// }
/// Squid.this.movementVector = new Vec3(fleeTo.x / 20.0, fleeTo.y / 20.0, fleeTo.z / 20.0);
/// ```
#[must_use]
pub fn calculate_squid_flee_movement(
    squid_pos: Vector3<f64>,
    attacker_pos: Vector3<f64>,
    is_target_air: bool,
) -> Vector3<f64> {
    let mut flee_to = squid_pos - attacker_pos;
    let length = (flee_to.x * flee_to.x + flee_to.y * flee_to.y + flee_to.z * flee_to.z).sqrt();
    if length > 0.0 {
        flee_to = flee_to * (1.0 / length);
        let mut avoid_speed = 3.0;
        if length > 5.0 {
            avoid_speed -= (length - 5.0) / 5.0;
        }
        if avoid_speed > 0.0 {
            flee_to = flee_to * avoid_speed;
        }
    }
    if is_target_air {
        flee_to.y = 0.0;
    }
    Vector3::new(flee_to.x / 20.0, flee_to.y / 20.0, flee_to.z / 20.0)
}

/// Evaluates whether a Squid or GlowSquid should flee from an attacker.
///
/// Mirrors vanilla `Squid.java:249`:
/// `Squid.this.isInWater() && entity != null ? Squid.this.distanceToSqr(entity) < 100.0 : false;`
#[must_use]
pub fn should_squid_flee(is_in_water: bool, has_attacker: bool, distance_sqr: f64) -> bool {
    is_in_water && has_attacker && distance_sqr < 100.0
}

/// Calculates ink offset scale for Squid ink particles.
///
/// Mirrors vanilla `Squid.java:199-200`:
/// `float inkPosOffsetScale = this.isBaby() ? 0.1F : 0.3F;`
/// `Vec3 dirOffset = dir.scale(inkPosOffsetScale + this.random.nextFloat() * 2.0F);`
#[must_use]
pub fn calculate_squid_ink_offset_scale(is_baby: bool, rand_factor: f32) -> f32 {
    let base = if is_baby { 0.1 } else { 0.3 };
    rand_factor.mul_add(2.0, base)
}

/// Rotates a 3D vector by entity pitch and yaw.
///
/// Mirrors vanilla `Squid.java:188-191`:
/// `Vec3 v = vec.xRot(this.xBodyRotO * (float) (Math.PI / 180.0)); return v.yRot(-this.yBodyRotO * (float) (Math.PI / 180.0));`
#[must_use]
pub fn rotate_squid_vector(vec: Vector3<f64>, pitch_deg: f32, yaw_deg: f32) -> Vector3<f64> {
    let pitch_rad = (pitch_deg as f64).to_radians();
    let yaw_rad = (yaw_deg as f64).to_radians();

    let cos_p = pitch_rad.cos();
    let sin_p = pitch_rad.sin();
    let y1 = vec.y * cos_p - vec.z * sin_p;
    let z1 = vec.y * sin_p + vec.z * cos_p;
    let x1 = vec.x;

    let cos_y = (-yaw_rad).cos();
    let sin_y = (-yaw_rad).sin();
    let x2 = x1 * cos_y + z1 * sin_y;
    let z2 = -x1 * sin_y + z1 * cos_y;
    let y2 = y1;

    Vector3::new(x2, y2, z2)
}

/// Calculates base experience reward dropped by Squid / GlowSquid.
///
/// Mirrors vanilla `AgeableWaterCreature.java:33-35`:
/// `return 1 + this.random.nextInt(3);`
#[must_use]
pub fn get_squid_xp_reward(rand_0_to_2: i32) -> u32 {
    (1 + rand_0_to_2).max(1) as u32
}

/// Determines whether an entity type can be leashed with a lead.
///
/// Mirrors vanilla `Mob.java:1288`, `AgeableWaterCreature.java:62`, `AmbientCreature.java:13`,
/// `Squid.java:88`, `Dolphin.java:376`, `Axolotl.java:369`, `Turtle.java:279`, `Zoglin.java:193`:
#[must_use]
pub fn is_mob_leashable(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    use pumpkin_data::entity::EntityType;

    if entity_type == &EntityType::ZOGLIN {
        return true;
    }
    if entity_type == &EntityType::SQUID
        || entity_type == &EntityType::GLOW_SQUID
        || entity_type == &EntityType::DOLPHIN
        || entity_type == &EntityType::AXOLOTL
    {
        return true;
    }

    if entity_type == &EntityType::TURTLE
        || entity_type == &EntityType::COD
        || entity_type == &EntityType::SALMON
        || entity_type == &EntityType::PUFFERFISH
        || entity_type == &EntityType::TROPICAL_FISH
        || entity_type == &EntityType::TADPOLE
    {
        return false;
    }

    if entity_type == &EntityType::BAT
        || entity_type == &EntityType::VILLAGER
        || entity_type == &EntityType::WANDERING_TRADER
        || entity_type == &EntityType::PANDA
    {
        return false;
    }

    entity_type.category != &pumpkin_data::entity::MobCategory::MONSTER
}

/// Determines whether a Frog can eat the given entity.
///
/// Mirrors vanilla `Frog.java:315-317`:
/// ```java
/// public static boolean canEat(final LivingEntity entity) {
///    return entity instanceof AbstractCubeMob cubeMob && cubeMob.getSize() != 1 ? false : entity.is(EntityTypeTags.FROG_FOOD);
/// }
/// ```
#[must_use]
pub fn can_frog_eat_entity(entity_type: &pumpkin_data::entity::EntityType, cube_size: i32) -> bool {
    use pumpkin_data::entity::EntityType;

    if entity_type == &EntityType::SLIME || entity_type == &EntityType::MAGMA_CUBE {
        cube_size <= 1
    } else {
        false
    }
}

/// Determines the drop item produced when a Frog swallows a victim entity.
///
/// Mirrors vanilla loot tables `slime.json` and `magma_cube.json`:
/// - Slime (size 1) killed by Frog drops 1 Slimeball (`SLIME_BALL`).
/// - Magma Cube (size 1) killed by Frog drops 1 Froglight corresponding to the Frog's variant:
///   - Temperate -> Ochre Froglight (`OCHRE_FROGLIGHT`)
///   - Warm -> Pearlescent Froglight (`PEARLESCENT_FROGLIGHT`)
///   - Cold -> Verdant Froglight (`VERDANT_FROGLIGHT`)
#[must_use]
pub fn get_frog_drop_item(
    victim_type: &pumpkin_data::entity::EntityType,
    variant: pumpkin_data::frog_variant::FrogVariant,
) -> Option<&'static pumpkin_data::item::Item> {
    use pumpkin_data::entity::EntityType;
    use pumpkin_data::frog_variant::FrogVariant;
    use pumpkin_data::item::Item;

    if victim_type == &EntityType::SLIME {
        Some(&Item::SLIME_BALL)
    } else if victim_type == &EntityType::MAGMA_CUBE {
        match variant {
            FrogVariant::Temperate => Some(&Item::OCHRE_FROGLIGHT),
            FrogVariant::Warm => Some(&Item::PEARLESCENT_FROGLIGHT),
            FrogVariant::Cold => Some(&Item::VERDANT_FROGLIGHT),
        }
    } else {
        None
    }
}

/// Calculates the pull velocity applied to the victim when a Frog uses its tongue.
///
/// Mirrors vanilla `ShootTongue.java:107`:
/// `target.setDeltaMovement(target.position().vectorTo(body.position()).normalize().scale(0.75));`
#[must_use]
pub fn calculate_frog_tongue_pull_velocity(
    target_pos: Vector3<f64>,
    frog_pos: Vector3<f64>,
) -> Vector3<f64> {
    let diff = frog_pos - target_pos;
    let len = (diff.x * diff.x + diff.y * diff.y + diff.z * diff.z).sqrt();
    if len > 0.0 {
        diff * (0.75 / len)
    } else {
        Vector3::new(0.0, 0.0, 0.0)
    }
}

/// Determines whether a Frog is close enough to its target to initiate eating.
///
/// Mirrors vanilla `ShootTongue.java:25, 104`:
/// `private static final float EATING_DISTANCE = 1.75F;`
/// `if (target.distanceTo(body) < 1.75F) { ... }`
#[must_use]
pub fn should_frog_start_eating(distance: f64) -> bool {
    distance < 1.75
}

/// Calculates fall damage points for a Frog with the 5-point vanilla reduction.
///
/// Mirrors vanilla `Frog.java:304-306`:
/// ```java
/// @Override
/// protected int calculateFallDamage(final double fallDistance, final float damageModifier) {
///    return super.calculateFallDamage(fallDistance, damageModifier) - 5;
/// }
/// ```
#[must_use]
pub fn calculate_frog_fall_damage_points(fall_distance: f64, damage_modifier: f32) -> i32 {
    let base_damage = ((fall_distance - 3.0).max(0.0) * damage_modifier as f64).ceil() as i32;
    (base_damage - 5).max(0)
}

/// Determines the Frog variant based on biome tags when a Tadpole matures into a Frog.
///
/// Mirrors vanilla `FrogVariants.java:26-29`:
/// - `#minecraft:spawns_warm_variant_frogs` -> `FrogVariant::Warm`
/// - `#minecraft:spawns_cold_variant_frogs` -> `FrogVariant::Cold`
/// - Fallback -> `FrogVariant::Temperate`
#[must_use]
pub fn determine_frog_variant_from_biome_tags(
    is_warm: bool,
    is_cold: bool,
) -> pumpkin_data::frog_variant::FrogVariant {
    use pumpkin_data::frog_variant::FrogVariant;

    if is_warm {
        FrogVariant::Warm
    } else if is_cold {
        FrogVariant::Cold
    } else {
        FrogVariant::Temperate
    }
}

/// Calculates fall damage points for a Goat with the 10-point vanilla reduction.
///
/// Mirrors vanilla `Goat.java:79, 124-126`:
/// ```java
/// @Override
/// protected int calculateFallDamage(final double fallDistance, final float damageModifier) {
///    return super.calculateFallDamage(fallDistance, damageModifier) - 10;
/// }
/// ```
#[must_use]
pub fn calculate_goat_fall_damage_points(fall_distance: f64, damage_modifier: f32) -> i32 {
    let base_damage = ((fall_distance - 3.0).max(0.0) * damage_modifier as f64).ceil() as i32;
    (base_damage - 10).max(0)
}

/// Evaluates whether a freshly spawned goat should be a Screaming Goat (2% chance).
///
/// Mirrors vanilla `Goat.java:80, 241`:
/// `public static final double GOAT_SCREAMING_CHANCE = 0.02;`
/// `this.setScreamingGoat(random.nextDouble() < 0.02);`
#[must_use]
pub fn should_spawn_screaming_goat(rand_f64: f64) -> bool {
    rand_f64 < 0.02
}

/// Evaluates whether a baby goat bred from parents inherits the screaming trait.
///
/// Mirrors vanilla `Goat.java:156-158`:
/// `boolean babyIsScreaming = (level.getRandom().nextBoolean() ? this : partner) instanceof Goat goat && goat.isScreamingGoat() || level.getRandom().nextDouble() < 0.02;`
#[must_use]
pub fn should_spawn_screaming_goat_from_parents(
    parent_a_screaming: bool,
    parent_b_screaming: bool,
    rand_bool: bool,
    rand_f64: f64,
) -> bool {
    (if rand_bool {
        parent_a_screaming
    } else {
        parent_b_screaming
    }) || rand_f64 < 0.02
}

/// Evaluates whether an adult goat spawns with only one horn (10% chance).
///
/// Mirrors vanilla `Goat.java:81, 243`:
/// `public static final double UNIHORN_CHANCE = 0.1F;`
/// `if (!this.isBaby() && random.nextFloat() < 0.1F) { ... }`
#[must_use]
pub fn should_goat_spawn_unihorn(is_baby: bool, rand_f32: f32) -> bool {
    !is_baby && rand_f32 < 0.1
}

/// Selects which goat horn to drop upon ramming a solid snap block.
///
/// Returns `Some(true)` to drop the left horn, `Some(false)` to drop the right horn,
/// or `None` if the goat has no remaining horns.
///
/// Mirrors vanilla `Goat.java:312-332`.
#[must_use]
pub fn select_goat_horn_to_drop(
    has_left: bool,
    has_right: bool,
    rand_bool: bool,
) -> Option<bool> {
    if !has_left && !has_right {
        None
    } else if !has_left {
        Some(false) // Only right horn remains, drop right
    } else if !has_right {
        Some(true) // Only left horn remains, drop left
    } else {
        Some(rand_bool) // Both remain, randomly drop left or right
    }
}

/// Calculates fall damage points for an Allay.
///
/// Mirrors vanilla `Allay.java:179-180`: Allays are flying mobs that are completely
/// immune to fall damage.
#[must_use]
pub const fn calculate_allay_fall_damage_points() -> i32 {
    0
}

/// Checks whether an item can be used to duplicate an Allay.
///
/// Mirrors vanilla `Allay.java:285`:
/// `interactionItem.is(ItemTags.DUPLICATES_ALLAYS)`
#[must_use]
pub fn is_allay_duplication_item(item: &pumpkin_data::item::Item) -> bool {
    use pumpkin_data::tag::Taggable;
    item.has_tag(&pumpkin_data::tag::Item::MINECRAFT_DUPLICATES_ALLAYS)
        || item == &pumpkin_data::item::Item::AMETHYST_SHARD
}

/// Selects the baby Axolotl variant when breeding two parents.
///
/// Mirrors vanilla `Axolotl.java:313-315, 346-361`:
/// - 1/1200 chance of rare Blue variant (`AxolotlVariant::Blue`).
/// - Otherwise, 50% chance of inheriting variant from parent A, 50% from parent B.
#[must_use]
pub fn select_axolotl_baby_variant(
    parent_a: crate::entity::passive::axolotl::AxolotlVariant,
    parent_b: crate::entity::passive::axolotl::AxolotlVariant,
    rand_1200: i32,
    rand_parent_bool: bool,
) -> crate::entity::passive::axolotl::AxolotlVariant {
    use crate::entity::passive::axolotl::AxolotlVariant;
    if rand_1200 == 0 {
        AxolotlVariant::Blue
    } else if rand_parent_bool {
        parent_a
    } else {
        parent_b
    }
}

/// Calculates the updated duration of the Regeneration buff granted to a player by an Axolotl.
///
/// Mirrors vanilla `Axolotl.java:490-499`:
/// Adds 100 ticks (5s) to previous duration, capped at 2400 ticks (2 minutes).
#[must_use]
pub fn calculate_axolotl_player_regen_duration(current_duration_ticks: Option<i32>) -> i32 {
    let previous = current_duration_ticks.unwrap_or(0);
    (100 + previous).min(2400)
}

/// Determines whether an Axolotl can be seen or targeted as an enemy by hostile mobs.
///
/// Mirrors vanilla `Axolotl.java:474-476`:
/// `return !this.isPlayingDead() && super.canBeSeenAsEnemy();`
#[must_use]
pub const fn can_axolotl_be_targeted(is_playing_dead: bool) -> bool {
    !is_playing_dead
}

/// Calculates fall damage points for a Bee.
///
/// Mirrors vanilla `Bee.java:610-611`: Bees are flying mobs that are completely
/// immune to fall damage.
#[must_use]
pub const fn calculate_bee_fall_damage_points() -> i32 {
    0
}

/// Calculates drowning damage for a Bee submerged in water.
///
/// Mirrors vanilla `Bee.java:365-373`:
/// Bees take 1.0 drowning damage each tick once submerged for more than 20 ticks (1s).
#[must_use]
pub const fn calculate_bee_underwater_damage(underwater_ticks: i32) -> f32 {
    if underwater_ticks > 20 {
        1.0
    } else {
        0.0
    }
}

/// Calculates the velocity impulse of a Camel dash.
///
/// Mirrors vanilla `Camel.java:305-317`:
/// - Horizontal impulse factor: `22.2222 * jump_amount * movement_speed * block_speed_factor`
/// - Vertical impulse factor: `1.4285 * jump_amount * jump_power`
#[must_use]
pub fn calculate_camel_dash_velocity(
    yaw_rad: f32,
    jump_amount: f32,
    movement_speed: f64,
    jump_power: f64,
    block_speed_factor: f64,
) -> pumpkin_util::math::vector3::Vector3<f64> {
    use pumpkin_util::math::vector3::Vector3;

    let forward_x = -yaw_rad.sin() as f64;
    let forward_z = yaw_rad.cos() as f64;
    let horizontal_scale = 22.2222 * jump_amount as f64 * movement_speed * block_speed_factor;
    let vertical_scale = 1.4285 * jump_amount as f64 * jump_power;

    Vector3::new(
        forward_x * horizontal_scale,
        vertical_scale,
        forward_z * horizontal_scale,
    )
}

/// Determines whether a Camel can accept another rider/passenger.
///
/// Mirrors vanilla `Camel.java:565-567`:
/// Camels have a maximum capacity of 2 passengers.
#[must_use]
pub const fn can_camel_accept_passenger(current_passenger_count: usize) -> bool {
    current_passenger_count < 2
}

/// Calculates the random Panda gene based on 1/16 distribution.
///
/// Mirrors vanilla `Panda.java:Gene.getRandom`:
/// - 0: Lazy (1)
/// - 1: Worried (2)
/// - 2: Playful (3)
/// - 4: Brown (4)
/// - 5: Weak (5)
/// - 6: Aggressive (6)
/// - other (3, 7..15): Normal (0)
#[must_use]
pub const fn calculate_panda_random_gene(rand_16: u32) -> u8 {
    match rand_16 % 16 {
        0 => 1, // Lazy
        1 => 2, // Worried
        2 => 3, // Playful
        4 => 4, // Brown
        5 => 5, // Weak
        6 => 6, // Aggressive
        _ => 0, // Normal
    }
}

/// Selects baby panda main and hidden genes from its parents,
/// including parental allele combination and 1/32 mutation chances.
///
/// Mirrors vanilla `Panda.java:231-255` (`setGeneFromParents`):
/// - Parent alleles are randomly chosen (main or hidden) with 50/50 probability.
/// - When 2 parents exist, baby receives one allele from parent 1 and one from parent 2.
/// - 1/32 chance each for main gene and hidden gene to mutate into a random gene.
#[must_use]
pub fn select_panda_baby_genes(
    parent1_main: u8,
    parent1_hidden: u8,
    parent2_main: u8,
    parent2_hidden: u8,
    rand_swap_parents: bool,
    rand_parent1_allele: bool,
    rand_parent2_allele: bool,
    rand_mutation_main: u32,
    rand_mutation_hidden: u32,
    mutation_gene_main: u8,
    mutation_gene_hidden: u8,
) -> (u8, u8) {
    let p1_allele = if rand_parent1_allele {
        parent1_main
    } else {
        parent1_hidden
    };
    let p2_allele = if rand_parent2_allele {
        parent2_main
    } else {
        parent2_hidden
    };

    let (mut main, mut hidden) = if rand_swap_parents {
        (p2_allele, p1_allele)
    } else {
        (p1_allele, p2_allele)
    };

    if rand_mutation_main % 32 == 0 {
        main = mutation_gene_main;
    }
    if rand_mutation_hidden % 32 == 0 {
        hidden = mutation_gene_hidden;
    }

    (main, hidden)
}

/// Determines whether a baby Panda should sneeze this tick.
///
/// Mirrors vanilla `Panda.java:PandaSneezeGoal.canUse`:
/// Weak baby pandas sneeze every ~500 ticks, normal baby pandas every ~6000 ticks.
#[must_use]
pub const fn should_panda_sneeze(is_baby: bool, is_weak: bool, rand_val: u32) -> bool {
    if !is_baby {
        return false;
    }
    if is_weak {
        rand_val % 500 == 0
    } else {
        rand_val % 6000 == 0
    }
}

/// Determines whether a sneezing Panda drops a Slime Ball.
///
/// Mirrors vanilla `panda_sneeze.json` gift loot table:
/// Slime ball has weight 1 out of 700 total entries (1/700 = ~0.143% chance).
#[must_use]
pub const fn should_panda_drop_slime_ball(rand_700: u32) -> bool {
    rand_700 % 700 == 0
}

/// Determines whether a Polar Bear should rear up and play its warning roar before an attack.
///
/// Mirrors vanilla `PolarBear.java:315-325` (`PolarBearMeleeAttackGoal`):
/// When the target is within `(target_width + 3.0)^2` distance and attack cooldown
/// ticks remaining is <= 10.
#[must_use]
pub fn should_polar_bear_stand_and_warn(
    target_distance_sq: f64,
    target_width: f32,
    ticks_until_attack: i32,
) -> bool {
    let reach = (target_width as f64 + 3.0).powi(2);
    target_distance_sq < reach && ticks_until_attack <= 10
}

/// Mirrors vanilla `fall_damage_immune.json`: Cats and Ocelots are completely
/// immune to fall damage.
#[must_use]
pub const fn calculate_feline_fall_damage_points() -> i32 {
    0
}

/// Selects a kitten's variant and collar color from its parents.
///
/// Mirrors vanilla `Cat.java:235-245` (`getBreedOffspring`):
/// - 50% chance to inherit variant from parent 1, 50% from parent 2.
/// - If parents are tamed, kitten inherits collar color (50% from parent 1, 50% from parent 2).
#[must_use]
pub const fn select_cat_baby_traits(
    parent1_variant: u8,
    parent2_variant: u8,
    parent1_collar: u8,
    parent2_collar: u8,
    rand_variant: bool,
    rand_collar: bool,
) -> (u8, u8) {
    let variant = if rand_variant {
        parent1_variant
    } else {
        parent2_variant
    };
    let collar = if rand_collar {
        parent1_collar
    } else {
        parent2_collar
    };
    (variant, collar)
}

/// Selects an item from the Cat morning gift loot table.
///
/// Mirrors vanilla `gameplay/cat_morning_gift.json`:
/// Total weight = 62:
/// - 10: Rabbit Hide
/// - 10: Rabbit Foot
/// - 10: Raw Chicken
/// - 10: Feather
/// - 10: Rotten Flesh
/// - 10: String
/// - 2: Phantom Membrane (rare gift, 2/62 = ~3.22%)
#[must_use]
pub fn select_cat_morning_gift(rand_62: u32) -> &'static pumpkin_data::item::Item {
    use pumpkin_data::item::Item;
    match rand_62 % 62 {
        0..10 => &Item::RABBIT_HIDE,
        10..20 => &Item::RABBIT_FOOT,
        20..30 => &Item::CHICKEN,
        30..40 => &Item::FEATHER,
        40..50 => &Item::ROTTEN_FLESH,
        50..60 => &Item::STRING,
        _ => &Item::PHANTOM_MEMBRANE,
    }
}

/// Determines whether a cat sleeping on a bed should deliver a morning gift.
///
/// Mirrors vanilla `Cat.java:500-505`:
/// Owner must have slept for at least 100 ticks and random roll must succeed
/// (default chance 0.7 = 70%).
#[must_use]
pub fn should_cat_give_morning_gift(sleep_timer: i32, rand_float: f32, gift_chance: f32) -> bool {
    sleep_timer >= 100 && rand_float < gift_chance
}

/// Determines whether an untrusted Ocelot should despawn when far away from players.
///
/// Mirrors vanilla `Ocelot.java:138-140`:
/// Untrusted ocelots despawn after 2400 ticks (2 minutes) when far away.
#[must_use]
pub const fn should_ocelot_despawn_when_far(is_trusting: bool, age_ticks: i32) -> bool {
    !is_trusting && age_ticks > 2400
}

/// Calculates the healing amount for a tamed wolf when fed food.
///
/// Mirrors vanilla `Wolf.java:mobInteract`:
/// `heal(2.0F * (foodProperties != null ? foodProperties.nutrition() : 1.0F))`
#[must_use]
pub fn calculate_wolf_food_healing(nutrition: Option<u32>) -> f32 {
    let nutrition = nutrition.unwrap_or(1) as f32;
    2.0 * nutrition
}

/// Selects a baby wolf's variant and collar color from its parents.
///
/// Mirrors vanilla `Wolf.java:getBreedOffspring`:
/// 50% chance to inherit variant and collar color from parent 1, 50% from parent 2.
#[must_use]
pub const fn select_wolf_baby_traits(
    parent1_variant: u8,
    parent2_variant: u8,
    parent1_collar: u8,
    parent2_collar: u8,
    rand_variant: bool,
    rand_collar: bool,
) -> (u8, u8) {
    let variant = if rand_variant {
        parent1_variant
    } else {
        parent2_variant
    };
    let collar = if rand_collar {
        parent1_collar
    } else {
        parent2_collar
    };
    (variant, collar)
}

/// Selects a baby fox's variant from its parents.
///
/// Mirrors vanilla `Fox.java:getBreedOffspring`:
/// 50% chance to inherit variant from parent 1, 50% from parent 2.
#[must_use]
pub const fn select_fox_baby_variant(
    parent1_variant: i32,
    parent2_variant: i32,
    rand_bool: bool,
) -> i32 {
    if rand_bool {
        parent1_variant
    } else {
        parent2_variant
    }
}

/// Calculates the safe fall distance for a Fox.
///
/// Mirrors vanilla `Fox.java:createAttributes`:
/// Foxes have a base `SAFE_FALL_DISTANCE` of 5.0 blocks (standard is 3.0 blocks).
#[must_use]
pub const fn get_fox_safe_fall_distance() -> f64 {
    5.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquineFoodProperties {
    pub heal: f32,
    pub age_up_seconds: i32,
    pub temper_increase: i32,
    pub can_breed: bool,
}

/// Returns the feeding properties of an item when given to an equine (Horse, Donkey, Mule, etc.).
///
/// Mirrors vanilla `AbstractHorse.java:handleEating`:
/// - `Wheat`: heal 2.0, ageUp 20s, temper +3
/// - `Sugar`: heal 1.0, ageUp 30s, temper +3
/// - `Hay Block`: heal 20.0, ageUp 180s, temper 0
/// - `Apple`: heal 3.0, ageUp 60s, temper +3
/// - `Carrot`: heal 3.0, ageUp 60s, temper +3
/// - `Golden Carrot`: heal 4.0, ageUp 60s, temper +5, can_breed: true
/// - `Golden Apple` & `Enchanted Golden Apple`: heal 10.0, ageUp 240s, temper +10, can_breed: true
#[must_use]
pub fn get_equine_food_properties(item_id: u16) -> Option<EquineFoodProperties> {
    use pumpkin_data::item::Item;
    if item_id == Item::WHEAT.id {
        Some(EquineFoodProperties {
            heal: 2.0,
            age_up_seconds: 20,
            temper_increase: 3,
            can_breed: false,
        })
    } else if item_id == Item::SUGAR.id {
        Some(EquineFoodProperties {
            heal: 1.0,
            age_up_seconds: 30,
            temper_increase: 3,
            can_breed: false,
        })
    } else if item_id == Item::HAY_BLOCK.id {
        Some(EquineFoodProperties {
            heal: 20.0,
            age_up_seconds: 180,
            temper_increase: 0,
            can_breed: false,
        })
    } else if item_id == Item::APPLE.id || item_id == Item::CARROT.id {
        Some(EquineFoodProperties {
            heal: 3.0,
            age_up_seconds: 60,
            temper_increase: 3,
            can_breed: false,
        })
    } else if item_id == Item::GOLDEN_CARROT.id {
        Some(EquineFoodProperties {
            heal: 4.0,
            age_up_seconds: 60,
            temper_increase: 5,
            can_breed: true,
        })
    } else if item_id == Item::GOLDEN_APPLE.id || item_id == Item::ENCHANTED_GOLDEN_APPLE.id {
        Some(EquineFoodProperties {
            heal: 10.0,
            age_up_seconds: 240,
            temper_increase: 10,
            can_breed: true,
        })
    } else {
        None
    }
}

/// Checks whether two equines can mate with each other.
///
/// Mirrors vanilla `Horse.java:canMate` and `Donkey.java:canMate`:
/// - Horse can mate with Horse or Donkey.
/// - Donkey can mate with Donkey or Horse.
/// - Mules are sterile hybrids and cannot mate with any equine (`Mule.java:canMate` returns false).
#[must_use]
pub fn can_equine_mate(type_a: &EntityType, type_b: &EntityType) -> bool {
    let is_horse_or_donkey_a = *type_a == EntityType::HORSE || *type_a == EntityType::DONKEY;
    let is_horse_or_donkey_b = *type_b == EntityType::HORSE || *type_b == EntityType::DONKEY;
    is_horse_or_donkey_a && is_horse_or_donkey_b
}

/// Determines the offspring entity type when two equines breed.
///
/// Mirrors vanilla `Horse.java:getBreedOffspring` and `Donkey.java:getBreedOffspring`:
/// - Horse x Donkey (or Donkey x Horse) -> Mule!
/// - Horse x Horse -> Horse
/// - Donkey x Donkey -> Donkey
#[must_use]
pub fn determine_equine_baby_type(parent_a: &EntityType, parent_b: &EntityType) -> &'static EntityType {
    if (parent_a.id == EntityType::HORSE.id && parent_b.id == EntityType::DONKEY.id)
        || (parent_a.id == EntityType::DONKEY.id && parent_b.id == EntityType::HORSE.id)
    {
        &EntityType::MULE
    } else if parent_a.id == EntityType::DONKEY.id && parent_b.id == EntityType::DONKEY.id {
        &EntityType::DONKEY
    } else {
        &EntityType::HORSE
    }
}

/// Computes the inherited attribute (Max Health, Jump Strength, Movement Speed) for horse/donkey/mule offspring.
///
/// Mirrors vanilla `AbstractHorse.java:createOffspringAttribute`:
#[must_use]
pub fn calculate_offspring_attribute(
    parent_a_val: f64,
    parent_b_val: f64,
    min_val: f64,
    max_val: f64,
    r1: f64,
    r2: f64,
    r3: f64,
) -> f64 {
    if max_val <= min_val {
        return min_val;
    }
    let p_a = parent_a_val.clamp(min_val, max_val);
    let p_b = parent_b_val.clamp(min_val, max_val);
    let margin = 0.15 * (max_val - min_val);
    let range = (p_a - p_b).abs() + margin * 2.0;
    let average = (p_a + p_b) / 2.0;
    let baby_quality = (r1 + r2 + r3) / 3.0 - 0.5;
    let new_val = average + range * baby_quality;
    if new_val > max_val {
        let diff = new_val - max_val;
        max_val - diff
    } else if new_val < min_val {
        let diff = min_val - new_val;
        min_val + diff
    } else {
        new_val
    }
}

/// Selects a baby horse's variant and markings based on parental genetics and mutation roll.
///
/// Mirrors vanilla `Horse.java:getBreedOffspring`:
/// - `selectSkin = random.nextInt(9)` (0..8):
///   - 0..3 (4/9 = 44.4%): parent A variant
///   - 4..7 (4/9 = 44.4%): parent B variant
///   - 8 (1/9 = 11.1%): random 0..6 variant
/// - `selectMarking = random.nextInt(5)` (0..4):
///   - 0..1 (2/5 = 40.0%): parent A markings
///   - 2..3 (2/5 = 40.0%): parent B markings
///   - 4 (1/5 = 20.0%): random 0..4 markings
#[must_use]
pub fn select_horse_baby_variant_and_markings(
    parent_a_variant: i32,
    parent_b_variant: i32,
    rand_skin: u32,
    rand_markings: u32,
    rand_variant: u8,
    rand_marking: u8,
) -> i32 {
    let parent_a_color = parent_a_variant & 0xFF;
    let parent_b_color = parent_b_variant & 0xFF;
    let parent_a_marking = (parent_a_variant >> 8) & 0xFF;
    let parent_b_marking = (parent_b_variant >> 8) & 0xFF;

    let variant = if rand_skin < 4 {
        parent_a_color
    } else if rand_skin < 8 {
        parent_b_color
    } else {
        (rand_variant % 7) as i32
    };

    let marking = if rand_markings < 2 {
        parent_a_marking
    } else if rand_markings < 4 {
        parent_b_marking
    } else {
        (rand_marking % 5) as i32
    };

    variant | (marking << 8)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LlamaFoodProperties {
    pub heal: f32,
    pub age_up_seconds: i32,
    pub temper_increase: i32,
    pub can_breed: bool,
}

/// Returns the feeding properties of an item when given to a Llama or Trader Llama.
///
/// Mirrors vanilla `Llama.java:handleEating`:
/// - `Wheat`: heal 2.0, ageUp 10s, temper +3, can_breed: false
/// - `Hay Block`: heal 10.0, ageUp 90s, temper +6, can_breed: true
#[must_use]
pub fn get_llama_food_properties(item_id: u16) -> Option<LlamaFoodProperties> {
    use pumpkin_data::item::Item;
    if item_id == Item::WHEAT.id {
        Some(LlamaFoodProperties {
            heal: 2.0,
            age_up_seconds: 10,
            temper_increase: 3,
            can_breed: false,
        })
    } else if item_id == Item::HAY_BLOCK.id {
        Some(LlamaFoodProperties {
            heal: 10.0,
            age_up_seconds: 90,
            temper_increase: 6,
            can_breed: true,
        })
    } else {
        None
    }
}

/// Calculates the inherited strength (1..5) for a baby llama based on parental strength.
///
/// Mirrors vanilla `Llama.java:getBreedOffspring`:
#[must_use]
pub fn calculate_llama_baby_strength(
    parent_a_strength: i32,
    parent_b_strength: i32,
    rand_int: u32,
    rand_float: f32,
) -> i32 {
    let max_strength = parent_a_strength.max(parent_b_strength).max(1) as u32;
    let mut baby_strength = (rand_int % max_strength) as i32 + 1;
    if rand_float < 0.03 {
        baby_strength += 1;
    }
    baby_strength.clamp(1, 5)
}

/// Selects a baby llama's variant from its parents.
///
/// Mirrors vanilla `Llama.java:getBreedOffspring`:
/// 50% chance to inherit variant from parent 1, 50% from parent 2.
#[must_use]
pub const fn select_llama_baby_variant(
    parent_a_variant: i32,
    parent_b_variant: i32,
    rand_bool: bool,
) -> i32 {
    if rand_bool {
        parent_a_variant
    } else {
        parent_b_variant
    }
}

/// Evaluates whether a skeleton trap horse should trigger based on nearby player proximity.
///
/// Mirrors vanilla `SkeletonTrapGoal.java:canUse`:
/// Triggers when an alive player is within 10 blocks (`distance < 10.0`).
#[must_use]
pub fn should_skeleton_trap_trigger(is_trap: bool, distance_to_player: f64) -> bool {
    is_trap && distance_to_player < 10.0
}

/// Evaluates whether an un-triggered skeleton trap horse should despawn.
///
/// Mirrors vanilla `SkeletonHorse.java:tick`:
/// Despawns after 18000 ticks (15 minutes).
#[must_use]
pub fn should_skeleton_trap_despawn(is_trap: bool, trap_time: i32) -> bool {
    is_trap && trap_time >= 18000
}

/// Selects a baby mooshroom's variant based on parental variants and the 1/1024 mutation roll.
///
/// Mirrors vanilla `MushroomCow.java:getBreedOffspring`:
/// - If parent variants are identical and `rand_1024 == 0`, mutates to the opposite variant
///   (Red -> Brown, Brown -> Red).
/// - Otherwise, 50% chance to inherit parent A's variant, 50% to inherit parent B's variant.
#[must_use]
pub const fn select_mooshroom_baby_variant(
    parent_a_variant: i32,
    parent_b_variant: i32,
    rand_1024: i32,
    rand_bool: bool,
) -> i32 {
    if parent_a_variant == parent_b_variant && rand_1024 == 0 {
        if parent_a_variant == 1 { 0 } else { 1 }
    } else if rand_bool {
        parent_a_variant
    } else {
        parent_b_variant
    }
}

/// Selects a baby pig's variant and sound variant based on parental inheritance.
///
/// Mirrors vanilla `Pig.java:getBreedOffspring`:
/// - 50% chance to inherit variant from parent A, 50% from parent B.
/// - 50% chance to inherit sound variant from parent A, 50% from parent B.
#[must_use]
pub const fn select_pig_baby_variant_and_sound(
    parent_a_var: u8,
    parent_b_var: u8,
    parent_a_sound: u8,
    parent_b_sound: u8,
    rand_var: bool,
    rand_sound: bool,
) -> (u8, u8) {
    let variant = if rand_var { parent_a_var } else { parent_b_var };
    let sound = if rand_sound { parent_a_sound } else { parent_b_sound };
    (variant, sound)
}

/// Computes the sheep offspring wool color based on vanilla dye mixing recipes or parental inheritance.
///
/// Mirrors vanilla `Sheep.java:getBreedOffspring` and `DyeColor.java:getMixedColor`:
/// - Checks if the two parental colors can combine into a crafted dye color.
///   - White (0) + Black (15) => Gray (7)
///   - White (0) + Gray (7) => Light Gray (8)
///   - White (0) + Red (14) => Pink (6)
///   - White (0) + Green (13) => Lime (5)
///   - White (0) + Blue (11) => Light Blue (3)
///   - Red (14) + Yellow (4) => Orange (1)
///   - Red (14) + Blue (11) => Purple (10)
///   - Green (13) + Blue (11) => Cyan (9)
///   - Purple (10) + Pink (6) => Magenta (2)
/// - If not mixable, 50% chance to inherit parent A's color, 50% to inherit parent B's color.
#[must_use]
pub const fn get_mixed_sheep_color(color_a: u8, color_b: u8, rand_bool: bool) -> u8 {
    let (min, max) = if color_a < color_b {
        (color_a, color_b)
    } else {
        (color_b, color_a)
    };

    match (min, max) {
        (0, 15) => 7,  // White + Black => Gray
        (0, 7) => 8,   // White + Gray => Light Gray
        (0, 14) => 6,  // White + Red => Pink
        (0, 13) => 5,  // White + Green => Lime
        (0, 11) => 3,  // White + Blue => Light Blue
        (4, 14) => 1,  // Yellow + Red => Orange
        (11, 14) => 10, // Blue + Red => Purple
        (11, 13) => 9,  // Blue + Green => Cyan
        (6, 10) => 2,   // Pink + Purple => Magenta
        _ => {
            if rand_bool {
                color_a
            } else {
                color_b
            }
        }
    }
}

/// Calculates the sound pitch for Sniffer egg plop on breeding.
///
/// Mirrors vanilla `Sniffer.java:350`:
/// `(random.nextFloat() - random.nextFloat()) * 0.2F + 0.5F`
#[must_use]
pub fn get_sniffer_egg_plop_pitch(rand_a: f32, rand_b: f32) -> f32 {
    (rand_a - rand_b) * 0.2 + 0.5
}

/// Calculates the sound pitch for Strider eating sound.
///
/// Mirrors vanilla `Strider.java:432`:
/// `1.0F + (this.random.nextFloat() - this.random.nextFloat()) * 0.2F`
#[must_use]
pub fn get_strider_eat_sound_pitch(rand_a: f32, rand_b: f32) -> f32 {
    1.0 + (rand_a - rand_b) * 0.2
}

/// Checks if an entity type is oviparous or handles breeding specially (egg drop / gravid instead of spawning baby).
///
/// Mirrors vanilla `Sniffer.java:spawnChildFromBreeding`, `Turtle.java:TurtleBreedGoal`, and `Frog.java:spawnChildFromBreeding`.
#[must_use]
pub fn is_special_oviparous_breeding(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    use pumpkin_data::entity::EntityType;
    entity_type == &EntityType::SNIFFER
        || entity_type == &EntityType::TURTLE
        || entity_type == &EntityType::FROG
}

/// Checks whether a Fox should avoid a player entity.
///
/// Mirrors vanilla `Fox.java:187`: `!this.trusts(entity) && !this.isDefending()`.
#[must_use]
pub fn should_fox_avoid_player(is_defending: bool, is_trusted: bool) -> bool {
    !is_defending && !is_trusted
}


/// Checks whether a pregnant Frog can lay its spawn block on fluid near land.
///
/// Mirrors vanilla `TryLaySpawnOnFluidNearLand.java:23`: `is_pregnant && on_ground && !in_water`.
#[must_use]
pub fn should_frog_lay_spawn_block(is_pregnant: bool, is_on_ground: bool, is_in_water: bool) -> bool {
    is_pregnant && is_on_ground && !is_in_water
}

/// Calculates Goat eating sound pitch between 0.8 and 1.2.
///
/// Mirrors vanilla `Goat.java:208`: `Mth.randomBetween(this.level().getRandom(), 0.8F, 1.2F)`.
#[must_use]
pub fn get_goat_eat_sound_pitch(rand: f32) -> f32 {
    0.8 + rand * 0.4
}

/// Calculates Ocelot trust feeding success chance (1 in 3, or 33.3%).
///
/// Mirrors vanilla `Ocelot.java:172`: `this.random.nextInt(3) == 0`.
#[must_use]
pub fn calculate_ocelot_trust_success(rand_3: u32) -> bool {
    rand_3 == 0
}

/// Checks whether an Ocelot should avoid player entities.
///
/// Mirrors vanilla `Ocelot.java:219-220`: `!this.isTrusting()`.
#[must_use]
pub fn should_ocelot_avoid_player(is_trusting: bool) -> bool {
    !is_trusting
}

/// Calculates Bat voice pitch: ((rand_a - rand_b) * 0.2 + 1.0) * 0.95.
///
/// Mirrors vanilla `Bat.java:67`: `super.getVoicePitch() * 0.95F` where `super.getVoicePitch()`
/// in `LivingEntity.java:386` is `(rand_a - rand_b) * 0.2F + 1.0F`.
#[must_use]
pub fn get_bat_voice_pitch(rand_a: f32, rand_b: f32) -> f32 {
    ((rand_a - rand_b) * 0.2 + 1.0) * 0.95
}

/// Checks whether a resting Bat should wake up and take flight.
///
/// Mirrors vanilla `Bat.java:135, 140, 147`: wakes if block above is not solid or player is within 4.0 blocks.
#[must_use]
pub fn should_bat_wake_from_resting(is_solid_above: bool, player_distance: Option<f64>) -> bool {
    !is_solid_above || player_distance.is_some_and(|d| d < 4.0)
}

/// Calculates natural spawn strength for Llama and Trader Llama.
///
/// Mirrors vanilla `Llama.java:93-96`:
/// 4% chance of maxStrength 5 (1..=5), otherwise maxStrength 3 (1..=3).
#[must_use]
pub fn calculate_initial_llama_strength(rand_float: f32, rand_int: u32) -> i32 {
    let max_strength = if rand_float < 0.04 { 5 } else { 3 };
    1 + (rand_int % max_strength) as i32
}

/// Calculates sound pitch for Llama eating and chest equipping.
///
/// Mirrors vanilla `Llama.java:228, 296`:
/// `1.0F + (random.nextFloat() - random.nextFloat()) * 0.2F`
#[must_use]
pub fn get_llama_sound_pitch(rand_a: f32, rand_b: f32) -> f32 {
    1.0 + (rand_a - rand_b) * 0.2
}

/// Calculates inventory columns for a chested Llama.
///
/// Mirrors vanilla `Llama.java:300-302`:
/// Returns `strength` (clamped 1..=5) if it has a chest, else 0.
#[must_use]
pub fn get_llama_inventory_columns(has_chest: bool, strength: i32) -> i32 {
    if has_chest {
        strength.clamp(1, 5)
    } else {
        0
    }
}

/// Checks if a llama is eligible to breed.
///
/// Mirrors vanilla `TraderLlama.java:37-39` and `Llama.java:315-317`:
/// Trader Llamas are sterile and cannot mate.
#[must_use]
pub fn can_llama_mate(partner_is_llama: bool, is_trader_llama: bool) -> bool {
    partner_is_llama && !is_trader_llama
}

/// Calculates Strider ridden speed based on suffocating (cold) state and steering boost.
///
/// Mirrors vanilla `Strider.java:273-275`:
/// `(float)(base_speed * (is_suffocating ? 0.35F : 0.55F) * boost_factor)`
#[must_use]
pub fn calculate_strider_ridden_speed(
    base_speed: f64,
    is_suffocating: bool,
    boost_factor: f32,
) -> f32 {
    let steering_mod = if is_suffocating { 0.35 } else { 0.55 };
    ((base_speed * steering_mod) as f32) * boost_factor
}

/// Checks if a Strider should make its happy sound when being tempted.
///
/// Mirrors vanilla `Strider.java:303-304`:
/// `if (this.isBeingTempted() && this.random.nextInt(140) == 0)`
#[must_use]
pub fn should_strider_be_happy(is_tempted: bool, rand_val: u32) -> bool {
    is_tempted && rand_val == 0
}

/// Checks if a Strider should make its retreat sound when panicking.
///
/// Mirrors vanilla `Strider.java:305-306`:
/// `else if (this.isPanicking() && this.random.nextInt(60) == 0)`
#[must_use]
pub fn should_strider_retreat(is_panicking: bool, rand_val: u32) -> bool {
    is_panicking && rand_val == 0
}

/// Checks if a squid (regular or glow) should squirt ink when hurt.
///
/// Mirrors vanilla `Squid.java:180`:
/// Squid only squirts ink when damaged by an attacker mob (`getLastHurtByMob() != null`).
#[must_use]
pub fn should_squid_squirt_ink(has_attacker: bool) -> bool {
    has_attacker
}

/// Returns total air supply for Dolphin (4800 ticks = 4 minutes).
///
/// Mirrors vanilla `Dolphin.java:78`:
/// `public static final int TOTAL_AIR_SUPPLY = 4800;`
#[must_use]
pub const fn get_dolphin_total_air_supply() -> i32 {
    4800
}

/// Returns total moistness level for Dolphin (2400 ticks = 2 minutes).
///
/// Mirrors vanilla `Dolphin.java:79`:
/// `private static final int TOTAL_MOISTNESS_LEVEL = 2400;`
#[must_use]
pub const fn get_dolphin_total_moistness() -> i32 {
    2400
}

/// Calculates Pufferfish sting poison duration in ticks.
///
/// Mirrors vanilla `Pufferfish.java:143, 160`:
/// `60 * puffState` ticks.
#[must_use]
pub fn calculate_pufferfish_poison_duration(puff_state: i32) -> i32 {
    (60 * puff_state).max(0)
}

/// Calculates Pufferfish sting attack damage.
///
/// Mirrors vanilla `Pufferfish.java:142, 155`:
/// `1 + puffState` damage.
#[must_use]
pub fn calculate_pufferfish_sting_damage(puff_state: i32) -> f32 {
    (1 + puff_state) as f32
}

/// Calculates Pufferfish hitbox scale based on inflation state.
///
/// Mirrors vanilla `Pufferfish.java:185-191`:
/// state 0 -> 0.5, state 1 -> 0.7, state 2 -> 1.0.
#[must_use]
pub fn calculate_pufferfish_scale(puff_state: i32) -> f32 {
    match puff_state {
        0 => 0.5,
        1 => 0.7,
        _ => 1.0,
    }
}

/// Calculates wolf tail rotation angle based on state and health.
///
/// Mirrors vanilla `Wolf.java:532-540`:
/// - Angry: 1.5393804F (~88.2 degrees)
/// - Tame: (0.55F - damageRatio * 0.4F) * PI
/// - Wild: PI / 5.0 (~36 degrees)
#[must_use]
pub fn calculate_wolf_tail_angle(is_angry: bool, is_tame: bool, health: f32, max_health: f32) -> f32 {
    if is_angry {
        1.539_380_4
    } else if is_tame {
        let max_h = if max_health <= 0.0 { 40.0 } else { max_health };
        let damage_ratio = ((max_h - health) / max_h).clamp(0.0, 1.0);
        (0.55 - damage_ratio * 0.4) * std::f32::consts::PI
    } else {
        std::f32::consts::PI / 5.0
    }
}

/// Calculates repair amount per Armadillo Scute on Wolf Armor (12.5% of max durability).
///
/// Mirrors vanilla `Wolf.java:475`:
/// `int repairUnit = (int)(armor.getMaxDamage() * 0.125F);`
#[must_use]
pub fn calculate_wolf_armor_repair_amount(max_damage: i32) -> i32 {
    ((max_damage as f32) * 0.125) as i32
}

/// Checks whether a wolf's armor can be repaired with an armadillo scute.
///
/// Mirrors vanilla `Wolf.java:467-471`:
/// Requires the wolf to be in sitting pose, wearing body armor, and the armor to be damaged.
#[must_use]
pub fn can_repair_wolf_armor(is_sitting: bool, is_damaged: bool, has_armor: bool) -> bool {
    is_sitting && is_damaged && has_armor
}

/// Returns minimum ticks a fox must hold food in mouth before consuming it (600 ticks = 30 seconds).
///
/// Mirrors vanilla `Fox.java:132, 217`:
/// `private static final int MIN_TICKS_BEFORE_EAT = 600;`
#[must_use]
pub const fn get_fox_min_ticks_before_eat() -> i32 {
    600
}

/// Checks if a fox is eligible to eat its held food item.
///
/// Mirrors vanilla `Fox.java:216-217`:
/// `if (this.canEat(itemInMouth) && this.ticksSinceEaten > 600)`
#[must_use]
pub fn can_fox_eat_item(is_food: bool, ticks_since_eaten: i32) -> bool {
    is_food && ticks_since_eaten > 600
}

/// Returns health restored when feeding a tamed cat (1.0 HP = half a heart).
///
/// Mirrors vanilla `Cat.java:425`:
/// `this.feed(player, hand, itemStack, 1.0F, 1.0F);`
#[must_use]
pub const fn get_cat_food_healing() -> f32 {
    1.0
}





/// Represents the biome category for rabbit variant spawning and breeding mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RabbitBiomeSpawnType {
    White,
    Gold,
    Normal,
}

/// Determines the baby rabbit variant based on parents and biome.
///
/// Mirrors vanilla `Rabbit.java:345-352` & `getRandomRabbitVariant`:
/// - 95% (`rand_20 != 0`): 50% inherit parent_a, 50% inherit parent_b.
/// - 5% (`rand_20 == 0`): Biome-based mutation:
///   - White biome: 80% WHITE (1), 20% WHITE_SPLOTCHED (3)
///   - Gold biome: 100% GOLD (4)
///   - Normal biome: 50% BROWN (0), 40% SALT (5), 10% BLACK (2)
#[must_use]
pub fn select_rabbit_baby_variant(
    parent_a: i32,
    parent_b: i32,
    rand_20: i32,
    rand_bool: bool,
    rand_100: i32,
    biome_type: RabbitBiomeSpawnType,
) -> i32 {
    if rand_20 != 0 {
        if rand_bool { parent_b } else { parent_a }
    } else {
        match biome_type {
            RabbitBiomeSpawnType::White => {
                if rand_100 < 80 { 1 } else { 3 }
            }
            RabbitBiomeSpawnType::Gold => 4,
            RabbitBiomeSpawnType::Normal => {
                if rand_100 < 50 {
                    0
                } else if rand_100 < 90 {
                    5
                } else {
                    2
                }
            }
        }
    }
}

/// Calculates the sound pitch for Parrot eating seeds.
///
/// Mirrors vanilla `Parrot.java:266`:
/// `1.0F + (this.random.nextFloat() - this.random.nextFloat()) * 0.2F`
#[must_use]
pub fn get_parrot_eat_sound_pitch(rand_a: f32, rand_b: f32) -> f32 {
    1.0 + (rand_a - rand_b) * 0.2
}

/// Determines whether parrot taming succeeds (10% chance).
///
/// Mirrors vanilla `Parrot.java:271`:
/// `this.random.nextInt(10) == 0`
#[must_use]
pub const fn calculate_parrot_tame_success(rand_10: u32) -> bool {
    rand_10 == 0
}

/// Determines baby variant for Chicken or Cow based on parents (50/50 inheritance).
///
/// Mirrors vanilla `Chicken.java:178` & `Cow.java:75`:
/// `this.random.nextBoolean() ? this.getVariant() : partner.getVariant()`
#[must_use]
pub const fn select_chicken_or_cow_baby_variant(
    parent_a: u8,
    parent_b: u8,
    rand_bool: bool,
) -> u8 {
    if rand_bool {
        parent_a
    } else {
        parent_b
    }
}

/// Calculates the sound pitch for Chicken laying an egg.
///
/// Mirrors vanilla `Chicken.java:137`:
/// `(this.random.nextFloat() - this.random.nextFloat()) * 0.2F + 1.0F`
#[must_use]
pub fn get_chicken_egg_lay_sound_pitch(rand_a: f32, rand_b: f32) -> f32 {
    (rand_a - rand_b) * 0.2 + 1.0
}

/// Determines whether a chicken is eligible to lay an egg.
///
/// Mirrors vanilla `Chicken.java:135`:
/// `!this.isBaby() && !this.isChickenJockey() && --this.eggTime <= 0`
#[must_use]
pub const fn should_chicken_lay_egg(is_baby: bool, is_jockey: bool, egg_time: i32) -> bool {
    !is_baby && !is_jockey && egg_time <= 0
}

/// Calculates experience dropped by chicken on death.
///
/// Mirrors vanilla `Chicken.java:199-201`:
/// `this.isChickenJockey() ? 10 : super.getBaseExperienceReward(level)`
#[must_use]
pub const fn get_chicken_xp_reward(is_jockey: bool, default_xp: u32) -> u32 {
    if is_jockey {
        10
    } else {
        default_xp
    }
}

/// Calculates fall damage taken by a parrot.
///
/// Mirrors vanilla `Parrot.java:293-294`:
/// Parrots override `checkFallDamage` with a no-op, taking 0 fall damage points.
#[must_use]
pub const fn calculate_parrot_fall_damage() -> i32 {
    0
}

/// Checks whether parrots take fall damage.
///
/// Mirrors vanilla `Parrot.java:293-294`:
/// Always false.
#[must_use]
pub const fn should_parrot_take_fall_damage() -> bool {
    false
}

pub const RABBIT_STROLL_SPEED_MOD: f64 = 0.6;
pub const RABBIT_BREED_SPEED_MOD: f64 = 0.8;
pub const RABBIT_FOLLOW_SPEED_MOD: f64 = 1.0;
pub const RABBIT_FLEE_SPEED_MOD: f64 = 2.2;
pub const RABBIT_ATTACK_SPEED_MOD: f64 = 1.4;
pub const RABBIT_EVIL_ARMOR: f64 = 8.0;
pub const RABBIT_EVIL_ATTACK_POWER_MODIFIER: f64 = 5.0;
pub const RABBIT_DEFAULT_ATTACK_POWER: f64 = 3.0;

/// Determines sound pitch for Killer Bunny attack.
///
/// Mirrors vanilla `Rabbit.java:319`:
/// `(this.random.nextFloat() - this.random.nextFloat()) * 0.2F + 1.0F`
#[must_use]
pub fn calculate_rabbit_attack_pitch(rand_a: f32, rand_b: f32) -> f32 {
    (rand_a - rand_b) * 0.2 + 1.0
}

/// Checks if rabbit variant is the Killer Bunny (Evil).
///
/// Mirrors vanilla `Rabbit.java:324-326`:
/// `this.getVariant() == Rabbit.Variant.EVIL ? SoundSource.HOSTILE : SoundSource.NEUTRAL`
#[must_use]
pub const fn is_rabbit_evil(variant_id: i32) -> bool {
    variant_id == 99
}

pub const AXOLOTL_REHYDRATE_AIR_SUPPLY: i32 = 1800;
pub const AXOLOTL_TOTAL_AIR_SUPPLY: i32 = 6000;

/// Calculates new air supply after Axolotl rehydration.
///
/// Mirrors vanilla `Axolotl.java:273-276`:
/// `int newAirSupply = this.getAirSupply() + 1800; Math.min(newAirSupply, this.getMaxAirSupply());`
#[must_use]
pub const fn calculate_axolotl_rehydrate_air(current_air: i32, max_air: i32) -> i32 {
    let new_air = current_air + AXOLOTL_REHYDRATE_AIR_SUPPLY;
    if new_air > max_air {
        max_air
    } else {
        new_air
    }
}

/// Determines whether an Axolotl should emit ambient sounds.
///
/// Mirrors vanilla `Axolotl.java:169-173`:
/// Silent while playing dead (`if (!this.isPlayingDead()) super.playAmbientSound()`).
#[must_use]
pub const fn should_axolotl_play_ambient_sound(is_playing_dead: bool, is_silent: bool) -> bool {
    !is_playing_dead && !is_silent
}

/// Calculates sound pitch for Sniffer eating food.
///
/// Mirrors vanilla `Sniffer.java:360`:
/// `Mth.randomBetween(this.level().getRandom(), 0.8F, 1.2F)`
#[must_use]
pub fn calculate_sniffer_eat_pitch(rand_val: f32) -> f32 {
    0.8 + rand_val * 0.4
}

/// Calculates reduced damage received by an Armadillo when hidden in its shell.
///
/// Mirrors vanilla `Armadillo.java:279-283`:
/// `damage = (damage - 1.0F) / 2.0F`
#[must_use]
pub fn calculate_armadillo_scared_damage(raw_damage: f32) -> f32 {
    ((raw_damage - 1.0) / 2.0).max(0.0)
}

/// Minimum ticks an Enderman remains aggressive after acquiring a target before it can de-aggro/teleport due to bright daylight.
///
/// Mirrors vanilla `Enderman.java:78, 248`:
/// `private static final int MIN_DEAGGRESSION_TIME = 600;`
pub const ENDERMAN_MIN_DEAGGRESSION_TIME: u64 = 600;

/// Delay in ticks between playing creepy stare sound when looked at.
///
/// Mirrors vanilla `Enderman.java:77, 177`:
/// `private static final int DELAY_BETWEEN_CREEPY_STARE_SOUND = 400;`
pub const ENDERMAN_DELAY_BETWEEN_CREEPY_STARE_SOUND: u64 = 400;

/// Sound volume for Enderman stare sound.
///
/// Mirrors vanilla `Enderman.java:180`:
/// `this.level().playLocalSound(..., SoundEvents.ENDERMAN_STARE, ..., 2.5F, 1.0F, false);`
pub const ENDERMAN_STARE_SOUND_VOLUME: f32 = 2.5;

/// Checks if an Enderman has held its target long enough (>= 600 ticks) to permit de-aggression in sunlight.
///
/// Mirrors vanilla `Enderman.java:248`:
/// `this.tickCount >= this.targetChangeTime + 600`
#[must_use]
pub const fn can_enderman_deaggro_in_sunlight(current_tick: u64, target_change_time: u64) -> bool {
    target_change_time == 0 || current_tick >= target_change_time + ENDERMAN_MIN_DEAGGRESSION_TIME
}

/// Evaluates whether an Enderman in daylight seeing sky should teleport away.
///
/// Mirrors vanilla `Enderman.java:249-250`:
/// `float br = this.getLightLevelDependentMagicValue();`
/// `if (br > 0.5F && level.canSeeSky(this.blockPosition()) && this.random.nextFloat() * 30.0F < (br - 0.4F) * 2.0F)`
#[must_use]
pub fn should_enderman_daylight_teleport(br: f32, rand_val: f32) -> bool {
    br > 0.5 && rand_val * 30.0 < (br - 0.4) * 2.0
}

/// Checks if an Enderman should play its stare sound when gazed at.
///
/// Mirrors vanilla `Enderman.java:177-182`:
/// `if (this.tickCount >= this.lastStareSound + 400) { if (!this.isSilent()) ... }`
#[must_use]
pub const fn should_enderman_play_stare_sound(
    current_tick: u64,
    last_stare_sound: u64,
    is_silent: bool,
) -> bool {
    !is_silent && (last_stare_sound == 0 || current_tick >= last_stare_sound + ENDERMAN_DELAY_BETWEEN_CREEPY_STARE_SOUND)
}

/// Blaze base max health (20.0 HP / 10 hearts).
/// Mirrors vanilla `Monster.java` (`Attributes.MAX_HEALTH, 20.0`).
pub const BLAZE_MAX_HEALTH: f32 = 20.0;

/// Blaze base movement speed (0.23).
/// Mirrors vanilla `Blaze.java:55` (`Attributes.MOVEMENT_SPEED, 0.23F`).
pub const BLAZE_BASE_MOVEMENT_SPEED: f64 = 0.23;

/// Blaze melee attack damage (6.0 HP / 3 hearts).
/// Mirrors vanilla `Blaze.java:55` (`Attributes.ATTACK_DAMAGE, 6.0`).
pub const BLAZE_ATTACK_DAMAGE: f32 = 6.0;

/// Blaze follow range (48.0 blocks).
/// Mirrors vanilla `Blaze.java:55` (`Attributes.FOLLOW_RANGE, 48.0`).
pub const BLAZE_FOLLOW_RANGE: f64 = 48.0;

/// Blaze experience reward on death (10 XP).
/// Mirrors vanilla `Blaze.java:40` (`this.xpReward = 10`).
pub const BLAZE_XP_REWARD: u32 = 10;

/// Multiplier applied to Y velocity when falling in air (0.6).
/// Mirrors vanilla `Blaze.java:87` (`multiply(1.0, 0.6, 1.0)`).
pub const BLAZE_FALL_SPEED_MULTIPLIER: f64 = 0.6;

/// Interval between allowed height offset re-calculations (100 ticks = 5.0s).
/// Mirrors vanilla `Blaze.java:122` (`nextHeightOffsetChangeTick = 100`).
pub const BLAZE_HEIGHT_OFFSET_INTERVAL_TICKS: i32 = 100;

/// Spread parameter for triangle distribution of allowed height offset (6.891).
/// Mirrors vanilla `Blaze.java:123` (`random.triangle(0.5, 6.891)`).
pub const BLAZE_HEIGHT_OFFSET_SPREAD: f64 = 6.891;

/// Vertical acceleration rate when ascending towards target (0.3).
/// Mirrors vanilla `Blaze.java:129` (`(0.3F - movement.y) * 0.3F`).
pub const BLAZE_ASCENT_ACCELERATION: f64 = 0.3;

/// Melee attack interval when within close range (< 4.0 sq distance, 20 ticks = 1.0s).
/// Mirrors vanilla `Blaze.java:208` (`this.attackTime = 20`).
pub const BLAZE_MELEE_ATTACK_INTERVAL: i32 = 20;

/// Warmup time before Blaze unleashes small fireball burst (60 ticks = 3.0s).
/// Mirrors vanilla `Blaze.java:220` (`this.attackTime = 60`).
pub const BLAZE_FIREBALL_WARMUP_TICKS: i32 = 60;

/// Interval between each small fireball in the 3-shot burst (6 ticks = 0.3s).
/// Mirrors vanilla `Blaze.java:223` (`this.attackTime = 6`).
pub const BLAZE_FIREBALL_BURST_INTERVAL_TICKS: i32 = 6;

/// Cooldown time after completing small fireball burst (100 ticks = 5.0s).
/// Mirrors vanilla `Blaze.java:225` (`this.attackTime = 100`).
pub const BLAZE_FIREBALL_COOLDOWN_TICKS: i32 = 100;

/// Sound event ID broadcast when Blaze shoots small fireball (1018).
/// Mirrors vanilla `Blaze.java:233` (`level.levelEvent(null, 1018, ...)`).
pub const BLAZE_SHOOT_SOUND_EVENT_ID: i32 = 1018;

/// Calculates sound pitch for Blaze burning/crackling sound.
///
/// Mirrors vanilla `Blaze.java:100`:
/// `this.random.nextFloat() * 0.7F + 0.3F`
#[must_use]
pub fn calculate_blaze_burn_pitch(rand_val: f32) -> f32 {
    rand_val * 0.7 + 0.3
}

/// Calculates sound volume for Blaze burning/crackling sound.
///
/// Mirrors vanilla `Blaze.java:99`:
/// `1.0F + this.random.nextFloat()`
#[must_use]
pub fn calculate_blaze_burn_volume(rand_val: f32) -> f32 {
    1.0 + rand_val
}

/// Checks whether Blaze should play its ambient crackling burn sound (1/24 chance per tick).
///
/// Mirrors vanilla `Blaze.java:91`:
/// `if (this.random.nextInt(24) == 0 && !this.isSilent())`
#[must_use]
pub const fn should_blaze_play_burn_sound(rand_int_24: u32, is_silent: bool) -> bool {
    rand_int_24 == 0 && !is_silent
}

/// Calculates small fireball spread for Blaze shooting goal.
///
/// Mirrors vanilla `Blaze.java:231, 237`:
/// `double sqd = Math.sqrt(Math.sqrt(distance)) * 0.5;`
/// `2.297 * sqd`
#[must_use]
pub fn calculate_blaze_small_fireball_spread(distance_sq: f64) -> f64 {
    2.297 * distance_sq.sqrt().sqrt() * 0.5
}

/// Calculates extra jump boost power for MagmaCube jumping from ground.
///
/// Mirrors vanilla `MagmaCube.java:96`:
/// `float sizeJumpBoostPower = this.getSize() * 0.1F;`
#[must_use]
pub fn calculate_magma_cube_jump_boost(size: i32) -> f64 {
    (size.max(1) as f64) * 0.1
}

/// Calculates vertical velocity for MagmaCube jumping in Lava.
///
/// Mirrors vanilla `MagmaCube.java:105`:
/// `0.22F + this.getSize() * 0.05F`
#[must_use]
pub fn calculate_magma_cube_lava_jump_boost(size: i32) -> f64 {
    0.22 + (size.max(1) as f64) * 0.05
}

/// Checks if a CubeMob should play its jump sound.
///
/// Mirrors vanilla `AbstractCubeMob.java:289`:
/// `return this.getSize() > 0;`
#[must_use]
pub const fn should_cube_mob_play_jump_sound(size: i32) -> bool {
    size > 0
}

/// Silverfish base attributes, mirroring vanilla `Silverfish.java:53`:
/// - Max Health: 8.0 HP
/// - Movement Speed: 0.25
/// - Attack Damage: 1.0 HP
/// - Experience: 5 XP
pub const SILVERFISH_MAX_HEALTH: f32 = 8.0;
pub const SILVERFISH_BASE_MOVEMENT_SPEED: f64 = 0.25;
pub const SILVERFISH_BASE_ATTACK_DAMAGE: f32 = 1.0;
pub const SILVERFISH_XP_REWARD: u32 = 5;

/// Number of ticks between Silverfish attempts to merge with nearby compatible stone blocks.
/// Mirrors vanilla `Silverfish.java:131`:
/// `super(silverfish, 1.0, 10);`
pub const SILVERFISH_MERGE_INTERVAL_TICKS: i32 = 10;

/// Number of ticks Silverfish continues waking up nearby infested block friends when hurt.
///
/// Mirrors vanilla `Silverfish.java:189`:
/// `silverfish.wakeUpFriendsTicks = 20;`
pub const SILVERFISH_WAKE_UP_FRIENDS_TICKS: i32 = 20;

/// Endermite base attributes, mirroring vanilla `Endermite.java:36, 52`:
/// - Max Health: 8.0 HP
/// - Movement Speed: 0.25
/// - Attack Damage: 2.0 HP
/// - Experience: 3 XP
pub const ENDERMITE_MAX_HEALTH: f32 = 8.0;
pub const ENDERMITE_BASE_MOVEMENT_SPEED: f64 = 0.25;
pub const ENDERMITE_BASE_ATTACK_DAMAGE: f32 = 2.0;
pub const ENDERMITE_XP_REWARD: u32 = 3;

/// Chance of Ender Pearl impact spawning an Endermite (5%).
/// Mirrors vanilla `ThrownEnderpearl.java:107`:
/// `if (random.nextFloat() < 0.05F)`
pub const ENDER_PEARL_ENDERMITE_SPAWN_CHANCE: f32 = 0.05;

/// Maximum lifetime in ticks of an Endermite before despawning (2400 ticks = 2 minutes).
///
/// Mirrors vanilla `Endermite.java:82`:
/// `if (this.life >= 2400) { this.discard(); }`
pub const ENDERMITE_LIFETIME_MAX: i32 = 2400;

/// Determines whether an Endermite should despawn based on lifetime and persistence.
///
/// Mirrors vanilla `Endermite.java:79-84`:
/// `if (!this.isPersistenceRequired()) { this.life++; if (this.life >= 2400) this.discard(); }`
#[must_use]
pub const fn should_endermite_despawn(lifetime: i32, persistence_required: bool) -> bool {
    !persistence_required && lifetime >= ENDERMITE_LIFETIME_MAX
}

/// Minimum fall distance in blocks before Breeze plays its landing sound.
///
/// Mirrors vanilla `Breeze.java:47, 275`:
/// `private static final float FALL_DISTANCE_SOUND_TRIGGER_THRESHOLD = 3.0F;`
pub const BREEZE_FALL_DISTANCE_SOUND_THRESHOLD: f32 = 3.0;

/// Checks if a Breeze should play its landing sound upon fall damage.
///
/// Mirrors vanilla `Breeze.java:275-276`:
/// `if (fallDistance > 3.0) { this.playSound(SoundEvents.BREEZE_LAND, 1.0F, 1.0F); }`
#[must_use]
pub const fn should_breeze_play_fall_sound(fall_distance: f32) -> bool {
    fall_distance > BREEZE_FALL_DISTANCE_SOUND_THRESHOLD
}

/// Calculates sound pitch for Breeze whirling sound.
///
/// Mirrors vanilla `Breeze.java:191`:
/// `float pitch = 0.7F + 0.4F * this.random.nextFloat();`
#[must_use]
pub fn calculate_breeze_whirl_pitch(rand_val: f32) -> f32 {
    0.7 + 0.4 * rand_val
}

/// Calculates sound volume for Breeze whirling sound.
///
/// Mirrors vanilla `Breeze.java:192`:
/// `float volume = 0.8F + 0.2F * this.random.nextFloat();`
#[must_use]
pub fn calculate_breeze_whirl_volume(rand_val: f32) -> f32 {
    0.8 + 0.2 * rand_val
}

/// Skeleton max health (20.0 HP).
///
/// Mirrors vanilla `AbstractSkeleton.java` / `Mob.java`:
pub const SKELETON_MAX_HEALTH: f32 = 20.0;

/// Base movement speed for skeletons (0.25).
/// Mirrors vanilla `AbstractSkeleton.java:90` (`Attributes.MOVEMENT_SPEED, 0.25`).
pub const SKELETON_BASE_MOVEMENT_SPEED: f64 = 0.25;

/// Skeleton experience reward (5 XP).
///
/// Mirrors vanilla `AbstractSkeleton.java` / `Mob.java`:
pub const SKELETON_XP_REWARD: u32 = 5;

/// Attack interval in ticks for standard Skeletons on Hard difficulty (20 ticks = 1.0s).
/// Mirrors vanilla `AbstractSkeleton.java:51` (`HARD_ATTACK_INTERVAL = 20`).
pub const SKELETON_HARD_ATTACK_INTERVAL: i32 = 20;

/// Attack interval in ticks for standard Skeletons on Normal/Easy difficulty (40 ticks = 2.0s).
/// Mirrors vanilla `AbstractSkeleton.java:52` (`NORMAL_ATTACK_INTERVAL = 40`).
pub const SKELETON_DEFAULT_ATTACK_INTERVAL: i32 = 40;

/// Number of ticks submerged in powder snow before Skeleton begins converting to Stray (140 ticks = 7.0s).
/// Mirrors vanilla `Skeleton.java:20` (`"FreezingTime", 140`).
pub const SKELETON_STRAY_CONVERSION_SUBMERGED_TICKS: i32 = 140;

/// Shaking conversion duration when Skeleton turns into Stray (300 ticks = 15.0s).
/// Mirrors vanilla `Skeleton.java:20` (`"StrayConversionTime", 300`).
pub const SKELETON_STRAY_CONVERSION_SHAKING_TICKS: i32 = 300;

/// Sound event ID broadcast when Skeleton converts into Stray (1048).
/// Mirrors vanilla `Skeleton.java:20`.
pub const SKELETON_STRAY_CONVERSION_SOUND_EVENT_ID: i32 = 1048;

/// Calculates ranged attack interval for Skeleton depending on difficulty.
#[must_use]
pub const fn calculate_skeleton_attack_interval(is_hard: bool) -> i32 {
    if is_hard {
        SKELETON_HARD_ATTACK_INTERVAL
    } else {
        SKELETON_DEFAULT_ATTACK_INTERVAL
    }
}

/// Arrow projectile initial launch speed when fired by a Skeleton (1.6).
/// Mirrors vanilla `AbstractSkeleton.java:170`.
pub const ABSTRACT_SKELETON_ARROW_VELOCITY: f32 = 1.6;

/// Movement speed multiplier when Skeleton attacks in melee mode (1.2).
/// Mirrors vanilla `AbstractSkeleton.java:56` (`MeleeAttackGoal(this, 1.2, false)`).
pub const ABSTRACT_SKELETON_MELEE_SPEED_MODIFIER: f64 = 1.2;

/// Movement speed multiplier when Skeleton flees sunlight (1.0).
/// Mirrors vanilla `AbstractSkeleton.java:78` (`FleeSunGoal(this, 1.0)`).
pub const ABSTRACT_SKELETON_FLEE_SUN_SPEED: f64 = 1.0;

/// Avoid distance from wolves for Skeletons (6.0 blocks).
/// Mirrors vanilla `AbstractSkeleton.java:79`.
pub const ABSTRACT_SKELETON_AVOID_WOLF_DISTANCE: f32 = 6.0;

/// Walk speed modifier when avoiding wolves (1.0).
/// Mirrors vanilla `AbstractSkeleton.java:79`.
pub const ABSTRACT_SKELETON_AVOID_WOLF_WALK_SPEED: f64 = 1.0;

/// Sprint speed modifier when avoiding wolves (1.2).
/// Mirrors vanilla `AbstractSkeleton.java:79`.
pub const ABSTRACT_SKELETON_AVOID_WOLF_SPRINT_SPEED: f64 = 1.2;

/// Wither Skeleton max health (20.0 HP).
///
/// Mirrors vanilla `WitherSkeleton.java:31`:
pub const WITHER_SKELETON_MAX_HEALTH: f32 = 20.0;

/// Wither Skeleton base attack damage (4.0 HP).
///
/// Mirrors vanilla `WitherSkeleton.java:87`:
pub const WITHER_SKELETON_BASE_ATTACK_DAMAGE: f32 = 4.0;

/// Wither Skeleton base movement speed (0.25).
/// Mirrors vanilla `AbstractSkeleton.java:90`.
pub const WITHER_SKELETON_BASE_MOVEMENT_SPEED: f64 = 0.25;

/// Pathfinding malus through lava for Wither Skeleton (8.0).
/// Mirrors vanilla `WitherSkeleton.java:34` (`setPathfindingMalus(PathType.LAVA, 8.0F)`).
pub const WITHER_SKELETON_LAVA_PATHFINDING_MALUS: f32 = 8.0;

/// Wither status effect duration applied by Wither Skeleton melee attack (200 ticks = 10 seconds).
///
/// Mirrors vanilla `WitherSkeleton.java:99`:
pub const WITHER_SKELETON_WITHER_DURATION_TICKS: i32 = 200;

/// Arrow fire duration when shot by Wither Skeleton (100.0 seconds).
///
/// Mirrors vanilla `WitherSkeleton.java:108`:
pub const WITHER_SKELETON_ARROW_IGNITE_SECONDS: f32 = 100.0;

/// Wither Skeleton experience reward (5 XP).
///
/// Mirrors vanilla `WitherSkeleton.java` / `Mob.java`:
pub const WITHER_SKELETON_XP_REWARD: u32 = 5;

/// Stray max health (20.0 HP).
///
/// Mirrors vanilla `Stray.java` / `AbstractSkeleton.java`:
pub const STRAY_MAX_HEALTH: f32 = 20.0;

/// Slowness status effect duration applied by Stray arrows (600 ticks = 30 seconds).
///
/// Mirrors vanilla `Stray.java:55`:
pub const STRAY_SLOWNESS_DURATION_TICKS: i32 = 600;

/// Ranged attack interval for Stray on Hard difficulty (20 ticks).
///
/// Mirrors vanilla `AbstractSkeleton.java:112`:
pub const STRAY_HARD_ATTACK_INTERVAL: i32 = 20;

/// Ranged attack interval for Stray on Normal/Easy difficulty (40 ticks).
///
/// Mirrors vanilla `AbstractSkeleton.java:117`:
pub const STRAY_DEFAULT_ATTACK_INTERVAL: i32 = 40;

/// Calculates ranged attack interval for Stray skeleton depending on difficulty.
#[must_use]
pub const fn calculate_stray_attack_interval(is_hard: bool) -> i32 {
    if is_hard {
        STRAY_HARD_ATTACK_INTERVAL
    } else {
        STRAY_DEFAULT_ATTACK_INTERVAL
    }
}

/// Stray experience reward (5 XP).
///
/// Mirrors vanilla `Stray.java` / `Mob.java`:
pub const STRAY_XP_REWARD: u32 = 5;

/// Bogged max health (16.0 HP).
///
/// Mirrors vanilla `Bogged.java:37`:
pub const BOGGED_MAX_HEALTH: f32 = 16.0;

/// Bogged experience reward (5 XP).
///
/// Mirrors vanilla `Bogged.java` / `Mob.java`:
pub const BOGGED_XP_REWARD: u32 = 5;

/// Duration in ticks of the Poison status effect applied by Bogged arrows (100 ticks = 5 seconds).
///
/// Mirrors vanilla `Bogged.java:110`:
/// `arrow.addEffect(new MobEffectInstance(MobEffects.POISON, 100));`
pub const BOGGED_POISON_DURATION_TICKS: i32 = 100;

/// Ranged attack interval for Bogged on Hard difficulty (50 ticks).
///
/// Mirrors vanilla `Bogged.java:118`:
/// `protected int getHardAttackInterval() { return 50; }`
pub const BOGGED_HARD_ATTACK_INTERVAL: i32 = 50;

/// Ranged attack interval for Bogged on Normal/Easy difficulty (70 ticks).
///
/// Mirrors vanilla `Bogged.java:123`:
/// `protected int getAttackInterval() { return 70; }`
pub const BOGGED_DEFAULT_ATTACK_INTERVAL: i32 = 70;

/// Calculates ranged attack interval for Bogged skeleton depending on difficulty.
#[must_use]
pub const fn calculate_bogged_attack_interval(is_hard: bool) -> i32 {
    if is_hard {
        BOGGED_HARD_ATTACK_INTERVAL
    } else {
        BOGGED_DEFAULT_ATTACK_INTERVAL
    }
}

/// Parched skeleton max health (16.0 HP).
///
/// Mirrors vanilla `Parched.java:33`:
/// `add(Attributes.MAX_HEALTH, 16.0)`
pub const PARCHED_MAX_HEALTH: f32 = 16.0;

/// Parched skeleton experience reward (5 XP).
///
/// Mirrors vanilla `Parched.java` / `Mob.java`:
pub const PARCHED_XP_REWARD: u32 = 5;

/// Duration in ticks of the Weakness status effect applied by Parched arrows (600 ticks = 30 seconds).
///
/// Mirrors vanilla `Parched.java:26`:
/// `arrow.addEffect(new MobEffectInstance(MobEffects.WEAKNESS, 600));`
pub const PARCHED_WEAKNESS_DURATION_TICKS: i32 = 600;

/// Ranged attack interval for Parched on Hard difficulty (50 ticks).
///
/// Mirrors vanilla `Parched.java:58`:
/// `protected int getHardAttackInterval() { return 50; }`
pub const PARCHED_HARD_ATTACK_INTERVAL: i32 = 50;

/// Ranged attack interval for Parched on Normal/Easy difficulty (70 ticks).
///
/// Mirrors vanilla `Parched.java:63`:
/// `protected int getAttackInterval() { return 70; }`
pub const PARCHED_DEFAULT_ATTACK_INTERVAL: i32 = 70;

/// Calculates ranged attack interval for Parched skeleton depending on difficulty.
#[must_use]
pub const fn calculate_parched_attack_interval(is_hard: bool) -> i32 {
    if is_hard {
        PARCHED_HARD_ATTACK_INTERVAL
    } else {
        PARCHED_DEFAULT_ATTACK_INTERVAL
    }
}

/// Invulnerability duration in ticks when Wither is summoned (220 ticks = 11 seconds).
///
/// Mirrors vanilla `WitherBoss.java:66`:
/// `private static final int INVULNERABLE_TICKS = 220;`
pub const WITHER_INVULNERABLE_TICKS: i32 = 220;

/// Maximum base health for Wither Boss.
///
/// Mirrors vanilla `WitherBoss.java:512`:
/// `add(Attributes.MAX_HEALTH, 300.0)`
pub const WITHER_MAX_HEALTH: f32 = 300.0;

/// Base armor attribute points for Wither Boss.
///
/// Mirrors vanilla `WitherBoss.java:516`:
/// `add(Attributes.ARMOR, 4.0)`
pub const WITHER_ARMOR_POINTS: f64 = 4.0;

/// Explosion radius when Wither finishes its invulnerable charging phase.
///
/// Mirrors vanilla `WitherBoss.java:266`:
/// `level.explode(this, this.getX(), this.getEyeY(), this.getZ(), 7.0F, false, Level.ExplosionInteraction.MOB);`
pub const WITHER_SPAWN_EXPLOSION_RADIUS: f32 = 7.0;

/// Calculates bossbar progress during Wither invulnerability countdown.
///
/// Mirrors vanilla `WitherBoss.java:264`:
/// `this.bossEvent.setProgress(1.0F - newCount / 220.0F);`
#[must_use]
pub fn calculate_wither_invulnerable_progress(ticks_remaining: i32) -> f32 {
    (1.0 - (ticks_remaining as f32) / (WITHER_INVULNERABLE_TICKS as f32)).clamp(0.0, 1.0)
}

/// Checks whether Wither has entered its powered Wither Armor phase (health <= half of max).
///
/// Mirrors vanilla `WitherBoss.java:211` & `isPowered()`:
/// `return this.getHealth() <= this.getMaxHealth() / 2.0F;`
#[must_use]
pub const fn is_wither_powered(health: f32, max_health: f32) -> bool {
    health <= max_health * 0.5
}

/// Determines whether Wither completely deflects projectiles when powered with Wither Armor.
///
/// Mirrors vanilla `WitherBoss.java:464-469`:
/// `if (this.isPowered()) { if (directEntity instanceof AbstractArrow || directEntity instanceof WindCharge) return false; }`
#[must_use]
pub const fn should_wither_deflect_projectile(is_powered: bool, is_projectile: bool) -> bool {
    is_powered && is_projectile
}

/// Magic damage dealt by Warden Sonic Boom attack.
///
/// Mirrors vanilla `SonicBoom.java:79`:
/// `target.hurtServer(level, level.damageSources().sonicBoom(body), 10.0F)`
pub const SONIC_BOOM_DAMAGE: f32 = 10.0;

/// Maximum horizontal range (XZ distance) for Warden Sonic Boom.
///
/// Mirrors vanilla `SonicBoom.java:19`:
/// `private static final int DISTANCE_XZ = 15;`
pub const SONIC_BOOM_DISTANCE_XZ: f64 = 15.0;

/// Maximum vertical range (Y distance) for Warden Sonic Boom.
///
/// Mirrors vanilla `SonicBoom.java:20`:
/// `private static final int DISTANCE_Y = 20;`
pub const SONIC_BOOM_DISTANCE_Y: f64 = 20.0;

/// Base horizontal knockback applied by Warden Sonic Boom before resistance.
///
/// Mirrors vanilla `SonicBoom.java:22`:
/// `private static final double KNOCKBACK_HORIZONTAL = 2.5;`
pub const SONIC_BOOM_HORIZONTAL_KNOCKBACK: f64 = 2.5;

/// Base vertical knockback applied by Warden Sonic Boom before resistance.
///
/// Mirrors vanilla `SonicBoom.java:21`:
/// `private static final double KNOCKBACK_VERTICAL = 0.5;`
pub const SONIC_BOOM_VERTICAL_KNOCKBACK: f64 = 0.5;

/// Calculates horizontal and vertical knockback applied by Warden Sonic Boom accounting for target's knockback resistance.
///
/// Mirrors vanilla `SonicBoom.java:80-81`:
/// `double knockbackVertical = 0.5 * (1.0 - target.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE));`
/// `double knockbackHorizontal = 2.5 * (1.0 - target.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE));`
#[must_use]
pub fn calculate_sonic_boom_knockback(knockback_resistance: f64) -> (f64, f64) {
    let factor = (1.0 - knockback_resistance).max(0.0);
    (
        SONIC_BOOM_HORIZONTAL_KNOCKBACK * factor,
        SONIC_BOOM_VERTICAL_KNOCKBACK * factor,
    )
}

/// Checks whether a target position is within Warden Sonic Boom targeting cylinder range (15m horizontal, 20m vertical).
///
/// Mirrors vanilla `SonicBoom.java:44`:
/// `body.closerThan(target, 15.0, 20.0)`
#[must_use]
pub fn is_sonic_boom_in_range(dx: f64, dy: f64, dz: f64) -> bool {
    (dx * dx + dz * dz).sqrt() <= SONIC_BOOM_DISTANCE_XZ && dy.abs() <= SONIC_BOOM_DISTANCE_Y
}

/// Warden base max health (500.0 HP / 250 hearts).
/// Mirrors vanilla `Warden.java:80, 187` (`Attributes.MAX_HEALTH, 500.0`).
pub const WARDEN_MAX_HEALTH: f32 = 500.0;

/// Warden base movement speed when fighting (0.3).
/// Mirrors vanilla `Warden.java:81, 188` (`Attributes.MOVEMENT_SPEED, 0.3F`).
pub const WARDEN_MOVEMENT_SPEED_WHEN_FIGHTING: f64 = 0.3;

/// Warden knockback resistance (1.0 = 100% immune to normal knockback).
/// Mirrors vanilla `Warden.java:82, 189` (`Attributes.KNOCKBACK_RESISTANCE, 1.0`).
pub const WARDEN_KNOCKBACK_RESISTANCE: f64 = 1.0;

/// Warden attack knockback bonus (1.5).
/// Mirrors vanilla `Warden.java:83, 190` (`Attributes.ATTACK_KNOCKBACK, 1.5`).
pub const WARDEN_ATTACK_KNOCKBACK: f64 = 1.5;

/// Warden melee attack damage (30.0 HP / 15 hearts).
/// Mirrors vanilla `Warden.java:84, 191` (`Attributes.ATTACK_DAMAGE, 30.0`).
pub const WARDEN_ATTACK_DAMAGE: f32 = 30.0;

/// Warden player follow range (24.0 blocks).
/// Mirrors vanilla `Warden.java:85, 192` (`Attributes.FOLLOW_RANGE, 24.0`).
pub const WARDEN_FOLLOW_RANGE: f64 = 24.0;

/// Experience reward dropped by a Warden on death (5 XP).
/// Mirrors vanilla `Warden.java` (`xpReward = 5`).
pub const WARDEN_XP_REWARD: u32 = 5;

/// Cooldown ticks between vibration detections for Warden (40 ticks = 2.0s).
/// Mirrors vanilla `Warden.java:78` (`VIBRATION_COOLDOWN_TICKS = 40`).
pub const WARDEN_VIBRATION_COOLDOWN_TICKS: i32 = 40;

/// Ticks of melee combat before Warden considers using Sonic Boom (200 ticks = 10.0s).
/// Mirrors vanilla `Warden.java:79` (`TIME_TO_USE_MELEE_UNTIL_SONIC_BOOM = 200`).
pub const WARDEN_TIME_TO_USE_MELEE_UNTIL_SONIC_BOOM: i32 = 200;

/// Darkness effect display limit in ticks for Warden pulsing darkness (200 ticks = 10.0s).
/// Mirrors vanilla `Warden.java:87` (`DARKNESS_DISPLAY_LIMIT = 200`).
pub const WARDEN_DARKNESS_DISPLAY_LIMIT: i32 = 200;

/// Darkness effect duration applied to nearby players (260 ticks = 13.0s).
/// Mirrors vanilla `Warden.java:88` (`DARKNESS_DURATION = 260`).
pub const WARDEN_DARKNESS_DURATION: i32 = 260;

/// Darkness effect search and application radius (20.0 blocks).
/// Mirrors vanilla `Warden.java:89` (`DARKNESS_RADIUS = 20`).
pub const WARDEN_DARKNESS_RADIUS: f64 = 20.0;

/// Interval between Warden darkness pulses (120 ticks = 6.0s).
/// Mirrors vanilla `Warden.java:90` (`DARKNESS_INTERVAL = 120`).
pub const WARDEN_DARKNESS_INTERVAL: i32 = 120;

/// Interval between anger management decay ticks (20 ticks = 1.0s).
/// Mirrors vanilla `Warden.java:91` (`ANGERMANAGEMENT_TICK_DELAY = 20`).
pub const WARDEN_ANGERMANAGEMENT_TICK_DELAY: i32 = 20;

/// Default anger increase when touched or disturbed by an entity (35).
/// Mirrors vanilla `Warden.java:92` (`DEFAULT_ANGER = 35`).
pub const WARDEN_DEFAULT_ANGER: i32 = 35;

/// Anger increase from projectile impact (10).
/// Mirrors vanilla `Warden.java:93` (`PROJECTILE_ANGER = 10`).
pub const WARDEN_PROJECTILE_ANGER: i32 = 10;

/// Anger boost added to minimum angry threshold when Warden is hurt (20).
/// Mirrors vanilla `Warden.java:94` (`ON_HURT_ANGER_BOOST = 20`).
pub const WARDEN_ON_HURT_ANGER_BOOST: i32 = 20;

/// Maximum ticks threshold to remember recent projectile hits (100 ticks = 5.0s).
/// Mirrors vanilla `Warden.java:95` (`RECENT_PROJECTILE_TICK_THRESHOLD = 100`).
pub const WARDEN_RECENT_PROJECTILE_TICK_THRESHOLD: i32 = 100;

/// Cooldown between physical touch anger increments (20 ticks = 1.0s).
/// Mirrors vanilla `Warden.java:96` (`TOUCH_COOLDOWN_TICKS = 20`).
pub const WARDEN_TOUCH_COOLDOWN_TICKS: i32 = 20;

/// Maximum distance from which a projectile impact angers the Warden (30.0 blocks).
/// Mirrors vanilla `Warden.java:100` (`PROJECTILE_ANGER_DISTANCE = 30`).
pub const WARDEN_PROJECTILE_ANGER_DISTANCE: f64 = 30.0;

/// Total animation duration for Warden emerge animation (134 ticks = 6.7s).
/// Mirrors vanilla `WardenAi.java:87` (`EMERGE_DURATION = 134`).
pub const WARDEN_EMERGE_DURATION: i32 = 134;

/// Total animation duration for Warden sniffing (84 ticks = 4.2s).
/// Mirrors vanilla `WardenAi.java:88` (`SNIFFING_DURATION = 84`).
pub const WARDEN_SNIFFING_DURATION: i32 = 84;

/// Total animation duration for Warden roar (84 ticks = 4.2s).
/// Mirrors vanilla `WardenAi.java:89` (`ROAR_DURATION = 84`).
pub const WARDEN_ROAR_DURATION: i32 = 84;

/// Total animation duration for Warden digging into ground (100 ticks = 5.0s).
/// Mirrors vanilla `WardenAi.java:90` (`DIGGING_DURATION = 100`).
pub const WARDEN_DIGGING_DURATION: i32 = 100;

/// Shield disable duration in seconds when hit by Warden melee attack (5.0s).
/// Mirrors vanilla `Warden.java:492` (`return 5.0F;`).
pub const WARDEN_DISABLE_SHIELD_SECONDS: f32 = 5.0;

/// Maximum anger value in anger management tracking (150).
/// Mirrors vanilla `AngerManagement.java:34` (`MAX_ANGER = 150`).
pub const WARDEN_MAX_ANGER: i32 = 150;

/// Default anger decrease per decay cycle (1).
/// Mirrors vanilla `AngerManagement.java:35` (`DEFAULT_ANGER_DECREASE = 1`).
pub const WARDEN_DEFAULT_ANGER_DECREASE: i32 = 1;

/// Minimum anger threshold for Agitated state (40).
/// Mirrors vanilla `AngerLevel.java:12` (`AGITATED(40, ...)`).
pub const WARDEN_ANGER_AGITATED_MIN: i32 = 40;

/// Minimum anger threshold for Angry state (80).
/// Mirrors vanilla `AngerLevel.java:13` (`ANGRY(80, ...)`).
pub const WARDEN_ANGER_ANGRY_MIN: i32 = 80;

/// Cooldown ticks for Sonic Boom attack (40 ticks = 2.0s).
/// Mirrors vanilla `SonicBoom.java:23` (`COOLDOWN = 40`).
pub const SONIC_BOOM_COOLDOWN: i32 = 40;

/// Charge ticks before Sonic Boom sound and effect trigger (34 ticks = 1.7s).
/// Mirrors vanilla `SonicBoom.java:24` (`TICKS_BEFORE_PLAYING_SOUND = 34`).
pub const SONIC_BOOM_CHARGE_DURATION: i32 = 34;

/// Total duration of Sonic Boom behavior (60 ticks = 3.0s).
/// Mirrors vanilla `SonicBoom.java:25` (`DURATION = 60`).
pub const SONIC_BOOM_TOTAL_DURATION: i32 = 60;

/// Calculates heartbeat delay in ticks based on Warden client anger level (40 ticks down to 10 ticks).
///
/// Mirrors vanilla `Warden.java:319-322`:
/// ```java
/// float anger = (float)this.getClientAngerLevel() / AngerLevel.ANGRY.getMinimumAnger();
/// return 40 - Mth.floor(Mth.clamp(anger, 0.0F, 1.0F) * 30.0F);
/// ```
#[must_use]
pub fn calculate_warden_heartbeat_delay(client_anger: i32) -> i32 {
    let ratio = (client_anger as f32 / WARDEN_ANGER_ANGRY_MIN as f32).clamp(0.0, 1.0);
    40 - (ratio * 30.0).floor() as i32
}

/// Maximum health for Ender Dragon (200.0 HP = 100 hearts).
///
/// Mirrors vanilla `EnderDragon.java:840`:
/// `.add(Attributes.MAX_HEALTH, 200.0)`
pub const ENDER_DRAGON_MAX_HEALTH: f32 = 200.0;

/// Mob attack damage dealt by Ender Dragon head/neck direct collision (10.0 HP).
///
/// Mirrors vanilla `EnderDragon.java:389`:
/// `target.hurtServer(level, damageSource, 10.0F);`
pub const ENDER_DRAGON_HEAD_COLLISION_DAMAGE: f32 = 10.0;

/// Attack damage dealt by Ender Dragon wing sweep (5.0 HP).
///
/// Mirrors vanilla `EnderDragon.java:379`:
/// `entity.hurtServer(serverLevel, damageSource, 5.0F);`
pub const ENDER_DRAGON_WING_DAMAGE: f32 = 5.0;

/// Damage dealt to Ender Dragon when a healing End Crystal is destroyed (10.0 HP explosion).
///
/// Mirrors vanilla `EnderDragon.java:819`:
/// `this.hurt(level, this.head, this.damageSources().explosion(crystal, player), 10.0F);`
pub const ENDER_DRAGON_CRYSTAL_EXPLOSION_DAMAGE: f32 = 10.0;

/// Interval in ticks between Ender Dragon healing from an active End Crystal (10 ticks = 0.5s).
///
/// Mirrors vanilla `EnderDragon.java:365`:
/// `else if (this.tickCount % 10 == 0 && this.getHealth() < this.getMaxHealth())`
pub const ENDER_DRAGON_CRYSTAL_HEAL_INTERVAL_TICKS: u64 = 10;

/// Allowed damage percentage the Ender Dragon can take while sitting before taking off (25% = 50 HP).
///
/// Mirrors vanilla `EnderDragon.java:61, 458-460`:
/// `private static final float SITTING_ALLOWED_DAMAGE_PERCENTAGE = 0.25F;`
/// `if (this.sittingDamageReceived > 0.25F * this.getMaxHealth())`
pub const ENDER_DRAGON_SITTING_DAMAGE_TAKEOFF_THRESHOLD: f32 = 50.0;

/// Total death animation duration for Ender Dragon before removal (200 ticks = 10s).
///
/// Mirrors vanilla `EnderDragon.java:547`:
/// `if (this.dragonDeathTime >= 200)`
pub const ENDER_DRAGON_DEATH_MAX_TICKS: i32 = 200;

/// Experience awarded for the first dragon kill in a world.
///
/// Mirrors vanilla `EnderDragon.java:525`:
/// `xpCount = 12000;`
pub const ENDER_DRAGON_FIRST_KILL_XP: u32 = 12000;

/// Experience awarded for subsequent dragon kills in a world.
///
/// Mirrors vanilla `EnderDragon.java:522`:
/// `int xpCount = 500;`
pub const ENDER_DRAGON_RESPAWN_KILL_XP: u32 = 500;

/// Calculates damage dealt to the Ender Dragon based on which body part was struck.
///
/// Head (part 0) and Neck (part 1) take 100% full damage.
/// All other parts (body, wings, tails) take reduced damage: `damage / 4.0 + min(damage, 1.0)`.
/// If reduced damage is less than 0.01, it is negated (returns 0.0).
///
/// Mirrors vanilla `EnderDragon.java:446-452`:
/// ```java
/// if (part != this.head && part != this.neck) {
///     damage = damage / 4.0F + Math.min(damage, 1.0F);
/// }
/// if (damage < 0.01F) {
///     return false;
/// }
/// ```
#[must_use]
pub fn calculate_dragon_part_damage(part_index: usize, incoming_damage: f32) -> f32 {
    let damage = if part_index == 0 || part_index == 1 {
        incoming_damage
    } else {
        incoming_damage / 4.0 + incoming_damage.min(1.0)
    };

    if damage < 0.01 { 0.0 } else { damage }
}

/// Checks whether the Ender Dragon should heal 1.0 HP from its nearest crystal on this tick.
///
/// Mirrors vanilla `EnderDragon.java:365`:
/// `else if (this.tickCount % 10 == 0 && this.getHealth() < this.getMaxHealth()) { this.setHealth(this.getHealth() + 1.0F); }`
#[must_use]
pub const fn should_dragon_crystal_heal(tick_count: u64, current_health: f32, max_health: f32) -> bool {
    tick_count % ENDER_DRAGON_CRYSTAL_HEAL_INTERVAL_TICKS == 0 && current_health < max_health
}

/// Calculates the knockback push vector applied by the Ender Dragon's wing sweep.
///
/// Mirrors vanilla `EnderDragon.java:372-376`:
/// ```java
/// double xd = entity.getX() - xm;
/// double zd = entity.getZ() - zm;
/// double dd = Math.max(xd * xd + zd * zd, 0.1);
/// entity.push(xd / dd * 4.0, 0.2F, zd / dd * 4.0);
/// ```
#[must_use]
pub fn calculate_dragon_wing_knockback(player_pos: Vector3<f64>, body_pos: Vector3<f64>) -> Vector3<f64> {
    let xd = player_pos.x - body_pos.x;
    let zd = player_pos.z - body_pos.z;
    let dd = (xd * xd + zd * zd).max(0.1);
    Vector3::new(xd / dd * 4.0, 0.2, zd / dd * 4.0)
}

/// Checks whether the dragon accumulated enough damage while sitting to trigger takeoff.
///
/// Mirrors vanilla `EnderDragon.java:458-460`:
/// `if (this.sittingDamageReceived > 0.25F * this.getMaxHealth())`
#[must_use]
pub const fn should_dragon_takeoff_from_sitting_damage(sitting_damage_received: f32) -> bool {
    sitting_damage_received > ENDER_DRAGON_SITTING_DAMAGE_TAKEOFF_THRESHOLD
}

/// Calculates the experience orb drop quantity during dragon death animation.
///
/// Mirrors vanilla `EnderDragon.java:528-550`:
/// - Between tick 151 and 199, every 5 ticks: 8% of total XP.
/// - At tick 200: 20% of total XP.
#[must_use]
pub fn calculate_dragon_death_xp(death_ticks: i32, has_previously_killed: bool) -> u32 {
    let total_xp = if has_previously_killed {
        ENDER_DRAGON_RESPAWN_KILL_XP
    } else {
        ENDER_DRAGON_FIRST_KILL_XP
    };

    if death_ticks >= ENDER_DRAGON_DEATH_MAX_TICKS {
        (total_xp as f32 * 0.2) as u32
    } else if death_ticks > 150 && death_ticks % 5 == 0 {
        (total_xp as f32 * 0.08) as u32
    } else {
        0
    }
}

/// Direct hit damage of Wither Skull when shot by a living entity (Wither Boss / Player / Mob).
///
/// Mirrors vanilla `WitherSkull.java:64`:
/// `wasHurt = var8.hurtServer(serverLevel, damageSource, 8.0F);`
pub const WITHER_SKULL_LIVING_DAMAGE: f32 = 8.0;

/// Direct hit damage of Wither Skull when not shot by a living entity (dispenser, etc.).
///
/// Mirrors vanilla `WitherSkull.java:73`:
/// `wasHurt = var8.hurtServer(serverLevel, this.damageSources().magic(), 5.0F);`
pub const WITHER_SKULL_MAGIC_DAMAGE: f32 = 5.0;

/// Maximum blast resistance for destroyable blocks against a dangerous (blue) Wither Skull.
///
/// Mirrors vanilla `WitherSkull.java:49`:
/// `return this.isDangerous() && WitherBoss.canDestroy(block) ? Math.min(0.8F, resistance) : resistance;`
pub const WITHER_SKULL_DANGEROUS_MAX_RESISTANCE: f32 = 0.8;

/// Calculates the direct damage dealt by a Wither Skull depending on whether it has a living owner.
///
/// Mirrors vanilla `WitherSkull.java:62-74`.
#[must_use]
pub const fn calculate_wither_skull_damage(has_living_owner: bool) -> f32 {
    if has_living_owner {
        WITHER_SKULL_LIVING_DAMAGE
    } else {
        WITHER_SKULL_MAGIC_DAMAGE
    }
}

/// Calculates effective explosion resistance of a block against a Wither Skull.
///
/// Mirrors vanilla `WitherSkull.java:49`.
#[must_use]
pub fn calculate_wither_skull_block_resistance(is_dangerous: bool, can_destroy: bool, default_resistance: f32) -> f32 {
    if is_dangerous && can_destroy {
        default_resistance.min(WITHER_SKULL_DANGEROUS_MAX_RESISTANCE)
    } else {
        default_resistance
    }
}

/// Direct entity hit damage for a regular player/dispenser Wind Charge (1.0 HP).
///
/// Mirrors vanilla `AbstractWindCharge.java:73`:
/// `if (entity.hurtServer(serverLevel, source, 1.0F))`
pub const WIND_CHARGE_DIRECT_DAMAGE: f32 = 1.0;

/// Explosion power / radius for a player thrown Wind Charge (1.2F).
///
/// Mirrors vanilla `WindCharge.java:23, 62`:
/// `private static final float RADIUS = 1.2F;`
pub const WIND_CHARGE_EXPLOSION_POWER: f32 = 1.2;

/// Explosion power / radius for a Breeze thrown Wind Charge (3.0F).
///
/// Mirrors vanilla `BreezeWindCharge.java:15, 30`:
/// `private static final float RADIUS = 3.0F;`
pub const BREEZE_WIND_CHARGE_EXPLOSION_POWER: f32 = 3.0;

/// Default deflect cooldown ticks for player Wind Charge (5 ticks).
///
/// Mirrors vanilla `WindCharge.java:25`:
/// `private int noDeflectTicks = 5;`
pub const WIND_CHARGE_DEFAULT_DEFLECT_COOLDOWN: u8 = 5;

/// Normal offset scale for block hit explosion on wind charges (0.25).
///
/// Mirrors vanilla `AbstractWindCharge.java:90`:
/// `Vec3 scaledNormal = Vec3.atLowerCornerOf(collisionNormal).multiply(0.25, 0.25, 0.25);`
pub const WIND_CHARGE_BLOCK_HIT_NORMAL_OFFSET: f64 = 0.25;

/// Direct entity hit damage for Large Fireball (6.0 HP).
///
/// Mirrors vanilla `LargeFireball.java:48`:
/// `var6.hurtServer(serverLevel, damageSource, 6.0F);`
pub const LARGE_FIREBALL_DIRECT_DAMAGE: f32 = 6.0;

/// Direct entity hit damage for Small Fireball (5.0 HP).
///
/// Mirrors vanilla `SmallFireball.java:43`:
/// `if (!var7.hurtServer(serverLevel, damageSource, 5.0F))`
pub const SMALL_FIREBALL_DIRECT_DAMAGE: f32 = 5.0;

/// Ignite duration in seconds when hit by Small Fireball (5.0s = 100 ticks).
///
/// Mirrors vanilla `SmallFireball.java:41`:
/// `var7.igniteForSeconds(5.0F);`
pub const SMALL_FIREBALL_IGNITE_SECONDS: f32 = 5.0;
pub const SMALL_FIREBALL_IGNITE_TICKS: u32 = 100;

/// Maximum health for Vex entity (14.0 HP).
///
/// Mirrors vanilla Vex.java:82:
/// .add(Attributes.MAX_HEALTH, 14.0)
pub const VEX_DEFAULT_HEALTH: f32 = 14.0;

/// Base attack damage for Vex entity (4.0 HP).
///
/// Mirrors vanilla Vex.java:82:
/// .add(Attributes.ATTACK_DAMAGE, 4.0)
pub const VEX_ATTACK_DAMAGE: f32 = 4.0;

/// Experience reward dropped by a Vex on death (3 XP).
///
/// Mirrors vanilla Vex.java:49:
/// 	his.xpReward = 3;
pub const VEX_XP_REWARD: u32 = 3;

/// Wing flap rotation speed in degrees per tick for Vex (45.836624 deg/tick).
///
/// Mirrors vanilla Vex.java:40:
/// public static final float FLAP_DEGREES_PER_TICK = 45.836624F;
pub const VEX_FLAP_DEGREES_PER_TICK: f32 = 45.836_624;

/// Ticks per wing flap cycle for Vex (4 ticks).
///
/// Mirrors vanilla Vex.java:41:
/// public static final int TICKS_PER_FLAP = Mth.ceil((float) (Math.PI * 5.0 / 4.0));
pub const VEX_TICKS_PER_FLAP: u64 = 4;

/// Interval in ticks between starve damage applications when Vex exceeds limited lifetime (20 ticks = 1s).
///
/// Mirrors vanilla Vex.java:65:
/// 	his.limitedLifeTicks = 20; this.hurt(this.damageSources().starve(), 1.0F);
pub const VEX_DECAY_DAMAGE_INTERVAL_TICKS: i32 = 20;

/// Evoker base max health (24.0 HP / 12 hearts).
/// Mirrors vanilla `Evoker.java:73` (`Attributes.MAX_HEALTH, 24.0`).
pub const EVOKER_MAX_HEALTH: f32 = 24.0;

/// Evoker base movement speed (0.5).
/// Mirrors vanilla `Evoker.java:73` (`Attributes.MOVEMENT_SPEED, 0.5`).
pub const EVOKER_BASE_MOVEMENT_SPEED: f64 = 0.5;

/// Evoker follow range (12.0 blocks).
/// Mirrors vanilla `Evoker.java:73` (`Attributes.FOLLOW_RANGE, 12.0`).
pub const EVOKER_FOLLOW_RANGE: f64 = 12.0;

/// Experience reward dropped by an Evoker on death (10 XP).
///
/// Mirrors vanilla Evoker.java:50:
/// this.xpReward = 10;
pub const EVOKER_XP_REWARD: u32 = 10;

/// Evoker fangs spell warmup ticks (20 ticks = 1.0s).
/// Mirrors vanilla `SpellcasterIllager.java:220` (`return 20;`).
pub const EVOKER_FANGS_CAST_WARMUP: i32 = 20;

/// Evoker fangs spell total casting time (40 ticks = 2.0s).
/// Mirrors vanilla `Evoker.java:136` (`return 40;`).
pub const EVOKER_FANGS_CASTING_TIME: i32 = 40;

/// Evoker fangs spell cooldown interval (100 ticks = 5.0s).
/// Mirrors vanilla `Evoker.java:141` (`return 100;`).
pub const EVOKER_FANGS_CASTING_INTERVAL: i32 = 100;

/// Maximum squared distance for concentric ring fangs attack (9.0 sq blocks = 3.0 blocks).
/// Mirrors vanilla `Evoker.java:150` (`distanceToSqr(target) < 9.0`).
pub const EVOKER_FANGS_RING_RANGE_SQ: f64 = 9.0;

/// Number of fangs summoned in a straight line attack (16 fangs).
/// Mirrors vanilla `Evoker.java:161` (`for (int i = 0; i < 16; i++)`).
pub const EVOKER_FANGS_LINE_COUNT: usize = 16;

/// Evoker summon vex spell warmup ticks (20 ticks = 1.0s).
/// Mirrors vanilla `SpellcasterIllager.java:220` (`return 20;`).
pub const EVOKER_SUMMON_CAST_WARMUP: i32 = 20;

/// Evoker summon vex spell total casting time (100 ticks = 5.0s).
/// Mirrors vanilla `Evoker.java:245` (`return 100;`).
pub const EVOKER_SUMMON_CASTING_TIME: i32 = 100;

/// Evoker summon vex spell cooldown interval (340 ticks = 17.0s).
/// Mirrors vanilla `Evoker.java:250` (`return 340;`).
pub const EVOKER_SUMMON_CASTING_INTERVAL: i32 = 340;

/// Number of Vexes summoned per cast by an Evoker (3).
/// Mirrors vanilla `Evoker.java:258` (`for (int i = 0; i < 3; i++)`).
pub const EVOKER_SUMMON_VEX_COUNT: usize = 3;

/// Evoker Wololo spell warmup ticks (40 ticks = 2.0s).
/// Mirrors vanilla `Evoker.java:342` (`return 40;`).
pub const EVOKER_WOLOLO_CAST_WARMUP: i32 = 40;

/// Evoker Wololo spell total casting time (60 ticks = 3.0s).
/// Mirrors vanilla `Evoker.java:347` (`return 60;`).
pub const EVOKER_WOLOLO_CASTING_TIME: i32 = 60;

/// Evoker Wololo spell cooldown interval (140 ticks = 7.0s).
/// Mirrors vanilla `Evoker.java:352` (`return 140;`).
pub const EVOKER_WOLOLO_CASTING_INTERVAL: i32 = 140;

/// Horizontal search radius for blue sheep target during Wololo spell (16.0 blocks).
/// Mirrors vanilla `Evoker.java:312` (`inflate(16.0, 4.0, 16.0)`).
pub const EVOKER_WOLOLO_SEARCH_RADIUS_XZ: f64 = 16.0;

/// Vertical search radius for blue sheep target during Wololo spell (4.0 blocks).
/// Mirrors vanilla `Evoker.java:312` (`inflate(16.0, 4.0, 16.0)`).
pub const EVOKER_WOLOLO_SEARCH_RADIUS_Y: f64 = 4.0;

/// Calculates total limited lifetime ticks for a Vex summoned by an Evoker.
///
/// Mirrors vanilla Evoker.java:266:
/// ex.setLimitedLife(20 * (30 + Evoker.this.random.nextInt(90)));
#[must_use]
pub const fn calculate_vex_summon_lifetime(rand_0_to_89: i32) -> i32 {
    20 * (30 + rand_0_to_89)
}

/// Hoglin combat and attribute constants, mirroring vanilla `Hoglin.java:95-101`, `HoglinBase.java:12-13`.
pub const HOGLIN_MAX_HEALTH: f64 = 40.0;
pub const HOGLIN_MOVEMENT_SPEED: f64 = 0.3;
pub const HOGLIN_KNOCKBACK_RESISTANCE: f64 = 0.6;
pub const HOGLIN_ATTACK_KNOCKBACK: f64 = 1.0;
pub const HOGLIN_ATTACK_DAMAGE: f64 = 6.0;
pub const HOGLIN_BABY_ATTACK_DAMAGE: f64 = 0.5;
pub const HOGLIN_CONVERSION_TIME_TICKS: i32 = 300;
pub const HOGLIN_XP_REWARD: u32 = 5;

/// Zoglin combat and attribute constants, mirroring vanilla `Zoglin.java:64-70`.
pub const ZOGLIN_MAX_HEALTH: f64 = 40.0;
pub const ZOGLIN_MOVEMENT_SPEED: f64 = 0.3;
pub const ZOGLIN_KNOCKBACK_RESISTANCE: f64 = 0.6;
pub const ZOGLIN_ATTACK_KNOCKBACK: f64 = 1.0;
pub const ZOGLIN_ATTACK_DAMAGE: f64 = 6.0;
pub const ZOGLIN_BABY_ATTACK_DAMAGE: f64 = 0.5;
pub const ZOGLIN_XP_REWARD: u32 = 5;

/// Adult Zoglin melee attack interval in ticks (40 ticks = 2 seconds).
/// Mirrors vanilla `Zoglin.java:71, 127`:
/// `private static final int ATTACK_INTERVAL = 40;`
pub const ZOGLIN_ADULT_ATTACK_INTERVAL: i32 = 40;

/// Baby Zoglin melee attack interval in ticks (15 ticks = 0.75 seconds).
/// Mirrors vanilla `Zoglin.java:72, 128`:
/// `private static final int BABY_ATTACK_INTERVAL = 15;`
pub const ZOGLIN_BABY_ATTACK_INTERVAL: i32 = 15;

/// Zoglin attack duration / expiry in ticks (200 ticks = 10 seconds).
/// Mirrors vanilla `Zoglin.java:73, 220`:
/// `private static final int ATTACK_DURATION = 200;`
pub const ZOGLIN_ATTACK_DURATION: i32 = 200;

/// Hoglin attack animation duration in ticks (10 ticks = 0.5 seconds).
/// Mirrors vanilla `HoglinBase.java:12`:
/// `int ATTACK_ANIMATION_DURATION = 10;`
pub const HOGLIN_ATTACK_ANIMATION_DURATION: i32 = 10;

/// Hoglin baby spawn probability (0.2 = 20%).
/// Mirrors vanilla `HoglinBase.java:13`:
/// `float PROBABILITY_OF_SPAWNING_AS_BABY = 0.2F;`
pub const HOGLIN_PROBABILITY_OF_SPAWNING_AS_BABY: f32 = 0.2;


/// Maximum number of baby piglins riding one adult hoglin (3).
/// Mirrors vanilla `Piglin.java:76`:
/// `private static final int MAX_PASSENGERS_ON_ONE_HOGLIN = 3;`
pub const PIGLIN_MAX_PASSENGERS_ON_ONE_HOGLIN: i32 = 3;

/// Calculates Zoglin attack interval based on baby status.
/// Mirrors vanilla `Zoglin.java:127-128`:
#[must_use]
pub const fn calculate_zoglin_attack_interval(is_baby: bool) -> i32 {
    if is_baby {
        ZOGLIN_BABY_ATTACK_INTERVAL
    } else {
        ZOGLIN_ADULT_ATTACK_INTERVAL
    }
}

/// Piglin Brute combat and attribute constants, mirroring vanilla `PiglinBrute.java:46, 50-55`.
pub const PIGLIN_BRUTE_MAX_HEALTH: f64 = 50.0;
pub const PIGLIN_BRUTE_MOVEMENT_SPEED: f64 = 0.35;
pub const PIGLIN_BRUTE_ATTACK_DAMAGE: f64 = 7.0;
pub const PIGLIN_BRUTE_FOLLOW_RANGE: f64 = 12.0;
pub const PIGLIN_BRUTE_XP_REWARD: u32 = 20;

/// Piglin combat and attribute constants, mirroring vanilla `Piglin.java:72-74`, `PiglinAi.java:86, 799`.
pub const PIGLIN_MAX_HEALTH: f64 = 16.0;
pub const PIGLIN_MOVEMENT_SPEED: f64 = 0.35;
pub const PIGLIN_ATTACK_DAMAGE: f64 = 5.0;
pub const PIGLIN_XP_REWARD: u32 = 5;
pub const PIGLIN_CONVERSION_TIME_TICKS: i32 = 300;
pub const PIGLIN_CONVERSION_NAUSEA_DURATION_TICKS: i32 = 200;
pub const PIGLIN_ADMIRE_DURATION_TICKS: i32 = 119;

/// Baby Piglin speed modifier identifier and amount, mirroring vanilla `Piglin.java:68-70`.
/// AttributeModifier.Operation.ADD_MULTIPLIED_BASE (+20% base speed).
pub const PIGLIN_BABY_SPEED_MODIFIER_ID: &str = "minecraft:baby";
pub const PIGLIN_BABY_SPEED_MODIFIER_AMOUNT: f64 = 0.2;

/// Piglin baby spawn probability (0.2 = 20%).
/// Mirrors vanilla `Piglin.java:77`:
/// `private static final float PROBABILITY_OF_SPAWNING_AS_BABY = 0.2F;`
pub const PIGLIN_PROBABILITY_OF_SPAWNING_AS_BABY: f32 = 0.2;

/// Piglin armor item wear chance (0.1 = 10% per slot).
/// Mirrors vanilla `Piglin.java:75`:
/// `private static final float CHANCE_OF_WEARING_EACH_ARMOUR_ITEM = 0.1F;`
pub const PIGLIN_CHANCE_OF_WEARING_EACH_ARMOUR_ITEM: f32 = 0.1;

/// Piglin crossbow vs melee spawn probability (0.5 = 50%).
/// Mirrors vanilla `Piglin.java:81`:
/// `private static final double PROBABILITY_OF_SPAWNING_WITH_CROSSBOW_INSTEAD_OF_SWORD = 0.5;`
pub const PIGLIN_PROBABILITY_OF_SPAWNING_WITH_CROSSBOW: f64 = 0.5;

/// Piglin crossbow projectile shooting velocity power (1.6).
/// Mirrors vanilla `Piglin.java:345`:
/// `this.performCrossbowAttack(this, 1.6F);`
pub const PIGLIN_CROSSBOW_ATTACK_SPEED: f32 = 1.6;

/// Piglin golden spear spawn chance (1 in 10 chance when not spawning with crossbow).
/// Mirrors vanilla `Piglin.java:288`:
/// `this.random.nextInt(10) == 0 ? Items.GOLDEN_SPEAR : Items.GOLDEN_SWORD`
pub const PIGLIN_SPEAR_SPAWN_CHANCE_IN_TEN: u32 = 10;

/// Zombified Piglin max health (20.0 HP).
/// Mirrors vanilla `ZombifiedPiglin.java` / `Zombie.java`:
pub const ZOMBIFIED_PIGLIN_MAX_HEALTH: f64 = 20.0;

/// Zombified Piglin attack damage (5.0 HP).
/// Mirrors vanilla `ZombifiedPiglin.java:84`:
/// `.add(Attributes.ATTACK_DAMAGE, 5.0)`
pub const ZOMBIFIED_PIGLIN_ATTACK_DAMAGE: f64 = 5.0;

/// Zombified Piglin natural armor (2.0).
/// Mirrors vanilla `ZombifiedPiglin.java` / `Zombie.java:162`:
pub const ZOMBIFIED_PIGLIN_ARMOR: f64 = 2.0;

/// Zombified Piglin follow range (35.0 blocks).
/// Mirrors vanilla `ZombifiedPiglin.java` / `Zombie.java:160`:
pub const ZOMBIFIED_PIGLIN_FOLLOW_RANGE: f64 = 35.0;

/// Zombified Piglin spawn reinforcement chance (0.0).
/// Mirrors vanilla `ZombifiedPiglin.java:82`:
/// `.add(Attributes.SPAWN_REINFORCEMENTS_CHANCE, 0.0)`
pub const ZOMBIFIED_PIGLIN_SPAWN_REINFORCEMENTS_CHANCE: f64 = 0.0;

/// Zombified Piglin alert vertical box radius (10 blocks).
/// Mirrors vanilla `ZombifiedPiglin.java:61`:
/// `private static final int ALERT_RANGE_Y = 10;`
pub const ZOMBIFIED_PIGLIN_ALERT_RANGE_Y: f64 = 10.0;

/// Zombified Piglin alert interval in ticks (80 to 120 ticks = 4 to 6 seconds).
/// Mirrors vanilla `ZombifiedPiglin.java:62`:
/// `private static final UniformInt ALERT_INTERVAL = TimeUtil.rangeOfSeconds(4, 6);`
pub const ZOMBIFIED_PIGLIN_ALERT_INTERVAL_MIN_TICKS: i32 = 80;
pub const ZOMBIFIED_PIGLIN_ALERT_INTERVAL_MAX_TICKS: i32 = 120;

/// Zombified Piglin golden spear spawn chance (1 in 20).
/// Mirrors vanilla `ZombifiedPiglin.java:225`:
/// `random.nextInt(20) == 0 ? Items.GOLDEN_SPEAR : Items.GOLDEN_SWORD`
pub const ZOMBIFIED_PIGLIN_SPEAR_SPAWN_CHANCE_IN_TWENTY: u32 = 20;

/// Zombified Piglin lava pathfinding malus (8.0).
/// Mirrors vanilla `ZombifiedPiglin.java:67`:
/// `this.setPathfindingMalus(PathType.LAVA, 8.0F);`
pub const ZOMBIFIED_PIGLIN_LAVA_PATHFINDING_MALUS: f32 = 8.0;

/// Calculates experience reward for Piglin based on baby status.
/// Mirrors vanilla `LivingEntity.java:1533` & `shouldDropExperience()`:
/// Babies do not drop experience (0 XP), adults drop 5 XP.
#[must_use]
pub const fn calculate_piglin_experience_reward(is_baby: bool) -> u32 {
    if is_baby {
        0
    } else {
        PIGLIN_XP_REWARD
    }
}

/// Zombified Piglin angry speed modifier identifier and amount, mirroring vanilla `ZombifiedPiglin.java:52-54`.
/// AttributeModifier.Operation.ADD_VALUE (+0.05 speed).
pub const ZOMBIFIED_PIGLIN_ATTACKING_SPEED_MODIFIER_ID: &str = "minecraft:attacking";
pub const ZOMBIFIED_PIGLIN_ATTACKING_SPEED_MODIFIER_AMOUNT: f64 = 0.05;
pub const ZOMBIFIED_PIGLIN_BASE_SPEED: f64 = 0.23;
pub const ZOMBIFIED_PIGLIN_XP_REWARD: u32 = 5;

/// Zombified Piglin first anger sound delay range (0 to 1 second = 0 to 20 ticks).
/// Mirrors vanilla `ZombifiedPiglin.java:56`:
/// `private static final UniformInt FIRST_ANGER_SOUND_DELAY = TimeUtil.rangeOfSeconds(0, 1);`
pub const ZOMBIFIED_PIGLIN_FIRST_ANGER_SOUND_DELAY_MAX_TICKS: i32 = 20;

/// Zombified Piglin persistent anger time minimum in ticks (20 seconds = 400 ticks).
/// Mirrors vanilla `ZombifiedPiglin.java:58`:
/// `private static final UniformInt PERSISTENT_ANGER_TIME = TimeUtil.rangeOfSeconds(20, 39);`
pub const ZOMBIFIED_PIGLIN_PERSISTENT_ANGER_MIN_TICKS: i32 = 400;

/// Zombified Piglin persistent anger time maximum in ticks (39 seconds = 780 ticks).
/// Mirrors vanilla `ZombifiedPiglin.java:58`.
pub const ZOMBIFIED_PIGLIN_PERSISTENT_ANGER_MAX_TICKS: i32 = 780;

/// Calculates Hoglin / Zoglin attack damage with vanilla random offset,
/// mirroring vanilla `HoglinBase.java:18-25`:
/// ```text
/// if (!body.isBaby() && (int)attackDamage > 0) {
///     actualDamage = attackDamage / 2.0F + level.getRandom().nextInt((int)attackDamage);
/// } else {
///     actualDamage = attackDamage;
/// }
/// ```
#[must_use]
pub fn calculate_hoglin_attack_damage(base_attack_damage: f32, is_baby: bool, rand_int: u32) -> f32 {
    let int_damage = base_attack_damage as u32;
    if !is_baby && int_damage > 0 {
        base_attack_damage * 0.5 + (rand_int % int_damage) as f32
    } else {
        base_attack_damage
    }
}

/// Calculates Hoglin / Zoglin throw impulse vector, mirroring vanilla `HoglinBase.java:38-53` and `Vec3.java:245-251`:
/// ```text
/// double knockbackPower = body.getAttributeValue(Attributes.ATTACK_KNOCKBACK);
/// double knockbackResistance = target.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE);
/// double effectiveKnockbackPower = knockbackPower - knockbackResistance;
/// if (!(effectiveKnockbackPower <= 0.0)) {
///     double xd = target.getX() - body.getX();
///     double zd = target.getZ() - body.getZ();
///     RandomSource random = body.level().getRandom();
///     float horizontalPushAngle = random.nextInt(21) - 10;
///     double horizontalScale = effectiveKnockbackPower * (random.nextFloat() * 0.5F + 0.2F);
///     Vec3 horizontalPushVector = new Vec3(xd, 0.0, zd).normalize().scale(horizontalScale).yRot(horizontalPushAngle);
///     double verticalScale = effectiveKnockbackPower * random.nextFloat() * 0.5;
///     target.push(horizontalPushVector.x, verticalScale, horizontalPushVector.z);
/// }
/// ```
#[must_use]
pub fn calculate_hoglin_throw_impulse(
    dx: f64,
    dz: f64,
    knockback_power: f64,
    knockback_resistance: f64,
    rand_angle_rad: f32,
    rand_h: f32,
    rand_v: f64,
) -> Option<Vector3<f64>> {
    let effective_kb = knockback_power - knockback_resistance;
    if effective_kb <= 0.0 {
        return None;
    }

    let dist = dx.hypot(dz).max(1e-4);
    let horizontal_scale = effective_kb * (f64::from(rand_h) * 0.5 + 0.2);
    let norm_x = (dx / dist) * horizontal_scale;
    let norm_z = (dz / dist) * horizontal_scale;

    let cos = f64::from(rand_angle_rad.cos());
    let sin = f64::from(rand_angle_rad.sin());

    let vx = norm_x * cos + norm_z * sin;
    let vz = norm_z * cos - norm_x * sin;
    let vy = effective_kb * rand_v * 0.5;

    Some(Vector3::new(vx, vy, vz))
}

/// Creates the Nausea status effect applied upon conversion from Piglin/Hoglin,
/// mirroring vanilla `AbstractPiglin.java:113` and `Hoglin.java:254`:
/// `zombified.addEffect(new MobEffectInstance(MobEffects.NAUSEA, 200, 0))`
#[must_use]
pub fn create_piglin_conversion_effect() -> pumpkin_data::potion::Effect {
    pumpkin_data::potion::Effect {
        effect_type: &pumpkin_data::effect::StatusEffect::NAUSEA,
        duration: PIGLIN_CONVERSION_NAUSEA_DURATION_TICKS,
        amplifier: 0,
        ambient: false,
        show_particles: true,
        show_icon: true,
        blend: false,
    }
}

/// Calculates Zombified Piglin movement speed based on anger and baby status,
/// mirroring vanilla `ZombifiedPiglin.java:52-54, 99-108`:
/// - Base speed: 0.23
/// - When angry and not baby: +0.05 (`ADD_VALUE`) => 0.28
#[must_use]
pub fn calculate_zombified_piglin_speed(base_speed: f64, is_angry: bool, is_baby: bool) -> f64 {
    if is_angry && !is_baby {
        base_speed + ZOMBIFIED_PIGLIN_ATTACKING_SPEED_MODIFIER_AMOUNT
    } else {
        base_speed
    }
}

/// Calculates Piglin movement speed based on baby status,
/// mirroring vanilla `Piglin.java:68-70, 242-244`:
/// - Base speed: 0.35
/// - When baby: +20% (`ADD_MULTIPLIED_BASE`) => 0.42
#[must_use]
pub fn calculate_piglin_baby_speed(base_speed: f64, is_baby: bool) -> f64 {
    if is_baby {
        base_speed * (1.0 + PIGLIN_BABY_SPEED_MODIFIER_AMOUNT)
    } else {
        base_speed
    }
}

/// Pillager combat and attribute constants, mirroring vanilla `Pillager.java:84-90`.
pub const PILLAGER_MAX_HEALTH: f64 = 24.0;
pub const PILLAGER_MOVEMENT_SPEED: f64 = 0.35;
pub const PILLAGER_ATTACK_DAMAGE: f64 = 5.0;
pub const PILLAGER_FOLLOW_RANGE: f64 = 32.0;
pub const PILLAGER_XP_REWARD: u32 = 5;

/// Vindicator combat and attribute constants, mirroring vanilla `Vindicator.java:51, 89-95`.
pub const VINDICATOR_MAX_HEALTH: f64 = 24.0;
pub const VINDICATOR_MOVEMENT_SPEED: f64 = 0.35;
pub const VINDICATOR_ATTACK_DAMAGE: f64 = 5.0;
pub const VINDICATOR_FOLLOW_RANGE: f64 = 12.0;
pub const VINDICATOR_XP_REWARD: u32 = 5;
pub const VINDICATOR_JOHNNY_TAG: &str = "Johnny";

/// Maximum door breaking duration for Vindicator during raids (6).
/// Mirrors vanilla `Vindicator.java:184` (`super(mob, 6, ...)`).
pub const VINDICATOR_BREAK_DOOR_TIME: i32 = 6;

/// Random tick delay check for Vindicator door breaking (10).
/// Mirrors vanilla `Vindicator.java:197` (`random.nextInt(reducedTickDelay(10)) == 0`).
pub const VINDICATOR_BREAK_DOOR_CHANCE_INTERVAL: i32 = 10;

/// Illusioner combat and attribute constants, mirroring vanilla `Illusioner.java:45-47, 53, 80-82, 238, 260, 270`.
pub const ILLUSIONER_MAX_HEALTH: f64 = 32.0;
pub const ILLUSIONER_MOVEMENT_SPEED: f64 = 0.5;
pub const ILLUSIONER_FOLLOW_RANGE: f64 = 18.0;
pub const ILLUSIONER_XP_REWARD: u32 = 5;
pub const ILLUSIONER_BLINDNESS_DURATION_TICKS: i32 = 400;
pub const ILLUSIONER_INVISIBILITY_DURATION_TICKS: i32 = 1200;
pub const ILLUSIONER_MIRROR_CASTING_TICKS: i32 = 20;
pub const ILLUSIONER_NUM_ILLUSIONS: usize = 4;
pub const ILLUSIONER_ILLUSION_SPREAD: i32 = 3;

/// Witch combat and attribute constants, mirroring vanilla `Witch.java:55, 107-109`.
pub const WITCH_MAX_HEALTH: f64 = 26.0;
pub const WITCH_MOVEMENT_SPEED: f64 = 0.25;
pub const WITCH_XP_REWARD: u32 = 5;

/// Movement speed modifier applied to Witch while drinking potions (-0.25).
/// Mirrors vanilla `Witch.java:49-51` (`new AttributeModifier(..., -0.25, Operation.ADD_VALUE)`).
pub const WITCH_DRINKING_SPEED_MODIFIER: f64 = -0.25;

/// Potion use duration in ticks for Witch (32 ticks = 1.6s).
/// Mirrors vanilla `Witch.java:153` (`this.usingTime = this.getMainHandItem().getUseDuration(this)`).
pub const WITCH_DRINKING_DURATION_TICKS: i32 = 32;

/// Witch damage reduction factor for magic damage types (0.15 = 85% damage reduction).
/// Mirrors vanilla `Witch.java:216` (`damage *= 0.15F`).
pub const WITCH_MAGIC_DAMAGE_ABSORPTION_FACTOR: f32 = 0.15;

/// Witch ranged attack cooldown interval (60 ticks = 3.0s).
/// Mirrors vanilla `Witch.java:69` (`new RangedAttackGoal(this, 1.0, 60, 10.0F)`).
pub const WITCH_RANGED_ATTACK_INTERVAL: i32 = 60;

/// Witch ranged attack maximum range (10.0 blocks).
/// Mirrors vanilla `Witch.java:69` (`new RangedAttackGoal(this, 1.0, 60, 10.0F)`).
pub const WITCH_RANGED_ATTACK_MAX_RANGE: f32 = 10.0;

/// Witch ranged attack throw inaccuracy / uncertainty (8.0).
/// Mirrors vanilla `Witch.java:273` (`return 8.0F;`).
pub const WITCH_RANGED_ATTACK_UNCERTAINTY: f32 = 8.0;

/// Witch close throw speed (0.45).
/// Mirrors vanilla `Witch.java:257` (`dist <= 2.0 ? 0.45F : 0.75F`).
pub const WITCH_CLOSE_THROW_SPEED: f32 = 0.45;

/// Witch far throw speed (0.75).
/// Mirrors vanilla `Witch.java:257` (`dist <= 2.0 ? 0.45F : 0.75F`).
pub const WITCH_FAR_THROW_SPEED: f32 = 0.75;

/// Distance threshold separating close and far splash potion throws for Witch (2.0 blocks).
/// Mirrors vanilla `Witch.java:257` (`dist <= 2.0 ? 0.45F : 0.75F`).
pub const WITCH_CLOSE_THROW_DIST_THRESHOLD: f64 = 2.0;

/// Calculates throw speed for a splash potion launched by a Witch.
/// Mirrors vanilla `Witch.java:257`.
#[must_use]
pub const fn calculate_witch_throw_speed(distance: f64) -> f32 {
    if distance <= WITCH_CLOSE_THROW_DIST_THRESHOLD {
        WITCH_CLOSE_THROW_SPEED
    } else {
        WITCH_FAR_THROW_SPEED
    }
}

/// Raider movement speed through village during raids (1.05).
/// Mirrors vanilla `Raider.java:66` (`new RaiderMoveThroughVillageGoal(this, 1.05F, 1)`).
pub const RAIDER_MOVE_THROUGH_VILLAGE_SPEED: f32 = 1.05;

/// Radius of hold ground goal for patrolling raiders (10.0 blocks).
/// Mirrors vanilla `Vindicator.java:67`, `Pillager.java:98` (`new Raider.HoldGroundAttackGoal(this, 10.0F)`).
pub const RAIDER_HOLD_GROUND_RADIUS: f32 = 10.0;

/// Interval in ticks between periodic checks for joining active raids (20 ticks = 1s).
/// Mirrors vanilla `Raider.java:92` (`this.level().getGameTime() % 20L == 0L`).
pub const RAIDER_JOIN_RAID_CHECK_INTERVAL_TICKS: u64 = 20;

/// Horizontal search radius for ominous banner when raider seeks to become leader (16.0 blocks).
/// Mirrors vanilla `Raider.java:312` (`inflate(16.0, 4.0, 16.0)`).
pub const RAIDER_PATROL_BANNER_SEARCH_XZ: f64 = 16.0;

/// Vertical search radius for ominous banner when raider seeks to become leader (4.0 blocks).
/// Mirrors vanilla `Raider.java:312` (`inflate(16.0, 4.0, 16.0)`).
pub const RAIDER_PATROL_BANNER_SEARCH_Y: f64 = 4.0;

/// Determines whether a target entity is valid for Johnny Vindicator attack,
/// mirroring vanilla `Vindicator.java:207-210`:
/// `NearestAttackableTargetGoal<>(this, LivingEntity.class, 0, true, true, (target, level) -> target.attackable())`
/// Johnny attacks any living entity except fellow Illagers / Raiders.
#[must_use]
pub fn is_johnny_targetable(entity_type: &pumpkin_data::entity::EntityType) -> bool {
    !is_illager_immune_to_ravager_roar(entity_type)
        && entity_type != &pumpkin_data::entity::EntityType::GHAST
        && entity_type != &pumpkin_data::entity::EntityType::ARMOR_STAND
}

/// Creates the Blindness status effect cast by an Illusioner, mirroring vanilla `Illusioner.java:238`:
/// `target.addEffect(new MobEffectInstance(MobEffects.BLINDNESS, 400), this)`
#[must_use]
pub fn create_illusioner_blindness_effect() -> pumpkin_data::potion::Effect {
    pumpkin_data::potion::Effect {
        effect_type: &pumpkin_data::effect::StatusEffect::BLINDNESS,
        duration: ILLUSIONER_BLINDNESS_DURATION_TICKS,
        amplifier: 0,
        ambient: false,
        show_particles: true,
        show_icon: true,
        blend: false,
    }
}

/// Creates the Invisibility status effect cast by an Illusioner, mirroring vanilla `Illusioner.java:270`:
/// `this.addEffect(new MobEffectInstance(MobEffects.INVISIBILITY, 1200))`
#[must_use]
pub fn create_illusioner_invisibility_effect() -> pumpkin_data::potion::Effect {
    pumpkin_data::potion::Effect {
        effect_type: &pumpkin_data::effect::StatusEffect::INVISIBILITY,
        duration: ILLUSIONER_INVISIBILITY_DURATION_TICKS,
        amplifier: 0,
        ambient: false,
        show_particles: true,
        show_icon: true,
        blend: false,
    }
}


// =========================================================================
// Gap 226: Wolf, Cat, Ocelot, Fox, Panda & Polar Bear Combat & Behavior Parity
// Mirroring vanilla Minecraft Java 26.3
// =========================================================================

/// Wolf combat and attribute constants, mirroring vanilla `Wolf.java:94-118, 432-446`.
pub const WOLF_START_HEALTH: f64 = 8.0;
pub const WOLF_TAME_HEALTH: f64 = 40.0;
pub const WOLF_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const WOLF_BASE_ATTACK_DAMAGE: f64 = 4.0;
pub const WOLF_XP_REWARD: u32 = 3;
pub const WOLF_DEFAULT_COLLAR_COLOR: u8 = 14; // DyeColor.RED
pub const WOLF_ARMOR_REPAIR_UNIT: f32 = 0.125;
pub const WOLF_TAME_CHANCE_IN_THREE: u32 = 3;
pub const WOLF_DEFAULT_TAIL_ANGLE: f32 = std::f32::consts::PI / 5.0;
pub const WOLF_ANGRY_TAIL_ANGLE: f32 = 1.539_380_4;
pub const WOLF_PERSISTENT_ANGER_MIN_TICKS: i32 = 400;
pub const WOLF_PERSISTENT_ANGER_MAX_TICKS: i32 = 780;

/// Cat combat and attribute constants, mirroring vanilla `Cat.java:73-88, 105-110`.
pub const CAT_MAX_HEALTH: f64 = 10.0;
pub const CAT_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const CAT_BASE_ATTACK_DAMAGE: f64 = 3.0;
pub const CAT_TEMPT_SPEED_MOD: f64 = 0.6;
pub const CAT_WALK_SPEED_MOD: f64 = 0.8;
pub const CAT_SPRINT_SPEED_MOD: f64 = 1.33;
pub const CAT_PANIC_SPEED_MOD: f64 = 1.5;
pub const CAT_DEFAULT_COLLAR_COLOR: u8 = 14; // DyeColor.RED
pub const CAT_XP_REWARD: u32 = 1;
pub const CAT_TAME_CHANCE_IN_THREE: u32 = 3;
pub const CAT_AVOID_PLAYER_DISTANCE_SPRINT: f32 = 16.0;

/// Ocelot combat and attribute constants, mirroring vanilla `Ocelot.java:59-64, 110-120`.
pub const OCELOT_MAX_HEALTH: f64 = 10.0;
pub const OCELOT_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const OCELOT_BASE_ATTACK_DAMAGE: f64 = 3.0;
pub const OCELOT_CROUCH_SPEED_MOD: f64 = 0.6;
pub const OCELOT_WALK_SPEED_MOD: f64 = 0.8;
pub const OCELOT_SPRINT_SPEED_MOD: f64 = 1.33;
pub const OCELOT_TRUST_CHANCE_IN_THREE: u32 = 3;
pub const OCELOT_TRUST_FEED_DISTANCE_SQR: f64 = 9.0;
pub const OCELOT_XP_REWARD: u32 = 1;
pub const OCELOT_AMBIENT_INTERVAL: i32 = 900;
pub const OCELOT_AVOID_PLAYER_DISTANCE: f32 = 16.0;
pub const OCELOT_EVENT_TRUST_PARTICLES: u8 = 41;
pub const OCELOT_EVENT_SMOKE_PARTICLES: u8 = 40;

/// Fox combat and attribute constants, mirroring vanilla `Fox.java:111-125, 140-155`.
pub const FOX_MAX_HEALTH: f64 = 10.0;
pub const FOX_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const FOX_BASE_ATTACK_DAMAGE: f64 = 2.0;
pub const FOX_SAFE_FALL_DISTANCE: f64 = 5.0;
pub const FOX_FOLLOW_RANGE: f64 = 32.0;
pub const FOX_XP_REWARD: u32 = 1;
pub const FOX_PANIC_SPEED: f64 = 2.2;
pub const FOX_BREED_SPEED: f64 = 1.0;
pub const FOX_SEEK_SHELTER_SPEED: f64 = 1.25;
pub const FOX_MELEE_ATTACK_SPEED: f32 = 1.2;
pub const FOX_AVOID_PLAYER_DISTANCE: f32 = 16.0;
pub const FOX_AVOID_PLAYER_WALK_SPEED: f64 = 1.6;
pub const FOX_AVOID_PLAYER_SPRINT_SPEED: f64 = 1.4;
pub const FOX_AVOID_WOLF_DISTANCE: f32 = 8.0;
pub const FOX_AVOID_POLAR_BEAR_DISTANCE: f32 = 8.0;
pub const FOX_FLAG_SITTING: u8 = 1;
pub const FOX_FLAG_CROUCHING: u8 = 4;
pub const FOX_FLAG_INTERESTED: u8 = 8;
pub const FOX_FLAG_POUNCING: u8 = 16;
pub const FOX_FLAG_SLEEPING: u8 = 32;
pub const FOX_FLAG_FACEPLANTED: u8 = 64;
pub const FOX_FLAG_DEFENDING: u8 = 128;

/// Panda combat and attribute constants, mirroring vanilla `Panda.java:80-95, 110-120, 480-490`.
pub const PANDA_MAX_HEALTH: f64 = 20.0;
pub const PANDA_WEAK_MAX_HEALTH: f64 = 10.0;
pub const PANDA_BASE_MOVEMENT_SPEED: f64 = 0.15;
pub const PANDA_LAZY_MOVEMENT_SPEED: f64 = 0.07;
pub const PANDA_BASE_ATTACK_DAMAGE: f64 = 6.0;
pub const PANDA_XP_REWARD: u32 = 1;
pub const PANDA_TOTAL_ROLL_STEPS: i32 = 32;
pub const PANDA_TOTAL_UNHAPPY_TIME: i32 = 32;
pub const PANDA_EAT_TICK_INTERVAL: i32 = 5;
pub const PANDA_BREED_TARGET_RANGE: f64 = 8.0;
pub const PANDA_FLAG_SNEEZE: u8 = 2;
pub const PANDA_FLAG_ROLL: u8 = 4;
pub const PANDA_FLAG_SIT: u8 = 8;
pub const PANDA_FLAG_ON_BACK: u8 = 16;

/// Polar Bear combat and attribute constants, mirroring vanilla `PolarBear.java:58-66, 110-120, 205-207`.
pub const POLAR_BEAR_MAX_HEALTH: f64 = 30.0;
pub const POLAR_BEAR_BASE_MOVEMENT_SPEED: f64 = 0.25;
pub const POLAR_BEAR_BASE_ATTACK_DAMAGE: f64 = 6.0;
pub const POLAR_BEAR_FOLLOW_RANGE: f64 = 20.0;
pub const POLAR_BEAR_XP_REWARD: u32 = 2;
pub const POLAR_BEAR_STAND_ANIMATION_TICKS: f32 = 6.0;
pub const POLAR_BEAR_WARNING_SOUND_TICKS: i32 = 40;
pub const POLAR_BEAR_PERSISTENT_ANGER_MIN_TICKS: i32 = 400;
pub const POLAR_BEAR_PERSISTENT_ANGER_MAX_TICKS: i32 = 780;


// =========================================================================
// Gap 227: Horse, Donkey, Mule, Skeleton Horse, Zombie Horse, Camel, Llama & Trader Llama Combat Parity
// Mirroring vanilla Minecraft Java 26.3
// =========================================================================

/// Abstract Horse combat and attribute constants, mirroring vanilla `AbstractHorse.java:60-70, 110-120`.
pub const ABSTRACT_HORSE_JUMP_STRENGTH: f64 = 0.7;
pub const ABSTRACT_HORSE_MAX_HEALTH_BOUND: f64 = 53.0;
pub const ABSTRACT_HORSE_BASE_MOVEMENT_SPEED: f64 = 0.225;
pub const ABSTRACT_HORSE_STEP_HEIGHT: f64 = 1.0;
pub const ABSTRACT_HORSE_SAFE_FALL_DISTANCE: f64 = 6.0;
pub const ABSTRACT_HORSE_FALL_DAMAGE_MULTIPLIER: f64 = 0.5;
pub const ABSTRACT_HORSE_MIN_NATURAL_HEALTH: f32 = 15.0;
pub const ABSTRACT_HORSE_MAX_NATURAL_HEALTH: f32 = 30.0;
pub const ABSTRACT_HORSE_XP_REWARD: u32 = 2;

/// Abstract Chested Horse (Donkey & Mule) constants, mirroring vanilla `AbstractChestedHorse.java:20-40`.
pub const CHESTED_HORSE_BASE_MOVEMENT_SPEED: f64 = 0.175;
pub const CHESTED_HORSE_JUMP_STRENGTH: f64 = 0.5;
pub const CHESTED_HORSE_CHEST_SLOTS: usize = 15;

/// Skeleton Horse constants, mirroring vanilla `SkeletonHorse.java:38-45, 90-100`.
pub const SKELETON_HORSE_MAX_HEALTH: f64 = 15.0;
pub const SKELETON_HORSE_MOVEMENT_SPEED: f64 = 0.2;
pub const SKELETON_HORSE_JUMP_STRENGTH: f64 = 0.7;
pub const SKELETON_HORSE_TRAP_CHANCE: f64 = 0.2;

/// Zombie Horse constants, mirroring vanilla `ZombieHorse.java:35-40`.
pub const ZOMBIE_HORSE_MAX_HEALTH: f64 = 25.0;
pub const ZOMBIE_HORSE_MOVEMENT_SPEED: f64 = 0.2;
pub const ZOMBIE_HORSE_JUMP_STRENGTH: f64 = 0.7;

/// Camel constants, mirroring vanilla `Camel.java:55-80, 110-140`.
pub const CAMEL_MAX_HEALTH: f64 = 32.0;
pub const CAMEL_MOVEMENT_SPEED: f64 = 0.09;
pub const CAMEL_JUMP_STRENGTH: f64 = 0.42;
pub const CAMEL_STEP_HEIGHT: f64 = 1.5;
pub const CAMEL_DASH_COOLDOWN_TICKS: i32 = 55;
pub const CAMEL_XP_REWARD: u32 = 2;

/// Llama & Trader Llama constants, mirroring vanilla `Llama.java:60-80, 120-150`.
pub const LLAMA_BASE_MOVEMENT_SPEED: f64 = 0.175;
pub const LLAMA_JUMP_STRENGTH: f64 = 0.5;
pub const LLAMA_MIN_STRENGTH: i32 = 1;
pub const LLAMA_MAX_STRENGTH: i32 = 5;
pub const LLAMA_CHEST_SLOTS_PER_STRENGTH: usize = 3;
pub const LLAMA_XP_REWARD: u32 = 2;
pub const TRADER_LLAMA_DESPAWN_DELAY_TICKS: i32 = 47999;


// =========================================================================
// Gap 228: Cow, Mooshroom, Sheep, Pig & Chicken Combat & Farm Mechanics Parity
// Mirroring vanilla Minecraft Java 26.3
// =========================================================================

/// Cow & Mooshroom attribute constants, mirroring vanilla `AbstractCow.java:18-20`.
pub const COW_MAX_HEALTH: f64 = 10.0;
pub const COW_BASE_MOVEMENT_SPEED: f64 = 0.2;
pub const COW_XP_REWARD: u32 = 2;
pub const MOOSHROOM_SHEAR_MUSHROOM_COUNT: usize = 5;

/// Sheep attribute and shearing constants, mirroring vanilla `Sheep.java:50-52`.
pub const SHEEP_MAX_HEALTH: f64 = 8.0;
pub const SHEEP_BASE_MOVEMENT_SPEED: f64 = 0.23;
pub const SHEEP_XP_REWARD: u32 = 2;
pub const SHEEP_SHEAR_WOOL_MIN: usize = 1;
pub const SHEEP_SHEAR_WOOL_MAX: usize = 3;

/// Pig attribute and saddled speed constants, mirroring vanilla `Pig.java:42-45`.
pub const PIG_MAX_HEALTH: f64 = 10.0;
pub const PIG_BASE_MOVEMENT_SPEED: f64 = 0.25;
pub const PIG_BOOST_SPEED: f64 = 0.35;
pub const PIG_XP_REWARD: u32 = 2;

/// Chicken attribute and egg timing constants, mirroring vanilla `Chicken.java:45-50`.
pub const CHICKEN_MAX_HEALTH: f64 = 4.0;
pub const CHICKEN_BASE_MOVEMENT_SPEED: f64 = 0.25;
pub const CHICKEN_XP_REWARD: u32 = 2;
pub const CHICKEN_EGG_TIME_MIN: i32 = 6000;
pub const CHICKEN_EGG_TIME_MAX: i32 = 12000;
pub const CHICKEN_JOCKEY_CHANCE: f64 = 0.05;


// =========================================================================
// Gap 229: Frog, Tadpole, Goat & Rabbit Combat, Attributes, Leaping & Breeding Parity
// Mirroring vanilla Minecraft Java 26.3
// =========================================================================

/// Frog attribute constants, mirroring vanilla `Frog.java:132-138`.
pub const FROG_MAX_HEALTH: f64 = 10.0;
pub const FROG_BASE_MOVEMENT_SPEED: f64 = 1.0;
pub const FROG_BASE_ATTACK_DAMAGE: f64 = 10.0;
pub const FROG_STEP_HEIGHT: f64 = 1.0;
pub const FROG_XP_REWARD: u32 = 2;

/// Tadpole attribute constants, mirroring vanilla `Tadpole.java:50-52`.
pub const TADPOLE_MAX_HEALTH: f64 = 6.0;
pub const TADPOLE_BASE_MOVEMENT_SPEED: f64 = 1.0;
pub const TADPOLE_XP_REWARD: u32 = 0;

/// Goat attribute, ram damage and long jump constants, mirroring vanilla `Goat.java:60-65, 120-130`.
pub const GOAT_MAX_HEALTH: f64 = 10.0;
pub const GOAT_BASE_MOVEMENT_SPEED: f64 = 0.2;
pub const GOAT_BASE_ATTACK_DAMAGE: f64 = 2.0;
pub const GOAT_BABY_ATTACK_DAMAGE: f64 = 1.0;
pub const GOAT_FALL_DAMAGE_REDUCTION: i32 = 10;
pub const GOAT_SCREAMING_CHANCE: f64 = 0.02;
pub const GOAT_XP_REWARD: u32 = 2;

/// Rabbit attribute and The Killer Bunny constants, mirroring vanilla `Rabbit.java:42-45, 120-135`.
pub const RABBIT_MAX_HEALTH: f64 = 3.0;
pub const RABBIT_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const RABBIT_BASE_ATTACK_DAMAGE: f64 = 3.0;
pub const RABBIT_EVIL_ATTACK_DAMAGE_BONUS: f64 = 5.0;
pub const RABBIT_EVIL_TYPE_ID: i32 = 99;
pub const RABBIT_XP_REWARD: u32 = 2;


// =========================================================================
// Gap 230: Bat, Parrot & Bee Flying Movement, Pollination, Sting Damage & Toxicity Parity
// Mirroring vanilla Minecraft Java 26.3
// =========================================================================

/// Bat attribute and resting flag constants, mirroring vanilla `Bat.java:42-45`.
pub const BAT_MAX_HEALTH: f64 = 6.0;
pub const BAT_XP_REWARD: u32 = 0;
pub const BAT_FLAG_RESTING: u8 = 1;

/// Parrot attribute and poison duration constants, mirroring vanilla `Parrot.java:48-52, 230-240`.
pub const PARROT_MAX_HEALTH: f64 = 6.0;
pub const PARROT_FLYING_SPEED: f64 = 0.4;
pub const PARROT_BASE_MOVEMENT_SPEED: f64 = 0.2;
pub const PARROT_BASE_ATTACK_DAMAGE: f64 = 3.0;
pub const PARROT_XP_REWARD: u32 = 2;
pub const PARROT_TAME_CHANCE_IN_THREE: u32 = 3;
pub const PARROT_POISON_DURATION_TICKS: i32 = 900;

/// Bee attribute, sting poison duration, death timer and anger constants, mirroring vanilla `Bee.java:60-65, 130-140`.
pub const BEE_MAX_HEALTH: f64 = 10.0;
pub const BEE_FLYING_SPEED: f64 = 0.6;
pub const BEE_BASE_MOVEMENT_SPEED: f64 = 0.3;
pub const BEE_BASE_ATTACK_DAMAGE: f64 = 2.0;
pub const BEE_XP_REWARD: u32 = 2;
pub const BEE_POISON_DURATION_NORMAL_TICKS: i32 = 200;
pub const BEE_POISON_DURATION_HARD_TICKS: i32 = 360;
pub const BEE_TIME_UNTIL_DEATH_AFTER_STING: i32 = 1200;
pub const BEE_PERSISTENT_ANGER_MIN_TICKS: i32 = 400;
pub const BEE_PERSISTENT_ANGER_MAX_TICKS: i32 = 780;

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

    #[test]
    fn test_breach_armor_reduction_parity() {
        // Vanilla CombatRules.java:20-31:
        // realArmor = clamp(totalArmor - damage / (2.0 + toughness / 4.0), totalArmor * 0.2, 20.0)
        // armorFraction = realArmor / 25.0
        // Breach reduces armorFraction by 0.15 per level:
        // modifiedArmorFraction = clamp(armorFraction - 0.15 * level, 0.0, 1.0)
        // damageMultiplier = 1.0 - modifiedArmorFraction
        //
        // Example: damage = 10.0, total_armor = 20.0, toughness = 0.0
        // toughness_div = 2.0
        // realArmor = 20.0 - 10.0 / 2.0 = 15.0
        // armorFraction = 15.0 / 25.0 = 0.60
        // Without breach: dmg = 10.0 * (1.0 - 0.60) = 4.0
        let no_breach = CombatRules::get_damage_after_absorb(10.0, 20.0, 0.0, 0);
        assert!((no_breach - 4.0).abs() < 1e-4);

        // With Breach I (level 1):
        // modifiedArmorFraction = 0.60 - 0.15 = 0.45
        // dmg = 10.0 * (1.0 - 0.45) = 5.5
        let breach_1 = CombatRules::get_damage_after_absorb(10.0, 20.0, 0.0, 1);
        assert!((breach_1 - 5.5).abs() < 1e-4);

        // With Breach IV (level 4):
        // modifiedArmorFraction = clamp(0.60 - 0.60, 0.0, 1.0) = 0.0
        // dmg = 10.0 * (1.0 - 0.0) = 10.0 (full damage through armor)
        let breach_4 = CombatRules::get_damage_after_absorb(10.0, 20.0, 0.0, 4);
        assert!((breach_4 - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_weapon_durability_and_item_used_conditions_parity() {
        // Vanilla Player.java:1086-1098 & ItemStack.java:539-558:
        // Attacking a living entity with a weapon consumes durability and increments ITEM_USED:
        assert!(should_weapon_damage_on_attack(true, true));

        // Attacking non-living entity (ArmorStand, Boat, Minecart) does NOT consume weapon durability:
        assert!(!should_weapon_damage_on_attack(false, true));

        // Attacking with an item without Weapon component (e.g. bare hand, stick) does not take weapon durability:
        assert!(!should_weapon_damage_on_attack(true, false));
        assert!(!should_weapon_damage_on_attack(false, false));
    }

    #[test]
    fn test_fall_damage_jump_boost_and_fall_immunity_parity() {
        // Vanilla LivingEntity.java:1852-1863 & MobEffects.java:51
        // 1. Default safe fall distance (3.0 blocks):
        let default_safe = calculate_safe_fall_distance(3.0, None);
        assert_eq!(default_safe, 3.0);

        // 2. Jump Boost I (amp 0): adds 1.0 block -> 4.0 blocks
        let jb1_safe = calculate_safe_fall_distance(3.0, Some(0));
        assert_eq!(jb1_safe, 4.0);

        // 3. Jump Boost II (amp 1): adds 2.0 blocks -> 5.0 blocks
        let jb2_safe = calculate_safe_fall_distance(3.0, Some(1));
        assert_eq!(jb2_safe, 5.0);

        // 4. Fall power calculations:
        // Falling 3 blocks with default safe: power = 3.0 + 1e-6 - 3.0 = 1e-6 (damage = 0)
        let power_3 = calculate_fall_power(3.0, default_safe);
        assert_eq!(calculate_fall_damage_raw(power_3, 1.0, 1.0), 0);

        // Falling 4 blocks with default safe: power = 4.0 + 1e-6 - 3.0 = 1.000001 (damage = 1)
        let power_4 = calculate_fall_power(4.0, default_safe);
        assert_eq!(calculate_fall_damage_raw(power_4, 1.0, 1.0), 1);

        // Falling 4 blocks with Jump Boost I (safe 4.0): power = 4.0 + 1e-6 - 4.0 = 1e-6 (damage = 0!)
        let power_4_jb1 = calculate_fall_power(4.0, jb1_safe);
        assert_eq!(calculate_fall_damage_raw(power_4_jb1, 1.0, 1.0), 0);

        // Falling 10 blocks with default safe: power = 10.0 + 1e-6 - 3.0 = 7.000001 (damage = 7)
        let power_10 = calculate_fall_power(10.0, default_safe);
        assert_eq!(calculate_fall_damage_raw(power_10, 1.0, 1.0), 7);

        // Falling 10 blocks with Jump Boost II (safe 5.0): power = 10.0 + 1e-6 - 5.0 = 5.000001 (damage = 5)
        let power_10_jb2 = calculate_fall_power(10.0, jb2_safe);
        assert_eq!(calculate_fall_damage_raw(power_10_jb2, 1.0, 1.0), 5);
    }

    #[test]
    fn test_kill_credit_timing_and_tame_wolf_owner_parity() {
        // Vanilla LivingEntity.java:1368, 1445:
        // Kill credit window is 100 ticks (5.0s) based on entity age ticks:
        assert!(is_kill_credit_active(123, 50, 100)); // 50 ticks ago (active)
        assert!(is_kill_credit_active(123, 50, 150)); // exactly 100 ticks ago (active)
        assert!(!is_kill_credit_active(123, 50, 151)); // 101 ticks ago (expired)
        assert!(!is_kill_credit_active(0, 50, 60)); // no attacker recorded (inactive)
    }

    #[test]
    fn test_experience_drop_conditions_and_player_death_reward_parity() {
        // 1. Player death experience reward formula (Player.java:1610-1612):
        // !keepInventory && !isSpectator ? min(level * 7, 100) : 0
        assert_eq!(calculate_player_death_experience_reward(0, false, false), 0);
        assert_eq!(calculate_player_death_experience_reward(1, false, false), 7);
        assert_eq!(calculate_player_death_experience_reward(5, false, false), 35);
        assert_eq!(calculate_player_death_experience_reward(10, false, false), 70);
        assert_eq!(calculate_player_death_experience_reward(14, false, false), 98);
        assert_eq!(calculate_player_death_experience_reward(15, false, false), 100); // capped at 100
        assert_eq!(calculate_player_death_experience_reward(100, false, false), 100);

        // keepInventory prevents XP drop
        assert_eq!(calculate_player_death_experience_reward(30, true, false), 0);
        // Spectator mode prevents XP drop
        assert_eq!(calculate_player_death_experience_reward(30, false, true), 0);
        // Negative level guard
        assert_eq!(calculate_player_death_experience_reward(-5, false, false), 0);

        // 2. Experience drop condition parity (LivingEntity.java:1529-1536):
        // !wasExperienceConsumed && (isAlwaysExperienceDropper || (lastHurtByPlayer && shouldDrop && mobDrops))

        // Sculk catalyst absorption always suppresses XP drop
        assert!(!should_entity_drop_experience(true, true, true, true, true));
        assert!(!should_entity_drop_experience(false, true, true, true, true));

        // Player (is_always_dropper = true):
        // Drops XP even if killed by environment (was_killed_by_player = false) and mobDrops is false
        assert!(should_entity_drop_experience(true, false, true, false, false));
        assert!(should_entity_drop_experience(true, false, false, false, false));
        assert!(should_entity_drop_experience(true, true, true, true, false));

        // Mob (is_always_dropper = false):
        // Valid player kill + should drop + mob drops enabled -> drops XP
        assert!(should_entity_drop_experience(false, true, true, true, false));

        // Mob killed without player (e.g. natural fall, fire, cactus) -> NO XP
        assert!(!should_entity_drop_experience(false, false, true, true, false));

        // Baby mob (should_drop = false) -> NO XP
        assert!(!should_entity_drop_experience(false, true, false, true, false));

        // mobDrops gamerule disabled -> NO XP
        assert!(!should_entity_drop_experience(false, true, true, false, false));
    }

    #[test]
    fn test_mob_anger_resolution_and_tag_exemptions_parity() {
        // Vanilla LivingEntity.java:1357-1363 resolveMobResponsibleForDamage:
        // livingSource && !source.is(NO_ANGER) && (!source.is(WIND_CHARGE) || !this.is(NO_ANGER_FROM_WIND_CHARGE))

        // 1. Living attacker + normal damage -> causes anger
        assert!(can_cause_mob_anger(true, false, false, false));
        assert!(can_cause_mob_anger(true, false, false, true));

        // 2. Non-living attacker (lightning, dispenser, cactus) -> NO anger
        assert!(!can_cause_mob_anger(false, false, false, false));

        // 3. Damage with #minecraft:no_anger tag -> NO anger
        assert!(!can_cause_mob_anger(true, true, false, false));
        assert!(!can_cause_mob_anger(true, true, false, true));

        // 4. Wind charge damage against entity with #minecraft:no_anger_from_wind_charge -> NO anger
        assert!(!can_cause_mob_anger(true, false, true, true));

        // 5. Wind charge damage against normal entity -> causes anger
        assert!(can_cause_mob_anger(true, false, true, false));
    }

    #[test]
    fn test_hurt_animation_and_knockback_no_knockback_parity() {
        // Vanilla LivingEntity.java:1250:
        // boolean fullyBlocked = blocked && damage <= 0.0F;
        // if (!source.is(DamageTypeTags.NO_KNOCKBACK) && !fullyBlocked) { dealDefaultKnockback(...); }

        // Normal damage (e.g. player hit, arrow, mob attack): has_no_knockback = false, fully_blocked = false
        assert!(should_deal_default_knockback_and_hurt_anim(false, false));

        // Damage with #minecraft:no_knockback (fire, drowning, starve, freeze, poison, wither):
        // NO directional hurt animation or knockback
        assert!(!should_deal_default_knockback_and_hurt_anim(true, false));
        assert!(!should_deal_default_knockback_and_hurt_anim(true, true));

        // Fully blocked by shield: NO knockback or hurt animation
        assert!(!should_deal_default_knockback_and_hurt_anim(false, true));
    }

    #[test]
    fn test_wither_rose_creation_parity() {
        // Vanilla LivingEntity.java:1494-1512 createWitherRose:
        // killer instanceof WitherBoss -> if mobGriefing && isAir && canSurvive -> PlaceBlock else DropItem

        // 1. Killer is NOT a wither -> None
        assert_eq!(
            determine_wither_rose_action(false, true, true),
            WitherRosePlacementAction::None
        );

        // 2. Killer IS a wither, mobGriefing enabled, can place (air above, supports_wither_rose below) -> PlaceBlock
        assert!(can_place_wither_rose(true, true));
        assert_eq!(
            determine_wither_rose_action(true, true, true),
            WitherRosePlacementAction::PlaceBlock
        );

        // 3. Killer IS a wither, mobGriefing enabled, but cannot place (target not air or below cannot support) -> DropItem
        assert!(!can_place_wither_rose(false, true));
        assert!(!can_place_wither_rose(true, false));
        assert_eq!(
            determine_wither_rose_action(true, true, false),
            WitherRosePlacementAction::DropItem
        );

        // 4. Killer IS a wither, mobGriefing DISABLED -> always DropItem
        assert_eq!(
            determine_wither_rose_action(true, false, true),
            WitherRosePlacementAction::DropItem
        );
        assert_eq!(
            determine_wither_rose_action(true, false, false),
            WitherRosePlacementAction::DropItem
        );
    }

    #[test]
    fn test_player_death_lifecycle_and_velocity_parity() {
        // Vanilla Player.java:534-542:
        // When source is null, delta movement is strictly (0.0, 0.1, 0.0)
        let vel_no_source = calculate_player_death_velocity(0.0, 0.0, false);
        assert_eq!(vel_no_source.x, 0.0);
        assert_eq!(vel_no_source.y, 0.1);
        assert_eq!(vel_no_source.z, 0.0);

        // When source is present and yaw = 0, angle = 0:
        // x = -cos(0) * 0.1 = -0.1, z = -sin(0) * 0.1 = -0.0, y = 0.1
        let vel_yaw_0 = calculate_player_death_velocity(0.0, 0.0, true);
        assert!((vel_yaw_0.x - (-0.1)).abs() < 1e-6);
        assert_eq!(vel_yaw_0.y, 0.1);
        assert!(vel_yaw_0.z.abs() < 1e-6);

        // When source is present and yaw = 90 deg:
        // x = -cos(90) * 0.1 ≈ 0.0, z = -sin(90) * 0.1 = -0.1, y = 0.1
        let vel_yaw_90 = calculate_player_death_velocity(90.0, 0.0, true);
        assert!(vel_yaw_90.x.abs() < 1e-6);
        assert_eq!(vel_yaw_90.y, 0.1);
        assert!((vel_yaw_90.z - (-0.1)).abs() < 1e-6);
    }

    #[test]
    fn test_stuck_projectiles_tracking_and_decay_parity() {
        // Vanilla LivingEntity.java:2755-2778:
        // Interval = 20 * (30 - count)
        assert_eq!(calculate_stuck_projectile_removal_time(1), 580);
        assert_eq!(calculate_stuck_projectile_removal_time(5), 500);
        assert_eq!(calculate_stuck_projectile_removal_time(10), 400);
        assert_eq!(calculate_stuck_projectile_removal_time(20), 200);
        assert_eq!(calculate_stuck_projectile_removal_time(30), 0);

        // 1. Count = 0 -> stays 0 with 0 timer
        assert_eq!(tick_stuck_projectile_counter(0, 0), (0, 0));

        // 2. Count = 1, fresh start (timer = 0) -> timer initialized to 580, ticked to 579, count remains 1
        let (count, timer) = tick_stuck_projectile_counter(1, 0);
        assert_eq!(count, 1);
        assert_eq!(timer, 579);

        // 3. Count = 1, timer running (timer = 50) -> timer decreases to 49, count remains 1
        let (count, timer) = tick_stuck_projectile_counter(1, 50);
        assert_eq!(count, 1);
        assert_eq!(timer, 49);

        // 4. Count = 1, timer expiring (timer = 1) -> count decrements to 0, timer resets to 0
        let (count, timer) = tick_stuck_projectile_counter(1, 1);
        assert_eq!(count, 0);
        assert_eq!(timer, 0);

        // 5. Count = 30, timer = 0 -> interval is 0, next_timer <= 0 -> count decrements to 29 immediately
        let (count, timer) = tick_stuck_projectile_counter(30, 0);
        assert_eq!(count, 29);
        assert_eq!(timer, 0);
    }

    #[test]
    fn test_equipment_hand_swap_parity() {
        // Vanilla LivingEntity.java:2989-3002:
        // Hand swap occurs when newMain matches oldOff and newOff matches oldMain
        let sword_id = 100u16;
        let shield_id = 200u16;
        let bow_id = 300u16;

        // Perfect swap: Main(sword) & Off(shield) -> Main(shield) & Off(sword)
        assert!(is_equipment_hand_swap(sword_id, shield_id, shield_id, sword_id));

        // Not a swap: Main replaced with bow, offhand unchanged
        assert!(!is_equipment_hand_swap(sword_id, shield_id, bow_id, shield_id));

        // Not a swap: both hands become shield
        assert!(!is_equipment_hand_swap(sword_id, shield_id, shield_id, shield_id));
    }

    #[test]
    fn test_consumable_use_effects_and_sound_pitch_parity() {
        // Vanilla Consumable.java:119-124:
        // Default consume: 1.6s * 20 = 32 ticks
        // waitTicksBeforeUseEffects = (int)(32 * 0.21875) = 7 ticks
        let consume_ticks = (DEFAULT_CONSUME_SECONDS * 20.0) as i32;
        assert_eq!(consume_ticks, 32);

        // Tick 32 (used = 0 <= 7): no effects
        assert!(!should_emit_consumable_use_effects(consume_ticks, 32));
        // Tick 28 (used = 4 <= 7): no effects
        assert!(!should_emit_consumable_use_effects(consume_ticks, 28));
        // Tick 25 (used = 7 <= 7): no effects
        assert!(!should_emit_consumable_use_effects(consume_ticks, 25));

        // Tick 24 (used = 8 > 7, 24 % 4 == 0): FIRST emit!
        assert!(should_emit_consumable_use_effects(consume_ticks, 24));
        // Tick 23 (23 % 4 != 0): no effects
        assert!(!should_emit_consumable_use_effects(consume_ticks, 23));

        // Every 4 ticks after start fraction:
        assert!(should_emit_consumable_use_effects(consume_ticks, 20));
        assert!(should_emit_consumable_use_effects(consume_ticks, 16));
        assert!(should_emit_consumable_use_effects(consume_ticks, 12));
        assert!(should_emit_consumable_use_effects(consume_ticks, 8));
        assert!(should_emit_consumable_use_effects(consume_ticks, 4));
        assert!(should_emit_consumable_use_effects(consume_ticks, 0));

        // Fast food (e.g. dried kelp: 0.8s * 20 = 16 ticks, waitTicks = (int)(16 * 0.21875) = 3)
        let fast_consume_ticks = 16;
        assert!(!should_emit_consumable_use_effects(fast_consume_ticks, 16)); // used = 0 <= 3
        assert!(should_emit_consumable_use_effects(fast_consume_ticks, 12));  // used = 4 > 3, 12 % 4 == 0
        assert!(should_emit_consumable_use_effects(fast_consume_ticks, 8));
        assert!(should_emit_consumable_use_effects(fast_consume_ticks, 4));

        // Consumable sound volume and pitch parity:
        // 1. Drink animation: fixed volume 0.5, pitch in [0.9, 1.0]
        let (drink_vol_min, drink_pitch_min) =
            calculate_consumable_sound_volume_and_pitch(true, false, 0.0, 0.0);
        assert_eq!(drink_vol_min, 0.5);
        assert!((drink_pitch_min - 0.9).abs() < 1e-6);

        let (drink_vol_max, drink_pitch_max) =
            calculate_consumable_sound_volume_and_pitch(true, true, 1.0, 0.0);
        assert_eq!(drink_vol_max, 0.5);
        assert!((drink_pitch_max - 1.0).abs() < 1e-6);

        // 2. Eat animation: volume 0.5 (true) or 1.0 (false), pitch in [0.8, 1.2]
        let (eat_vol_50, eat_pitch_center) =
            calculate_consumable_sound_volume_and_pitch(false, true, 0.5, 0.5);
        assert_eq!(eat_vol_50, 0.5);
        assert!((eat_pitch_center - 1.0).abs() < 1e-6);

        let (eat_vol_100, eat_pitch_max) =
            calculate_consumable_sound_volume_and_pitch(false, false, 1.0, 0.0);
        assert_eq!(eat_vol_100, 1.0);
        assert!((eat_pitch_max - 1.2).abs() < 1e-6);

        let (_, eat_pitch_min) =
            calculate_consumable_sound_volume_and_pitch(false, false, 0.0, 1.0);
        assert!((eat_pitch_min - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_riptide_strength_velocity_and_conditions_parity() {
        // Vanilla riptide.json: base: 1.5, per_level_above_first: 0.75
        assert_eq!(calculate_riptide_strength(0), 0.0);
        assert_eq!(calculate_riptide_strength(1), 1.5);
        assert_eq!(calculate_riptide_strength(2), 2.25);
        assert_eq!(calculate_riptide_strength(3), 3.0);

        // Vanilla Riptide sounds (riptide.json)
        assert_eq!(get_riptide_sound(1), Sound::ItemTridentRiptide1);
        assert_eq!(get_riptide_sound(2), Sound::ItemTridentRiptide2);
        assert_eq!(get_riptide_sound(3), Sound::ItemTridentRiptide3);
        assert_eq!(get_riptide_sound(5), Sound::ItemTridentRiptide3);

        // Vanilla TridentItem.java:70:
        // riptideStrength > 0.0 && isInWaterOrRain && !isPassenger
        assert!(!can_perform_riptide(0, true, false)); // no riptide enchantment
        assert!(!can_perform_riptide(1, false, false)); // not in water or rain
        assert!(!can_perform_riptide(1, true, true)); // riding vehicle
        assert!(can_perform_riptide(1, true, false)); // valid

        // Vanilla TridentItem.java:93-102:
        // 1. Horizontal forward along +Z (yaw = 0, pitch = 0):
        let v_forward = calculate_riptide_velocity(0.0, 0.0, 1.5);
        assert!(v_forward.x.abs() < 1e-6);
        assert!(v_forward.y.abs() < 1e-6);
        assert!((v_forward.z - 1.5).abs() < 1e-6);

        // 2. Straight up (pitch = -90):
        let v_up = calculate_riptide_velocity(0.0, -90.0, 3.0);
        assert!(v_up.x.abs() < 1e-6);
        assert!((v_up.y - 3.0).abs() < 1e-6);
        assert!(v_up.z.abs() < 1e-6);

        // 3. Zero strength gives zero vector:
        let v_zero = calculate_riptide_velocity(45.0, -30.0, 0.0);
        assert_eq!(v_zero, Vector3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_elytra_decay_timing_parity() {
        // Vanilla LivingEntity.java:3188-3198:
        // checkFallFlyTicks = fallFlyTicks + 1
        // Event emitted every 10 ticks: checkFallFlyTicks % 10 == 0
        // Durability damaged every 20 ticks: freeFallInterval = checkFallFlyTicks / 10, freeFallInterval % 2 == 0

        // Ticks 1..9: neither event nor damage
        for t in 1..=9 {
            assert!(!should_emit_elytra_game_event(t));
            assert!(!should_damage_elytra(t));
        }

        // Tick 10: game event emitted, NO durability damage (interval = 1 % 2 != 0)
        assert!(should_emit_elytra_game_event(10));
        assert!(!should_damage_elytra(10));

        // Ticks 11..19: neither
        for t in 11..=19 {
            assert!(!should_emit_elytra_game_event(t));
            assert!(!should_damage_elytra(t));
        }

        // Tick 20: game event emitted AND durability damage applied (interval = 2 % 2 == 0)
        assert!(should_emit_elytra_game_event(20));
        assert!(should_damage_elytra(20));

        // Tick 30: event only
        assert!(should_emit_elytra_game_event(30));
        assert!(!should_damage_elytra(30));

        // Tick 40: event AND damage
        assert!(should_emit_elytra_game_event(40));
        assert!(should_damage_elytra(40));
    }

    #[test]
    fn test_firework_flight_duration_and_elytra_boost_parity() {
        // Vanilla FireworkRocketEntity.java:62-70:
        // flightCount = 1
        // if (fireworks != null) flightCount += fireworks.flightDuration()
        // lifetime = 10 * flightCount + rand(6) + rand(7)
        // With rand(6) in [0, 5] and rand(7) in [0, 6], random term is [0, 11].

        // Duration None: flightCount = 1 -> lifetime in [10, 21]
        let lt_none_min = calculate_firework_lifetime(None, 0, 0);
        let lt_none_max = calculate_firework_lifetime(None, 5, 6);
        assert_eq!(lt_none_min, 10);
        assert_eq!(lt_none_max, 21);

        // Duration 1: flightCount = 1 + 1 = 2 -> lifetime in [20, 31]
        let lt_1_min = calculate_firework_lifetime(Some(1), 0, 0);
        let lt_1_max = calculate_firework_lifetime(Some(1), 5, 6);
        assert_eq!(lt_1_min, 20);
        assert_eq!(lt_1_max, 31);

        // Duration 2: flightCount = 1 + 2 = 3 -> lifetime in [30, 41]
        let lt_2_min = calculate_firework_lifetime(Some(2), 0, 0);
        let lt_2_max = calculate_firework_lifetime(Some(2), 5, 6);
        assert_eq!(lt_2_min, 30);
        assert_eq!(lt_2_max, 41);

        // Duration 3: flightCount = 1 + 3 = 4 -> lifetime in [40, 51]
        let lt_3_min = calculate_firework_lifetime(Some(3), 0, 0);
        let lt_3_max = calculate_firework_lifetime(Some(3), 5, 6);
        assert_eq!(lt_3_min, 40);
        assert_eq!(lt_3_max, 51);

        // Vanilla FireworkRocketEntity.java:134-138:
        // new_vel = current_vel + (look * 0.1 + (look * 1.5 - current_vel) * 0.5)
        // = current_vel * 0.5 + look * 0.85
        let current_vel = Vector3::new(0.0, 0.0, 0.0);
        let look = Vector3::new(0.0, 1.0, 0.0);
        let boosted = calculate_elytra_firework_boost_velocity(current_vel, look);
        assert!((boosted.y - 0.85).abs() < 1e-6);
        assert_eq!(boosted.x, 0.0);
        assert_eq!(boosted.z, 0.0);

        // Moving forward while looking forward
        let current_forward = Vector3::new(0.0, 0.0, 1.0);
        let look_forward = Vector3::new(0.0, 0.0, 1.0);
        let boosted_forward = calculate_elytra_firework_boost_velocity(current_forward, look_forward);
        // 1.0 * 0.5 + 1.0 * 0.85 = 1.35
        assert!((boosted_forward.z - 1.35).abs() < 1e-6);

        // Vanilla FireworkRocketEntity.java:226, 231, 251:
        // Direct damage: 5.0 + explosions * 2
        assert_eq!(calculate_firework_direct_damage(0), 0.0);
        assert_eq!(calculate_firework_direct_damage(1), 7.0);
        assert_eq!(calculate_firework_direct_damage(3), 11.0);

        // Radial damage falloff: damageAmount * sqrt((5.0 - dist) / 5.0)
        assert_eq!(calculate_firework_explosion_damage(0, 0.0), 0.0);
        assert_eq!(calculate_firework_explosion_damage(1, 0.0), 7.0); // at center: full damage
        assert_eq!(calculate_firework_explosion_damage(1, 5.0), 0.0); // at border (5.0m): 0 damage
        assert_eq!(calculate_firework_explosion_damage(1, 6.0), 0.0); // beyond 5m: 0 damage

        // At distance 3.75m: (5.0 - 3.75) / 5.0 = 1.25 / 5.0 = 0.25 -> sqrt(0.25) = 0.5 -> 7.0 * 0.5 = 3.5
        let dmg_quarter = calculate_firework_explosion_damage(1, 3.75);
        assert!((dmg_quarter - 3.5).abs() < 1e-5);
    }

    #[test]
    fn test_player_pose_dimensions_and_crawling_parity() {
        use pumpkin_data::entity::EntityPose;

        // Dimensions and eye height verification (Avatar.java:21-37)
        assert_eq!(get_player_pose_dimensions(EntityPose::Standing), (0.6, 1.8, 1.62));
        assert_eq!(get_player_pose_dimensions(EntityPose::Crouching), (0.6, 1.5, 1.27));
        assert_eq!(get_player_pose_dimensions(EntityPose::Swimming), (0.6, 0.6, 0.4));
        assert_eq!(get_player_pose_dimensions(EntityPose::FallFlying), (0.6, 0.6, 0.4));
        assert_eq!(get_player_pose_dimensions(EntityPose::SpinAttack), (0.6, 0.6, 0.4));
        assert_eq!(get_player_pose_dimensions(EntityPose::Sleeping), (0.2, 0.2, 0.2));
        assert_eq!(get_player_pose_dimensions(EntityPose::Dying), (0.2, 0.2, 1.62));

        // Actual pose determination logic (Player.java:345-358)
        // 1. Normal open space: player wants Standing, fits Standing -> Standing
        let pose = determine_player_actual_pose(
            EntityPose::Standing,
            false,
            false,
            true, // can fit swimming
            true, // can fit standing
            true, // can fit crouching
            EntityPose::Standing,
        );
        assert!(pose == EntityPose::Standing);

        // 2. Sneaking key pressed in open space: wants Crouching, fits Crouching -> Crouching
        let pose = determine_player_actual_pose(
            EntityPose::Crouching,
            false,
            false,
            true,
            true,
            true,
            EntityPose::Standing,
        );
        assert!(pose == EntityPose::Crouching);

        // 3. Forced sneak (ceiling height 1.5m): player wants Standing, but cannot fit Standing; fits Crouching -> Forced Crouching
        let pose = determine_player_actual_pose(
            EntityPose::Standing,
            false,
            false,
            true,  // can fit swimming
            false, // cannot fit standing (ceiling blocks 1.8m)
            true,  // can fit crouching (1.5m clearance)
            EntityPose::Standing,
        );
        assert!(pose == EntityPose::Crouching);

        // 4. Crawling / 1-block gap (ceiling height 1.0m, trapdoor down): player cannot fit Standing nor Crouching; fits Swimming -> Swimming (Crawling)
        let pose = determine_player_actual_pose(
            EntityPose::Standing,
            false,
            false,
            true,  // can fit swimming (0.6m clearance)
            false, // cannot fit standing
            false, // cannot fit crouching
            EntityPose::Standing,
        );
        assert!(pose == EntityPose::Swimming);

        // 5. Completely suffocated/blocked space (cannot fit even Swimming 0.6m): keeps current pose without crash/glitch
        let pose = determine_player_actual_pose(
            EntityPose::Standing,
            false,
            false,
            false, // cannot fit swimming
            false,
            false,
            EntityPose::Crouching,
        );
        assert!(pose == EntityPose::Crouching);

        // 6. Spectator / Passenger bypasses collision clearance
        let pose_spectator = determine_player_actual_pose(
            EntityPose::Standing,
            true,  // spectator
            false,
            true,
            false, // no clearance
            false,
            EntityPose::Swimming,
        );
        assert!(pose_spectator == EntityPose::Standing);
    }

    #[test]
    fn test_swimming_vertical_impulse_and_multiplier_parity() {
        // Vanilla Player.java:1392-1402:
        // double lookAngleY = this.getLookAngle().y;
        // double multiplier = lookAngleY < -0.2 ? 0.085 : 0.06;
        // if (lookAngleY <= 0.0 || this.jumping || !fluidAtHead.isEmpty()) {
        //     movement.add(0.0, (lookAngleY - movement.y) * multiplier, 0.0);
        // }

        // 1. Diving steep down: lookAngleY = -0.6 (< -0.2) -> multiplier = 0.085
        let (impulse_dive, should_apply) = calculate_swimming_vertical_impulse(-0.6, 0.0, false, false);
        assert!(should_apply);
        // impulse = (-0.6 - 0.0) * 0.085 = -0.051
        assert!((impulse_dive - (-0.051)).abs() < 1e-6);

        // 2. Horizontal swimming: lookAngleY = 0.0 (>= -0.2) -> multiplier = 0.06
        let (impulse_horiz, should_apply) = calculate_swimming_vertical_impulse(0.0, -0.1, false, false);
        assert!(should_apply);
        // impulse = (0.0 - (-0.1)) * 0.06 = 0.006
        assert!((impulse_horiz - 0.006).abs() < 1e-6);

        // 3. Swimming up with head submerged: lookAngleY = 0.5 (> 0.0), head_submerged = true -> multiplier = 0.06
        let (impulse_up, should_apply) = calculate_swimming_vertical_impulse(0.5, 0.0, false, true);
        assert!(should_apply);
        // impulse = (0.5 - 0.0) * 0.06 = 0.03
        assert!((impulse_up - 0.03).abs() < 1e-6);

        // 4. Swimming up with Space pressed: lookAngleY = 0.5, is_jumping = true
        let (impulse_jump, should_apply) = calculate_swimming_vertical_impulse(0.5, 0.0, true, false);
        assert!(should_apply);
        assert!((impulse_jump - 0.03).abs() < 1e-6);

        // 5. Head above water, looking up, space NOT pressed: does not launch out of water
        let (impulse_none, should_apply) = calculate_swimming_vertical_impulse(0.5, 0.0, false, false);
        assert!(!should_apply);
        assert_eq!(impulse_none, 0.0);
    }

    #[test]
    fn test_air_supply_respiration_drowning_and_turtle_helmet_parity() {
        // 1. Oxygen bonus calculation: base 0.0, Respiration III (level 3) -> 3.0
        let bonus_none = calculate_respiration_oxygen_bonus(0, 0.0);
        assert_eq!(bonus_none, 0.0);
        let bonus_resp3 = calculate_respiration_oxygen_bonus(3, 0.0);
        assert_eq!(bonus_resp3, 3.0);
        // Base bonus + enchantment
        let bonus_custom = calculate_respiration_oxygen_bonus(2, 1.5);
        assert_eq!(bonus_custom, 3.5);

        // 2. Air depletion probability check:
        // Without respiration: always decreases (should_decrease = true)
        assert!(should_decrease_air_supply(0.0, 0.0));
        assert!(should_decrease_air_supply(0.0, 0.999));

        // With Respiration I (oxygen_bonus = 1.0): chance to decrease is 1 / (1 + 1) = 0.5
        // random < 0.5 -> decreases; random >= 0.5 -> does not decrease (preserves air)
        assert!(should_decrease_air_supply(1.0, 0.49));
        assert!(!should_decrease_air_supply(1.0, 0.51));

        // With Respiration III (oxygen_bonus = 3.0): chance to decrease is 1 / (3 + 1) = 0.25
        assert!(should_decrease_air_supply(3.0, 0.20));
        assert!(!should_decrease_air_supply(3.0, 0.26));

        // 3. Air recovery rate: +4 per tick, capped at max_air (300)
        assert_eq!(calculate_increased_air_supply(0, 300), 4);
        assert_eq!(calculate_increased_air_supply(295, 300), 299);
        assert_eq!(calculate_increased_air_supply(298, 300), 300);
        assert_eq!(calculate_increased_air_supply(300, 300), 300);

        // 4. Drowning damage threshold: air <= -20
        assert!(!should_take_drowning_damage(0));
        assert!(!should_take_drowning_damage(-1));
        assert!(!should_take_drowning_damage(-19));
        assert!(should_take_drowning_damage(-20));
        assert!(should_take_drowning_damage(-25));

        // 5. Water breathing effect conditions:
        // Any of WATER_BREATHING, CONDUIT_POWER, or BREATH_OF_THE_NAUTILUS allows breathing underwater
        assert!(has_water_breathing_effects(true, false, false));
        assert!(has_water_breathing_effects(false, true, false));
        assert!(has_water_breathing_effects(false, false, true));
        assert!(!has_water_breathing_effects(false, false, false));

        // Refill air underwater: allowed unless only BREATH_OF_THE_NAUTILUS is active
        assert!(should_effects_refill_air_supply(true, false, false));
        assert!(should_effects_refill_air_supply(false, true, false));
        assert!(should_effects_refill_air_supply(true, false, true));
        assert!(!should_effects_refill_air_supply(false, false, true));
        assert!(should_effects_refill_air_supply(false, false, false));
    }

    #[test]
    fn test_submerged_mining_speed_aqua_affinity_and_destroy_speed_parity() {
        // 1. Aqua Affinity and Submerged Mining Speed:
        // Base submerged speed = 0.2
        let base_submerged = 0.2_f32;
        let speed_no_aqua = calculate_submerged_mining_speed(base_submerged, 0);
        assert!((speed_no_aqua - 0.2).abs() < 1e-6);

        // Aqua Affinity I: 0.2 * (1.0 + 4.0 * 1) = 1.0 (full normal speed underwater)
        let speed_aqua1 = calculate_submerged_mining_speed(base_submerged, 1);
        assert!((speed_aqua1 - 1.0).abs() < 1e-6);

        // 2. Mining Fatigue exponent parity: 0.3^(amp + 1)
        assert!((calculate_mining_fatigue_scale(0) - 0.3).abs() < 1e-6);
        assert!((calculate_mining_fatigue_scale(1) - 0.09).abs() < 1e-6);
        assert!((calculate_mining_fatigue_scale(2) - 0.027).abs() < 1e-6); // Previously 0.0027 in Neth (10x bug)
        assert!((calculate_mining_fatigue_scale(3) - 0.0081).abs() < 1e-6); // Previously 0.00081 in Neth (10x bug)

        // 3. Destroy speed full pipeline:
        // Diamond pickaxe on stone: tool_speed = 8.0, Efficiency V = 26.0 -> 34.0
        // On ground, dry air: 34.0
        let speed_normal = calculate_player_destroy_speed(8.0, 26.0, None, None, 1.0, false, 0.2, true);
        assert!((speed_normal - 34.0).abs() < 1e-6);

        // Underwater without Aqua Affinity: 34.0 * 0.2 = 6.8
        let speed_underwater = calculate_player_destroy_speed(8.0, 26.0, None, None, 1.0, true, 0.2, true);
        assert!((speed_underwater - 6.8).abs() < 1e-6);

        // Underwater with Aqua Affinity: 34.0 * 1.0 = 34.0
        let speed_underwater_aqua = calculate_player_destroy_speed(8.0, 26.0, None, None, 1.0, true, 1.0, true);
        assert!((speed_underwater_aqua - 34.0).abs() < 1e-6);

        // Airborne (floating in water or jumping): divided by 5.0
        let speed_airborne = calculate_player_destroy_speed(8.0, 26.0, None, None, 1.0, false, 0.2, false);
        assert!((speed_airborne - 6.8).abs() < 1e-6);

        // Underwater without Aqua AND Airborne: 34.0 * 0.2 / 5.0 = 1.36
        let speed_submerged_airborne = calculate_player_destroy_speed(8.0, 26.0, None, None, 1.0, true, 0.2, false);
        assert!((speed_submerged_airborne - 1.36).abs() < 1e-6);

        // 4. Destroy progress calculation (BlockBehaviour.java:345-352)
        // Stone hardness = 1.5, correct tool (can_harvest = true -> modifier 30.0)
        let progress = calculate_block_destroy_progress(34.0, 1.5, true);
        // 34.0 / 1.5 / 30.0 = 0.75555...
        assert!((progress - (34.0 / 1.5 / 30.0)).abs() < 1e-6);

        // Bedrock hardness = -1.0 -> unbreakable (0.0)
        assert_eq!(calculate_block_destroy_progress(34.0, -1.0, true), 0.0);
    }

    #[test]
    fn test_wind_charge_throw_pitch_spawn_and_impact_parity() {
        // 1. Wind Charge sound pitch: 0.4 / (rand * 0.4 + 0.8)
        // rand = 0.0 -> pitch = 0.4 / 0.8 = 0.5
        assert!((calculate_wind_charge_throw_pitch(0.0) - 0.5).abs() < 1e-6);
        // rand = 1.0 -> pitch = 0.4 / 1.2 = 0.333333...
        assert!((calculate_wind_charge_throw_pitch(1.0) - (0.4 / 1.2)).abs() < 1e-6);
        // rand = 0.5 -> pitch = 0.4 / (0.2 + 0.8) = 0.4
        assert!((calculate_wind_charge_throw_pitch(0.5) - 0.4).abs() < 1e-6);

        // 2. Wind charge spawn position: eye level y
        let player_pos = Vector3::new(10.0, 64.0, -20.0);
        let eye_y = 65.62;
        let spawn_pos = calculate_wind_charge_spawn_position(player_pos, eye_y);
        assert_eq!(spawn_pos.x, 10.0);
        assert_eq!(spawn_pos.y, 65.62);
        assert_eq!(spawn_pos.z, -20.0);

        // 3. Block impact normal offset: face_normal * 0.25
        let hit_pos = Vector3::new(5.0, 64.0, 5.0);
        let top_face_normal = Vector3::new(0.0, 1.0, 0.0);
        let exp_pos = calculate_wind_charge_block_explosion_pos(hit_pos, top_face_normal);
        assert_eq!(exp_pos, Vector3::new(5.0, 64.25, 5.0));

        // 4. Cooldown ticks: 10 ticks (0.5s)
        assert_eq!(calculate_wind_charge_cooldown_ticks(), 10);
    }

    #[test]
    fn test_thrown_trident_loyalty_physics_void_recall_and_channeling_parity() {
        // 1. Loyalty Return Physics (ThrownTrident.java:83-87)
        // pos = (0, 0, 0), vel = (1, 0, 0), owner_eye = (0, 10, 0), loyalty = 3
        let pos = Vector3::new(0.0, 0.0, 0.0);
        let vel = Vector3::new(1.0, 0.0, 0.0);
        let owner_eye = Vector3::new(0.0, 10.0, 0.0);
        let (new_pos, new_vel) = calculate_trident_loyalty_step(pos, vel, owner_eye, 3);

        // new_vel = (1, 0, 0) * 0.95 + (0, 1, 0) * (0.05 * 3) = (0.95, 0.15, 0.0)
        assert!((new_vel.x - 0.95).abs() < 1e-6);
        assert!((new_vel.y - 0.15).abs() < 1e-6);
        assert!((new_vel.z - 0.0).abs() < 1e-6);

        // y_offset = 10.0 * 0.015 * 3 = 0.45
        // new_pos = (0.95, 0.45 + 0.15, 0.0) = (0.95, 0.60, 0.0)
        assert!((new_pos.x - 0.95).abs() < 1e-6);
        assert!((new_pos.y - 0.60).abs() < 1e-6);
        assert!((new_pos.z - 0.0).abs() < 1e-6);

        // 2. Loyalty Return Trigger Conditions (ThrownTrident.java:70)
        assert!(!should_trident_return_loyalty(0, true, true, true));
        assert!(!should_trident_return_loyalty(1, false, false, true));
        assert!(should_trident_return_loyalty(1, true, false, true));
        assert!(should_trident_return_loyalty(1, false, true, true));
        assert!(!should_trident_return_loyalty(1, true, false, false));

        // 3. In-ground recall delay (ThrownTrident.java:64-66)
        assert!(!should_trident_trigger_return_after_in_ground(0));
        assert!(!should_trident_trigger_return_after_in_ground(4));
        assert!(should_trident_trigger_return_after_in_ground(5));

        // 4. Acceptable return owner check (ThrownTrident.java:100-102)
        assert!(is_acceptable_trident_return_owner(true, false));
        assert!(!is_acceptable_trident_return_owner(false, false));
        assert!(!is_acceptable_trident_return_owner(true, true));

        // 5. Void auto-recall safeguard (Entity.java:605 min_y - 64)
        assert!(should_trident_auto_recall_from_void(-65.0, 1, true, 0.0));
        assert!(!should_trident_auto_recall_from_void(-63.0, 1, true, 0.0));
        assert!(!should_trident_auto_recall_from_void(-150.0, 0, true, 0.0));
        assert!(!should_trident_auto_recall_from_void(-150.0, 1, false, 0.0));

        // 6. Channeling entity strike conditions (Enchantments.java:1070-1074)
        assert!(should_channeling_strike_entity(true, true, true));
        assert!(!should_channeling_strike_entity(false, true, true));
        assert!(!should_channeling_strike_entity(true, false, true));
        assert!(!should_channeling_strike_entity(true, true, false));

        // 7. Channeling block strike conditions (Enchantments.java:1086-1095)
        // Must be a lightning rod!
        assert!(should_channeling_strike_block(true, true, true, true));
        assert!(!should_channeling_strike_block(true, true, true, false)); // regular blocks do not trigger channeling
        assert!(!should_channeling_strike_block(false, true, true, true));
        assert!(!should_channeling_strike_block(true, false, true, true));
        assert!(!should_channeling_strike_block(true, true, false, true));
    }

    #[test]
    fn test_elytra_kinetic_damage_wall_collision_and_aerodynamics_parity() {
        // 1. Kinetic damage from flying into wall (LivingEntity.java:2616-2617)
        // diff <= 0.3 -> no damage (diff * 10 - 3 <= 0)
        assert_eq!(calculate_fly_into_wall_damage(0.1), 0.0);
        assert_eq!(calculate_fly_into_wall_damage(0.3), 0.0);

        // diff = 0.5 -> 0.5 * 10 - 3 = 2.0
        assert!((calculate_fly_into_wall_damage(0.5) - 2.0).abs() < 1e-6);

        // diff = 1.0 -> 1.0 * 10 - 3 = 7.0
        assert!((calculate_fly_into_wall_damage(1.0) - 7.0).abs() < 1e-6);

        // 2. Sound played on wall impact (LivingEntity.java:2619)
        assert_eq!(get_fly_into_wall_sound(2.0), pumpkin_data::sound::Sound::EntityGenericSmallFall);
        assert_eq!(get_fly_into_wall_sound(4.0), pumpkin_data::sound::Sound::EntityGenericSmallFall);
        assert_eq!(get_fly_into_wall_sound(4.5), pumpkin_data::sound::Sound::EntityGenericBigFall);

        // 3. Can glide conditions (LivingEntity.java:3202-3214)
        assert!(can_entity_glide(false, false, false, true));
        assert!(!can_entity_glide(true, false, false, true)); // on ground
        assert!(!can_entity_glide(false, true, false, true)); // is passenger
        assert!(!can_entity_glide(false, false, true, true)); // levitation
        assert!(!can_entity_glide(false, false, false, false)); // no elytra

        // 4. Climbable stops fall flying (LivingEntity.java:2567-2570)
        assert!(should_stop_fall_flying_on_climbable(true));
        assert!(!should_stop_fall_flying_on_climbable(false));

        // 5. Aerodynamics step calculation (LivingEntity.java:2587-2612)
        // Level flight: pitch = 0, yaw = 0, movement = (0, 0, 1.0), gravity = 0.08
        let init_move = Vector3::new(0.0, 0.0, 1.0);
        let next_move = calculate_fall_flying_movement_step(init_move, 0.0, 0.0, 0.08);

        // cos(0)^2 = 1.0, gravity * (-1.0 + 0.75) = 0.08 * -0.25 = -0.02
        // dive convert: convert = -0.02 * -0.1 * 1.0 = +0.002
        // y becomes (-0.02 + 0.002) * 0.98 = -0.01764
        assert!((next_move.y - (-0.01764)).abs() < 1e-4);
        // forward z with convert and direction alignment drag: ~0.9918
        assert!((next_move.z - 0.9918).abs() < 1e-3);
        // x remains 0
        assert!(next_move.x.abs() < 1e-6);
    }

    #[test]
    fn test_splash_potion_scale_duration_and_sound_events_parity() {
        // 1. Distance-based scale calculation (ThrownSplashPotion.java:54-56)
        // At impact center (dist_sq = 0.0) -> scale 1.0
        assert!((calculate_splash_potion_scale(0.0) - 1.0).abs() < 1e-6);
        // At 2 blocks away (dist_sq = 4.0) -> scale 1.0 - 2/4 = 0.5
        assert!((calculate_splash_potion_scale(4.0) - 0.5).abs() < 1e-6);
        // At 3 blocks away (dist_sq = 9.0) -> scale 1.0 - 3/4 = 0.25
        assert!((calculate_splash_potion_scale(9.0) - 0.25).abs() < 1e-6);
        // At or beyond 4 blocks (dist_sq >= 16.0) -> scale 0.0
        assert_eq!(calculate_splash_potion_scale(16.0), 0.0);
        assert_eq!(calculate_splash_potion_scale(25.0), 0.0);

        // 2. Scaled effect duration (ThrownSplashPotion.java:63)
        // Base 900 ticks (45s), scale 0.5 -> 450 ticks
        assert_eq!(calculate_scaled_effect_duration(900, 0.5, 1.0), 450);
        // Base 100 ticks, scale 0.25 -> 25 ticks
        assert_eq!(calculate_scaled_effect_duration(100, 0.25, 1.0), 25);

        // 3. Minimum duration threshold !endsWithin(20) (ThrownSplashPotion.java:67)
        assert!(!should_apply_scaled_effect(20)); // 20 ticks or less is dropped
        assert!(!should_apply_scaled_effect(15));
        assert!(should_apply_scaled_effect(21)); // > 20 ticks is applied
        assert!(should_apply_scaled_effect(100));

        // 4. Break sound events (AbstractThrownPotion.java:83, 88)
        assert_eq!(get_splash_potion_break_sound_event(false), 1053);
        assert_eq!(get_splash_potion_break_sound_event(true), 1054);

        // 5. Break particle events (AbstractThrownPotion.java:81, 86)
        assert_eq!(get_splash_potion_particle_event(false), 2002);
        assert_eq!(get_splash_potion_particle_event(true), 2007);

        // 6. Water potion extinguish entity condition (AbstractThrownPotion.java:100, 109)
        assert!(should_water_potion_extinguish_entity(true, true, true));
        assert!(!should_water_potion_extinguish_entity(false, true, true));
        assert!(!should_water_potion_extinguish_entity(true, false, true));
        assert!(!should_water_potion_extinguish_entity(true, true, false));
    }

    #[test]
    fn test_lingering_potion_and_area_effect_cloud_parity() {
        // 1. Initial lingering potion parameters (ThrownLingeringPotion.java:44-51)
        assert_eq!(get_lingering_potion_initial_duration(), 600);
        assert_eq!(get_lingering_potion_initial_radius(), 3.0);
        assert_eq!(get_lingering_potion_wait_time(), 10);
        assert_eq!(get_lingering_potion_radius_on_use(), -0.5);
        assert_eq!(get_lingering_potion_duration_on_use(), 0);
        assert_eq!(get_lingering_potion_reapplication_delay(), 20);

        // 2. Radius per tick calculation (ThrownLingeringPotion.java:48: -radius / duration)
        let radius_per_tick = calculate_lingering_potion_radius_per_tick(3.0, 600);
        assert!((radius_per_tick - (-0.005)).abs() < 1e-6);

        // 3. Cloud discard condition (AreaEffectCloud.java:191-192, 204-206, 239-241)
        // Age not expired (age - wait_time < duration)
        assert!(!should_area_effect_cloud_discard(609, 10, 600, 2.0));
        // Age expired (age - wait_time >= duration)
        assert!(should_area_effect_cloud_discard(610, 10, 600, 2.0));
        // Minimum radius threshold (radius < 0.5)
        assert!(should_area_effect_cloud_discard(100, 10, 600, 0.49));
        assert!(!should_area_effect_cloud_discard(100, 10, 600, 0.5));

        // 4. Horizontal distance check (AreaEffectCloud.java:223-226)
        assert!(is_entity_in_area_effect_cloud_radius(10.0, 20.0, 12.0, 21.0, 3.0)); // dist_sq = 4 + 1 = 5 <= 9
        assert!(!is_entity_in_area_effect_cloud_radius(10.0, 20.0, 13.0, 21.0, 3.0)); // dist_sq = 9 + 1 = 10 > 9
    }

    #[test]
    fn test_arrow_damage_punch_knockback_and_spectral_duration_parity() {
        // 1. Arrow base damage & crit calculation (AbstractArrow.java:434, 452-455)
        let non_crit_dmg = calculate_arrow_damage(3.0, 2.0, false, 0);
        assert_eq!(non_crit_dmg, 6);

        // Crit damage bonus: raw = 6, max_bonus = 6 / 2 + 2 = 5, bonus = 3 % 5 = 3 -> 9
        let crit_dmg = calculate_arrow_damage(3.0, 2.0, true, 3);
        assert_eq!(crit_dmg, 9);

        // 2. Punch knockback with victim knockback resistance (AbstractArrow.java:517-518)
        let kb_zero_res = calculate_arrow_punch_knockback(2.0, 0.0);
        assert!((kb_zero_res - 1.2).abs() < 1e-6);

        let kb_half_res = calculate_arrow_punch_knockback(2.0, 0.5);
        assert!((kb_half_res - 0.6).abs() < 1e-6);

        let kb_full_res = calculate_arrow_punch_knockback(2.0, 1.0);
        assert_eq!(kb_full_res, 0.0);

        // 3. Arrow stick condition based on pierce level (AbstractArrow.java:468-470)
        assert!(should_arrow_stick_in_entity(0));
        assert!(!should_arrow_stick_in_entity(1));
        assert!(!should_arrow_stick_in_entity(4));

        // 4. Hit sound pitch calculation (AbstractArrow.java:495, 539)
        let pitch = calculate_arrow_hit_sound_pitch(0.5);
        assert!((pitch - 1.2).abs() < 1e-6);

        // 5. Spectral arrow default duration (SpectralArrow.java:18)
        assert_eq!(get_spectral_arrow_duration(), 200);
    }

    #[test]
    fn test_fireball_and_dragon_breath_cloud_parity() {
        // 1. Dragon Fireball cloud expansion per tick (DragonFireball.java:43: (7.0 - 3.0) / 600)
        let rate = calculate_dragon_fireball_radius_per_tick(3.0, 7.0, 600);
        assert!((rate - (4.0 / 600.0)).abs() < 1e-6);

        // 2. Mob griefing fire placement permission (SmallFireball.java:55)
        assert!(can_small_fireball_place_fire(false, false)); // Player always allowed
        assert!(can_small_fireball_place_fire(false, true));
        assert!(!can_small_fireball_place_fire(true, false)); // Mob denied if mob_griefing is false
        assert!(can_small_fireball_place_fire(true, true)); // Mob allowed if mob_griefing is true

        // 3. Dragon Fireball constants (DragonFireball.java:41-43, 56)
        assert_eq!(get_dragon_fireball_initial_radius(), 3.0);
        assert_eq!(get_dragon_fireball_max_radius(), 7.0);
        assert_eq!(get_dragon_fireball_duration(), 600);
        assert_eq!(get_dragon_breath_level_event(), 2006);
    }

    #[test]
    fn test_wither_skull_inertia_difficulty_effect_and_heal_parity() {
        // 1. Inertia values for dangerous vs normal skull (WitherSkull.java:42-44)
        assert_eq!(get_wither_skull_inertia(true), 0.73);
        assert_eq!(get_wither_skull_inertia(false), 0.95);

        // 2. Wither effect duration by difficulty (WitherSkull.java:79-88)
        assert_eq!(get_wither_skull_effect_duration(pumpkin_util::Difficulty::Hard), 800);
        assert_eq!(get_wither_skull_effect_duration(pumpkin_util::Difficulty::Normal), 200);
        assert_eq!(get_wither_skull_effect_duration(pumpkin_util::Difficulty::Easy), 0);
        assert_eq!(get_wither_skull_effect_duration(pumpkin_util::Difficulty::Peaceful), 0);

        // 3. Kill heal amount for Wither boss owner (WitherSkull.java:71)
        assert_eq!(get_wither_skull_kill_heal_amount(), 5.0);
    }

    #[test]
    fn test_experience_bottle_shulker_bullet_and_llama_spit_parity() {
        // 1. Experience bottle XP bounds (ThrownExperienceBottle.java:48: 3 + rand(5) + rand(5))
        assert_eq!(calculate_experience_bottle_xp(0, 0), 3);
        assert_eq!(calculate_experience_bottle_xp(4, 4), 11);
        assert_eq!(calculate_experience_bottle_xp(2, 2), 7);
        assert_eq!(get_experience_bottle_gravity(), 0.07);
        assert_eq!(get_experience_bottle_splash_color(), -13_083_194);

        // 2. Shulker bullet damage, levitation duration, peaceful despawn, and deflect particles (ShulkerBullet.java:179-181, 285, 292, 333)
        assert_eq!(get_shulker_bullet_damage(), 4.0);
        assert_eq!(get_shulker_bullet_levitation_duration(), 200);
        assert!(should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Peaceful));
        assert!(!should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Easy));
        assert!(!should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Normal));
        assert!(!should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Hard));
        assert_eq!(get_shulker_bullet_deflect_particle_count(), 15);

        // 3. Llama spit damage, gravity, and air drag (LlamaSpit.java:39, 66, 75)
        assert_eq!(get_llama_spit_damage(), 1.0);
        assert_eq!(get_llama_spit_gravity(), 0.06);
        assert_eq!(get_llama_spit_air_drag(), 0.99);
    }

    #[test]
    fn test_egg_snowball_ender_pearl_and_evoker_fangs_parity() {
        // 1. Egg hatching probability (ThrownEgg.java:70-74: 1/8 chance -> 1/32 gives 4, else 1)
        assert_eq!(calculate_egg_hatch_count(0, 0), 4);
        assert_eq!(calculate_egg_hatch_count(0, 1), 1);
        assert_eq!(calculate_egg_hatch_count(0, 31), 1);
        assert_eq!(calculate_egg_hatch_count(1, 0), 0);
        assert_eq!(calculate_egg_hatch_count(7, 15), 0);

        // 2. Snowball blaze damage parity (Snowball.java:58-59)
        assert_eq!(get_snowball_damage(true), 3.0);
        assert_eq!(get_snowball_damage(false), 0.0);

        // 3. Ender Pearl Endermite spawn condition and self-damage (ThrownEnderpearl.java:107, 125)
        assert!(should_ender_pearl_spawn_endermite(0.04, true, pumpkin_util::Difficulty::Normal));
        assert!(should_ender_pearl_spawn_endermite(0.01, true, pumpkin_util::Difficulty::Hard));
        // Over chance threshold (> 0.05)
        assert!(!should_ender_pearl_spawn_endermite(0.06, true, pumpkin_util::Difficulty::Normal));
        // Peaceful difficulty exemption (never spawn in peaceful)
        assert!(!should_ender_pearl_spawn_endermite(0.01, true, pumpkin_util::Difficulty::Peaceful));
        // Monsters not spawning
        assert!(!should_ender_pearl_spawn_endermite(0.01, false, pumpkin_util::Difficulty::Normal));
        assert_eq!(get_ender_pearl_self_damage(), 5.0);

        // 4. Evoker Fangs damage, attack tick, and pitch (EvokerFangs.java:89, 110, 138)
        assert_eq!(get_evoker_fangs_damage(), 6.0);
        assert!(is_evoker_fangs_attack_tick(-8));
        assert!(!is_evoker_fangs_attack_tick(-7));
        assert!(!is_evoker_fangs_attack_tick(0));
        let pitch_low = calculate_evoker_fangs_sound_pitch(0.0);
        let pitch_high = calculate_evoker_fangs_sound_pitch(1.0);
        assert!((pitch_low - 0.85).abs() < 1e-6);
        assert!((pitch_high - 1.05).abs() < 1e-6);
    }

    #[test]
    fn test_large_fireball_and_eye_of_ender_parity() {
        // 1. Large Fireball hit damage & mob griefing check (LargeFireball.java:35, 48)
        assert_eq!(get_large_fireball_damage(), 6.0);
        assert!(can_large_fireball_grief(true));
        assert!(!can_large_fireball_grief(false));

        // 2. Eye of Ender survive logic (EyeOfEnder.java:84, 103, 109)
        // 4 in 5 survive (rand_5 in 1..=4)
        assert!(should_eye_of_ender_survive(1));
        assert!(should_eye_of_ender_survive(2));
        assert!(should_eye_of_ender_survive(3));
        assert!(should_eye_of_ender_survive(4));
        // 1 in 5 shatters (rand_5 == 0)
        assert!(!should_eye_of_ender_survive(0));
        assert_eq!(get_eye_of_ender_max_life(), 80);
        assert_eq!(get_eye_of_ender_break_level_event(), 2003);
    }

    #[test]
    fn test_food_data_saturation_sprint_and_starvation_parity() {
        // 1. Saturation modifier calculation (FoodConstants.java:31: nutrition * modifier * 2.0)
        // Cooked beef: 8 * 0.8 * 2.0 = 12.8
        let beef_sat = calculate_food_saturation_from_modifier(8, 0.8);
        assert!((beef_sat - 12.8).abs() < 1e-5);
        // Golden carrot: 6 * 1.2 * 2.0 = 14.4
        let carrot_sat = calculate_food_saturation_from_modifier(6, 1.2);
        assert!((carrot_sat - 14.4).abs() < 1e-5);

        // 2. Sprint permission by food level (FoodData.java:93: food > 6)
        assert!(can_player_sprint_with_food(7, false));
        assert!(can_player_sprint_with_food(20, false));
        assert!(!can_player_sprint_with_food(6, false));
        assert!(!can_player_sprint_with_food(0, false));
        // Flying overrides food restriction
        assert!(can_player_sprint_with_food(0, true));

        // 3. Starvation damage condition by difficulty (FoodData.java:63-65)
        // Hard: always takes damage even at 0.5 HP
        assert!(should_starve_take_damage(0.5, pumpkin_util::Difficulty::Hard));
        // Normal: takes damage until down to 1.0 HP (stops at 1.0)
        assert!(should_starve_take_damage(2.0, pumpkin_util::Difficulty::Normal));
        assert!(!should_starve_take_damage(1.0, pumpkin_util::Difficulty::Normal));
        // Easy: takes damage until down to 10.0 HP (stops at 10.0)
        assert!(should_starve_take_damage(11.0, pumpkin_util::Difficulty::Easy));
        assert!(!should_starve_take_damage(10.0, pumpkin_util::Difficulty::Easy));
        assert!(!should_starve_take_damage(5.0, pumpkin_util::Difficulty::Easy));

        // 4. Fast regen heal & cost (FoodData.java:48-50)
        let (heal_full, cost_full) = calculate_fast_regen_heal_and_cost(10.0);
        assert_eq!(cost_full, 6.0);
        assert_eq!(heal_full, 1.0);

        let (heal_partial, cost_partial) = calculate_fast_regen_heal_and_cost(3.0);
        assert_eq!(cost_partial, 3.0);
        assert_eq!(heal_partial, 0.5);

        // 5. Needs food check (FoodData.java:97)
        assert!(is_food_needs_eating(19));
        assert!(!is_food_needs_eating(20));
    }

    #[test]
    fn test_totem_death_protection_water_sensitivity_and_suffocation_parity() {
        // 1. Totem prevents death (LivingEntity.java:1394-1408)
        // Bypasses invulnerability (void / kill) prevents totem from triggering
        assert!(!should_totem_prevent_death(true, true));
        assert!(!should_totem_prevent_death(false, false));
        assert!(should_totem_prevent_death(false, true));

        // 2. Sound suppression (LivingEntity.java:1255-1267)
        // When totem triggers, neither death sound nor hurt sound is played
        assert!(!should_play_death_sound(true, true, true));
        assert!(!should_play_hurt_sound(true, true));
        // When dying without totem, death sound is played if took full damage
        assert!(should_play_death_sound(true, false, true));
        assert!(!should_play_death_sound(true, false, false));
        // When surviving, hurt sound is played
        assert!(should_play_hurt_sound(false, true));
        assert!(!should_play_hurt_sound(false, false));

        // 3. Water sensitivity (LivingEntity.java:3162-3174)
        assert!(is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::BLAZE));
        assert!(is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::ENDERMAN));
        assert!(is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::SNOW_GOLEM));
        assert!(is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::STRIDER));
        assert!(!is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::ZOMBIE));
        assert!(!is_entity_sensitive_to_water(&pumpkin_data::entity::EntityType::PLAYER));

        // 4. Entity cramming damage (LivingEntity.java:3223-3235)
        assert_eq!(CRAMMING_DAMAGE, 6.0);
        // maxCramming = 24. 23 non-passenger entities: no cramming
        assert!(!should_take_cramming_damage(24, 23, 0));
        // 24 entities (count > 24 - 1): random 1-in-4 check
        assert!(should_take_cramming_damage(24, 24, 0));
        assert!(!should_take_cramming_damage(24, 24, 1));
        assert!(!should_take_cramming_damage(0, 50, 0)); // maxCramming <= 0 disables cramming

        // 5. Out of border damage (LivingEntity.java:432-437)
        // Inside border safe zone
        assert_eq!(calculate_out_of_border_damage(5.0, 0.0, 0.2), None);
        // Outside border by 10 blocks with damage per block 0.2: ceil/floor(-(-10) * 0.2) = 2.0
        assert_eq!(calculate_out_of_border_damage(-10.0, 0.0, 0.2), Some(2.0));
        // Minimum damage is 1.0
        assert_eq!(calculate_out_of_border_damage(-1.0, 0.0, 0.2), Some(1.0));

        // 6. In-wall suffocation damage (LivingEntity.java:429-430, 3877-3878)
        assert_eq!(IN_WALL_DAMAGE, 1.0);
        assert!(should_take_in_wall_damage(false, true));
        assert!(!should_take_in_wall_damage(true, true)); // sleeping suppresses in_wall
        assert!(!should_take_in_wall_damage(false, false));
    }

    #[test]
    fn test_thrown_trident_water_inertia_loyalty_despawn_and_douse_parity() {
        // 1. Trident water inertia (ThrownTrident.java:34, 220-222)
        assert_eq!(calculate_trident_water_inertia(), 0.99);

        // 2. Trident Loyalty despawn immunity (ThrownTrident.java:212-217)
        // With loyalty > 0 and pickup allowed, trident never despawns
        assert!(!should_trident_despawn(1200, 1200, 1, true));
        assert!(!should_trident_despawn(5000, 1200, 3, true));
        // Without loyalty (loyalty == 0) and pickup allowed, it despawns at >= 1200
        assert!(should_trident_despawn(1200, 1200, 0, true));
        assert!(!should_trident_despawn(1199, 1200, 0, true));
        // Disallowed pickup with loyalty still despawns
        assert!(should_trident_despawn(1200, 1200, 1, false));

        // 3. Fishing tick speed (FishingHook.java:298-306)
        // Base speed = 1
        assert_eq!(calculate_fishing_tick_speed(false, true, 0.5, 0.8), 1);
        // Raining at hook with rand < 0.25 increases speed (+1)
        assert_eq!(calculate_fishing_tick_speed(true, true, 0.1, 0.8), 2);
        // No sky access with rand < 0.5 decreases speed (-1)
        assert_eq!(calculate_fishing_tick_speed(false, false, 0.5, 0.2), 0);
        // Both raining and no sky: 1 + 1 - 1 = 1
        assert_eq!(calculate_fishing_tick_speed(true, false, 0.1, 0.2), 1);

        // 4. Douse fire / campfire check (AbstractThrownPotion.java:124-135)
        assert!(is_douseable_fire_or_campfire(true, false, false));
        assert!(is_douseable_fire_or_campfire(false, true, false));
        assert!(is_douseable_fire_or_campfire(false, false, true));
        assert!(!is_douseable_fire_or_campfire(false, false, false));
    }

    #[test]
    fn test_fishing_rod_and_crossbow_multishot_parity() {
        // 1. Fishing rod retrieve durability damage (FishingHook.java:446, 472, 476)
        assert_eq!(calculate_fishing_rod_damage(Some(HookedTargetType::Item), false, false), 3);
        assert_eq!(calculate_fishing_rod_damage(Some(HookedTargetType::OtherEntity), false, false), 5);
        assert_eq!(calculate_fishing_rod_damage(None, true, false), 1);
        assert_eq!(calculate_fishing_rod_damage(None, false, true), 2);
        assert_eq!(calculate_fishing_rod_damage(None, false, false), 0);

        // 2. Fishing hook pull motion (FishingHook.java:503)
        let owner = Vector3::new(10.0, 64.0, 10.0);
        let hook = Vector3::new(0.0, 64.0, 0.0);
        let pull = calculate_fishing_hook_pull_motion(owner, hook);
        assert!((pull.x - 1.0).abs() < 1e-6);
        assert!((pull.y - 0.0).abs() < 1e-6);
        assert!((pull.z - 1.0).abs() < 1e-6);

        // 3. Caught item loot velocity (FishingHook.java:464)
        let caught_vel = calculate_caught_item_velocity(owner, hook);
        assert!((caught_vel.x - 1.0).abs() < 1e-6);
        // dist_sq = 100 + 0 + 100 = 200. sqrt(sqrt(200)) = (200)^0.25 ≈ 3.7606
        // y = 0.0 * 0.1 + 3.7606 * 0.08 ≈ 0.3008
        assert!(caught_vel.y > 0.25 && caught_vel.y < 0.35);

        // 4. Bobber throw & retrieve sound pitch (FishingRodItem.java:38, 50)
        let min_pitch = calculate_bobber_sound_pitch(1.0); // 0.4 / 1.2 ≈ 0.333
        let max_pitch = calculate_bobber_sound_pitch(0.0); // 0.4 / 0.8 = 0.5
        assert!((min_pitch - (0.4 / 1.2)).abs() < 1e-5);
        assert!((max_pitch - 0.5).abs() < 1e-5);

        // 5. Crossbow durability consumption (CrossbowItem.java:167-169 & ProjectileWeaponItem.java:71)
        assert_eq!(calculate_crossbow_durability_use(false, 1), 1);
        assert_eq!(calculate_crossbow_durability_use(false, 3), 3); // Multishot arrows = 3
        assert_eq!(calculate_crossbow_durability_use(true, 1), 3);
        assert_eq!(calculate_crossbow_durability_use(true, 3), 9);  // Multishot fireworks = 9

        // 6. Multishot projectile angles (ProjectileWeaponItem.java:54-63)
        // 3 projectiles with 10.0 spread: center (0°), left (-10°), right (+10°)
        assert_eq!(calculate_multishot_spread_angle(0, 3, 10.0), 0.0);
        assert_eq!(calculate_multishot_spread_angle(1, 3, 10.0), -10.0);
        assert_eq!(calculate_multishot_spread_angle(2, 3, 10.0), 10.0);
        // Single projectile: 0°
        assert_eq!(calculate_multishot_spread_angle(0, 1, 10.0), 0.0);

        // 7. Crossbow shot pitch (CrossbowItem.java:193-200)
        assert_eq!(calculate_crossbow_shot_pitch(0, 0.5), 1.0);
        let odd_pitch = calculate_crossbow_shot_pitch(1, 0.0); // 1.0/1.8 + 0.63 ≈ 1.185
        let even_pitch = calculate_crossbow_shot_pitch(2, 0.0); // 1.0/1.8 + 0.43 ≈ 0.985
        assert!(odd_pitch > even_pitch);

        // 8. Crossbow loading end pitch (CrossbowItem.java:234)
        let load_end_pitch = calculate_crossbow_loading_end_pitch(0.0); // 1.0/1.0 + 0.2 = 1.2
        assert!((load_end_pitch - 1.2).abs() < 1e-5);
    }

    #[test]
    fn test_bow_power_firework_mending_and_thorns_parity() {
        // 1. Bow power calculation (BowItem.java:74-82)
        assert_eq!(calculate_bow_power_for_time(0), 0.0);
        let mid_power = calculate_bow_power_for_time(10); // pow = 0.5; (0.25 + 1.0)/3.0 = 1.25/3.0 ≈ 0.4166667
        assert!((mid_power - (1.25 / 3.0)).abs() < 1e-5);
        assert_eq!(calculate_bow_power_for_time(20), 1.0);
        assert_eq!(calculate_bow_power_for_time(40), 1.0); // clamped at 1.0

        // 2. Bow release threshold (BowItem.java:35)
        assert!(!can_bow_release(0.09));
        assert!(can_bow_release(0.10));
        assert!(can_bow_release(1.0));

        // 3. Bow shoot sound pitch (BowItem.java:52)
        let bow_pitch = calculate_bow_sound_pitch(1.0, 0.0); // 1.0 / 1.2 + 0.5 ≈ 1.333
        assert!((bow_pitch - (1.0 / 1.2 + 0.5)).abs() < 1e-5);

        // 4. Firework horizontal collision acceleration (FireworkRocketEntity.java:152)
        assert_eq!(calculate_firework_horizontal_acceleration(true), 1.0);  // with horizontal collision: no acceleration
        assert_eq!(calculate_firework_horizontal_acceleration(false), 1.15); // without collision: 1.15 multiplier

        // 5. Mending repair calculation (ExperienceOrb.java:296-308)
        // 10 XP can repair up to 20 durability. Item has 20 damage -> full 20 repaired, 0 XP left
        let (repaired1, remaining1) = calculate_mending_repair(10, 20);
        assert_eq!(repaired1, 20);
        assert_eq!(remaining1, 0);

        // 10 XP can repair up to 20 durability. Item has 6 damage -> 6 repaired, 10 - (6 * 10) / 20 = 7 XP left
        let (repaired2, remaining2) = calculate_mending_repair(10, 6);
        assert_eq!(repaired2, 6);
        assert_eq!(remaining2, 7);

        // Item has 0 damage -> 0 repaired, all 10 XP left
        let (repaired3, remaining3) = calculate_mending_repair(10, 0);
        assert_eq!(repaired3, 0);
        assert_eq!(remaining3, 10);

        // 6. Thorns damage & durability loss (Enchantments.java:540-565)
        // Level 3 Thorns: 45% chance
        assert!(calculate_thorns_damage(3, 0.10, 0.5).is_some());
        assert!(calculate_thorns_damage(3, 0.50, 0.5).is_none());
        let (thorns_dmg, extra_durability) = calculate_thorns_damage(3, 0.10, 0.5).unwrap();
        assert!(thorns_dmg >= 1.0 && thorns_dmg <= 4.0);
        assert_eq!(extra_durability, 2); // 2 extra durability damage per trigger
    }

    #[test]
    fn test_tnt_and_lightning_parity() {
        // 1. Lightning thunder & impact pitch (LightningBolt.java:89, 100)
        assert_eq!(calculate_lightning_thunder_pitch(0.0), 0.8);
        assert_eq!(calculate_lightning_thunder_pitch(1.0), 1.0);
        assert_eq!(calculate_lightning_impact_pitch(0.0), 0.5);
        assert_eq!(calculate_lightning_impact_pitch(1.0), 0.7);

        // 2. Lightning fire spawn difficulty check (LightningBolt.java:105-108)
        assert!(!can_lightning_spawn_fire(pumpkin_util::Difficulty::Peaceful));
        assert!(!can_lightning_spawn_fire(pumpkin_util::Difficulty::Easy));
        assert!(can_lightning_spawn_fire(pumpkin_util::Difficulty::Normal));
        assert!(can_lightning_spawn_fire(pumpkin_util::Difficulty::Hard));

        // 3. TNT short fuse for chain reactions (PrimedTnt.java:183-185)
        // With default fuse 80: bound = 20, min = 10, max = 29
        let min_fuse = calculate_tnt_short_fuse(80, 0);
        let max_fuse = calculate_tnt_short_fuse(80, 19);
        assert_eq!(min_fuse, 10);
        assert_eq!(max_fuse, 29);

        // 4. TNT explosion origin Y offset (PrimedTnt.java:135)
        let tnt_y = calculate_tnt_explosion_pos_y(64.0, 0.98);
        assert!((tnt_y - (64.0 + 0.98 * 0.0625)).abs() < 1e-6);

        // 5. Primed TNT invulnerability (PrimedTnt.java:210-212)
        assert!(!should_tnt_take_damage());
    }

    #[test]
    fn test_bubble_column_and_lightning_conversion_parity() {
        // 1. Entity above bubble column (Entity.java:2945-2954)
        // Drag down: (y - 0.03).max(-0.9)
        assert!((calculate_entity_bubble_column_above_y(0.0, true) - (-0.03)).abs() < 1e-6);
        assert!((calculate_entity_bubble_column_above_y(-1.0, true) - (-0.9)).abs() < 1e-6);
        // Upwards: (y + 0.1).min(1.8)
        assert!((calculate_entity_bubble_column_above_y(0.0, false) - 0.1).abs() < 1e-6);
        assert!((calculate_entity_bubble_column_above_y(2.0, false) - 1.8).abs() < 1e-6);

        // 2. Entity inside bubble column (Entity.java:2980-2991)
        // Drag down: (y - 0.03).max(-0.3)
        assert!((calculate_entity_bubble_column_inside_y(0.0, true) - (-0.03)).abs() < 1e-6);
        assert!((calculate_entity_bubble_column_inside_y(-0.5, true) - (-0.3)).abs() < 1e-6);
        // Upwards: (y + 0.06).min(0.7)
        assert!((calculate_entity_bubble_column_inside_y(0.0, false) - 0.06).abs() < 1e-6);
        assert!((calculate_entity_bubble_column_inside_y(1.0, false) - 0.7).abs() < 1e-6);

        // 3. Boat bubble column effect (AbstractBoat.java:346-354)
        // Drag down: y - 0.7
        assert!((calculate_boat_bubble_column_effect_y(0.1, true) - (-0.6)).abs() < 1e-6);
        // Upwards: fixed 0.6
        assert!((calculate_boat_bubble_column_effect_y(0.0, false) - 0.6).abs() < 1e-6);
        assert!((calculate_boat_bubble_column_effect_y(-0.5, false) - 0.6).abs() < 1e-6);

        // 4. Turtle lightning hit damage (Turtle.java:283-285)
        assert_eq!(get_turtle_thunder_hit_damage(), f32::MAX);

        // 5. Lightning mob conversion difficulty gating (Pig.java:201, Villager.java:745)
        assert!(!can_lightning_convert_mob(pumpkin_util::Difficulty::Peaceful));
        assert!(can_lightning_convert_mob(pumpkin_util::Difficulty::Easy));
        assert!(can_lightning_convert_mob(pumpkin_util::Difficulty::Normal));
        assert!(can_lightning_convert_mob(pumpkin_util::Difficulty::Hard));
    }

    #[test]
    fn test_iron_golem_and_snow_golem_parity() {
        // 0. Golem constants parity (IronGolem.java & SnowGolem.java)
        assert_eq!(IRON_GOLEM_MAX_HEALTH, 100.0);
        assert_eq!(IRON_GOLEM_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(IRON_GOLEM_KNOCKBACK_RESISTANCE, 1.0);
        assert_eq!(IRON_GOLEM_BASE_ATTACK_DAMAGE, 15.0);
        assert_eq!(IRON_GOLEM_STEP_HEIGHT, 1.0);
        assert_eq!(IRON_GOLEM_IRON_INGOT_HEAL_AMOUNT, 25.0);
        assert_eq!(IRON_GOLEM_ATTACK_ANIMATION_TICKS, 10);
        assert_eq!(IRON_GOLEM_OFFER_FLOWER_TICKS, 400);
        assert_eq!(IRON_GOLEM_FLING_UPWARD_VELOCITY, 0.4);
        assert_eq!(IRON_GOLEM_XP_REWARD, 0);

        assert_eq!(SNOW_GOLEM_MAX_HEALTH, 4.0);
        assert_eq!(SNOW_GOLEM_BASE_MOVEMENT_SPEED, 0.2);
        assert_eq!(SNOW_GOLEM_SHOOT_VELOCITY, 1.6);
        assert_eq!(SNOW_GOLEM_SHOOT_INACCURACY, 12.0);
        assert_eq!(SNOW_GOLEM_MELT_DAMAGE, 1.0);
        assert_eq!(SNOW_GOLEM_XP_REWARD, 0);

        // 1. Iron Golem attack damage random formula (IronGolem.java:193)
        // With base_attack = 14.0: damage = 7.0 + random.nextInt(14) (range 7.0 to 20.0)
        let min_dmg = calculate_iron_golem_attack_damage(14.0, 0);
        let max_dmg = calculate_iron_golem_attack_damage(14.0, 13);
        assert_eq!(min_dmg, 7.0);
        assert_eq!(max_dmg, 20.0);

        // 2. Iron Golem fling upward velocity (IronGolem.java:198-199)
        // Without knockback resistance (0.0): +0.4 Y velocity
        assert!((calculate_iron_golem_fling_velocity_y(0.0, 0.0) - 0.4).abs() < 1e-6);
        // With 50% knockback resistance (0.5): +0.2 Y velocity
        assert!((calculate_iron_golem_fling_velocity_y(0.0, 0.5) - 0.2).abs() < 1e-6);
        // With 100% knockback resistance (1.0): 0.0 Y velocity added
        assert_eq!(calculate_iron_golem_fling_velocity_y(0.0, 1.0), 0.0);

        // 3. Iron Golem repair pitch (IronGolem.java:273)
        // 1.0 + (rand1 - rand2) * 0.2
        assert_eq!(calculate_iron_golem_repair_pitch(0.5, 0.5), 1.0);
        assert_eq!(calculate_iron_golem_repair_pitch(1.0, 0.0), 1.2);
        assert_eq!(calculate_iron_golem_repair_pitch(0.0, 1.0), 0.8);

        // 4. Iron Golem crackiness tiers (IronGolem.java:219)
        // Max health 100.0:
        assert_eq!(calculate_iron_golem_crackiness(100.0, 100.0), 0); // None
        assert_eq!(calculate_iron_golem_crackiness(76.0, 100.0), 0);  // None
        assert_eq!(calculate_iron_golem_crackiness(75.0, 100.0), 1);  // Low
        assert_eq!(calculate_iron_golem_crackiness(51.0, 100.0), 1);  // Low
        assert_eq!(calculate_iron_golem_crackiness(50.0, 100.0), 2);  // Medium
        assert_eq!(calculate_iron_golem_crackiness(26.0, 100.0), 2);  // Medium
        assert_eq!(calculate_iron_golem_crackiness(25.0, 100.0), 3);  // High
        assert_eq!(calculate_iron_golem_crackiness(5.0, 100.0), 3);   // High

        // 5. Snow Golem shoot pitch (SnowGolem.java:133)
        // 0.4 / (rand * 0.4 + 0.8)
        assert_eq!(calculate_snow_golem_shoot_pitch(0.0), 0.5); // 0.4 / 0.8 = 0.5
        assert!((calculate_snow_golem_shoot_pitch(1.0) - (0.4 / 1.2)).abs() < 1e-6);
    }

    #[test]
    fn test_enderman_and_shulker_parity() {
        // 1. Enderman projectile dodge (EnderMan.java:374)
        // When not a passenger: evades projectile (cancel=true, teleport=true)
        let (cancel, teleport) = should_enderman_evade_damage(true, false, false, 0);
        assert!(cancel);
        assert!(teleport);

        // When riding a boat/minecart (is_passenger=true): cannot evade projectile
        let (cancel, teleport) = should_enderman_evade_damage(true, true, false, 0);
        assert!(!cancel);
        assert!(!teleport);

        // 2. Enderman environmental/water/rain damage evasion (EnderMan.java:386-388)
        // Non-living damage source: damage NOT cancelled, 90% teleport chance (rand_int_10 != 0)
        let (cancel, teleport) = should_enderman_evade_damage(false, false, false, 1);
        assert!(!cancel);
        assert!(teleport);
        let (cancel, teleport) = should_enderman_evade_damage(false, false, false, 0);
        assert!(!cancel);
        assert!(!teleport);

        // Attack from living entity: damage NOT cancelled, does not auto-teleport from this formula
        let (cancel, teleport) = should_enderman_evade_damage(false, false, true, 1);
        assert!(!cancel);
        assert!(!teleport);

        // Shulker constants parity (Shulker.java:68, 69, 76, 79, 92, 148, 453-455, 632)
        assert_eq!(SHULKER_MAX_HEALTH, 30.0);
        assert_eq!(SHULKER_COVERED_ARMOR_BONUS, 20.0);
        assert_eq!(SHULKER_COVERED_ARMOR_MODIFIER_ID, "minecraft:covered");
        assert_eq!(SHULKER_XP_REWARD, 5);
        assert_eq!(SHULKER_PEEK_PER_TICK, 0.05);
        assert_eq!(SHULKER_MAX_TELEPORT_DISTANCE, 8);
        assert_eq!(SHULKER_ATTACK_RANGE_SQ, 400.0);
        assert_eq!(SHULKER_BULLET_SPEED, 0.15);
        assert!(is_shulker_closed(0));
        assert!(!is_shulker_closed(1));
        assert!(!is_shulker_closed(100));

        // 3. Shulker arrow deflection when closed (Shulker.java:430-435)
        assert!(should_shulker_deflect_arrow(true, true));   // closed + arrow -> deflects
        assert!(!should_shulker_deflect_arrow(false, true));  // open + arrow -> does not deflect
        assert!(!should_shulker_deflect_arrow(true, false));  // closed + non-arrow -> does not deflect

        // 4. Shulker low-health teleport (Shulker.java:441)
        // Health < 50% max and rand % 4 == 0
        assert!(should_shulker_teleport_on_low_health(14.0, 30.0, 0));
        assert!(!should_shulker_teleport_on_low_health(14.0, 30.0, 1));
        assert!(!should_shulker_teleport_on_low_health(15.0, 30.0, 0)); // 15.0 is not < 15.0

        // 5. Shulker duplication failure chance (Shulker.java:461-463)
        // (count - 1) / 5.0
        assert_eq!(calculate_shulker_duplication_failure_chance(1), 0.0); // 100% success
        assert_eq!(calculate_shulker_duplication_failure_chance(2), 0.2); // 80% success
        assert_eq!(calculate_shulker_duplication_failure_chance(3), 0.4); // 60% success
        assert_eq!(calculate_shulker_duplication_failure_chance(6), 1.0); // 0% success (cap)
        assert_eq!(calculate_shulker_duplication_failure_chance(10), 1.0);

        // 6. Shulker duplication conditions (Shulker.java:460-464)
        assert!(can_shulker_duplicate(false, true, 0.2, 0.5)); // open, teleported, rand >= 0.2
        assert!(!can_shulker_duplicate(true, true, 0.2, 0.5));  // closed cannot duplicate
        assert!(!can_shulker_duplicate(false, false, 0.2, 0.5)); // failed teleport cannot duplicate
        assert!(!can_shulker_duplicate(false, true, 0.2, 0.1));  // rand < failure_chance fails
    }

    #[test]
    fn test_creeper_and_blaze_parity() {
        // 1. Creeper igniters (Creeper.java:212, ItemTags.CREEPER_IGNITERS)
        assert!(is_creeper_igniter(pumpkin_data::item::Item::FLINT_AND_STEEL.id));
        assert!(is_creeper_igniter(pumpkin_data::item::Item::FIRE_CHARGE.id));
        assert!(!is_creeper_igniter(pumpkin_data::item::Item::TORCH.id));

        // 2. Creeper ignite sound pitch (Creeper.java:214)
        // rand * 0.4 + 0.8
        assert_eq!(calculate_creeper_ignite_sound_pitch(0.0), 0.8);
        assert!((calculate_creeper_ignite_sound_pitch(1.0) - 1.2).abs() < 1e-6);

        // 3. Creeper explosion radius multiplier (Creeper.java:232)
        // Base 3: normal -> 3.0, charged (isPowered) -> 6.0
        assert_eq!(calculate_creeper_explosion_radius(3.0, false), 3.0);
        assert_eq!(calculate_creeper_explosion_radius(3.0, true), 6.0);

        // 4. Creeper fall swell accumulation (Creeper.java:89-92)
        // swell += (int)(fallDistance * 1.5), capped at maxSwell - 5 (25 for max 30)
        assert_eq!(calculate_creeper_fall_swell(0, 30, 2.0), 3);
        assert_eq!(calculate_creeper_fall_swell(10, 30, 10.0), 25); // 10 + 15 = 25 (capped)
        assert_eq!(calculate_creeper_fall_swell(0, 30, 20.0), 25);  // 30 clamped to 25

        // 5. Creeper target filtering (Creeper.java:154-158)
        // Creeper never targets Goat
        assert!(!can_creeper_target_entity(&pumpkin_data::entity::EntityType::GOAT));
        assert!(can_creeper_target_entity(&pumpkin_data::entity::EntityType::PLAYER));
        assert!(can_creeper_target_entity(&pumpkin_data::entity::EntityType::IRON_GOLEM));

        // 6. Blaze water/rain sensitivity (LivingEntity.java:3162-3174, Blaze.java:114)
        assert!(should_blaze_take_water_damage(true, false));  // touching water
        assert!(should_blaze_take_water_damage(false, true));  // raining
        assert!(!should_blaze_take_water_damage(false, false)); // dry

        // 7. Blaze combat constants parity (Blaze.java:40, 55, 87, 122, 123, 129, 208, 220, 223, 225, 233)
        assert_eq!(BLAZE_MAX_HEALTH, 20.0);
        assert_eq!(BLAZE_BASE_MOVEMENT_SPEED, 0.23);
        assert_eq!(BLAZE_ATTACK_DAMAGE, 6.0);
        assert_eq!(BLAZE_FOLLOW_RANGE, 48.0);
        assert_eq!(BLAZE_XP_REWARD, 10);
        assert_eq!(BLAZE_FALL_SPEED_MULTIPLIER, 0.6);
        assert_eq!(BLAZE_HEIGHT_OFFSET_INTERVAL_TICKS, 100);
        assert_eq!(BLAZE_HEIGHT_OFFSET_SPREAD, 6.891);
        assert_eq!(BLAZE_ASCENT_ACCELERATION, 0.3);
        assert_eq!(BLAZE_MELEE_ATTACK_INTERVAL, 20);
        assert_eq!(BLAZE_FIREBALL_WARMUP_TICKS, 60);
        assert_eq!(BLAZE_FIREBALL_BURST_INTERVAL_TICKS, 6);
        assert_eq!(BLAZE_FIREBALL_COOLDOWN_TICKS, 100);
        assert_eq!(BLAZE_SHOOT_SOUND_EVENT_ID, 1018);
    }

    #[test]
    fn test_guardian_parity() {
        // 1. Guardian thorns spikes retaliation (Guardian.java:311-317)
        // Only retaliates when !isMoving(), !is(THORNS), !is(AVOIDS_GUARDIAN_THORNS), and direct attacker is LivingEntity
        assert!(should_guardian_retaliate_thorns(
            false,
            &pumpkin_data::damage::DamageType::MOB_ATTACK,
            true
        ));
        assert!(should_guardian_retaliate_thorns(
            false,
            &pumpkin_data::damage::DamageType::PLAYER_ATTACK,
            true
        ));

        // When moving, spikes are retracted -> no retaliation
        assert!(!should_guardian_retaliate_thorns(
            true,
            &pumpkin_data::damage::DamageType::PLAYER_ATTACK,
            true
        ));

        // Damage type THORNS never triggers counter-thorns (infinite loop prevention)
        assert!(!should_guardian_retaliate_thorns(
            false,
            &pumpkin_data::damage::DamageType::THORNS,
            true
        ));

        // Non-living direct entity (e.g. arrow, dispenser projectile) does NOT take thorns
        assert!(!should_guardian_retaliate_thorns(
            false,
            &pumpkin_data::damage::DamageType::ARROW,
            false
        ));

        // Thorns damage constant is exactly 2.0F
        assert_eq!(GUARDIAN_THORNS_DAMAGE, 2.0);

        // 2. Guardian & Elder Guardian laser damage (Guardian.java:403-410)
        // Base 1.0F; +2.0 Hard difficulty; +2.0 Elder Guardian
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Normal, false),
            1.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Easy, false),
            1.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Hard, false),
            3.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Normal, true),
            3.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Hard, true),
            5.0
        );

        // 3. Attack durations and constants parity (Guardian.java & ElderGuardian.java)
        assert_eq!(GUARDIAN_MAX_HEALTH, 30.0);
        assert_eq!(GUARDIAN_BASE_MOVEMENT_SPEED, 0.5);
        assert_eq!(GUARDIAN_BASE_ATTACK_DAMAGE, 6.0);
        assert_eq!(GUARDIAN_XP_REWARD, 10);
        assert_eq!(GUARDIAN_LASER_WARMUP_TICKS, 10);
        assert_eq!(GUARDIAN_ATTACK_DURATION_TICKS, 80);
        assert_eq!(get_guardian_attack_duration(false), 80);

        assert_eq!(ELDER_GUARDIAN_MAX_HEALTH, 80.0);
        assert_eq!(ELDER_GUARDIAN_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(ELDER_GUARDIAN_BASE_ATTACK_DAMAGE, 8.0);
        assert_eq!(ELDER_GUARDIAN_XP_REWARD, 10);
        assert_eq!(ELDER_GUARDIAN_ATTACK_DURATION_TICKS, 60);
        assert_eq!(get_guardian_attack_duration(true), 60);

        // 4. Elder Guardian curse constants & effect instance (ElderGuardian.java:23-26, 70)
        assert_eq!(ELDER_GUARDIAN_CURSE_INTERVAL_TICKS, 1200);
        assert_eq!(ELDER_GUARDIAN_CURSE_RADIUS, 50.0);
        assert_eq!(ELDER_GUARDIAN_CURSE_DURATION_TICKS, 6000);
        assert_eq!(ELDER_GUARDIAN_CURSE_AMPLIFIER, 2);

        let curse = create_elder_guardian_curse_effect();
        assert_eq!(curse.effect_type, &pumpkin_data::effect::StatusEffect::MINING_FATIGUE);
        assert_eq!(curse.duration, 6000);
        assert_eq!(curse.amplifier, 2);
        assert!(curse.show_particles);
        assert!(curse.show_icon);

        // 5. Extended Guardian & Elder Guardian Parity (ElderGuardian.java:20, 25, Guardian.java:260-262, 360, 433)
        assert_eq!(ELDER_GUARDIAN_SIZE_SCALE, 2.35);
        assert_eq!(GUARDIAN_MIN_ATTACK_DISTANCE_SQ, 9.0);
        assert_eq!(ELDER_GUARDIAN_CURSE_DISPLAY_LIMIT_TICKS, 1200);
        assert_eq!(GUARDIAN_FLOP_VERTICAL_VELOCITY, 0.5);
        assert_eq!(GUARDIAN_FLOP_HORIZONTAL_AMPLITUDE, 0.4);
    }

    #[test]
    fn test_witch_parity() {
        use pumpkin_data::potion::Potion;

        // 1. Witch drink & throw sound pitches (Witch.java:165, 265: 0.8F + rand * 0.4F)
        assert_eq!(calculate_witch_drink_pitch(0.0), 0.8);
        assert!((calculate_witch_drink_pitch(1.0) - 1.2).abs() < 1e-6);
        assert_eq!(calculate_witch_throw_pitch(0.0), 0.8);
        assert!((calculate_witch_throw_pitch(1.0) - 1.2).abs() < 1e-6);

        // 2. Witch drinking decisions (Witch.java:136-149)
        // Water breathing
        assert!(should_witch_drink_water_breathing(0.1, true, false));
        assert!(!should_witch_drink_water_breathing(0.2, true, false));
        assert!(!should_witch_drink_water_breathing(0.1, false, false));
        assert!(!should_witch_drink_water_breathing(0.1, true, true));

        // Fire resistance
        assert!(should_witch_drink_fire_resistance(0.1, true, false, false));
        assert!(should_witch_drink_fire_resistance(0.1, false, true, false));
        assert!(!should_witch_drink_fire_resistance(0.2, true, false, false));
        assert!(!should_witch_drink_fire_resistance(0.1, true, false, true));

        // Healing
        assert!(should_witch_drink_healing(0.04, 20.0, 26.0));
        assert!(!should_witch_drink_healing(0.06, 20.0, 26.0));
        assert!(!should_witch_drink_healing(0.04, 26.0, 26.0));

        // Swiftness
        assert!(should_witch_drink_speed(0.4, true, 144.0, false));
        assert!(!should_witch_drink_speed(0.6, true, 144.0, false));
        assert!(!should_witch_drink_speed(0.4, false, 144.0, false));
        assert!(!should_witch_drink_speed(0.4, true, 100.0, false));
        assert!(!should_witch_drink_speed(0.4, true, 144.0, true));

        // 3. Witch thrown potion selection (Witch.java:230-246)
        // Raider ally
        assert_eq!(determine_witch_thrown_potion(true, 4.0, 5.0, false, false, false, 0.5).id, Potion::HEALING.id);
        assert_eq!(determine_witch_thrown_potion(true, 10.0, 5.0, false, false, false, 0.5).id, Potion::REGENERATION.id);

        // Enemy: Slowness if dist >= 8.0 and not slowness
        assert_eq!(determine_witch_thrown_potion(false, 20.0, 10.0, false, false, false, 0.5).id, Potion::SLOWNESS.id);

        // Enemy: Poison if dist < 8.0, health >= 8.0 and not poison
        assert_eq!(determine_witch_thrown_potion(false, 10.0, 5.0, false, false, false, 0.5).id, Potion::POISON.id);

        // Enemy: Weakness if dist <= 3.0, not weakness and rand < 0.25
        assert_eq!(determine_witch_thrown_potion(false, 6.0, 2.0, false, true, false, 0.1).id, Potion::WEAKNESS.id);

        // Enemy: Default is Harming
        assert_eq!(determine_witch_thrown_potion(false, 6.0, 5.0, false, true, false, 0.5).id, Potion::HARMING.id);

        // 4. Witch self-damage immunity & magic resistance (Witch.java:211-217)
        assert_eq!(calculate_witch_damage_received(10.0, true, true), 0.0);
        assert_eq!(calculate_witch_damage_received(10.0, true, false), 0.0);
        assert_eq!(calculate_witch_damage_received(10.0, false, true), 1.5);
        assert_eq!(calculate_witch_damage_received(10.0, false, false), 10.0);
    }

    #[test]
    fn test_phantom_and_ghast_parity() {
        // Phantom constants & flap parity (Phantom.java:45, 46, 54, 84, 89, 222, 463)
        assert_eq!(PHANTOM_MAX_HEALTH, 20.0);
        assert_eq!(PHANTOM_BASE_ATTACK_DAMAGE, 6.0);
        assert_eq!(PHANTOM_XP_REWARD, 5);
        assert_eq!(PHANTOM_MAX_SIZE, 64);
        assert_eq!(PHANTOM_TICKS_PER_FLAP, 25);
        assert!((PHANTOM_FLAP_DEGREES_PER_TICK - 7.448451).abs() < 1e-6);
        assert_eq!(PHANTOM_CAT_CHECK_RADIUS, 16.0);
        assert_eq!(PHANTOM_TARGET_SEARCH_XZ, 16.0);
        assert_eq!(PHANTOM_TARGET_SEARCH_Y, 64.0);
        assert!(is_phantom_flapping(0, 0));
        assert!(is_phantom_flapping(0, 25));
        assert!(!is_phantom_flapping(0, 1));
        assert!(is_phantom_flapping(1, 22)); // (1*3 + 22) = 25 % 25 == 0

        // 1. Phantom attack damage & dimension scaling (Phantom.java:89, 197)
        assert_eq!(calculate_phantom_attack_damage(0), 6.0);
        assert_eq!(calculate_phantom_attack_damage(1), 7.0);
        assert_eq!(calculate_phantom_attack_damage(64), 70.0);
        assert_eq!(calculate_phantom_attack_damage(-5), 6.0);
        assert_eq!(calculate_phantom_attack_damage(100), 70.0);

        assert_eq!(calculate_phantom_dimension_scale(0), 1.0);
        assert!((calculate_phantom_dimension_scale(1) - 1.15).abs() < 1e-6);
        assert!((calculate_phantom_dimension_scale(10) - 2.5).abs() < 1e-6);

        // 2. Phantom swoop & flap pitch formulas (Phantom.java:277, 118)
        assert_eq!(calculate_phantom_swoop_pitch(0.0), 0.95);
        assert!((calculate_phantom_swoop_pitch(1.0) - 1.05).abs() < 1e-6);
        assert_eq!(calculate_phantom_flap_pitch(0.0), 0.95);
        assert!((calculate_phantom_flap_pitch(1.0) - 1.00).abs() < 1e-6);

        // 3. Phantom swoop abort conditions (Phantom.java:493)
        assert!(should_phantom_abort_swoop_on_collision_or_damage(true, false));
        assert!(should_phantom_abort_swoop_on_collision_or_damage(false, true));
        assert!(!should_phantom_abort_swoop_on_collision_or_damage(false, false));

        // 4. Ghast reflected fireball detection & instakill damage (Ghast.java:76-78, 101-108)
        assert!(is_ghast_reflected_fireball(true, true));
        assert!(!is_ghast_reflected_fireball(true, false));  // non-player reflected
        assert!(!is_ghast_reflected_fireball(false, true));  // non-fireball player attack

        assert_eq!(calculate_ghast_damage_received(6.0, true), 1000.0);
        assert_eq!(calculate_ghast_damage_received(6.0, false), 6.0);

        // 5. Ghast constants parity (Ghast.java:46, 51, 118-121, 146, 195, 361, 365, 379)
        assert_eq!(GHAST_MAX_HEALTH, 10.0);
        assert_eq!(GHAST_FOLLOW_RANGE, 100.0);
        assert_eq!(GHAST_CAMERA_DISTANCE, 8.0);
        assert_eq!(GHAST_FLYING_SPEED, 0.06);
        assert_eq!(GHAST_XP_REWARD, 5);
        assert_eq!(GHAST_SOUND_VOLUME, 5.0);
        assert_eq!(GHAST_DEFAULT_EXPLOSION_POWER, 1);
        assert_eq!(GHAST_REFLECTED_FIREBALL_DAMAGE, 1000.0);
        assert_eq!(GHAST_ATTACK_RANGE_SQ, 4096.0);
        assert_eq!(GHAST_FIREBALL_WARN_TICKS, 10);
        assert_eq!(GHAST_FIREBALL_SHOOT_TICKS, 20);
        assert_eq!(GHAST_FIREBALL_COOLDOWN_TICKS, -40);
        assert_eq!(GHAST_WARN_SOUND_EVENT_ID, 1015);
        assert_eq!(GHAST_SHOOT_SOUND_EVENT_ID, 1016);
        assert_eq!(GHAST_FLOAT_MOVE_SCALE, 0.1);
        assert_eq!(GHAST_FLOAT_WANDER_RADIUS, 16.0);
        assert_eq!(GHAST_FLOAT_MIN_DIST_SQ, 1.0);
        assert_eq!(GHAST_FLOAT_MAX_DIST_SQ, 3600.0);
    }

    #[test]
    fn test_slime_and_magma_cube_parity() {
        // 1. Attack damage scaling & contact damage permission
        // Slime: tiny (size 1) deals 0 damage, size 2 deals 2, size 4 deals 4
        assert_eq!(calculate_cube_mob_attack_damage(1, false), 0.0);
        assert_eq!(calculate_cube_mob_attack_damage(2, false), 2.0);
        assert_eq!(calculate_cube_mob_attack_damage(4, false), 4.0);
        assert!(!should_cube_mob_deal_contact_damage(1, false, true));
        assert!(should_cube_mob_deal_contact_damage(2, false, true));
        assert!(!should_cube_mob_deal_contact_damage(2, false, false)); // no effective AI

        // Magma Cube: size 1 deals 3 damage (size + 2), size 2 deals 4, size 4 deals 6
        assert_eq!(calculate_cube_mob_attack_damage(1, true), 3.0);
        assert_eq!(calculate_cube_mob_attack_damage(2, true), 4.0);
        assert_eq!(calculate_cube_mob_attack_damage(4, true), 6.0);
        assert!(should_cube_mob_deal_contact_damage(1, true, true));
        assert!(!should_cube_mob_deal_contact_damage(1, true, false)); // no effective AI

        // 2. Armor points
        assert_eq!(calculate_cube_mob_armor(1, false), 0.0);
        assert_eq!(calculate_cube_mob_armor(4, false), 0.0);
        assert_eq!(calculate_cube_mob_armor(1, true), 3.0);
        assert_eq!(calculate_cube_mob_armor(2, true), 6.0);
        assert_eq!(calculate_cube_mob_armor(4, true), 12.0);

        // 3. Experience reward
        assert_eq!(calculate_cube_mob_xp_reward(1), 1);
        assert_eq!(calculate_cube_mob_xp_reward(2), 2);
        assert_eq!(calculate_cube_mob_xp_reward(4), 4);

        // 4. Squish decay factor
        assert_eq!(calculate_cube_mob_squish_decay(false), 0.6);
        assert_eq!(calculate_cube_mob_squish_decay(true), 0.9);

        // 5. Jump velocity boost & lava jump
        assert_eq!(calculate_cube_mob_jump_velocity(0.42, 1, false), 0.42);
        assert!((calculate_cube_mob_jump_velocity(0.42, 1, true) - 0.52).abs() < 1e-6);
        assert!((calculate_cube_mob_jump_velocity(0.42, 4, true) - 0.82).abs() < 1e-6);

        assert!((calculate_magma_cube_lava_jump_velocity(1) - 0.27).abs() < 1e-6);
        assert!((calculate_magma_cube_lava_jump_velocity(4) - 0.42).abs() < 1e-6);

        // 6. Sound pitch formulas
        // Tiny jump pitch: adjuster 1.4
        assert!((calculate_cube_mob_sound_pitch(true, 0.0) - 1.4).abs() < 1e-6);
        // Normal jump pitch: adjuster 0.8
        assert!((calculate_cube_mob_sound_pitch(false, 0.0) - 0.8).abs() < 1e-6);
        // Squish landing pitch: div 0.8 = 1.25
        assert!((calculate_cube_mob_squish_pitch(0.0) - 1.25).abs() < 1e-6);
        // Attack pitch
        assert!((calculate_cube_mob_attack_pitch(0.0) - 1.0).abs() < 1e-6);

        // 7. Fire & Fall damage immunity for Magma Cube (EntityTypeTags.FALL_DAMAGE_IMMUNE)
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::IN_FIRE));
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::ON_FIRE));
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::LAVA));
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::HOT_FLOOR));
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::CAMPFIRE));
        assert!(is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::FALL));
        assert!(!is_magma_cube_damage_negated(&pumpkin_data::damage::DamageType::PLAYER_ATTACK));

        // 8. Size limits, passenger attachment & natural spawn size parity
        assert_eq!(CUBE_MOB_MIN_SIZE, 1);
        assert_eq!(CUBE_MOB_MAX_SIZE, 127);
        assert_eq!(CUBE_MOB_MAX_NATURAL_SIZE, 4);
        assert_eq!(CUBE_MOB_ATTACK_TIRED_TICKS, 300);
        assert_eq!(CUBE_MOB_TARGET_PLAYER_MAX_Y_DIFF, 4.0);
        assert_eq!(MAGMA_CUBE_JUMP_DELAY_MULTIPLIER, 4);
        assert_eq!(MAGMA_CUBE_ATTACK_DAMAGE_BONUS, 2.0);
        assert_eq!(MAGMA_CUBE_ARMOR_PER_SIZE, 3.0);
        assert_eq!(MAGMA_CUBE_JUMP_BOOST_PER_SIZE, 0.1);
        assert_eq!(MAGMA_CUBE_LAVA_JUMP_BASE, 0.22);
        assert_eq!(MAGMA_CUBE_LAVA_JUMP_PER_SIZE, 0.05);
        assert_eq!(SLIME_SQUISH_DECAY, 0.6);
        assert_eq!(MAGMA_CUBE_SQUISH_DECAY, 0.9);
        assert_eq!(CUBE_MOB_FLOAT_JUMP_CHANCE, 0.8);
        assert_eq!(CUBE_MOB_FLOAT_WANTED_MOVEMENT, 1.2);
        assert_eq!(CUBE_MOB_KEEP_JUMPING_WANTED_MOVEMENT, 1.0);
        assert_eq!(CUBE_MOB_RANDOM_DIRECTION_MIN_INTERVAL, 40);
        assert_eq!(CUBE_MOB_RANDOM_DIRECTION_EXTRA_INTERVAL, 60);
        assert_eq!(CUBE_MOB_SPLIT_MIN_COUNT, 2);
        assert_eq!(CUBE_MOB_SPLIT_EXTRA_COUNT, 3);

        // Passenger attachment test: height - 0.015625 * size * scale
        let pass_y = calculate_cube_mob_passenger_attachment_y(2.0, 4, 1.0);
        assert!((pass_y - (2.0 - 0.015625 * 4.0)).abs() < 1e-6);

        // Targeting Y difference test
        assert!(is_target_player_within_cube_y_range(64.0, 68.0));
        assert!(is_target_player_within_cube_y_range(68.0, 64.0));
        assert!(!is_target_player_within_cube_y_range(64.0, 68.1));

        // Natural spawn size test
        assert_eq!(calculate_cube_mob_natural_spawn_size(0, 1.0, 0.8), 1);
        assert_eq!(calculate_cube_mob_natural_spawn_size(1, 1.0, 0.8), 2);
        assert_eq!(calculate_cube_mob_natural_spawn_size(2, 1.0, 0.8), 4);
        assert_eq!(calculate_cube_mob_natural_spawn_size(0, 1.0, 0.1), 2); // promoted
        assert_eq!(calculate_cube_mob_natural_spawn_size(1, 1.0, 0.1), 4); // promoted
    }

    #[test]
    fn test_ravager_vex_and_zoglin_parity() {
        // 0. Ravager constants parity (Ravager.java)
        assert_eq!(RAVAGER_MAX_HEALTH, 100.0);
        assert_eq!(RAVAGER_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(RAVAGER_ATTACK_MOVEMENT_SPEED, 0.35);
        assert_eq!(RAVAGER_KNOCKBACK_RESISTANCE, 0.75);
        assert_eq!(RAVAGER_ATTACK_DAMAGE, 12.0);
        assert_eq!(RAVAGER_ATTACK_KNOCKBACK, 1.5);
        assert_eq!(RAVAGER_FOLLOW_RANGE, 32.0);
        assert_eq!(RAVAGER_STEP_HEIGHT, 1.0);
        assert_eq!(RAVAGER_XP_REWARD, 20);
        assert_eq!(RAVAGER_ATTACK_DURATION_TICKS, 10);
        assert_eq!(RAVAGER_STUN_DURATION_TICKS, 40);
        assert_eq!(RAVAGER_ROAR_DURATION_TICKS, 20);
        assert_eq!(RAVAGER_ROAR_BURST_TICK, 10);
        assert_eq!(RAVAGER_ROAR_RADIUS, 4.0);
        assert_eq!(RAVAGER_ROAR_DAMAGE, 6.0);
        assert_eq!(RAVAGER_STUN_CHANCE, 0.5);

        // 1. Ravager target movement speed (Ravager.java:139-145)
        assert_eq!(calculate_ravager_target_speed(true, false), 0.0);
        assert_eq!(calculate_ravager_target_speed(true, true), 0.0);
        assert_eq!(calculate_ravager_target_speed(false, true), 0.35);
        assert_eq!(calculate_ravager_target_speed(false, false), 0.3);

        // 2. Ravager strong knockback calculation (Ravager.java:250-254)
        let (kx, ky, kz) = calculate_ravager_strong_knockback(0.0, 0.0, 3.0, 4.0);
        // dd = max(9 + 16, 0.001) = 25.0, push_x = 3.0 / 25.0 * 4.0 = 0.48, push_z = 4.0 / 25.0 * 4.0 = 0.64
        assert!((kx - 0.48).abs() < 1e-6);
        assert_eq!(ky, 0.2);
        assert!((kz - 0.64).abs() < 1e-6);

        // 3. Ravager roar knockback exemption: Player is exempt from strong knockback (Ravager.java:233)
        assert!(!should_ravager_roar_knockback_target(true));
        assert!(should_ravager_roar_knockback_target(false));

        // 4. Ravager roar damage exemption: Illagers exempt (Ravager.java:229-231)
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::PILLAGER));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::EVOKER));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::VINDICATOR));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::ILLUSIONER));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::WITCH));
        assert!(is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::RAVAGER));
        assert!(!is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::PLAYER));
        assert!(!is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::IRON_GOLEM));
        assert!(!is_illager_immune_to_ravager_roar(&pumpkin_data::entity::EntityType::VILLAGER));

        // 5. XP rewards
        assert_eq!(get_ravager_xp_reward(), 20);
        assert_eq!(get_vex_xp_reward(), 3);

        // 6. Vex flapping ticks (Vex.java:49, 64-66: tickCount % 4 == 0)
        assert!(is_vex_flapping(0u32));
        assert!(!is_vex_flapping(1u32));
        assert!(!is_vex_flapping(2));
        assert!(!is_vex_flapping(3u32));
        assert!(is_vex_flapping(4u32));

        // 7. Zoglin targeting & attack damage parity (Zoglin.java:69-70, 139)
        assert!(!can_zoglin_target_entity(&pumpkin_data::entity::EntityType::ZOGLIN));
        assert!(!can_zoglin_target_entity(&pumpkin_data::entity::EntityType::CREEPER));
        assert!(can_zoglin_target_entity(&pumpkin_data::entity::EntityType::PLAYER));
        assert!(can_zoglin_target_entity(&pumpkin_data::entity::EntityType::VILLAGER));
        assert!(can_zoglin_target_entity(&pumpkin_data::entity::EntityType::IRON_GOLEM));

        assert_eq!(calculate_zoglin_attack_damage(false), 6.0);
        assert_eq!(calculate_zoglin_attack_damage(true), 0.5);

        // 8. Zoglin sound pitch (Zoglin.java:203-207)
        assert_eq!(calculate_zoglin_sound_pitch(false, 0.0), 1.0);
        assert_eq!(calculate_zoglin_sound_pitch(true, 0.0), 1.5);
    }

    #[test]
    fn test_spider_cave_spider_and_dolphin_parity() {
        use pumpkin_data::effect::StatusEffect;
        use pumpkin_data::entity::EntityType;

        // Spider and CaveSpider constants & poison duration parity (Spider.java:60, 90, CaveSpider.java:26, 33-40)
        assert_eq!(SPIDER_MAX_HEALTH, 16.0);
        assert_eq!(SPIDER_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(SPIDER_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(SPIDER_XP_REWARD, 5);
        assert_eq!(SPIDER_LEAP_VELOCITY, 0.4);
        assert_eq!(CAVE_SPIDER_MAX_HEALTH, 12.0);
        assert_eq!(CAVE_SPIDER_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(CAVE_SPIDER_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(CAVE_SPIDER_XP_REWARD, 5);
        assert_eq!(CAVE_SPIDER_POISON_NORMAL_TICKS, 140);
        assert_eq!(CAVE_SPIDER_POISON_HARD_TICKS, 300);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Peaceful), 0);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Easy), 0);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Normal), 140);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Hard), 300);

        // 1. Poison immunity for Spider, CaveSpider, Nautilus, ZombieNautilus (Spider.java:126, AbstractNautilus.java:462)
        assert!(is_spider_or_nautilus_immune_to_poison(&EntityType::SPIDER, &StatusEffect::POISON));
        assert!(is_spider_or_nautilus_immune_to_poison(&EntityType::CAVE_SPIDER, &StatusEffect::POISON));
        assert!(is_spider_or_nautilus_immune_to_poison(&EntityType::NAUTILUS, &StatusEffect::POISON));
        assert!(is_spider_or_nautilus_immune_to_poison(&EntityType::ZOMBIE_NAUTILUS, &StatusEffect::POISON));

        assert!(!is_spider_or_nautilus_immune_to_poison(&EntityType::SPIDER, &StatusEffect::SLOWNESS));
        assert!(!is_spider_or_nautilus_immune_to_poison(&EntityType::PLAYER, &StatusEffect::POISON));
        assert!(!is_spider_or_nautilus_immune_to_poison(&EntityType::COW, &StatusEffect::POISON));

        // 2. CaveSpider spawn parity (CaveSpider.java:52: groupData unchanged, no jockey, no effects)
        assert!(!should_cave_spider_spawn_jockey_or_effects());

        // 3. Dolphin swim with player conditions (Dolphin.java:482-489)
        // Valid initiation: swimming, not targeted, within 10 blocks (dist_sqr <= 100.0)
        assert!(should_dolphin_swim_with_player(true, 64.0, false));
        assert!(should_dolphin_swim_with_player(true, 100.0, false));
        // Not swimming
        assert!(!should_dolphin_swim_with_player(false, 64.0, false));
        // Targeted by dolphin (hostile/retaliating)
        assert!(!should_dolphin_swim_with_player(true, 64.0, true));
        // Too far to initiate (> 10 blocks)
        assert!(!should_dolphin_swim_with_player(true, 100.1, false));

        // 4. Dolphin continue swimming with player conditions (Dolphin.java:487-489)
        assert!(should_dolphin_continue_swimming_with_player(true, 100.0));
        assert!(should_dolphin_continue_swimming_with_player(true, 255.9));
        // Lost when not swimming or >= 16 blocks (256.0)
        assert!(!should_dolphin_continue_swimming_with_player(false, 100.0));
        assert!(!should_dolphin_continue_swimming_with_player(true, 256.0));

        // 5. Dolphin grace duration
        assert_eq!(calculate_dolphin_grace_duration(), 100);
    }

    #[test]
    fn test_squid_glow_squid_turtle_and_leash_parity() {
        use pumpkin_data::entity::EntityType;

        // 1. Squid flee motion (Squid.java:267-290)
        let squid_pos = Vector3::new(10.0, 60.0, 10.0);
        let attacker_pos = Vector3::new(7.0, 60.0, 10.0);
        let flee_vec = calculate_squid_flee_movement(squid_pos, attacker_pos, false);
        // Distance is 3.0 (< 5.0), so avoidSpeed is 3.0.
        // Direction is (1.0, 0.0, 0.0), scaled by 3.0, divided by 20.0 = 0.15
        assert!((flee_vec.x - 0.15).abs() < 1e-4);
        assert_eq!(flee_vec.y, 0.0);
        assert_eq!(flee_vec.z, 0.0);

        // When target block is air, vertical velocity is zeroed
        let attacker_below = Vector3::new(10.0, 58.0, 10.0);
        let flee_air = calculate_squid_flee_movement(squid_pos, attacker_below, true);
        assert_eq!(flee_air.y, 0.0);

        // 2. Squid flee trigger condition (Squid.java:249: in water, has attacker, dist_sq < 100.0)
        assert!(should_squid_flee(true, true, 25.0));
        assert!(should_squid_flee(true, true, 99.9));
        assert!(!should_squid_flee(false, true, 25.0));
        assert!(!should_squid_flee(true, false, 25.0));
        assert!(!should_squid_flee(true, true, 100.0));

        // 3. Squid ink offset scale (Squid.java:199-200)
        assert_eq!(calculate_squid_ink_offset_scale(true, 0.0), 0.1);
        assert_eq!(calculate_squid_ink_offset_scale(false, 0.0), 0.3);
        assert!((calculate_squid_ink_offset_scale(false, 0.5) - 1.3).abs() < 1e-6);

        // 4. Squid experience reward (AgeableWaterCreature.java:33-35: 1 + nextInt(3))
        assert_eq!(get_squid_xp_reward(0), 1);
        assert_eq!(get_squid_xp_reward(1), 2);
        assert_eq!(get_squid_xp_reward(2), 3);

        // 5. Leashing parity (Mob.java:1288, AgeableWaterCreature.java:62, AmbientCreature.java:13, etc.)
        // Monsters cannot be leashed, except Zoglin
        assert!(!is_mob_leashable(&EntityType::ZOMBIE));
        assert!(!is_mob_leashable(&EntityType::SKELETON));
        assert!(!is_mob_leashable(&EntityType::CREEPER));
        assert!(!is_mob_leashable(&EntityType::RAVAGER));
        assert!(is_mob_leashable(&EntityType::ZOGLIN));

        // Squid, GlowSquid, Dolphin, Axolotl CAN be leashed
        assert!(is_mob_leashable(&EntityType::SQUID));
        assert!(is_mob_leashable(&EntityType::GLOW_SQUID));
        assert!(is_mob_leashable(&EntityType::DOLPHIN));
        assert!(is_mob_leashable(&EntityType::AXOLOTL));

        // Turtles, fish and bat CANNOT be leashed
        assert!(!is_mob_leashable(&EntityType::TURTLE));
        assert!(!is_mob_leashable(&EntityType::COD));
        assert!(!is_mob_leashable(&EntityType::SALMON));
        assert!(!is_mob_leashable(&EntityType::PUFFERFISH));
        assert!(!is_mob_leashable(&EntityType::TROPICAL_FISH));
        assert!(!is_mob_leashable(&EntityType::TADPOLE));
        assert!(!is_mob_leashable(&EntityType::BAT));

        // Villagers & Wandering Traders CANNOT be leashed
        assert!(!is_mob_leashable(&EntityType::VILLAGER));
        assert!(!is_mob_leashable(&EntityType::WANDERING_TRADER));

        // Common passive animals CAN be leashed
        assert!(is_mob_leashable(&EntityType::COW));
        assert!(is_mob_leashable(&EntityType::SHEEP));
        assert!(is_mob_leashable(&EntityType::PIG));
        assert!(is_mob_leashable(&EntityType::CHICKEN));
        assert!(is_mob_leashable(&EntityType::HORSE));
        assert!(is_mob_leashable(&EntityType::LLAMA));
        assert!(is_mob_leashable(&EntityType::FROG));
        assert!(is_mob_leashable(&EntityType::CAT));
        assert!(is_mob_leashable(&EntityType::WOLF));
    }

    #[test]
    fn test_frog_tongue_eat_and_froglight_parity() {
        use pumpkin_data::entity::EntityType;
        use pumpkin_data::frog_variant::FrogVariant;
        use pumpkin_data::item::Item;

        // 1. Frog food consumption eligibility (Frog.java:315-317)
        // Small Slime (size 1) can be eaten
        assert!(can_frog_eat_entity(&EntityType::SLIME, 1));
        // Medium / Large Slime (size 2, 4) cannot be eaten
        assert!(!can_frog_eat_entity(&EntityType::SLIME, 2));
        assert!(!can_frog_eat_entity(&EntityType::SLIME, 4));

        // Small Magma Cube (size 1) can be eaten
        assert!(can_frog_eat_entity(&EntityType::MAGMA_CUBE, 1));
        // Medium / Large Magma Cube (size 2, 4) cannot be eaten
        assert!(!can_frog_eat_entity(&EntityType::MAGMA_CUBE, 2));
        assert!(!can_frog_eat_entity(&EntityType::MAGMA_CUBE, 4));

        // Other mobs cannot be eaten by Frog
        assert!(!can_frog_eat_entity(&EntityType::ZOMBIE, 1));
        assert!(!can_frog_eat_entity(&EntityType::PLAYER, 1));
        assert!(!can_frog_eat_entity(&EntityType::COW, 1));
        assert!(!can_frog_eat_entity(&EntityType::BAT, 1));

        // 2. Drop item when frog swallows victim (loot tables: slime.json, magma_cube.json)
        // Slime always drops Slimeball regardless of variant
        assert_eq!(get_frog_drop_item(&EntityType::SLIME, FrogVariant::Temperate).unwrap().id, Item::SLIME_BALL.id);
        assert_eq!(get_frog_drop_item(&EntityType::SLIME, FrogVariant::Warm).unwrap().id, Item::SLIME_BALL.id);
        assert_eq!(get_frog_drop_item(&EntityType::SLIME, FrogVariant::Cold).unwrap().id, Item::SLIME_BALL.id);

        // Magma Cube drops variant-specific Froglight
        assert_eq!(get_frog_drop_item(&EntityType::MAGMA_CUBE, FrogVariant::Temperate).unwrap().id, Item::OCHRE_FROGLIGHT.id);
        assert_eq!(get_frog_drop_item(&EntityType::MAGMA_CUBE, FrogVariant::Warm).unwrap().id, Item::PEARLESCENT_FROGLIGHT.id);
        assert_eq!(get_frog_drop_item(&EntityType::MAGMA_CUBE, FrogVariant::Cold).unwrap().id, Item::VERDANT_FROGLIGHT.id);

        // Non-food yields no drop
        assert!(get_frog_drop_item(&EntityType::ZOMBIE, FrogVariant::Temperate).is_none());

        // 3. Frog tongue pull velocity (ShootTongue.java:107)
        let frog_pos = Vector3::new(0.0, 64.0, 0.0);
        let target_pos = Vector3::new(2.0, 64.0, 0.0);
        let pull_vel = calculate_frog_tongue_pull_velocity(target_pos, frog_pos);
        // Direction is (-1.0, 0.0, 0.0) scaled by 0.75
        assert!((pull_vel.x - (-0.75)).abs() < 1e-6);
        assert_eq!(pull_vel.y, 0.0);
        assert_eq!(pull_vel.z, 0.0);

        // 4. Frog eating distance threshold (ShootTongue.java:25, 104: dist < 1.75)
        assert!(should_frog_start_eating(1.74));
        assert!(should_frog_start_eating(1.0));
        assert!(!should_frog_start_eating(1.75));
        assert!(!should_frog_start_eating(2.0));

        // 5. Frog fall damage reduction (Frog.java:304-306: damage - 5)
        // Fall distance 3.0: 0 damage
        assert_eq!(calculate_frog_fall_damage_points(3.0, 1.0), 0);
        // Fall distance 8.0: base damage 5, reduced by 5 -> 0
        assert_eq!(calculate_frog_fall_damage_points(8.0, 1.0), 0);
        // Fall distance 10.0: base damage 7, reduced by 5 -> 2
        assert_eq!(calculate_frog_fall_damage_points(10.0, 1.0), 2);
        // Fall distance 13.0: base damage 10, reduced by 5 -> 5
        assert_eq!(calculate_frog_fall_damage_points(13.0, 1.0), 5);
    }

    #[test]
    fn test_tadpole_and_goat_parity() {
        use pumpkin_data::frog_variant::FrogVariant;

        // 1. Tadpole maturation variant determination (FrogVariants.java:26-29)
        assert_eq!(determine_frog_variant_from_biome_tags(true, false), FrogVariant::Warm);
        assert_eq!(determine_frog_variant_from_biome_tags(false, true), FrogVariant::Cold);
        assert_eq!(determine_frog_variant_from_biome_tags(false, false), FrogVariant::Temperate);

        // 2. Goat fall damage reduction (Goat.java:79, 124-126: damage - 10)
        // Fall distance 3.0: 0 damage
        assert_eq!(calculate_goat_fall_damage_points(3.0, 1.0), 0);
        // Fall distance 13.0: base damage 10, reduced by 10 -> 0
        assert_eq!(calculate_goat_fall_damage_points(13.0, 1.0), 0);
        // Fall distance 15.0: base damage 12, reduced by 10 -> 2
        assert_eq!(calculate_goat_fall_damage_points(15.0, 1.0), 2);
        // Fall distance 18.0: base damage 15, reduced by 10 -> 5
        assert_eq!(calculate_goat_fall_damage_points(18.0, 1.0), 5);

        // 3. Screaming goat spawn chance (Goat.java:80, 241: 2% chance)
        assert!(should_spawn_screaming_goat(0.01));
        assert!(should_spawn_screaming_goat(0.0199));
        assert!(!should_spawn_screaming_goat(0.02));
        assert!(!should_spawn_screaming_goat(0.5));

        // 4. Screaming goat breeding inheritance (Goat.java:156-158)
        // Parent A screaming, selected -> true
        assert!(should_spawn_screaming_goat_from_parents(true, false, true, 0.5));
        // Parent B screaming, selected -> true
        assert!(should_spawn_screaming_goat_from_parents(false, true, false, 0.5));
        // Neither parent screaming, but random < 0.02 -> true
        assert!(should_spawn_screaming_goat_from_parents(false, false, true, 0.01));
        // Neither parent screaming and random >= 0.02 -> false
        assert!(!should_spawn_screaming_goat_from_parents(false, false, true, 0.05));

        // 5. Unihorn goat spawn chance (Goat.java:81, 243: 10% chance for adult)
        assert!(should_goat_spawn_unihorn(false, 0.05));
        assert!(!should_goat_spawn_unihorn(false, 0.1));
        assert!(!should_goat_spawn_unihorn(true, 0.01)); // baby never spawns unihorn

        // 6. Goat horn drop on collision (Goat.java:312-332)
        // No horns remaining: returns None
        assert_eq!(select_goat_horn_to_drop(false, false, true), None);
        // Only right horn remaining: drops right (false)
        assert_eq!(select_goat_horn_to_drop(false, true, true), Some(false));
        // Only left horn remaining: drops left (true)
        assert_eq!(select_goat_horn_to_drop(true, false, false), Some(true));
        // Both horns remaining: randomly selects left or right
        assert_eq!(select_goat_horn_to_drop(true, true, true), Some(true));
        assert_eq!(select_goat_horn_to_drop(true, true, false), Some(false));
    }

    #[test]
    fn test_allay_and_axolotl_parity() {
        use crate::entity::passive::axolotl::AxolotlVariant;
        use pumpkin_data::item::Item;

        // 1. Allay fall damage immunity (Allay.java:179-180)
        assert_eq!(calculate_allay_fall_damage_points(), 0);

        // 2. Allay duplication item check (Allay.java:285)
        assert!(is_allay_duplication_item(&Item::AMETHYST_SHARD));
        assert!(!is_allay_duplication_item(&Item::DIAMOND));

        // 3. Axolotl breeding variant inheritance (Axolotl.java:313-315, 346-361)
        // 1/1200 chance of rare Blue variant
        assert_eq!(
            select_axolotl_baby_variant(AxolotlVariant::Lucy, AxolotlVariant::Wild, 0, true),
            AxolotlVariant::Blue
        );
        // Inherit from parent A when rand_bool is true
        assert_eq!(
            select_axolotl_baby_variant(AxolotlVariant::Lucy, AxolotlVariant::Wild, 1, true),
            AxolotlVariant::Lucy
        );
        // Inherit from parent B when rand_bool is false
        assert_eq!(
            select_axolotl_baby_variant(AxolotlVariant::Lucy, AxolotlVariant::Wild, 1, false),
            AxolotlVariant::Wild
        );

        // 4. Axolotl regeneration buff duration calculation (Axolotl.java:490-499)
        // Initial application (no previous effect): 100 ticks
        assert_eq!(calculate_axolotl_player_regen_duration(None), 100);
        // Stacking with existing 200 ticks: 300 ticks
        assert_eq!(calculate_axolotl_player_regen_duration(Some(200)), 300);
        // Stacking capped at 2400 ticks (2 minutes)
        assert_eq!(calculate_axolotl_player_regen_duration(Some(2350)), 2400);
        assert_eq!(calculate_axolotl_player_regen_duration(Some(2500)), 2400);

        // 5. Axolotl play-dead enemy invisibility (Axolotl.java:474-476)
        assert!(!can_axolotl_be_targeted(true));
        assert!(can_axolotl_be_targeted(false));
    }

    #[test]
    fn test_bee_and_camel_parity() {
        // 1. Bee fall damage immunity (Bee.java:610-611)
        assert_eq!(calculate_bee_fall_damage_points(), 0);

        // 2. Bee drowning damage (Bee.java:365-373: 1.0 damage when underwater_ticks > 20)
        assert_eq!(calculate_bee_underwater_damage(10), 0.0);
        assert_eq!(calculate_bee_underwater_damage(20), 0.0);
        assert_eq!(calculate_bee_underwater_damage(21), 1.0);
        assert_eq!(calculate_bee_underwater_damage(40), 1.0);

        // 3. Camel dash velocity impulse (Camel.java:305-317)
        let vel = calculate_camel_dash_velocity(0.0, 1.0, 0.09, 0.42, 1.0);
        // forward_z = cos(0) = 1.0; scale = 22.2222 * 1.0 * 0.09 * 1.0 = ~1.999998
        assert!((vel.z - 2.0).abs() < 0.01);
        assert_eq!(vel.x, 0.0);
        // vertical = 1.4285 * 1.0 * 0.42 = ~0.59997
        assert!((vel.y - 0.6).abs() < 0.01);

        // 4. Camel passenger capacity (Camel.java:565-567: max 2 passengers)
        assert!(can_camel_accept_passenger(0));
        assert!(can_camel_accept_passenger(1));
        assert!(!can_camel_accept_passenger(2));
    }

    #[test]
    fn test_polar_bear_and_panda_parity() {
        // 1. Panda random gene distribution (Panda.java:Gene.getRandom)
        assert_eq!(calculate_panda_random_gene(0), 1); // Lazy
        assert_eq!(calculate_panda_random_gene(1), 2); // Worried
        assert_eq!(calculate_panda_random_gene(2), 3); // Playful
        assert_eq!(calculate_panda_random_gene(3), 0); // Normal
        assert_eq!(calculate_panda_random_gene(4), 4); // Brown
        assert_eq!(calculate_panda_random_gene(5), 5); // Weak
        assert_eq!(calculate_panda_random_gene(6), 6); // Aggressive
        assert_eq!(calculate_panda_random_gene(7), 0); // Normal
        assert_eq!(calculate_panda_random_gene(15), 0); // Normal

        // 2. Panda genetics inheritance & mutation (Panda.java:231-255)
        // No mutation, inherit parent1 allele (main) & parent2 allele (hidden)
        let (main, hidden) = select_panda_baby_genes(
            1, 2, // Parent 1: main=Lazy, hidden=Worried
            3, 4, // Parent 2: main=Playful, hidden=Brown
            false, // no swap
            true,  // parent 1 picks main (Lazy)
            false, // parent 2 picks hidden (Brown)
            1, 1,  // no mutation
            0, 0,
        );
        assert_eq!(main, 1); // Lazy
        assert_eq!(hidden, 4); // Brown

        // Mutation on main gene (1/32 chance)
        let (mutated_main, unmutated_hidden) = select_panda_baby_genes(
            1, 2, 3, 4, false, true, true,
            0, // rand_mutation_main == 0 -> triggers mutation
            1, // no hidden mutation
            6, // mutate to Aggressive
            0,
        );
        assert_eq!(mutated_main, 6);
        assert_eq!(unmutated_hidden, 3);

        // 3. Panda sneeze conditions (Panda.java:PandaSneezeGoal)
        assert!(!should_panda_sneeze(false, false, 0)); // Adult never sneezes
        assert!(should_panda_sneeze(true, true, 0)); // Weak baby sneezes at rand % 500 == 0
        assert!(should_panda_sneeze(true, true, 500));
        assert!(!should_panda_sneeze(true, true, 250));
        assert!(should_panda_sneeze(true, false, 6000)); // Normal baby sneezes at rand % 6000 == 0
        assert!(!should_panda_sneeze(true, false, 500));

        // 4. Panda sneeze slime ball drop (panda_sneeze.json: 1/700 chance)
        assert!(should_panda_drop_slime_ball(0));
        assert!(should_panda_drop_slime_ball(700));
        assert!(!should_panda_drop_slime_ball(1));
        assert!(!should_panda_drop_slime_ball(699));

        // 5. Polar bear attack warning stand & roar (PolarBear.java:315-325)
        // target width = 0.6 -> reach = (0.6 + 3.0)^2 = 3.6^2 = 12.96
        assert!(should_polar_bear_stand_and_warn(10.0, 0.6, 5));
        assert!(should_polar_bear_stand_and_warn(12.0, 0.6, 10));
        // Too far away (> 12.96)
        assert!(!should_polar_bear_stand_and_warn(14.0, 0.6, 5));
        // Ticks until attack > 10
        assert!(!should_polar_bear_stand_and_warn(10.0, 0.6, 11));
    }

    #[test]
    fn test_cat_and_ocelot_parity() {
        // 1. Feline fall damage immunity (fall_damage_immune.json)
        assert_eq!(calculate_feline_fall_damage_points(), 0);

        // 2. Cat kitten breeding traits inheritance (Cat.java:235-245)
        let (var_a, col_a) = select_cat_baby_traits(1, 2, 14, 11, true, true);
        assert_eq!(var_a, 1);
        assert_eq!(col_a, 14);

        let (var_b, col_b) = select_cat_baby_traits(1, 2, 14, 11, false, false);
        assert_eq!(var_b, 2);
        assert_eq!(col_b, 11);

        // 3. Cat morning gift loot table (cat_morning_gift.json)
        assert_eq!(select_cat_morning_gift(0).id, pumpkin_data::item::Item::RABBIT_HIDE.id);
        assert_eq!(select_cat_morning_gift(15).id, pumpkin_data::item::Item::RABBIT_FOOT.id);
        assert_eq!(select_cat_morning_gift(25).id, pumpkin_data::item::Item::CHICKEN.id);
        assert_eq!(select_cat_morning_gift(35).id, pumpkin_data::item::Item::FEATHER.id);
        assert_eq!(select_cat_morning_gift(45).id, pumpkin_data::item::Item::ROTTEN_FLESH.id);
        assert_eq!(select_cat_morning_gift(55).id, pumpkin_data::item::Item::STRING.id);
        // Phantom membrane is the rare gift at indices 60 and 61 (2/62)
        assert_eq!(select_cat_morning_gift(60).id, pumpkin_data::item::Item::PHANTOM_MEMBRANE.id);
        assert_eq!(select_cat_morning_gift(61).id, pumpkin_data::item::Item::PHANTOM_MEMBRANE.id);

        // 4. Cat morning gift delivery conditions (Cat.java:500-505)
        assert!(should_cat_give_morning_gift(100, 0.5, 0.7));
        assert!(should_cat_give_morning_gift(150, 0.69, 0.7));
        // Sleep timer too short (< 100 ticks)
        assert!(!should_cat_give_morning_gift(99, 0.1, 0.7));
        // Roll failed (>= 0.7)
        assert!(!should_cat_give_morning_gift(100, 0.71, 0.7));

        // 5. Ocelot despawn mechanics (Ocelot.java:138-140)
        assert!(!should_ocelot_despawn_when_far(true, 5000)); // Trusting ocelot never despawns
        assert!(!should_ocelot_despawn_when_far(false, 2400)); // Within 2400 ticks (2 min)
        assert!(should_ocelot_despawn_when_far(false, 2401)); // Untrusted despawns after 2400 ticks
    }

    #[test]
    fn test_wolf_and_fox_parity() {
        // 1. Wolf food nutrition scaling (Wolf.java:mobInteract: heal = 2.0 * nutrition)
        // Cooked beef nutrition = 8 -> 16.0 HP heal
        assert_eq!(calculate_wolf_food_healing(Some(8)), 16.0);
        // Raw beef nutrition = 3 -> 6.0 HP heal
        assert_eq!(calculate_wolf_food_healing(Some(3)), 6.0);
        // Rotten flesh nutrition = 4 -> 8.0 HP heal
        assert_eq!(calculate_wolf_food_healing(Some(4)), 8.0);
        // Default / None -> 2.0 HP heal
        assert_eq!(calculate_wolf_food_healing(None), 2.0);

        // 2. Wolf baby traits inheritance (Wolf.java:getBreedOffspring)
        let (var_a, col_a) = select_wolf_baby_traits(0, 1, 14, 11, true, true);
        assert_eq!(var_a, 0);
        assert_eq!(col_a, 14);

        let (var_b, col_b) = select_wolf_baby_traits(0, 1, 14, 11, false, false);
        assert_eq!(var_b, 1);
        assert_eq!(col_b, 11);

        // 3. Fox baby variant inheritance (Fox.java:getBreedOffspring)
        assert_eq!(select_fox_baby_variant(0, 1, true), 0); // Red
        assert_eq!(select_fox_baby_variant(0, 1, false), 1); // Snow

        // 4. Fox safe fall distance attribute (Fox.java:createAttributes: 5.0 blocks)
        assert_eq!(get_fox_safe_fall_distance(), 5.0);
    }

    #[test]
    fn test_equine_food_and_breeding_parity() {
        use pumpkin_data::entity::EntityType;
        use pumpkin_data::item::Item;

        // 1. Equine food feeding properties (AbstractHorse.java:handleEating)
        let wheat = get_equine_food_properties(Item::WHEAT.id).unwrap();
        assert_eq!(wheat.heal, 2.0);
        assert_eq!(wheat.age_up_seconds, 20);
        assert_eq!(wheat.temper_increase, 3);
        assert!(!wheat.can_breed);

        let sugar = get_equine_food_properties(Item::SUGAR.id).unwrap();
        assert_eq!(sugar.heal, 1.0);
        assert_eq!(sugar.age_up_seconds, 30);
        assert_eq!(sugar.temper_increase, 3);
        assert!(!sugar.can_breed);

        let hay = get_equine_food_properties(Item::HAY_BLOCK.id).unwrap();
        assert_eq!(hay.heal, 20.0);
        assert_eq!(hay.age_up_seconds, 180);
        assert_eq!(hay.temper_increase, 0);
        assert!(!hay.can_breed);

        let golden_carrot = get_equine_food_properties(Item::GOLDEN_CARROT.id).unwrap();
        assert_eq!(golden_carrot.heal, 4.0);
        assert_eq!(golden_carrot.age_up_seconds, 60);
        assert_eq!(golden_carrot.temper_increase, 5);
        assert!(golden_carrot.can_breed);

        let golden_apple = get_equine_food_properties(Item::GOLDEN_APPLE.id).unwrap();
        assert_eq!(golden_apple.heal, 10.0);
        assert_eq!(golden_apple.age_up_seconds, 240);
        assert_eq!(golden_apple.temper_increase, 10);
        assert!(golden_apple.can_breed);

        assert!(get_equine_food_properties(Item::DIRT.id).is_none());

        // 2. Equine mating compatibility (Horse.java:canMate, Donkey.java:canMate)
        assert!(can_equine_mate(&EntityType::HORSE, &EntityType::HORSE));
        assert!(can_equine_mate(&EntityType::HORSE, &EntityType::DONKEY));
        assert!(can_equine_mate(&EntityType::DONKEY, &EntityType::HORSE));
        assert!(can_equine_mate(&EntityType::DONKEY, &EntityType::DONKEY));
        assert!(!can_equine_mate(&EntityType::MULE, &EntityType::HORSE));
        assert!(!can_equine_mate(&EntityType::MULE, &EntityType::DONKEY));
        assert!(!can_equine_mate(&EntityType::HORSE, &EntityType::COW));

        // 3. Offspring entity type (Horse.java:getBreedOffspring, Donkey.java:getBreedOffspring)
        assert_eq!(determine_equine_baby_type(&EntityType::HORSE, &EntityType::DONKEY), &EntityType::MULE);
        assert_eq!(determine_equine_baby_type(&EntityType::DONKEY, &EntityType::HORSE), &EntityType::MULE);
        assert_eq!(determine_equine_baby_type(&EntityType::HORSE, &EntityType::HORSE), &EntityType::HORSE);
        assert_eq!(determine_equine_baby_type(&EntityType::DONKEY, &EntityType::DONKEY), &EntityType::DONKEY);

        // 4. Horse variant and markings inheritance (Horse.java:getBreedOffspring)
        // Parent A: variant 2 (Chestnut), marking 1 (White) -> 2 | (1 << 8) = 258
        // Parent B: variant 5 (Gray), marking 3 (White Dots) -> 5 | (3 << 8) = 773
        let parent_a = 2 | (1 << 8);
        let parent_b = 5 | (3 << 8);

        // rand_skin 0..3: inherit parent A variant (2)
        // rand_markings 0..1: inherit parent A markings (1)
        let baby_1 = select_horse_baby_variant_and_markings(parent_a, parent_b, 1, 0, 0, 0);
        assert_eq!(baby_1 & 0xFF, 2);
        assert_eq!((baby_1 >> 8) & 0xFF, 1);

        // rand_skin 4..7: inherit parent B variant (5)
        // rand_markings 2..3: inherit parent B markings (3)
        let baby_2 = select_horse_baby_variant_and_markings(parent_a, parent_b, 6, 2, 0, 0);
        assert_eq!(baby_2 & 0xFF, 5);
        assert_eq!((baby_2 >> 8) & 0xFF, 3);

        // rand_skin 8: random variant (mutation)
        // rand_markings 4: random markings (mutation)
        let baby_mut = select_horse_baby_variant_and_markings(parent_a, parent_b, 8, 4, 4, 2);
        assert_eq!(baby_mut & 0xFF, 4); // variant 4 (Black)
        assert_eq!((baby_mut >> 8) & 0xFF, 2); // markings 2 (Whitefield)

        // 5. Equine offspring attribute inheritance (AbstractHorse.java:createOffspringAttribute)
        // Average when quality = 0.0 ((0.5 + 0.5 + 0.5)/3 - 0.5 = 0.0)
        let avg_health = calculate_offspring_attribute(20.0, 24.0, 15.0, 30.0, 0.5, 0.5, 0.5);
        assert!((avg_health - 22.0).abs() < 1e-4);

        // Quality clamped within range
        let low_health = calculate_offspring_attribute(15.0, 15.0, 15.0, 30.0, 0.0, 0.0, 0.0);
        assert!(low_health >= 15.0 && low_health <= 30.0);

        let high_health = calculate_offspring_attribute(30.0, 30.0, 15.0, 30.0, 1.0, 1.0, 1.0);
        assert!(high_health >= 15.0 && high_health <= 30.0);
    }

    #[test]
    fn test_llama_food_and_genetics_and_skeleton_horse_trap_parity() {
        use pumpkin_data::item::Item;

        // 1. Llama food feeding properties (Llama.java:handleEating)
        let wheat = get_llama_food_properties(Item::WHEAT.id).unwrap();
        assert_eq!(wheat.heal, 2.0);
        assert_eq!(wheat.age_up_seconds, 10);
        assert_eq!(wheat.temper_increase, 3);
        assert!(!wheat.can_breed);

        let hay = get_llama_food_properties(Item::HAY_BLOCK.id).unwrap();
        assert_eq!(hay.heal, 10.0);
        assert_eq!(hay.age_up_seconds, 90);
        assert_eq!(hay.temper_increase, 6);
        assert!(hay.can_breed);

        assert!(get_llama_food_properties(Item::APPLE.id).is_none());
        assert!(get_llama_food_properties(Item::SUGAR.id).is_none());
        assert!(get_llama_food_properties(Item::GOLDEN_CARROT.id).is_none());

        // 2. Llama baby strength inheritance (Llama.java:getBreedOffspring)
        // Parent A strength 2, Parent B strength 4 -> max 4
        // rand_int 2 -> (2 % 4) + 1 = 3
        assert_eq!(calculate_llama_baby_strength(2, 4, 2, 0.5), 3);
        // Mutation (rand_float < 0.03) -> 3 + 1 = 4
        assert_eq!(calculate_llama_baby_strength(2, 4, 2, 0.01), 4);
        // Clamped at 5
        assert_eq!(calculate_llama_baby_strength(5, 5, 4, 0.01), 5);

        // 3. Llama baby variant inheritance (Llama.java:getBreedOffspring: 50/50)
        assert_eq!(select_llama_baby_variant(0, 3, true), 0);
        assert_eq!(select_llama_baby_variant(0, 3, false), 3);

        // 4. Skeleton horse trap trigger (SkeletonTrapGoal.java:canUse)
        assert!(should_skeleton_trap_trigger(true, 9.9));
        assert!(!should_skeleton_trap_trigger(true, 10.0));
        assert!(!should_skeleton_trap_trigger(true, 15.0));
        assert!(!should_skeleton_trap_trigger(false, 5.0));

        // 5. Skeleton horse trap despawn (SkeletonHorse.java:tick)
        assert!(should_skeleton_trap_despawn(true, 18000));
        assert!(should_skeleton_trap_despawn(true, 20000));
        assert!(!should_skeleton_trap_despawn(true, 17999));
        assert!(!should_skeleton_trap_despawn(false, 30000));
    }

    #[test]
    fn test_mooshroom_pig_and_sheep_parity() {
        // 1. Mooshroom 50/50 inheritance and 1/1024 mutation (MushroomCow.java:getBreedOffspring)
        // Red (0) and Brown (1)
        assert_eq!(select_mooshroom_baby_variant(0, 1, 500, true), 0);
        assert_eq!(select_mooshroom_baby_variant(0, 1, 500, false), 1);
        // Mutation when parents are the same:
        // Red x Red with roll == 0 -> Brown (1)
        assert_eq!(select_mooshroom_baby_variant(0, 0, 0, true), 1);
        // Red x Red with roll != 0 -> Red (0)
        assert_eq!(select_mooshroom_baby_variant(0, 0, 1, true), 0);
        // Brown x Brown with roll == 0 -> Red (0)
        assert_eq!(select_mooshroom_baby_variant(1, 1, 0, true), 0);
        // Brown x Brown with roll != 0 -> Brown (1)
        assert_eq!(select_mooshroom_baby_variant(1, 1, 1, true), 1);

        // 2. Pig variant & sound variant inheritance (Pig.java:getBreedOffspring: 50/50)
        let (var1, sound1) = select_pig_baby_variant_and_sound(0, 1, 0, 1, true, true);
        assert_eq!(var1, 0);
        assert_eq!(sound1, 0);
        let (var2, sound2) = select_pig_baby_variant_and_sound(0, 1, 0, 1, false, false);
        assert_eq!(var2, 1);
        assert_eq!(sound2, 1);

        // 3. Sheep wool color mixing & genetics (Sheep.java:getBreedOffspring & DyeColor.java:getMixedColor)
        // White (0) + Black (15) => Gray (7)
        assert_eq!(get_mixed_sheep_color(0, 15, true), 7);
        assert_eq!(get_mixed_sheep_color(15, 0, false), 7);
        // White (0) + Red (14) => Pink (6)
        assert_eq!(get_mixed_sheep_color(0, 14, false), 6);
        // Red (14) + Yellow (4) => Orange (1)
        assert_eq!(get_mixed_sheep_color(14, 4, true), 1);
        // Blue (11) + Green (13) => Cyan (9)
        assert_eq!(get_mixed_sheep_color(11, 13, false), 9);
        // Blue (11) + Red (14) => Purple (10)
        assert_eq!(get_mixed_sheep_color(11, 14, true), 10);
        // Purple (10) + Pink (6) => Magenta (2)
        assert_eq!(get_mixed_sheep_color(10, 6, false), 2);
        // Green (13) + White (0) => Lime (5)
        assert_eq!(get_mixed_sheep_color(13, 0, true), 5);
        // Blue (11) + White (0) => Light Blue (3)
        assert_eq!(get_mixed_sheep_color(11, 0, true), 3);
        // White (0) + Gray (7) => Light Gray (8)
        assert_eq!(get_mixed_sheep_color(0, 7, false), 8);

        // Non-mixable colors fallback to 50/50 parental color
        // Red (14) + Black (15)
        assert_eq!(get_mixed_sheep_color(14, 15, true), 14);
        assert_eq!(get_mixed_sheep_color(14, 15, false), 15);
    }

    #[test]
    fn test_oviparous_and_special_breeding_parity() {
        use pumpkin_data::entity::EntityType;

        // 1. Special oviparous breeding check (Sniffer egg drop & Turtle gravid egg)
        assert!(is_special_oviparous_breeding(&EntityType::SNIFFER));
        assert!(is_special_oviparous_breeding(&EntityType::TURTLE));
        assert!(!is_special_oviparous_breeding(&EntityType::COW));
        assert!(!is_special_oviparous_breeding(&EntityType::SHEEP));
        assert!(!is_special_oviparous_breeding(&EntityType::PIG));

        // 2. Sniffer egg plop sound pitch (Sniffer.java:350: (rand - rand) * 0.2 + 0.5)
        let pitch_neutral = get_sniffer_egg_plop_pitch(0.5, 0.5);
        assert!((pitch_neutral - 0.5).abs() < 1e-6);
        let pitch_high = get_sniffer_egg_plop_pitch(1.0, 0.0);
        assert!((pitch_high - 0.7).abs() < 1e-6);
        let pitch_low = get_sniffer_egg_plop_pitch(0.0, 1.0);
        assert!((pitch_low - 0.3).abs() < 1e-6);

        // 3. Strider eat sound pitch (Strider.java:432: 1.0 + (rand - rand) * 0.2)
        let strider_pitch_neutral = get_strider_eat_sound_pitch(0.5, 0.5);
        assert!((strider_pitch_neutral - 1.0).abs() < 1e-6);
        let strider_pitch_high = get_strider_eat_sound_pitch(1.0, 0.0);
        assert!((strider_pitch_high - 1.2).abs() < 1e-6);
        let strider_pitch_low = get_strider_eat_sound_pitch(0.0, 1.0);
        assert!((strider_pitch_low - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_rabbit_and_parrot_parity() {
        // 1. Rabbit breeding inheritance (Rabbit.java:345-352)
        // 95% chance (rand_20 != 0): inherit parent_a or parent_b
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 1, false, 50, RabbitBiomeSpawnType::Normal),
            0 // parent_a
        );
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 1, true, 50, RabbitBiomeSpawnType::Normal),
            1 // parent_b
        );

        // 5% chance (rand_20 == 0): mutation by biome
        // Normal biome: <50 Brown (0), <90 Salt (5), >=90 Black (2)
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 10, RabbitBiomeSpawnType::Normal),
            0
        );
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 60, RabbitBiomeSpawnType::Normal),
            5
        );
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 95, RabbitBiomeSpawnType::Normal),
            2
        );

        // White biome: <80 White (1), >=80 White Splotched (3)
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 50, RabbitBiomeSpawnType::White),
            1
        );
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 85, RabbitBiomeSpawnType::White),
            3
        );

        // Gold biome: always Gold (4)
        assert_eq!(
            select_rabbit_baby_variant(0, 1, 0, false, 50, RabbitBiomeSpawnType::Gold),
            4
        );

        // 2. Parrot taming success chance (Parrot.java:271: rand.nextInt(10) == 0)
        assert!(calculate_parrot_tame_success(0));
        assert!(!calculate_parrot_tame_success(1));
        assert!(!calculate_parrot_tame_success(9));

        // 3. Parrot eating seed sound pitch (Parrot.java:266: 1.0 + (rand - rand) * 0.2)
        let pitch_neutral = get_parrot_eat_sound_pitch(0.5, 0.5);
        assert!((pitch_neutral - 1.0).abs() < 1e-6);
        let pitch_high = get_parrot_eat_sound_pitch(1.0, 0.0);
        assert!((pitch_high - 1.2).abs() < 1e-6);
        let pitch_low = get_parrot_eat_sound_pitch(0.0, 1.0);
        assert!((pitch_low - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_chicken_and_cow_parity() {
        // 1. 50/50 Variant inheritance (Chicken.java:178 & Cow.java:75)
        assert_eq!(select_chicken_or_cow_baby_variant(1, 2, true), 1);
        assert_eq!(select_chicken_or_cow_baby_variant(1, 2, false), 2);

        // 2. Chicken egg lay sound pitch (Chicken.java:137: 1.0 + (rand - rand) * 0.2)
        let pitch_neutral = get_chicken_egg_lay_sound_pitch(0.5, 0.5);
        assert!((pitch_neutral - 1.0).abs() < 1e-6);
        let pitch_high = get_chicken_egg_lay_sound_pitch(1.0, 0.0);
        assert!((pitch_high - 1.2).abs() < 1e-6);
        let pitch_low = get_chicken_egg_lay_sound_pitch(0.0, 1.0);
        assert!((pitch_low - 0.8).abs() < 1e-6);

        // 3. Egg laying eligibility (Chicken.java:135)
        // Adult non-jockey with time <= 0 can lay
        assert!(should_chicken_lay_egg(false, false, 0));
        assert!(should_chicken_lay_egg(false, false, -1));
        // Adult with time > 0 cannot lay
        assert!(!should_chicken_lay_egg(false, false, 100));
        // Baby cannot lay
        assert!(!should_chicken_lay_egg(true, false, 0));
        // Chicken jockey cannot lay
        assert!(!should_chicken_lay_egg(false, true, 0));

        // 4. Chicken XP reward (Chicken.java:199-201)
        assert_eq!(get_chicken_xp_reward(false, 2), 2);
        assert_eq!(get_chicken_xp_reward(true, 2), 10);
    }

    #[test]
    fn test_frog_and_fox_parity() {
        use crate::entity::passive::fox::FoxVariant;

        // 1. Oviparous breeding parity: Sniffer, Turtle, Frog are oviparous
        assert!(is_special_oviparous_breeding(&EntityType::SNIFFER));
        assert!(is_special_oviparous_breeding(&EntityType::TURTLE));
        assert!(is_special_oviparous_breeding(&EntityType::FROG));
        assert!(!is_special_oviparous_breeding(&EntityType::FOX));
        assert!(!is_special_oviparous_breeding(&EntityType::COW));
        assert!(!is_special_oviparous_breeding(&EntityType::CHICKEN));

        // 2. Fox 50/50 variant inheritance (Fox.java:325)
        assert_eq!(
            select_fox_baby_variant(FoxVariant::Red.id(), FoxVariant::Snow.id(), true),
            FoxVariant::Red.id()
        );
        assert_eq!(
            select_fox_baby_variant(FoxVariant::Red.id(), FoxVariant::Snow.id(), false),
            FoxVariant::Snow.id()
        );


        // 3. Fox player avoidance parity (Fox.java:187: !trusts && !isDefending)
        // Untrusted & not defending -> avoid
        assert!(should_fox_avoid_player(false, false));
        // Trusted -> do not avoid
        assert!(!should_fox_avoid_player(false, true));
        // Defending -> do not avoid (even if untrusted)
        assert!(!should_fox_avoid_player(true, false));
        // Defending & trusted -> do not avoid
        assert!(!should_fox_avoid_player(true, true));

        // 4. Frog lay spawn block condition (TryLaySpawnOnFluidNearLand.java:23)
        // Pregnant on ground not in water -> can lay
        assert!(should_frog_lay_spawn_block(true, true, false));
        // In water -> cannot lay
        assert!(!should_frog_lay_spawn_block(true, true, true));
        // In air -> cannot lay
        assert!(!should_frog_lay_spawn_block(true, false, false));
        // Not pregnant -> cannot lay
        assert!(!should_frog_lay_spawn_block(false, true, false));
    }

    #[test]
    fn test_goat_eating_and_ocelot_trust_parity() {
        // 1. Goat eat sound pitch (Goat.java:208: 0.8 to 1.2)
        let pitch_low = get_goat_eat_sound_pitch(0.0);
        assert!((pitch_low - 0.8).abs() < 1e-6);
        let pitch_mid = get_goat_eat_sound_pitch(0.5);
        assert!((pitch_mid - 1.0).abs() < 1e-6);
        let pitch_high = get_goat_eat_sound_pitch(1.0);
        assert!((pitch_high - 1.2).abs() < 1e-6);

        // 2. Ocelot trust feeding chance (Ocelot.java:172: 1/3)
        assert!(calculate_ocelot_trust_success(0));
        assert!(!calculate_ocelot_trust_success(1));
        assert!(!calculate_ocelot_trust_success(2));

        // 3. Ocelot avoid player goal condition (Ocelot.java:219: !isTrusting)
        assert!(should_ocelot_avoid_player(false));
        assert!(!should_ocelot_avoid_player(true));
    }

    #[test]
    fn test_bat_and_polar_bear_parity() {
        // 1. Bat voice pitch: ((rand_a - rand_b) * 0.2 + 1.0) * 0.95 (Bat.java:67)
        let pitch_neutral = get_bat_voice_pitch(0.5, 0.5);
        assert!((pitch_neutral - 0.95).abs() < 1e-6);
        let pitch_high = get_bat_voice_pitch(1.0, 0.0);
        assert!((pitch_high - 1.14).abs() < 1e-6);
        let pitch_low = get_bat_voice_pitch(0.0, 1.0);
        assert!((pitch_low - 0.76).abs() < 1e-6);

        // 2. Bat waking conditions (Bat.java:135, 140, 147)
        // Solid above, no player nearby -> remains resting
        assert!(!should_bat_wake_from_resting(true, None));
        assert!(!should_bat_wake_from_resting(true, Some(5.0)));
        // Solid above, player within 4 blocks -> wakes
        assert!(should_bat_wake_from_resting(true, Some(3.9)));
        // Non-solid above -> wakes regardless of players
        assert!(should_bat_wake_from_resting(false, None));
        assert!(should_bat_wake_from_resting(false, Some(10.0)));
    }

    #[test]
    fn test_llama_and_trader_llama_parity() {
        // 1. Initial spawn strength distribution (Llama.java:93-96)
        // >= 0.04 -> maxStrength = 3 -> strength in 1..=3
        let normal_low = calculate_initial_llama_strength(0.5, 0);
        assert_eq!(normal_low, 1);
        let normal_mid = calculate_initial_llama_strength(0.5, 1);
        assert_eq!(normal_mid, 2);
        let normal_high = calculate_initial_llama_strength(0.5, 2);
        assert_eq!(normal_high, 3);
        let normal_wrap = calculate_initial_llama_strength(0.5, 3);
        assert_eq!(normal_wrap, 1);

        // < 0.04 (4% rare) -> maxStrength = 5 -> strength in 1..=5
        let rare_5 = calculate_initial_llama_strength(0.01, 4);
        assert_eq!(rare_5, 5);

        // 2. Sound pitch (Llama.java:228, 296)
        let pitch_neutral = get_llama_sound_pitch(0.5, 0.5);
        assert!((pitch_neutral - 1.0).abs() < 1e-6);
        let pitch_high = get_llama_sound_pitch(1.0, 0.0);
        assert!((pitch_high - 1.2).abs() < 1e-6);
        let pitch_low = get_llama_sound_pitch(0.0, 1.0);
        assert!((pitch_low - 0.8).abs() < 1e-6);

        // 3. Inventory columns (Llama.java:300-302)
        assert_eq!(get_llama_inventory_columns(false, 5), 0);
        assert_eq!(get_llama_inventory_columns(true, 1), 1);
        assert_eq!(get_llama_inventory_columns(true, 3), 3);
        assert_eq!(get_llama_inventory_columns(true, 5), 5);
        assert_eq!(get_llama_inventory_columns(true, 6), 5); // clamped

        // 4. Mating eligibility (TraderLlama.java:37-39)
        assert!(can_llama_mate(true, false));
        assert!(!can_llama_mate(true, true));
        assert!(!can_llama_mate(false, false));
    }

    #[test]
    fn test_glow_squid_and_strider_parity() {
        // 1. Strider ridden speed (Strider.java:273-275)
        let warm_speed = calculate_strider_ridden_speed(0.175, false, 1.0);
        let cold_speed = calculate_strider_ridden_speed(0.175, true, 1.0);
        assert!((warm_speed - (0.175 * 0.55) as f32).abs() < 1e-6);
        assert!((cold_speed - (0.175 * 0.35) as f32).abs() < 1e-6);
        let boosted = calculate_strider_ridden_speed(0.175, false, 1.5);
        assert!((boosted - (0.175 * 0.55 * 1.5) as f32).abs() < 1e-6);

        // 2. Strider tick sounds (Strider.java:303-306)
        assert!(should_strider_be_happy(true, 0));
        assert!(!should_strider_be_happy(true, 1));
        assert!(!should_strider_be_happy(false, 0));

        assert!(should_strider_retreat(true, 0));
        assert!(!should_strider_retreat(true, 1));
        assert!(!should_strider_retreat(false, 0));

        // 3. Squid ink trigger (Squid.java:180)
        assert!(should_squid_squirt_ink(true));
        assert!(!should_squid_squirt_ink(false));
    }

    #[test]
    fn test_dolphin_and_pufferfish_parity() {
        // Dolphin air supply & moistness (Dolphin.java:78-79)
        assert_eq!(get_dolphin_total_air_supply(), 4800);
        assert_eq!(get_dolphin_total_moistness(), 2400);

        // Pufferfish poison duration & damage (Pufferfish.java:142-143, 185-191)
        assert_eq!(calculate_pufferfish_poison_duration(0), 0);
        assert_eq!(calculate_pufferfish_poison_duration(1), 60);
        assert_eq!(calculate_pufferfish_poison_duration(2), 120);

        assert_eq!(calculate_pufferfish_sting_damage(0), 1.0);
        assert_eq!(calculate_pufferfish_sting_damage(1), 2.0);
        assert_eq!(calculate_pufferfish_sting_damage(2), 3.0);

        assert_eq!(calculate_pufferfish_scale(0), 0.5);
        assert_eq!(calculate_pufferfish_scale(1), 0.7);
        assert_eq!(calculate_pufferfish_scale(2), 1.0);
    }

    #[test]
    fn test_wolf_fox_and_cat_parity() {
        // 1. Wolf tail angle (Wolf.java:532-540)
        let angry_angle = calculate_wolf_tail_angle(true, false, 8.0, 8.0);
        assert!((angry_angle - 1.539_380_4).abs() < 1e-6);

        let wild_angle = calculate_wolf_tail_angle(false, false, 8.0, 8.0);
        assert!((wild_angle - std::f32::consts::PI / 5.0).abs() < 1e-6);

        let full_health_angle = calculate_wolf_tail_angle(false, true, 40.0, 40.0);
        assert!((full_health_angle - 0.55 * std::f32::consts::PI).abs() < 1e-6);

        let half_health_angle = calculate_wolf_tail_angle(false, true, 20.0, 40.0);
        assert!((half_health_angle - 0.35 * std::f32::consts::PI).abs() < 1e-6);

        // 2. Wolf armor repair (Wolf.java:467-475)
        assert_eq!(calculate_wolf_armor_repair_amount(64), 8);
        assert_eq!(calculate_wolf_armor_repair_amount(100), 12);
        assert!(can_repair_wolf_armor(true, true, true));
        assert!(!can_repair_wolf_armor(false, true, true)); // not sitting
        assert!(!can_repair_wolf_armor(true, false, true)); // not damaged
        assert!(!can_repair_wolf_armor(true, true, false)); // no armor

        // 3. Fox eating timer (Fox.java:132, 216-217)
        assert_eq!(get_fox_min_ticks_before_eat(), 600);
        assert!(can_fox_eat_item(true, 601));
        assert!(!can_fox_eat_item(true, 600));
        assert!(!can_fox_eat_item(false, 601));

        // 4. Cat feeding health (Cat.java:425)
        assert_eq!(get_cat_food_healing(), 1.0);
    }

    #[test]
    fn test_passive_mob_details_parity() {
        // 1. Parrot fall damage immunity (Parrot.java:293-294)
        assert_eq!(calculate_parrot_fall_damage(), 0);
        assert!(!should_parrot_take_fall_damage());

        // 2. Rabbit speed mods and evil bunny properties (Rabbit.java:82-95, 317-326)
        assert_eq!(RABBIT_STROLL_SPEED_MOD, 0.6);
        assert_eq!(RABBIT_BREED_SPEED_MOD, 0.8);
        assert_eq!(RABBIT_FOLLOW_SPEED_MOD, 1.0);
        assert_eq!(RABBIT_FLEE_SPEED_MOD, 2.2);
        assert_eq!(RABBIT_ATTACK_SPEED_MOD, 1.4);
        assert_eq!(RABBIT_EVIL_ARMOR, 8.0);
        assert_eq!(RABBIT_EVIL_ATTACK_POWER_MODIFIER, 5.0);
        assert_eq!(RABBIT_DEFAULT_ATTACK_POWER, 3.0);
        assert!(is_rabbit_evil(99));
        assert!(!is_rabbit_evil(0));
        let pitch = calculate_rabbit_attack_pitch(0.5, 0.5);
        assert!((pitch - 1.0).abs() < 1e-6);

        // 3. Axolotl rehydration and ambient sound (Axolotl.java:169-173, 273-276)
        assert_eq!(AXOLOTL_TOTAL_AIR_SUPPLY, 6000);
        assert_eq!(AXOLOTL_REHYDRATE_AIR_SUPPLY, 1800);
        assert_eq!(calculate_axolotl_rehydrate_air(0, 6000), 1800);
        assert_eq!(calculate_axolotl_rehydrate_air(5000, 6000), 6000);
        assert!(should_axolotl_play_ambient_sound(false, false));
        assert!(!should_axolotl_play_ambient_sound(true, false)); // playing dead
        assert!(!should_axolotl_play_ambient_sound(false, true)); // silent

        // 4. Sniffer eating pitch (Sniffer.java:360: 0.8 .. 1.2)
        assert!((calculate_sniffer_eat_pitch(0.0) - 0.8).abs() < 1e-6);
        assert!((calculate_sniffer_eat_pitch(1.0) - 1.2).abs() < 1e-6);
        assert!((calculate_sniffer_eat_pitch(0.5) - 1.0).abs() < 1e-6);

        // 5. Armadillo scared damage reduction (Armadillo.java:279-283)
        assert_eq!(calculate_armadillo_scared_damage(1.0), 0.0);
        assert_eq!(calculate_armadillo_scared_damage(5.0), 2.0);
        assert_eq!(calculate_armadillo_scared_damage(11.0), 5.0);
        assert_eq!(calculate_armadillo_scared_damage(0.5), 0.0);
    }

    #[test]
    fn test_hostile_mob_details_parity() {
        // 1. Enderman de-aggression & stare constants and conditions (Enderman.java:77-78, 177-182, 248-254)
        assert_eq!(ENDERMAN_MIN_DEAGGRESSION_TIME, 600);
        assert_eq!(ENDERMAN_DELAY_BETWEEN_CREEPY_STARE_SOUND, 400);
        assert_eq!(ENDERMAN_STARE_SOUND_VOLUME, 2.5);
        assert!(can_enderman_deaggro_in_sunlight(1000, 400));
        assert!(can_enderman_deaggro_in_sunlight(1000, 0)); // No target change time
        assert!(!can_enderman_deaggro_in_sunlight(999, 400)); // Within 600 ticks
        assert!(should_enderman_daylight_teleport(1.0, 0.03));
        assert!(!should_enderman_daylight_teleport(0.4, 0.01)); // br <= 0.5
        assert!(should_enderman_play_stare_sound(500, 100, false));
        assert!(!should_enderman_play_stare_sound(499, 100, false)); // Within 400 ticks
        assert!(!should_enderman_play_stare_sound(500, 100, true)); // Silent

        // 2. Blaze crackle burn sound and fireball spread (Blaze.java:91-103, 231-237)
        assert!((calculate_blaze_burn_pitch(0.0) - 0.3).abs() < 1e-6);
        assert!((calculate_blaze_burn_pitch(1.0) - 1.0).abs() < 1e-6);
        assert!((calculate_blaze_burn_volume(0.0) - 1.0).abs() < 1e-6);
        assert!((calculate_blaze_burn_volume(1.0) - 2.0).abs() < 1e-6);
        assert!(should_blaze_play_burn_sound(0, false));
        assert!(!should_blaze_play_burn_sound(1, false));
        assert!(!should_blaze_play_burn_sound(0, true));
        let spread = calculate_blaze_small_fireball_spread(16.0); // distance = 4, sqrt(4) = 2, sqd = 1.0, spread = 2.297
        assert!((spread - 2.297).abs() < 1e-6);

        // 3. Magma Cube jump boosts and CubeMob jump sound (MagmaCube.java:96, 105; AbstractCubeMob.java:289)
        assert_eq!(calculate_magma_cube_jump_boost(1), 0.1);
        assert_eq!(calculate_magma_cube_jump_boost(2), 0.2);
        assert_eq!(calculate_magma_cube_jump_boost(4), 0.4);
        assert!((calculate_magma_cube_lava_jump_boost(1) - 0.27).abs() < 1e-6);
        assert!((calculate_magma_cube_lava_jump_boost(4) - 0.42).abs() < 1e-6);
        assert!(should_cube_mob_play_jump_sound(1));
        assert!(!should_cube_mob_play_jump_sound(0));

        // 4. Silverfish wake friends duration & Endermite lifetime (Silverfish.java:53, 189, Endermite.java:36, 52, 82)
        assert_eq!(SILVERFISH_MAX_HEALTH, 8.0);
        assert_eq!(SILVERFISH_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(SILVERFISH_BASE_ATTACK_DAMAGE, 1.0);
        assert_eq!(SILVERFISH_XP_REWARD, 5);
        assert_eq!(SILVERFISH_MERGE_INTERVAL_TICKS, 10);
        assert_eq!(SILVERFISH_WAKE_UP_FRIENDS_TICKS, 20);

        assert_eq!(ENDERMITE_MAX_HEALTH, 8.0);
        assert_eq!(ENDERMITE_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(ENDERMITE_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(ENDERMITE_XP_REWARD, 3);
        assert_eq!(ENDER_PEARL_ENDERMITE_SPAWN_CHANCE, 0.05);
        assert_eq!(ENDERMITE_LIFETIME_MAX, 2400);
        assert!(should_endermite_despawn(2400, false));
        assert!(should_endermite_despawn(2500, false));
        assert!(!should_endermite_despawn(2399, false));
        assert!(!should_endermite_despawn(3000, true)); // persistence required
    }

    #[test]
    fn test_boss_and_mini_boss_combat_details_parity() {
        // 1. Breeze fall threshold and whirl sound (Breeze.java:47, 191-192, 275)
        assert_eq!(BREEZE_FALL_DISTANCE_SOUND_THRESHOLD, 3.0);
        assert!(should_breeze_play_fall_sound(3.1));
        assert!(!should_breeze_play_fall_sound(3.0));
        assert!((calculate_breeze_whirl_pitch(0.0) - 0.7).abs() < 1e-6);
        assert!((calculate_breeze_whirl_pitch(1.0) - 1.1).abs() < 1e-6);
        assert!((calculate_breeze_whirl_volume(0.0) - 0.8).abs() < 1e-6);
        assert!((calculate_breeze_whirl_volume(1.0) - 1.0).abs() < 1e-6);

        // Skeleton family constants and effects parity (Skeleton.java, WitherSkeleton.java, Stray.java, Bogged.java, Parched.java)
        assert_eq!(SKELETON_MAX_HEALTH, 20.0);
        assert_eq!(SKELETON_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(SKELETON_XP_REWARD, 5);
        assert_eq!(SKELETON_HARD_ATTACK_INTERVAL, 20);
        assert_eq!(SKELETON_DEFAULT_ATTACK_INTERVAL, 40);
        assert_eq!(calculate_skeleton_attack_interval(true), 20);
        assert_eq!(calculate_skeleton_attack_interval(false), 40);
        assert_eq!(SKELETON_STRAY_CONVERSION_SUBMERGED_TICKS, 140);
        assert_eq!(SKELETON_STRAY_CONVERSION_SHAKING_TICKS, 300);
        assert_eq!(SKELETON_STRAY_CONVERSION_SOUND_EVENT_ID, 1048);
        assert_eq!(ABSTRACT_SKELETON_ARROW_VELOCITY, 1.6);
        assert_eq!(ABSTRACT_SKELETON_MELEE_SPEED_MODIFIER, 1.2);
        assert_eq!(ABSTRACT_SKELETON_FLEE_SUN_SPEED, 1.0);
        assert_eq!(ABSTRACT_SKELETON_AVOID_WOLF_DISTANCE, 6.0);
        assert_eq!(ABSTRACT_SKELETON_AVOID_WOLF_WALK_SPEED, 1.0);
        assert_eq!(ABSTRACT_SKELETON_AVOID_WOLF_SPRINT_SPEED, 1.2);

        assert_eq!(WITHER_SKELETON_MAX_HEALTH, 20.0);
        assert_eq!(WITHER_SKELETON_BASE_ATTACK_DAMAGE, 4.0);
        assert_eq!(WITHER_SKELETON_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(WITHER_SKELETON_LAVA_PATHFINDING_MALUS, 8.0);
        assert_eq!(WITHER_SKELETON_WITHER_DURATION_TICKS, 200);
        assert_eq!(WITHER_SKELETON_ARROW_IGNITE_SECONDS, 100.0);
        assert_eq!(WITHER_SKELETON_XP_REWARD, 5);

        assert_eq!(STRAY_MAX_HEALTH, 20.0);
        assert_eq!(STRAY_SLOWNESS_DURATION_TICKS, 600);
        assert_eq!(STRAY_HARD_ATTACK_INTERVAL, 20);
        assert_eq!(STRAY_DEFAULT_ATTACK_INTERVAL, 40);
        assert_eq!(calculate_stray_attack_interval(true), 20);
        assert_eq!(calculate_stray_attack_interval(false), 40);
        assert_eq!(STRAY_XP_REWARD, 5);

        assert_eq!(BOGGED_MAX_HEALTH, 16.0);
        assert_eq!(BOGGED_XP_REWARD, 5);
        assert_eq!(BOGGED_POISON_DURATION_TICKS, 100);
        assert_eq!(BOGGED_HARD_ATTACK_INTERVAL, 50);
        assert_eq!(BOGGED_DEFAULT_ATTACK_INTERVAL, 70);
        assert_eq!(calculate_bogged_attack_interval(true), 50);
        assert_eq!(calculate_bogged_attack_interval(false), 70);

        // Parched weakness duration and attack intervals (Parched.java:26, 33, 58, 63)
        assert_eq!(PARCHED_MAX_HEALTH, 16.0);
        assert_eq!(PARCHED_XP_REWARD, 5);
        assert_eq!(PARCHED_WEAKNESS_DURATION_TICKS, 600);
        assert_eq!(PARCHED_HARD_ATTACK_INTERVAL, 50);
        assert_eq!(PARCHED_DEFAULT_ATTACK_INTERVAL, 70);
        assert_eq!(calculate_parched_attack_interval(true), 50);
        assert_eq!(calculate_parched_attack_interval(false), 70);

        // 3. Wither boss invulnerability, health and shield (WitherBoss.java:66, 264, 464-469, 512, 516)
        assert_eq!(WITHER_INVULNERABLE_TICKS, 220);
        assert_eq!(WITHER_MAX_HEALTH, 300.0);
        assert_eq!(WITHER_ARMOR_POINTS, 4.0);
        assert_eq!(WITHER_SPAWN_EXPLOSION_RADIUS, 7.0);
        assert_eq!(calculate_wither_invulnerable_progress(220), 0.0);
        assert_eq!(calculate_wither_invulnerable_progress(0), 1.0);
        assert_eq!(calculate_wither_invulnerable_progress(110), 0.5);
        assert!(is_wither_powered(150.0, 300.0));
        assert!(is_wither_powered(100.0, 300.0));
        assert!(!is_wither_powered(151.0, 300.0));
        assert!(should_wither_deflect_projectile(true, true));
        assert!(!should_wither_deflect_projectile(false, true));
        assert!(!should_wither_deflect_projectile(true, false));

        // 4. Warden Sonic Boom damage, ranges and knockback (SonicBoom.java:19-22, 79-84)
        // Warden constants parity (Warden.java:78-100, 185-193, 319-322, SonicBoom.java:19-25, 79-82)
        assert_eq!(WARDEN_MAX_HEALTH, 500.0);
        assert_eq!(WARDEN_MOVEMENT_SPEED_WHEN_FIGHTING, 0.3);
        assert_eq!(WARDEN_KNOCKBACK_RESISTANCE, 1.0);
        assert_eq!(WARDEN_ATTACK_KNOCKBACK, 1.5);
        assert_eq!(WARDEN_ATTACK_DAMAGE, 30.0);
        assert_eq!(WARDEN_FOLLOW_RANGE, 24.0);
        assert_eq!(WARDEN_XP_REWARD, 5);
        assert_eq!(WARDEN_VIBRATION_COOLDOWN_TICKS, 40);
        assert_eq!(WARDEN_TIME_TO_USE_MELEE_UNTIL_SONIC_BOOM, 200);
        assert_eq!(WARDEN_DARKNESS_DISPLAY_LIMIT, 200);
        assert_eq!(WARDEN_DARKNESS_DURATION, 260);
        assert_eq!(WARDEN_DARKNESS_RADIUS, 20.0);
        assert_eq!(WARDEN_DARKNESS_INTERVAL, 120);
        assert_eq!(WARDEN_ANGERMANAGEMENT_TICK_DELAY, 20);
        assert_eq!(WARDEN_DEFAULT_ANGER, 35);
        assert_eq!(WARDEN_PROJECTILE_ANGER, 10);
        assert_eq!(WARDEN_ON_HURT_ANGER_BOOST, 20);
        assert_eq!(WARDEN_RECENT_PROJECTILE_TICK_THRESHOLD, 100);
        assert_eq!(WARDEN_TOUCH_COOLDOWN_TICKS, 20);
        assert_eq!(WARDEN_PROJECTILE_ANGER_DISTANCE, 30.0);
        assert_eq!(WARDEN_EMERGE_DURATION, 134);
        assert_eq!(WARDEN_SNIFFING_DURATION, 84);
        assert_eq!(WARDEN_ROAR_DURATION, 84);
        assert_eq!(WARDEN_DIGGING_DURATION, 100);
        assert_eq!(WARDEN_DISABLE_SHIELD_SECONDS, 5.0);
        assert_eq!(WARDEN_MAX_ANGER, 150);
        assert_eq!(WARDEN_DEFAULT_ANGER_DECREASE, 1);
        assert_eq!(WARDEN_ANGER_AGITATED_MIN, 40);
        assert_eq!(WARDEN_ANGER_ANGRY_MIN, 80);

        // Heartbeat calculation: anger 0 -> 40, anger 40 -> 25, anger 80+ -> 10
        assert_eq!(calculate_warden_heartbeat_delay(0), 40);
        assert_eq!(calculate_warden_heartbeat_delay(20), 33);
        assert_eq!(calculate_warden_heartbeat_delay(40), 25);
        assert_eq!(calculate_warden_heartbeat_delay(80), 10);
        assert_eq!(calculate_warden_heartbeat_delay(150), 10);

        assert_eq!(SONIC_BOOM_COOLDOWN, 40);
        assert_eq!(SONIC_BOOM_CHARGE_DURATION, 34);
        assert_eq!(SONIC_BOOM_TOTAL_DURATION, 60);
        assert_eq!(SONIC_BOOM_DAMAGE, 10.0);
        assert_eq!(SONIC_BOOM_DISTANCE_XZ, 15.0);
        assert_eq!(SONIC_BOOM_DISTANCE_Y, 20.0);
        assert_eq!(SONIC_BOOM_HORIZONTAL_KNOCKBACK, 2.5);
        assert_eq!(SONIC_BOOM_VERTICAL_KNOCKBACK, 0.5);
        let (kh, kv) = calculate_sonic_boom_knockback(0.0);
        assert_eq!(kh, 2.5);
        assert_eq!(kv, 0.5);
        let (kh_res, kv_res) = calculate_sonic_boom_knockback(0.5);
        assert_eq!(kh_res, 1.25);
        assert_eq!(kv_res, 0.25);
        let (kh_full, kv_full) = calculate_sonic_boom_knockback(1.0);
        assert_eq!(kh_full, 0.0);
        assert_eq!(kv_full, 0.0);
        assert!(is_sonic_boom_in_range(10.0, 15.0, 10.0)); // sqrt(200) = 14.14 <= 15, dy = 15 <= 20
        assert!(!is_sonic_boom_in_range(12.0, 15.0, 12.0)); // sqrt(288) = 16.97 > 15
        assert!(!is_sonic_boom_in_range(5.0, 21.0, 5.0)); // dy = 21 > 20
    }

    #[test]
    fn test_dragon_wither_skull_wind_charge_fireball_parity() {
        // 1. Ender Dragon health, damage, parts, takeoff and death (EnderDragon.java:61, 379, 389, 446-460, 522-550, 819, 840)
        assert_eq!(ENDER_DRAGON_MAX_HEALTH, 200.0);
        assert_eq!(ENDER_DRAGON_HEAD_COLLISION_DAMAGE, 10.0);
        assert_eq!(ENDER_DRAGON_WING_DAMAGE, 5.0);
        assert_eq!(ENDER_DRAGON_CRYSTAL_EXPLOSION_DAMAGE, 10.0);
        assert_eq!(ENDER_DRAGON_CRYSTAL_HEAL_INTERVAL_TICKS, 10);
        assert_eq!(ENDER_DRAGON_SITTING_DAMAGE_TAKEOFF_THRESHOLD, 50.0);
        assert_eq!(ENDER_DRAGON_DEATH_MAX_TICKS, 200);
        assert_eq!(ENDER_DRAGON_FIRST_KILL_XP, 12000);
        assert_eq!(ENDER_DRAGON_RESPAWN_KILL_XP, 500);

        // Part damage: Head(0) and Neck(1) full damage; others reduced damage: d/4 + min(d, 1)
        assert_eq!(calculate_dragon_part_damage(0, 12.0), 12.0); // Head
        assert_eq!(calculate_dragon_part_damage(1, 8.0), 8.0);   // Neck
        assert_eq!(calculate_dragon_part_damage(2, 12.0), 4.0);  // Body: 12/4 + 1 = 4
        assert_eq!(calculate_dragon_part_damage(6, 0.4), 0.5);   // Wing: 0.4/4 + 0.4 = 0.5
        assert_eq!(calculate_dragon_part_damage(3, 0.005), 0.0); // Negated under 0.01

        // Crystal heal check: every 10 ticks when health < max_health
        assert!(should_dragon_crystal_heal(10, 150.0, 200.0));
        assert!(should_dragon_crystal_heal(20, 0.0, 200.0));
        assert!(!should_dragon_crystal_heal(15, 150.0, 200.0));
        assert!(!should_dragon_crystal_heal(10, 200.0, 200.0));

        // Sitting damage takeoff check
        assert!(should_dragon_takeoff_from_sitting_damage(50.1));
        assert!(!should_dragon_takeoff_from_sitting_damage(50.0));
        assert!(!should_dragon_takeoff_from_sitting_damage(20.0));

        // Wing knockback formula: xd / dd * 4.0, 0.2, zd / dd * 4.0
        let push = calculate_dragon_wing_knockback(Vector3::new(4.0, 65.0, 0.0), Vector3::new(0.0, 65.0, 0.0));
        assert!((push.x - 1.0).abs() < 1e-6); // xd = 4, dd = 16, xd/dd*4 = 4/16*4 = 1.0
        assert_eq!(push.y, 0.2);
        assert_eq!(push.z, 0.0);

        // Dragon death experience drops
        assert_eq!(calculate_dragon_death_xp(160, false), 960); // 12000 * 0.08
        assert_eq!(calculate_dragon_death_xp(160, true), 40);   // 500 * 0.08
        assert_eq!(calculate_dragon_death_xp(200, false), 2400); // 12000 * 0.2
        assert_eq!(calculate_dragon_death_xp(200, true), 100);  // 500 * 0.2
        assert_eq!(calculate_dragon_death_xp(140, false), 0);   // Not yet 150
        assert_eq!(calculate_dragon_death_xp(162, false), 0);   // Not % 5

        // 2. Wither Skull damage and resistance (WitherSkull.java:49, 64, 73)
        assert_eq!(WITHER_SKULL_LIVING_DAMAGE, 8.0);
        assert_eq!(WITHER_SKULL_MAGIC_DAMAGE, 5.0);
        assert_eq!(WITHER_SKULL_DANGEROUS_MAX_RESISTANCE, 0.8);
        assert_eq!(calculate_wither_skull_damage(true), 8.0);
        assert_eq!(calculate_wither_skull_damage(false), 5.0);
        assert_eq!(calculate_wither_skull_block_resistance(true, true, 6.0), 0.8);
        assert_eq!(calculate_wither_skull_block_resistance(true, false, 6.0), 6.0);
        assert_eq!(calculate_wither_skull_block_resistance(false, true, 6.0), 6.0);

        // 3. Wind Charge explosion power, damage and offsets (WindCharge.java:23, 25; BreezeWindCharge.java:15; AbstractWindCharge.java:73, 90)
        assert_eq!(WIND_CHARGE_DIRECT_DAMAGE, 1.0);
        assert_eq!(WIND_CHARGE_EXPLOSION_POWER, 1.2);
        assert_eq!(BREEZE_WIND_CHARGE_EXPLOSION_POWER, 3.0);
        assert_eq!(WIND_CHARGE_DEFAULT_DEFLECT_COOLDOWN, 5);
        assert_eq!(WIND_CHARGE_BLOCK_HIT_NORMAL_OFFSET, 0.25);

        // 4. Fireball direct damages and ignite duration (LargeFireball.java:48; SmallFireball.java:41, 43)
        assert_eq!(LARGE_FIREBALL_DIRECT_DAMAGE, 6.0);
        assert_eq!(SMALL_FIREBALL_DIRECT_DAMAGE, 5.0);
        assert_eq!(SMALL_FIREBALL_IGNITE_SECONDS, 5.0);
        assert_eq!(SMALL_FIREBALL_IGNITE_TICKS, 100);
    }

    #[test]
    fn test_vex_and_evoker_parity() {
        // 1. Vex constants & attributes (Vex.java:40-41, 49, 65, 82)
        assert_eq!(VEX_DEFAULT_HEALTH, 14.0);
        assert_eq!(VEX_ATTACK_DAMAGE, 4.0);
        assert_eq!(VEX_XP_REWARD, 3);
        assert!((VEX_FLAP_DEGREES_PER_TICK - 45.836_624).abs() < 1e-5);
        assert_eq!(VEX_TICKS_PER_FLAP, 4);
        assert_eq!(VEX_DECAY_DAMAGE_INTERVAL_TICKS, 20);

        // Flapping tick check: tick % 4 == 0
        assert!(is_vex_flapping(0u32));
        assert!(is_vex_flapping(4u32));
        assert!(is_vex_flapping(8u32));
        assert!(!is_vex_flapping(1u32));
        assert!(!is_vex_flapping(3u32));

        // Summon lifetime calculation: 20 * (30 + rand(90)) => 600..2400 ticks (30..120s)
        assert_eq!(calculate_vex_summon_lifetime(0), 600);
        assert_eq!(calculate_vex_summon_lifetime(89), 2380);
        assert_eq!(calculate_vex_summon_lifetime(45), 1500);

        // 2. Evoker constants & spells (Evoker.java:50, 73, 136, 141, 150, 161, 245, 250, 258, 312, 342, 347, 352)
        assert_eq!(EVOKER_MAX_HEALTH, 24.0);
        assert_eq!(EVOKER_BASE_MOVEMENT_SPEED, 0.5);
        assert_eq!(EVOKER_FOLLOW_RANGE, 12.0);
        assert_eq!(EVOKER_XP_REWARD, 10);
        assert_eq!(EVOKER_FANGS_CAST_WARMUP, 20);
        assert_eq!(EVOKER_FANGS_CASTING_TIME, 40);
        assert_eq!(EVOKER_FANGS_CASTING_INTERVAL, 100);
        assert_eq!(EVOKER_FANGS_RING_RANGE_SQ, 9.0);
        assert_eq!(EVOKER_FANGS_LINE_COUNT, 16);
        assert_eq!(EVOKER_SUMMON_CAST_WARMUP, 20);
        assert_eq!(EVOKER_SUMMON_CASTING_TIME, 100);
        assert_eq!(EVOKER_SUMMON_CASTING_INTERVAL, 340);
        assert_eq!(EVOKER_SUMMON_VEX_COUNT, 3);
        assert_eq!(EVOKER_WOLOLO_CAST_WARMUP, 40);
        assert_eq!(EVOKER_WOLOLO_CASTING_TIME, 60);
        assert_eq!(EVOKER_WOLOLO_CASTING_INTERVAL, 140);
        assert_eq!(EVOKER_WOLOLO_SEARCH_RADIUS_XZ, 16.0);
        assert_eq!(EVOKER_WOLOLO_SEARCH_RADIUS_Y, 4.0);
    }

    #[test]
    fn test_hoglin_zoglin_piglin_combat_parity() {
        // 1. Hoglin constants & damage formulas (Hoglin.java:95-101, HoglinBase.java:18-53)
        assert_eq!(HOGLIN_MAX_HEALTH, 40.0);
        assert_eq!(HOGLIN_MOVEMENT_SPEED, 0.3);
        assert_eq!(HOGLIN_KNOCKBACK_RESISTANCE, 0.6);
        assert_eq!(HOGLIN_ATTACK_KNOCKBACK, 1.0);
        assert_eq!(HOGLIN_ATTACK_DAMAGE, 6.0);
        assert_eq!(HOGLIN_BABY_ATTACK_DAMAGE, 0.5);
        assert_eq!(HOGLIN_CONVERSION_TIME_TICKS, 300);
        assert_eq!(HOGLIN_XP_REWARD, 5);

        // Adult attack damage: base / 2.0 + rand(base) => 3.0 + (0..6) => 3.0..=8.0
        assert_eq!(calculate_hoglin_attack_damage(6.0, false, 0), 3.0);
        assert_eq!(calculate_hoglin_attack_damage(6.0, false, 5), 8.0);
        assert_eq!(calculate_hoglin_attack_damage(6.0, false, 2), 5.0);
        // Baby attack damage: stays base (0.5)
        assert_eq!(calculate_hoglin_attack_damage(0.5, true, 0), 0.5);
        assert_eq!(calculate_hoglin_attack_damage(0.5, true, 99), 0.5);

        // 2. Hoglin throw impulse (HoglinBase.java:38-53)
        // Zero effective knockback (attacker 1.0, defender 1.0 resistance) => None
        assert!(calculate_hoglin_throw_impulse(1.0, 0.0, 1.0, 1.0, 0.0, 0.5, 0.5).is_none());
        assert!(calculate_hoglin_throw_impulse(1.0, 0.0, 1.0, 1.5, 0.0, 0.5, 0.5).is_none());

        // Normal effective knockback (attacker 1.0, defender 0.0 resistance)
        let impulse = calculate_hoglin_throw_impulse(1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
        assert!(impulse.is_some());
        let imp = impulse.unwrap();
        // rand_h = 0.0 => horizontalScale = 1.0 * (0.0 * 0.5 + 0.2) = 0.2
        // angle = 0.0 => cos = 1, sin = 0 => vx = 0.2, vz = 0.0
        // rand_v = 1.0 => vy = 1.0 * 1.0 * 0.5 = 0.5
        assert!((imp.x - 0.2).abs() < 1e-5);
        assert!((imp.y - 0.5).abs() < 1e-5);
        assert!(imp.z.abs() < 1e-5);

        // 3. Zoglin constants (Zoglin.java:64-70)
        assert_eq!(ZOGLIN_MAX_HEALTH, 40.0);
        assert_eq!(ZOGLIN_MOVEMENT_SPEED, 0.3);
        assert_eq!(ZOGLIN_KNOCKBACK_RESISTANCE, 0.6);
        assert_eq!(ZOGLIN_ATTACK_KNOCKBACK, 1.0);
        assert_eq!(ZOGLIN_ATTACK_DAMAGE, 6.0);
        assert_eq!(ZOGLIN_BABY_ATTACK_DAMAGE, 0.5);
        assert_eq!(ZOGLIN_XP_REWARD, 5);

        // 4. Piglin Brute constants (PiglinBrute.java:46, 50-55)
        assert_eq!(PIGLIN_BRUTE_MAX_HEALTH, 50.0);
        assert_eq!(PIGLIN_BRUTE_MOVEMENT_SPEED, 0.35);
        assert_eq!(PIGLIN_BRUTE_ATTACK_DAMAGE, 7.0);
        assert_eq!(PIGLIN_BRUTE_FOLLOW_RANGE, 12.0);
        assert_eq!(PIGLIN_BRUTE_XP_REWARD, 20);

        // 5. Piglin constants and conversion nausea (Piglin.java:72-74, AbstractPiglin.java:113, PiglinAi.java:86)
        assert_eq!(PIGLIN_MAX_HEALTH, 16.0);
        assert_eq!(PIGLIN_MOVEMENT_SPEED, 0.35);
        assert_eq!(PIGLIN_ATTACK_DAMAGE, 5.0);
        assert_eq!(PIGLIN_XP_REWARD, 5);
        assert_eq!(PIGLIN_CONVERSION_TIME_TICKS, 300);
        assert_eq!(PIGLIN_CONVERSION_NAUSEA_DURATION_TICKS, 200);
        assert_eq!(PIGLIN_ADMIRE_DURATION_TICKS, 119);

        let effect = create_piglin_conversion_effect();
        assert_eq!(effect.effect_type, &pumpkin_data::effect::StatusEffect::NAUSEA);
        assert_eq!(effect.duration, 200);
        assert_eq!(effect.amplifier, 0);

        // 6. Speed modifiers: Piglin Baby (+20% base) and Zombified Piglin Angry (+0.05)
        assert_eq!(PIGLIN_BABY_SPEED_MODIFIER_ID, "minecraft:baby");
        assert_eq!(PIGLIN_BABY_SPEED_MODIFIER_AMOUNT, 0.2);
        assert!((calculate_piglin_baby_speed(0.35, true) - 0.42).abs() < 1e-6);
        assert_eq!(calculate_piglin_baby_speed(0.35, false), 0.35);

        assert_eq!(ZOMBIFIED_PIGLIN_ATTACKING_SPEED_MODIFIER_ID, "minecraft:attacking");
        assert_eq!(ZOMBIFIED_PIGLIN_ATTACKING_SPEED_MODIFIER_AMOUNT, 0.05);
        assert_eq!(ZOMBIFIED_PIGLIN_BASE_SPEED, 0.23);
        assert_eq!(ZOMBIFIED_PIGLIN_XP_REWARD, 5);
        assert!((calculate_zombified_piglin_speed(0.23, true, false) - 0.28).abs() < 1e-6);
        assert_eq!(calculate_zombified_piglin_speed(0.23, false, false), 0.23);
        // Baby zombified piglins do not gain angry speed modifier in vanilla
        assert_eq!(calculate_zombified_piglin_speed(0.23, true, true), 0.23);

        // 7. Extended Piglin Family Parity (Piglin.java:75-81, 288, 345, ZombifiedPiglin.java:61-67, 82, 84, 225)
        assert_eq!(PIGLIN_PROBABILITY_OF_SPAWNING_AS_BABY, 0.2);
        assert_eq!(PIGLIN_CHANCE_OF_WEARING_EACH_ARMOUR_ITEM, 0.1);
        assert_eq!(PIGLIN_PROBABILITY_OF_SPAWNING_WITH_CROSSBOW, 0.5);
        assert_eq!(PIGLIN_CROSSBOW_ATTACK_SPEED, 1.6);
        assert_eq!(PIGLIN_SPEAR_SPAWN_CHANCE_IN_TEN, 10);
        assert_eq!(calculate_piglin_experience_reward(false), 5);
        assert_eq!(calculate_piglin_experience_reward(true), 0);

        assert_eq!(ZOMBIFIED_PIGLIN_MAX_HEALTH, 20.0);
        assert_eq!(ZOMBIFIED_PIGLIN_ATTACK_DAMAGE, 5.0);
        assert_eq!(ZOMBIFIED_PIGLIN_ARMOR, 2.0);
        assert_eq!(ZOMBIFIED_PIGLIN_FOLLOW_RANGE, 35.0);
        assert_eq!(ZOMBIFIED_PIGLIN_SPAWN_REINFORCEMENTS_CHANCE, 0.0);
        assert_eq!(ZOMBIFIED_PIGLIN_ALERT_RANGE_Y, 10.0);
        assert_eq!(ZOMBIFIED_PIGLIN_ALERT_INTERVAL_MIN_TICKS, 80);
        assert_eq!(ZOMBIFIED_PIGLIN_ALERT_INTERVAL_MAX_TICKS, 120);
        assert_eq!(ZOMBIFIED_PIGLIN_SPEAR_SPAWN_CHANCE_IN_TWENTY, 20);
        assert_eq!(ZOMBIFIED_PIGLIN_LAVA_PATHFINDING_MALUS, 8.0);
    }

    #[test]
    fn test_illager_raider_family_parity() {
        use pumpkin_data::entity::EntityType;

        // 1. Pillager constants (Pillager.java:84-90)
        assert_eq!(PILLAGER_MAX_HEALTH, 24.0);
        assert_eq!(PILLAGER_MOVEMENT_SPEED, 0.35);
        assert_eq!(PILLAGER_ATTACK_DAMAGE, 5.0);
        assert_eq!(PILLAGER_FOLLOW_RANGE, 32.0);
        assert_eq!(PILLAGER_XP_REWARD, 5);

        // 2. Vindicator constants & Johnny logic (Vindicator.java:51, 89-95, 207-210)
        assert_eq!(VINDICATOR_MAX_HEALTH, 24.0);
        assert_eq!(VINDICATOR_MOVEMENT_SPEED, 0.35);
        assert_eq!(VINDICATOR_ATTACK_DAMAGE, 5.0);
        assert_eq!(VINDICATOR_FOLLOW_RANGE, 12.0);
        assert_eq!(VINDICATOR_XP_REWARD, 5);
        assert_eq!(VINDICATOR_JOHNNY_TAG, "Johnny");

        // Johnny attacks players, villagers, golems, monsters, animals
        assert!(is_johnny_targetable(&EntityType::PLAYER));
        assert!(is_johnny_targetable(&EntityType::VILLAGER));
        assert!(is_johnny_targetable(&EntityType::IRON_GOLEM));
        assert!(is_johnny_targetable(&EntityType::ZOMBIE));
        assert!(is_johnny_targetable(&EntityType::SKELETON));
        assert!(is_johnny_targetable(&EntityType::COW));
        assert!(is_johnny_targetable(&EntityType::SHEEP));
        // Johnny spares fellow illagers/raiders
        assert!(!is_johnny_targetable(&EntityType::VINDICATOR));
        assert!(!is_johnny_targetable(&EntityType::PILLAGER));
        assert!(!is_johnny_targetable(&EntityType::EVOKER));
        assert!(!is_johnny_targetable(&EntityType::ILLUSIONER));
        assert!(!is_johnny_targetable(&EntityType::RAVAGER));
        assert!(!is_johnny_targetable(&EntityType::WITCH));

        // 3. Illusioner constants & spell effects (Illusioner.java:45-47, 53, 80-82, 238, 270)
        assert_eq!(ILLUSIONER_MAX_HEALTH, 32.0);
        assert_eq!(ILLUSIONER_MOVEMENT_SPEED, 0.5);
        assert_eq!(ILLUSIONER_FOLLOW_RANGE, 18.0);
        assert_eq!(ILLUSIONER_XP_REWARD, 5);
        assert_eq!(ILLUSIONER_BLINDNESS_DURATION_TICKS, 400);
        assert_eq!(ILLUSIONER_INVISIBILITY_DURATION_TICKS, 1200);
        assert_eq!(ILLUSIONER_MIRROR_CASTING_TICKS, 20);
        assert_eq!(ILLUSIONER_NUM_ILLUSIONS, 4);
        assert_eq!(ILLUSIONER_ILLUSION_SPREAD, 3);

        let blindness = create_illusioner_blindness_effect();
        assert_eq!(blindness.effect_type, &pumpkin_data::effect::StatusEffect::BLINDNESS);
        assert_eq!(blindness.duration, 400);
        assert_eq!(blindness.amplifier, 0);

        let invisibility = create_illusioner_invisibility_effect();
        assert_eq!(invisibility.effect_type, &pumpkin_data::effect::StatusEffect::INVISIBILITY);
        assert_eq!(invisibility.duration, 1200);
        assert_eq!(invisibility.amplifier, 0);

        assert_eq!(VINDICATOR_BREAK_DOOR_TIME, 6);
        assert_eq!(VINDICATOR_BREAK_DOOR_CHANCE_INTERVAL, 10);

        // 3. Raider common constants (Raider.java:66, 92, 312, Vindicator.java:67)
        assert_eq!(RAIDER_MOVE_THROUGH_VILLAGE_SPEED, 1.05);
        assert_eq!(RAIDER_HOLD_GROUND_RADIUS, 10.0);
        assert_eq!(RAIDER_JOIN_RAID_CHECK_INTERVAL_TICKS, 20);
        assert_eq!(RAIDER_PATROL_BANNER_SEARCH_XZ, 16.0);
        assert_eq!(RAIDER_PATROL_BANNER_SEARCH_Y, 4.0);

        // 4. Witch constants (Witch.java:49-51, 55, 69, 107-109, 153, 216, 257, 273)
        assert_eq!(WITCH_MAX_HEALTH, 26.0);
        assert_eq!(WITCH_MOVEMENT_SPEED, 0.25);
        assert_eq!(WITCH_XP_REWARD, 5);
        assert_eq!(WITCH_DRINKING_SPEED_MODIFIER, -0.25);
        assert_eq!(WITCH_DRINKING_DURATION_TICKS, 32);
        assert_eq!(WITCH_MAGIC_DAMAGE_ABSORPTION_FACTOR, 0.15);
        assert_eq!(WITCH_RANGED_ATTACK_INTERVAL, 60);
        assert_eq!(WITCH_RANGED_ATTACK_MAX_RANGE, 10.0);
        assert_eq!(WITCH_RANGED_ATTACK_UNCERTAINTY, 8.0);
        assert_eq!(WITCH_CLOSE_THROW_SPEED, 0.45);
        assert_eq!(WITCH_FAR_THROW_SPEED, 0.75);
        assert_eq!(WITCH_CLOSE_THROW_DIST_THRESHOLD, 2.0);
        assert_eq!(calculate_witch_throw_speed(1.5), 0.45);
        assert_eq!(calculate_witch_throw_speed(2.0), 0.45);
        assert_eq!(calculate_witch_throw_speed(2.5), 0.75);
    }

    #[test]
    fn test_zombie_family_combat_and_conversion_parity() {
        // 1. Zombie base attributes & baby speed (Zombie.java:55-58, 93-97)
        assert_eq!(ZOMBIE_MAX_HEALTH, 20.0);
        assert_eq!(ZOMBIE_FOLLOW_RANGE, 35.0);
        assert_eq!(ZOMBIE_BASE_MOVEMENT_SPEED, 0.23);
        assert_eq!(ZOMBIE_BASE_ATTACK_DAMAGE, 3.0);
        assert_eq!(ZOMBIE_BASE_ARMOR, 2.0);
        assert_eq!(ZOMBIE_XP_REWARD, 5);
        assert_eq!(ZOMBIE_BABY_SPEED_MODIFIER_ID, "minecraft:baby");
        assert_eq!(ZOMBIE_BABY_SPEED_MODIFIER_AMOUNT, 0.5);

        // 2. Conversion timings (Zombie.java:101-105, Husk.java:30-45)
        assert_eq!(ZOMBIE_DROWN_CONVERSION_SUBMERGED_TICKS, 600);
        assert_eq!(ZOMBIE_DROWN_CONVERSION_SHAKING_TICKS, 300);

        // 3. Husk hunger duration formula (Husk.java:59-60)
        assert_eq!(calculate_husk_hunger_duration(pumpkin_util::Difficulty::Peaceful), 0);
        assert_eq!(calculate_husk_hunger_duration(pumpkin_util::Difficulty::Easy), 140);
        assert_eq!(calculate_husk_hunger_duration(pumpkin_util::Difficulty::Normal), 280);
        assert_eq!(calculate_husk_hunger_duration(pumpkin_util::Difficulty::Hard), 420);

        // 4. Drowned trident uncertainty & shoot speed (Drowned.java:82, 319-327)
        assert_eq!(DROWNED_TRIDENT_SHOOT_VELOCITY, 1.6);
        assert_eq!(DROWNED_STEP_HEIGHT, 1.0);
        assert_eq!(calculate_drowned_trident_uncertainty(pumpkin_util::Difficulty::Easy), 14.0);
        assert_eq!(calculate_drowned_trident_uncertainty(pumpkin_util::Difficulty::Normal), 10.0);
        assert_eq!(calculate_drowned_trident_uncertainty(pumpkin_util::Difficulty::Hard), 6.0);

        // 5. Extended Husk & Drowned Parity (Husk.java:84, Drowned.java:69, 70, 95, 200-203)
        assert_eq!(HUSK_TO_ZOMBIE_CONVERSION_SOUND_EVENT_ID, 1041);
        assert_eq!(DROWNED_NAUTILUS_SHELL_CHANCE, 0.03);
        assert_eq!(DROWNED_WEAPON_CHANCE, 0.10);
        assert_eq!(DROWNED_TRIDENT_ROLL_THRESHOLD, 10);
        assert_eq!(DROWNED_WEAPON_ROLL_MAX, 16);
        assert_eq!(DROWNED_ZOMBIE_NAUTILUS_JOCKEY_CHANCE, 0.5);
        assert_eq!(DROWNED_TRIDENT_ATTACK_INTERVAL_TICKS, 40);
        assert_eq!(DROWNED_TRIDENT_ATTACK_RANGE, 10.0);
    }
}
