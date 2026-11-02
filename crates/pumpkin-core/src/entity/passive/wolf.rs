use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::data_component_impl::FoodImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::wolf_sound_variant::WolfSoundVariant;
use pumpkin_data::wolf_variant::WolfVariant;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use rand::RngExt;

use crate::entity::combat;
use crate::entity::custom_sound::CustomSound;
use crate::entity::living::LivingEntity;
use crate::world::World;
use crate::entity::{
    Entity, EntityBase,
    ageable::AgeableMob,
    ai::behavior::neutral::apply_targets,
    ai::goal::{
        active_target::{ActiveTargetGoal, TargetCondition}, avoid_entity::AvoidEntityGoal, beg::BegGoal,
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_owner::FollowOwnerGoal,
        leap_at_target::LeapAtTargetGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, owner_hurt_by_target::OwnerHurtByTargetGoal,
        owner_hurt_target::OwnerHurtTargetGoal, revenge::RevengeGoal,
        sit_when_ordered_to::SitWhenOrderedToGoal, swim::SwimGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{
        Mob, MobEntity,
        neutral::{NeutralData, NeutralMob},
    },
    passive::{
        animal::Animal,
        tamable::{TamableAnimal, TamableData},
    },
    player::Player,
};

pub struct WolfEntity {
    pub mob_entity: MobEntity,
    pub variant: AtomicU8,
    pub sound_variant: AtomicU8,
    pub collar_color: AtomicU8,
    pub tamable_data: TamableData,
    pub ageable_data: crate::entity::ageable::AgeableData,
    pub neutral_data: NeutralData,
    pub is_wet: AtomicBool,
    pub is_shaking: AtomicBool,
    pub shake_anim: AtomicCell<f32>,
    pub shake_anim_o: AtomicCell<f32>,
}

impl WolfEntity {
    #[must_use]
    pub fn pick_random_sound_variant() -> WolfSoundVariant {
        let mut rng = rand::rng();
        WolfSoundVariant::ALL[rng.random_range(0..WolfSoundVariant::ALL.len())]
    }

    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        true
    }
    pub const XP_REWARD: u32 = combat::WOLF_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let wolf = Self {
            mob_entity,
            variant: AtomicU8::new(WolfVariant::Pale.id()),
            sound_variant: AtomicU8::new(Self::pick_random_sound_variant() as u8),
            collar_color: AtomicU8::new(combat::WOLF_DEFAULT_COLLAR_COLOR),
            tamable_data: TamableData::default(),
            ageable_data: crate::entity::ageable::AgeableData::default(),
            neutral_data: NeutralData::default(),
            is_wet: AtomicBool::new(false),
            is_shaking: AtomicBool::new(false),
            shake_anim: AtomicCell::new(0.0),
            shake_anim_o: AtomicCell::new(0.0),
        };
        let mob_arc = Arc::new(wolf);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Wolf attributes: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 4.0
        mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::MAX_HEALTH, combat::WOLF_START_HEALTH);
        mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::MOVEMENT_SPEED, combat::WOLF_BASE_MOVEMENT_SPEED);
        mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::ATTACK_DAMAGE, combat::WOLF_BASE_ATTACK_DAMAGE);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(combat::WOLF_START_HEALTH as f32);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Goal selector (matching Vanilla registerGoals):
            // 1: SwimGoal (FloatGoal)
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            // 1: EscapeDangerGoal (TamableAnimalPanicGoal) — only environmental damage causes panic
            goal_selector.add_goal(1, EscapeDangerGoal::new_environmental(1.5));
            // 2: SitWhenOrderedToGoal
            goal_selector.add_goal(2, Box::new(SitWhenOrderedToGoal::new()));
            // 3: Avoid Llama & Trader Llama (only untamed wolves avoid llamas)
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::LLAMA,
                    24.0,
                    1.5,
                    1.5,
                    |mob| mob.as_tamable().is_none_or(|t| !t.is_tamed()),
                )),
            );
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::TRADER_LLAMA,
                    24.0,
                    1.5,
                    1.5,
                    |mob| mob.as_tamable().is_none_or(|t| !t.is_tamed()),
                )),
            );
            // 4: LeapAtTargetGoal
            goal_selector.add_goal(4, Box::new(LeapAtTargetGoal::new(0.4)));
            // 5: MeleeAttackGoal
            goal_selector.add_goal(5, Box::new(MeleeAttackGoal::new(1.0, true)));
            // 6: FollowOwnerGoal
            goal_selector.add_goal(6, FollowOwnerGoal::new(1.0, 10.0, 2.0));
            // 7: BreedGoal
            goal_selector.add_goal(7, BreedGoal::new(1.0));
            // 8: WaterAvoidingRandomStrollGoal
            goal_selector.add_goal(8, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            // 9: BegGoal
            goal_selector.add_goal(9, BegGoal::new(8.0));
            // 10: LookAtPlayer & RandomLookAround
            goal_selector.add_goal(
                10,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Target selector (matching Vanilla registerGoals):
            // 1: OwnerHurtByTargetGoal
            target_selector.add_goal(1, OwnerHurtByTargetGoal::new());
            // 2: OwnerHurtTargetGoal
            target_selector.add_goal(2, OwnerHurtTargetGoal::new());
            // 3: HurtByTargetGoal (RevengeGoal)
            target_selector.add_goal(3, Box::new(RevengeGoal::new(true).alerting_others()));
            // 4: NearestAttackableTarget (Player, angry only), 8: ResetUniversalAnger
            apply_targets(&mut target_selector, &mob_arc.mob_entity, 4, 8, true);
            // 5: NonTameRandomTarget (Sheep, Rabbit, Fox)
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SHEEP, false)
                    .when(TargetCondition::NonTamed),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::RABBIT, false)
                    .when(TargetCondition::NonTamed),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::FOX, false)
                    .when(TargetCondition::NonTamed),
            );
            // 6: NonTameRandomTarget (Baby Turtle on land)
            target_selector.add_goal(
                6,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::TURTLE,
                    10,
                    false,
                    false,
                    Some(|target: &LivingEntity, _world: &World| {
                        target.entity.age.load(Ordering::Relaxed) < 0
                            && !target.entity.touching_water.load(Ordering::SeqCst)
                    }),
                ))
                .when(TargetCondition::NonTamed),
            );

            // 7: NearestAttackableTarget (Skeletons: Skeleton, Wither Skeleton, Stray, Bogged, Parched)
            for skeleton in [
                &EntityType::SKELETON,
                &EntityType::WITHER_SKELETON,
                &EntityType::STRAY,
                &EntityType::BOGGED,
                &EntityType::PARCHED,
            ] {
                target_selector.add_goal(
                    7,
                    ActiveTargetGoal::with_default(&mob_arc.mob_entity, skeleton, false),
                );
            }
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let is_baby = self.is_baby();
        let (w, h, eye) = combat::calculate_wolf_dimensions(
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

    pub fn get_tame_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_in_sitting_pose() {
            flags |= 0x01;
        }
        if self.is_tame() {
            flags |= 0x04;
        }
        flags
    }

    #[must_use]
    pub fn get_tail_angle(&self) -> f32 {
        let max_health = self.mob_entity.living_entity.get_max_health();
        let health = self.mob_entity.living_entity.health.load();
        combat::calculate_wolf_tail_angle(
            self.is_angry(),
            self.is_tame(),
            health,
            max_health,
        )
    }
}

