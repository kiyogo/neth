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
}