impl AgeableMob for WolfEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }

    fn set_baby(&self, baby: bool) {
        self.set_age(if baby { self.get_baby_start_age() } else { 0 });
        self.refresh_dimensions();
    }

    fn age_boundary_reached(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        self.refresh_dimensions();
        crate::entity::ageable::handle_age_boundary_vehicle_dismount(self.is_baby(), entity);
    }
}

impl Animal for WolfEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        item.has_tag(&tag::Item::MINECRAFT_WOLF_FOOD) || item == &Item::BONE
    }
}

impl TamableAnimal for WolfEntity {
    fn get_tamable_data(&self) -> &TamableData {
        &self.tamable_data
    }

    fn is_unable_to_move_to_owner(&self) -> bool {
        self.is_ordered_to_sit() || self.is_shaking()
    }
}

impl NeutralMob for WolfEntity {
    fn get_neutral_data(&self) -> &NeutralData {
        &self.neutral_data
    }
}

impl Mob for WolfEntity {
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

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_tamable(&self) -> Option<&dyn TamableAnimal> {
        Some(self)
    }

    fn can_shear_equipment(&self, player: &Arc<Player>) -> bool {
        self.is_owned_by(&player.gameprofile.id)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_neutral(&self) -> Option<&dyn NeutralMob> {
        Some(self)
    }

    fn as_wolf(&self) -> Option<&WolfEntity> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<pumpkin_data::sound::Sound> {
        let is_baby = self.is_baby();
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        let mut rng = rand::rng();
        let rand_three = rng.random_range(0..3);
        let sound_type = combat::select_wolf_ambient_sound_type(
            self.is_angry(),
            self.is_tame(),
            self.mob_entity.living_entity.health.load(),
            rand_three,
        );
        match sound_type {
            combat::WolfAmbientSoundType::Growl => Some(sound_variant.growl_sound(is_baby)),
            combat::WolfAmbientSoundType::Whine => Some(sound_variant.whine_sound(is_baby)),
            combat::WolfAmbientSoundType::Pant => Some(sound_variant.pant_sound(is_baby)),
            combat::WolfAmbientSoundType::Ambient => Some(sound_variant.ambient_sound(is_baby)),
        }
    }


    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        self.set_ordered_to_sit(false);
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        let entity = self.get_entity();
        if !entity.is_alive() {
            self.is_wet.store(false, Ordering::Relaxed);
            self.cancel_shake();
            return;
        }

        let world = entity.world.load();

        if entity.is_in_water_or_rain() {
            self.is_wet.store(true, Ordering::Relaxed);
            if self.is_shaking.load(Ordering::Relaxed) {
                world.broadcast_entity_event(
                    entity,
                    pumpkin_data::entity_status::EntityStatus::CancelShakeWetness,
                    Some(pumpkin_protocol::bedrock::server::actor_event::ActorEventID::ShakeWetnessStop),
                );
                self.cancel_shake();
            }
        } else if self.is_wet.load(Ordering::Relaxed)
            && !self.is_shaking.load(Ordering::Relaxed)
            && (self.is_navigator_idle()
                || self
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .map_or(true, |n| n.is_done()))
            && entity.on_ground.load(Ordering::Relaxed)
        {
            self.is_shaking.store(true, Ordering::Relaxed);
            self.shake_anim.store(0.0);
            self.shake_anim_o.store(0.0);
            if let Ok(mut nav) = self.get_mob_entity().navigator.lock() {
                nav.stop();
            }
            world.broadcast_entity_event(
                entity,
                pumpkin_data::entity_status::EntityStatus::ShakeWetness,
                Some(pumpkin_protocol::bedrock::server::actor_event::ActorEventID::ShakeWetness),
            );
        } else if self.is_shaking.load(Ordering::Relaxed) {
            let pos = entity.pos.load();
            let anim = self.shake_anim.load();
            if anim == 0.0 {
                world.play_sound(
                    pumpkin_data::sound::Sound::EntityWolfShake,
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &pos,
                );
            }

            self.shake_anim_o.store(anim);
            let next_anim = anim + 0.05;
            self.shake_anim.store(next_anim);
            if self.shake_anim_o.load() >= 2.0 {
                self.is_wet.store(false, Ordering::Relaxed);
                self.is_shaking.store(false, Ordering::Relaxed);
                self.shake_anim_o.store(0.0);
                self.shake_anim.store(0.0);
            }

            if next_anim > 0.4 {
                let shake_count = (((next_anim - 0.4) * std::f32::consts::PI).sin() * 7.0) as i32;
                let vel = entity.velocity.load();
                let width = f64::from(entity.width());
                let mut rng = rand::rng();
                for _ in 0..shake_count {
                    let xo = (rng.random::<f64>() * 2.0 - 1.0) * width * 0.5;
                    let zo = (rng.random::<f64>() * 2.0 - 1.0) * width * 0.5;
                    world.spawn_particle(
                        pumpkin_util::math::vector3::Vector3::new(pos.x + xo, pos.y + 0.8, pos.z + zo),
                        pumpkin_util::math::vector3::Vector3::new(vel.x as f32, vel.y as f32, vel.z as f32),
                        1.0,
                        0,
                        pumpkin_data::particle::Particle::Splash,
                    );
                }
            }
        }
    }

    fn can_attack_with_owner(&self, target: &dyn EntityBase, owner: &dyn EntityBase) -> bool {
        let target_entity = target.get_entity();
        let target_type = target_entity.entity_type;
        if *target_type == EntityType::CREEPER
            || *target_type == EntityType::GHAST
            || *target_type == EntityType::ARMOR_STAND
        {
            return false;
        }

        if *target_type == EntityType::WOLF {
            if let Some(target_mob) = target.get_mob()
                && let Some(tamable) = target_mob.as_tamable()
                && tamable.is_tame()
                && let Some(target_owner) = tamable.get_owner()
                && let Some(owner_player) = owner.get_player()
                && target_owner == owner_player.gameprofile.id
            {
                return false;
            }
            return true;
        }

        if *target_type == EntityType::PLAYER {
            if let Some(owner_player) = owner.get_player()
                && let Some(target_player) = target.get_player()
            {
                if owner_player.gameprofile.id == target_player.gameprofile.id {
                    return false;
                }

                if !owner_player.can_harm_player(target_player) {
                    return false;
                }
            }
            return true;
        }

        if let Some(target_mob) = target.get_mob()
            && let Some(tamable) = target_mob.as_tamable()
            && tamable.is_tame()
        {
            return false;
        }

        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        let variant_id = self.variant.load(Ordering::Relaxed);
        let variant_str = WolfVariant::all()
            .get(variant_id as usize)
            .map_or("minecraft:pale", WolfVariant::asset_id);
        nbt.put_string("variant", variant_str.to_string());
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        nbt.put_string(
            "sound_variant",
            format!("minecraft:{}", sound_variant.to_name()),
        );
        nbt.put_byte(
            "CollarColor",
            self.collar_color.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant_str) = nbt.get_string("variant") {
            let variant =
                WolfVariant::from_name(variant_str).map_or(WolfVariant::Pale.id(), |v| v.id());
            self.variant.store(variant, Ordering::Relaxed);
        }
        if let Some(sound_str) = nbt.get_string("sound_variant")
            && let Some(sound_variant) = WolfSoundVariant::from_name(sound_str)
        {
            self.sound_variant
                .store(sound_variant as u8, Ordering::Relaxed);
        }
        if let Some(collar) = nbt.get_byte("CollarColor") {
            self.collar_color.store(collar as u8, Ordering::Relaxed);
        } else if let Some(collar_int) = nbt.get_int("CollarColor") {
            self.collar_color.store(collar_int as u8, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        let variant = WolfVariant::from_name(name).map_or(WolfVariant::Pale.id(), |v| v.id());
        self.variant.store(variant, Ordering::Relaxed);
    }

    fn mob_set_sound_variant_name(&self, name: &str) {
        if let Some(v) = WolfSoundVariant::from_name(name) {
            self.set_sound_variant(v);
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::wolf::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::TAMEABLE_FLAGS,
            self.get_tame_flags(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::COLLAR_COLOR,
            VarInt(self.collar_color.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::WOLF_VARIANT_ID,
            VarInt(self.variant.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::DATA_SOUND_VARIANT_ID,
            VarInt(self.sound_variant.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::OWNER_UUID,
            self.get_owner(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::DATA_ANGER_END_TIME,
            self.get_anger_end_time(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        let ambient = sound_variant.ambient_sound(self.is_baby());
        if self.is_tame() {
            if self.is_food(item_stack)
                && self.mob_entity.living_entity.health.load()
                    < self.mob_entity.living_entity.get_max_health()
            {
                let food_comp = item_stack.get_data_component::<FoodImpl>();
                let nutrition = food_comp.map(|f| f.nutrition.max(0) as u32);
                let heal_amount = combat::calculate_wolf_food_healing(nutrition);
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.mob_entity.living_entity.heal(heal_amount);
                self.play_eating_sound(ambient);
                return true;
            }

            if self.is_owned_by(&player.gameprofile.id) {
                if let Some(color) = super::animal::get_dye_color_from_item(item)
                    && color != self.get_collar_color()
                {
                    self.set_collar_color(color);
                    item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    return true;
                }

                let current_armor = {
                    let equip = self
                        .mob_entity
                        .living_entity
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    equip
                        .equipment
                        .get(&pumpkin_data::data_component_impl::EquipmentSlot::BODY)
                        .cloned()
                        .unwrap_or_else(|| ItemStack::EMPTY.clone())
                };

                // Equip wolf armor
                if item.id == Item::WOLF_ARMOR.id && current_armor.is_empty() && !self.is_baby() {
                    let mut armor_to_equip = item_stack.clone();
                    armor_to_equip.set_count(1);
                    item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    {
                        let mut equip = self
                            .mob_entity
                            .living_entity
                            .entity_equipment
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        equip
                            .equipment
                            .insert(pumpkin_data::data_component_impl::EquipmentSlot::BODY, armor_to_equip.clone());
                    }
                    self.mob_entity.living_entity.send_equipment_changes(&[(
                        pumpkin_data::data_component_impl::EquipmentSlot::BODY,
                        armor_to_equip,
                    )]);
                    let pos = self.get_entity().pos.load();
                    self.get_entity().world.load().play_sound(
                        pumpkin_data::sound::Sound::ItemArmorEquipWolf,
                        pumpkin_data::sound::SoundCategory::Neutral,
                        &pos,
                    );
                    return true;
                }

                // Shear wolf armor
                if item.id == Item::SHEARS.id && !current_armor.is_empty() {
                    return self.attempt_to_shear_equipment(player, pumpkin_util::Hand::Right, item_stack);
                }

                // Repair wolf armor with armadillo scute
                if item.id == Item::ARMADILLO_SCUTE.id && !current_armor.is_empty() {
                    let is_sitting = self.is_in_sitting_pose();
                    let current_damage = current_armor.get_damage();
                    let is_damaged = current_damage > 0;
                    if combat::can_repair_wolf_armor(is_sitting, is_damaged, true) {
                        let max_damage = current_armor.get_max_damage().unwrap_or(0);
                        let repair_unit = combat::calculate_wolf_armor_repair_amount(max_damage);
                        let mut repaired_armor = current_armor.clone();
                        repaired_armor.repair_item(repair_unit);
                        {
                            let mut equip = self
                                .mob_entity
                                .living_entity
                                .entity_equipment
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            equip
                                .equipment
                                .insert(pumpkin_data::data_component_impl::EquipmentSlot::BODY, repaired_armor.clone());
                        }
                        self.mob_entity.living_entity.send_equipment_changes(&[(
                            pumpkin_data::data_component_impl::EquipmentSlot::BODY,
                            repaired_armor,
                        )]);
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        let pos = self.get_entity().pos.load();
                        self.get_entity().world.load().play_sound(
                            pumpkin_data::sound::Sound::ItemWolfArmorRepair,
                            pumpkin_data::sound::SoundCategory::Neutral,
                            &pos,
                        );
                        player.trigger_advancement(
                            crate::entity::player::advancement::trigger::AdvancementTrigger::RepairWolfArmor,
                        );
                        return true;
                    }
                }

                let parent_interaction = self.animal_interact(player, item_stack, ambient);
                if !parent_interaction {
                    self.set_ordered_to_sit(!self.is_ordered_to_sit());
                    return true;
                }
                return parent_interaction;
            }
        } else if item == &Item::BONE && !self.mob_entity.is_attacking() {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let mut rng = rand::rng();
            if rng.random_range(0..combat::WOLF_TAME_CHANCE_IN_THREE) == 0 {
                TamableAnimal::tame(self, player.gameprofile.id);
                self.set_ordered_to_sit(true);
                self.mob_entity.living_entity.set_max_health(combat::WOLF_TAME_HEALTH as f32);
                self.mob_entity.living_entity.set_health(combat::WOLF_TAME_HEALTH as f32);
                self.spawn_taming_particles(true);
                let variant_id = self.variant.load(Ordering::Relaxed);
                let variant_str = WolfVariant::all()
                    .get(variant_id as usize)
                    .map_or("minecraft:pale", WolfVariant::asset_id);
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::TamedAnimal {
                        animal_type: "minecraft:wolf".to_string(),
                        variant: Some(variant_str.to_string()),
                    },
                );
            } else {
                self.spawn_taming_particles(false);
            }
            return true;
        }

        self.animal_interact(player, item_stack, ambient)
    }
}

impl CustomSound for WolfEntity {
    fn death_sound(&self) -> Option<pumpkin_data::sound::Sound> {
        let is_baby = self.is_baby();
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        Some(sound_variant.death_sound(is_baby))
    }

    fn hurt_sound(&self) -> Option<pumpkin_data::sound::Sound> {
        let is_baby = self.is_baby();
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        Some(sound_variant.hurt_sound(is_baby))
    }

    fn step_sound(&self) -> Option<pumpkin_data::sound::Sound> {
        let is_baby = self.is_baby();
        let sound_variant = WolfSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
            .unwrap_or_default();
        Some(sound_variant.step_sound(is_baby))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wolf_tail_and_armor_repair() {
        assert!(combat::can_repair_wolf_armor(true, true, true));
        assert!(!combat::can_repair_wolf_armor(false, true, true));
        assert!(!combat::can_repair_wolf_armor(true, false, true));
        assert_eq!(combat::calculate_wolf_armor_repair_amount(64), 8);

        let tail = combat::calculate_wolf_tail_angle(false, true, 40.0, 40.0);
        assert!((tail - 0.55 * std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn test_wolf_parity_constants() {
        assert_eq!(WolfEntity::XP_REWARD, 3);
        assert_eq!(combat::WOLF_START_HEALTH, 8.0);
        assert_eq!(combat::WOLF_TAME_HEALTH, 40.0);
        assert_eq!(combat::WOLF_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(combat::WOLF_BASE_ATTACK_DAMAGE, 4.0);
        assert_eq!(combat::WOLF_DEFAULT_COLLAR_COLOR, 14);
        assert_eq!(combat::WOLF_ARMOR_REPAIR_UNIT, 0.125);
        assert_eq!(combat::WOLF_TAME_CHANCE_IN_THREE, 3);
        assert_eq!(combat::WOLF_DEFAULT_TAIL_ANGLE, std::f32::consts::PI / 5.0);
        assert_eq!(combat::WOLF_ANGRY_TAIL_ANGLE, 1.539_380_4);
        assert_eq!(combat::WOLF_PERSISTENT_ANGER_MIN_TICKS, 400);
        assert_eq!(combat::WOLF_PERSISTENT_ANGER_MAX_TICKS, 780);
    }

    #[test]
    fn test_wolf_sound_variants_and_biome_spawn_priorities_parity() {
        // 1. All 7 Wolf Sound Variants parity (WolfSoundVariants.java:13-19, 45-53)
        assert_eq!(WolfSoundVariant::ALL.len(), 7);
        let names = ["angry", "big", "classic", "cute", "grumpy", "puglin", "sad"];
        for name in names {
            let variant = WolfSoundVariant::from_name(name).expect("Variant must exist by name");
            assert_eq!(variant.to_name(), name);
            let namespaced = format!("minecraft:{}", name);
            assert_eq!(WolfSoundVariant::from_name(&namespaced), Some(variant));
            assert_eq!(WolfSoundVariant::from_id(variant.id()), Some(variant));

            // Verify adult and baby sound sets are distinct and functional
            let _adult_ambient = variant.ambient_sound(false);
            let baby_ambient = variant.ambient_sound(true);
            assert_eq!(baby_ambient, pumpkin_data::sound::Sound::EntityBabyWolfAmbient);
            let _adult_hurt = variant.hurt_sound(false);
            let baby_hurt = variant.hurt_sound(true);
            assert_eq!(baby_hurt, pumpkin_data::sound::Sound::EntityBabyWolfHurt);
            let _adult_death = variant.death_sound(false);
            let baby_death = variant.death_sound(true);
            assert_eq!(baby_death, pumpkin_data::sound::Sound::EntityBabyWolfDeath);
            let _adult_step = variant.step_sound(false);
            let baby_step = variant.step_sound(true);
            assert_eq!(baby_step, pumpkin_data::sound::Sound::EntityBabyWolfStep);

            // Growl, pant, whine
            let _ = variant.growl_sound(false);
            let _ = variant.pant_sound(false);
            let _ = variant.whine_sound(false);
        }

        // 2. Ambient sound sub-type selection logic (Wolf.java:280-288)
        // Angry -> Growl
        assert_eq!(
            combat::select_wolf_ambient_sound_type(true, false, 8.0, 0),
            combat::WolfAmbientSoundType::Growl
        );
        assert_eq!(
            combat::select_wolf_ambient_sound_type(true, true, 40.0, 1),
            combat::WolfAmbientSoundType::Growl
        );

        // rand_three % 3 == 0:
        // Tame and health < 20.0 -> Whine
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, true, 19.9, 0),
            combat::WolfAmbientSoundType::Whine
        );
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, true, 5.0, 3),
            combat::WolfAmbientSoundType::Whine
        );
        // Tame and health >= 20.0 -> Pant
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, true, 20.0, 0),
            combat::WolfAmbientSoundType::Pant
        );
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, true, 40.0, 3),
            combat::WolfAmbientSoundType::Pant
        );
        // Untamed and health < 20.0 -> Pant
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, false, 5.0, 0),
            combat::WolfAmbientSoundType::Pant
        );

        // rand_three % 3 != 0 -> Ambient
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, true, 15.0, 1),
            combat::WolfAmbientSoundType::Ambient
        );
        assert_eq!(
            combat::select_wolf_ambient_sound_type(false, false, 8.0, 2),
            combat::WolfAmbientSoundType::Ambient
        );

        // 3. Biome variant selector parity (WolfVariants.java:73-83)
        assert_eq!(combat::determine_wolf_variant_from_biome("grove"), WolfVariant::Snowy);
        assert_eq!(combat::determine_wolf_variant_from_biome("minecraft:grove"), WolfVariant::Snowy);
        assert_eq!(combat::determine_wolf_variant_from_biome("old_growth_pine_taiga"), WolfVariant::Black);
        assert_eq!(combat::determine_wolf_variant_from_biome("snowy_taiga"), WolfVariant::Ashen);
        assert_eq!(combat::determine_wolf_variant_from_biome("forest"), WolfVariant::Woods);
        assert_eq!(combat::determine_wolf_variant_from_biome("old_growth_spruce_taiga"), WolfVariant::Chestnut);
        assert_eq!(combat::determine_wolf_variant_from_biome("savanna"), WolfVariant::Spotted);
        assert_eq!(combat::determine_wolf_variant_from_biome("savanna_plateau"), WolfVariant::Spotted);
        assert_eq!(combat::determine_wolf_variant_from_biome("windswept_savanna"), WolfVariant::Spotted);
        assert_eq!(combat::determine_wolf_variant_from_biome("jungle"), WolfVariant::Rusty);
        assert_eq!(combat::determine_wolf_variant_from_biome("sparse_jungle"), WolfVariant::Rusty);
        assert_eq!(combat::determine_wolf_variant_from_biome("bamboo_jungle"), WolfVariant::Rusty);
        assert_eq!(combat::determine_wolf_variant_from_biome("badlands"), WolfVariant::Striped);
        assert_eq!(combat::determine_wolf_variant_from_biome("eroded_badlands"), WolfVariant::Striped);
        assert_eq!(combat::determine_wolf_variant_from_biome("wooded_badlands"), WolfVariant::Striped);
        assert_eq!(combat::determine_wolf_variant_from_biome("taiga"), WolfVariant::Pale);
        assert_eq!(combat::determine_wolf_variant_from_biome("plains"), WolfVariant::Pale);

        // 4. Constants parity
        assert_eq!(combat::WOLF_STEP_SOUND_VOLUME, 0.15);
        assert_eq!(combat::WOLF_STEP_SOUND_PITCH, 1.0);
        assert_eq!(combat::WOLF_ARMOR_CRACK_PARTICLE_COUNT, 20);
        assert_eq!(combat::WOLF_ARMOR_CRACK_PARTICLE_OFFSET, (0.2, 0.1, 0.2));
        assert_eq!(combat::WOLF_ARMOR_CRACK_PARTICLE_SPEED, 0.1);
    }

    #[test]
    fn test_wolf_baby_dimensions_and_parity() {
        let adult = combat::calculate_wolf_dimensions(false, 0.6, 0.85, 0.68);
        assert_eq!(adult, (0.6, 0.85, 0.68));
        let baby = combat::calculate_wolf_dimensions(true, 0.6, 0.85, 0.68);
        assert_eq!(baby, (0.3, 0.425, 0.34375));
    }
}

impl WolfEntity {
    pub fn handle_wolf_armor_damage(&self, damage: f32, damage_type_name: &str) -> bool {
        if !combat::can_wolf_armor_absorb_damage(damage_type_name) {
            return false;
        }

        let mut current_armor = {
            let equip = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equip
                .equipment
                .get(&pumpkin_data::data_component_impl::EquipmentSlot::BODY)
                .cloned()
                .unwrap_or_else(|| ItemStack::EMPTY.clone())
        };

        if current_armor.is_empty() || current_armor.item.id != Item::WOLF_ARMOR.id {
            return false;
        }

        let max_damage = current_armor.get_max_damage().unwrap_or(combat::WOLF_ARMOR_MAX_DURABILITY);
        let damage_before = current_armor.get_damage();
        let (_rem_damage, new_damage, crack_changed, broken) =
            combat::calculate_wolf_armor_damage_absorption(damage, damage_before, max_damage);

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();

        if broken {
            {
                let mut equip = self
                    .mob_entity
                    .living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equip
                    .equipment
                    .remove(&pumpkin_data::data_component_impl::EquipmentSlot::BODY);
            }
            self.mob_entity.living_entity.send_equipment_changes(&[(
                pumpkin_data::data_component_impl::EquipmentSlot::BODY,
                ItemStack::EMPTY.clone(),
            )]);
            world.play_sound(
                pumpkin_data::sound::Sound::ItemWolfArmorBreak,
                pumpkin_data::sound::SoundCategory::Neutral,
                &pos,
            );
        } else {
            current_armor.set_damage(new_damage);
            {
                let mut equip = self
                    .mob_entity
                    .living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equip
                    .equipment
                    .insert(pumpkin_data::data_component_impl::EquipmentSlot::BODY, current_armor.clone());
            }
            self.mob_entity.living_entity.send_equipment_changes(&[(
                pumpkin_data::data_component_impl::EquipmentSlot::BODY,
                current_armor,
            )]);

            world.play_sound(
                pumpkin_data::sound::Sound::ItemWolfArmorDamage,
                pumpkin_data::sound::SoundCategory::Neutral,
                &pos,
            );

            if crack_changed {
                world.play_sound(
                    pumpkin_data::sound::Sound::ItemWolfArmorCrack,
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &pos,
                );
                world.spawn_particles(
                    pumpkin_data::particle::Particle::Item,
                    pumpkin_util::math::vector3::Vector3::new(pos.x, pos.y + 1.0, pos.z),
                    combat::WOLF_ARMOR_CRACK_PARTICLE_COUNT,
                    pumpkin_util::math::vector3::Vector3::new(
                        combat::WOLF_ARMOR_CRACK_PARTICLE_OFFSET.0,
                        combat::WOLF_ARMOR_CRACK_PARTICLE_OFFSET.1,
                        combat::WOLF_ARMOR_CRACK_PARTICLE_OFFSET.2,
                    ),
                    combat::WOLF_ARMOR_CRACK_PARTICLE_SPEED,
                );
            }
        }

        true
    }

    
    pub fn get_collar_color(&self) -> u8 {
        self.collar_color.load(Ordering::Relaxed)
    }

    pub fn set_collar_color(&self, color: u8) {
        self.collar_color.store(color, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::COLLAR_COLOR,
            VarInt(color as i32),
        );
    }

    pub fn get_variant(&self) -> WolfVariant {
        let variant_id = self.variant.load(Ordering::Relaxed);
        WolfVariant::all()
            .get(variant_id as usize)
            .copied()
            .unwrap_or(WolfVariant::Pale)
    }

    pub fn set_variant(&self, variant: WolfVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::WOLF_VARIANT_ID,
            VarInt(variant.id() as i32),
        );
    }

    pub fn set_sound_variant(&self, sound_variant: WolfSoundVariant) {
        self.sound_variant
            .store(sound_variant as u8, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::wolf::DATA_SOUND_VARIANT_ID,
            VarInt(sound_variant as u8 as i32),
        );
    }

    pub fn is_wet(&self) -> bool {
        self.is_wet.load(Ordering::Relaxed)
    }

    pub fn is_shaking(&self) -> bool {
        self.is_shaking.load(Ordering::Relaxed)
    }

    pub fn cancel_shake(&self) {
        self.is_shaking.store(false, Ordering::Relaxed);
        self.shake_anim.store(0.0);
        self.shake_anim_o.store(0.0);
    }
}

