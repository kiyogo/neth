use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use uuid::Uuid;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;

use crate::entity::{
    Entity, EntityBase,
    ai::control::flying_move_control::FlyingMoveControl,
    ai::pathfinder::Navigator,
    ai::goal::{
        escape_danger::EscapeDangerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        water_avoiding_random_flying::WaterAvoidingRandomFlyingGoal,
    },
    custom_sound::CustomSound,
    combat,
    mob::{Mob, MobEntity},
    player::Player,
};

pub struct AllayEntity {
    pub mob_entity: MobEntity,
    pub dancing: AtomicBool,
    pub can_duplicate: AtomicBool,
    pub duplication_cooldown: AtomicI32,
    pub jukebox_pos: AtomicCell<Option<BlockPos>>,
    pub owner: AtomicCell<Option<Uuid>>,
    pub heal_timer: AtomicI32,
}

impl AllayEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::ALLAY_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FlyingMoveControl::new(20, true));
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::flying();

        let allay = Self {
            mob_entity,
            dancing: AtomicBool::new(false),
            can_duplicate: AtomicBool::new(true),
            duplication_cooldown: AtomicI32::new(0),
            jukebox_pos: AtomicCell::new(None),
            owner: AtomicCell::new(None),
            heal_timer: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(allay);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Allay: MAX_HEALTH = 20.0, FLYING_SPEED = 0.1, MOVEMENT_SPEED = 0.1, ATTACK_DAMAGE = 2.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::ALLAY_MAX_HEALTH as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::FLYING_SPEED, crate::entity::combat::ALLAY_FLYING_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::ALLAY_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, crate::entity::combat::ALLAY_BASE_ATTACK_DAMAGE as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::ALLAY_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Vanilla AllayAi: SPEED_MULTIPLIER_WHEN_PANICKING = 2.5
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.5));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomFlyingGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn is_dancing(&self) -> bool {
        self.dancing.load(Ordering::Relaxed)
    }

    pub fn set_dancing(&self, dancing: bool) {
        self.dancing.store(dancing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::allay::DATA_DANCING, dancing);
    }

    #[must_use]
    pub fn get_jukebox_pos(&self) -> Option<BlockPos> {
        self.jukebox_pos.load()
    }

    /// Vanilla Allay.java:319-329: setJukeboxPlaying
    pub fn set_jukebox_playing(&self, jukebox: BlockPos, is_playing: bool) {
        if is_playing {
            if !self.is_dancing() {
                self.jukebox_pos.store(Some(jukebox));
                self.set_dancing(true);
            }
        } else if self.jukebox_pos.load() == Some(jukebox) || self.jukebox_pos.load().is_none() {
            self.jukebox_pos.store(None);
            self.set_dancing(false);
        }
    }

    /// Vanilla Allay.java:388-392: shouldStopDancing
    pub fn should_stop_dancing(&self) -> bool {
        let Some(pos) = self.jukebox_pos.load() else {
            return true;
        };
        let entity_pos = self.mob_entity.living_entity.entity.pos.load();
        let dx = f64::from(pos.0.x) + 0.5 - entity_pos.x;
        let dy = f64::from(pos.0.y) + 0.5 - entity_pos.y;
        let dz = f64::from(pos.0.z) + 0.5 - entity_pos.z;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        if !crate::block::blocks::jukebox::JukeboxBlock::can_allay_hear_jukebox(dist_sq) {
            return true;
        }
        let world = self.mob_entity.living_entity.entity.world.load();
        world.get_block(&pos).id != pumpkin_data::Block::JUKEBOX.id
    }

    #[must_use]
    pub fn can_duplicate(&self) -> bool {
        self.can_duplicate.load(Ordering::Relaxed)
    }

    pub fn set_can_duplicate(&self, can_duplicate: bool) {
        self.can_duplicate.store(can_duplicate, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_CAN_DUPLICATE,
            can_duplicate,
        );
    }

    #[must_use]
    pub fn has_item_in_hand(&self) -> bool {
        let equip = self
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equip
            .equipment
            .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
            .is_some_and(|item| !item.is_empty())
    }
}

impl Mob for AllayEntity {
    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("CanDuplicate", self.can_duplicate());
        nbt.put_int(
            "DuplicationCooldown",
            self.duplication_cooldown.load(Ordering::Relaxed),
        );
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(can) = nbt.get_bool("CanDuplicate") {
            self.set_can_duplicate(can);
        }
        if let Some(cd) = nbt.get_int("DuplicationCooldown") {
            self.duplication_cooldown.store(cd, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_allay(&self) -> Option<&AllayEntity> {
        Some(self)
    }

    fn can_be_leashed(&self) -> bool {
        true
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let cd = self.duplication_cooldown.load(Ordering::Relaxed);
        if cd > 0 {
            let next = cd - 1;
            self.duplication_cooldown.store(next, Ordering::Relaxed);
            if next == 0 {
                self.set_can_duplicate(true);
            }
        }

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla Allay.aiStep: heals 1.0 HP every 10 ticks while alive.
        let timer = self.heal_timer.fetch_add(1, Ordering::Relaxed) + 1;
        if timer % 10 == 0 && entity.is_alive() {
            let living = &self.mob_entity.living_entity;
            if living.health.load() < living.get_max_health() {
                living.heal(1.0);
            }
        }

        // Vanilla Allay.java:221-224: if (this.isDancing() && this.shouldStopDancing() && this.tickCount % 20 == 0)
        if self.is_dancing() && timer % 20 == 0 && self.should_stop_dancing() {
            self.set_dancing(false);
            self.jukebox_pos.store(None);
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if self.has_item_in_hand() {
                Sound::EntityAllayAmbientWithItem
            } else {
                Sound::EntityAllayAmbientWithoutItem
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        if self.has_item_in_hand() {
            Some(Sound::EntityAllayAmbientWithItem)
        } else {
            Some(Sound::EntityAllayAmbientWithoutItem)
        }
    }

    fn pre_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        // Vanilla Allay.hurtServer: immune to damage from liked player / owner.
        if let Some(source) = source
            && let Some(owner) = self.owner.load()
            && let Some(player) = source.cast_any().downcast_ref::<Player>()
            && player.gameprofile.id == owner
        {
            return false;
        }
        true
    }

    /// Vanilla `Allay.java:179-180`: Allays are flying mobs that are completely immune to fall damage.
    fn calculate_fall_damage(&self, _fall_distance: f64, _damage_modifier: f32) -> i32 {
        combat::calculate_allay_fall_damage_points()
    }

    fn adjust_damage_amount(&self, amount: f32, damage_type: &DamageType) -> f32 {
        if *damage_type == DamageType::FALL {
            0.0
        } else {
            amount
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_DANCING,
            self.is_dancing(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_CAN_DUPLICATE,
            self.can_duplicate(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_dancing() && self.can_duplicate() && combat::is_allay_duplication_item(item) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            self.set_can_duplicate(false);
            self.duplication_cooldown.store(6000, Ordering::Relaxed);

            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.spawn_particle(
                pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                Vector3::new(0.5, 0.5, 0.5),
                1.0,
                3, // Vanilla Allay.java:79: NUM_OF_DUPLICATION_HEARTS = 3
                Particle::Heart,
            );
            world.play_sound_fine(
                Sound::BlockAmethystBlockChime,
                SoundCategory::Neutral,
                &pos,
                2.0,
                1.0,
            );

            let new_allay = Self::new(Entity::new(world.clone(), pos, &EntityType::ALLAY));
            world.spawn_entity(new_allay);
            return true;
        }

        let current_item = {
            let equip = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equip
                .equipment
                .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
                .cloned()
                .unwrap_or_else(|| ItemStack::EMPTY.clone())
        };

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();

        if current_item.is_empty() && !item_stack.is_empty() {
            let mut item_to_give = item_stack.clone();
            item_to_give.set_count(1);
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
                    .insert(pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND, item_to_give.clone());
            }
            self.mob_entity.living_entity.send_equipment_changes(&[(
                pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
                item_to_give.clone(),
            )]);

            self.owner.store(Some(player.gameprofile.id));
            world.play_sound_fine(Sound::EntityAllayItemGiven, SoundCategory::Neutral, &pos, 2.0, 1.0);

            if item_to_give.item.id == Item::CAKE.id {
                let block_pos = pos.to_block_pos();
                let mut near_noteblock = false;
                for dx in -3..=3 {
                    for dy in -3..=3 {
                        for dz in -3..=3 {
                            let check_pos = pumpkin_util::math::position::BlockPos(
                                pumpkin_util::math::vector3::Vector3::new(
                                    block_pos.0.x + dx,
                                    block_pos.0.y + dy,
                                    block_pos.0.z + dz,
                                ),
                            );
                            if world.get_block(&check_pos).id == pumpkin_data::Block::NOTE_BLOCK.id {
                                near_noteblock = true;
                                break;
                            }
                        }
                        if near_noteblock {
                            break;
                        }
                    }
                    if near_noteblock {
                        break;
                    }
                }
                if near_noteblock {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::AllayDeliverCakeToNoteBlock,
                    );
                }
            }

            return true;
        }

        if !current_item.is_empty() && item_stack.is_empty() {
            {
                let mut equip = self
                    .mob_entity
                    .living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equip
                    .equipment
                    .remove(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND);
            }
            self.mob_entity.living_entity.send_equipment_changes(&[(
                pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
                ItemStack::EMPTY.clone(),
            )]);

            world.play_sound_fine(Sound::EntityAllayItemTaken, SoundCategory::Neutral, &pos, 2.0, 1.0);

            let mut to_give = current_item.clone();
            let was_added = player.inventory().insert_stack_anywhere(&mut to_give);
            if !was_added && !to_give.is_empty() {
                world.drop_stack(&player.position().to_block_pos(), to_give);
            }
            player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .send_content_updates();

            player.trigger_advancement(
                crate::entity::player::advancement::trigger::AdvancementTrigger::AllayDeliverItemToPlayer,
            );

            if current_item.item.id == Item::CAKE.id {
                let block_pos = pos.to_block_pos();
                let mut near_noteblock = false;
                for dx in -3..=3 {
                    for dy in -3..=3 {
                        for dz in -3..=3 {
                            let check_pos = pumpkin_util::math::position::BlockPos(
                                pumpkin_util::math::vector3::Vector3::new(
                                    block_pos.0.x + dx,
                                    block_pos.0.y + dy,
                                    block_pos.0.z + dz,
                                ),
                            );
                            if world.get_block(&check_pos).id == pumpkin_data::Block::NOTE_BLOCK.id {
                                near_noteblock = true;
                                break;
                            }
                        }
                        if near_noteblock {
                            break;
                        }
                    }
                    if near_noteblock {
                        break;
                    }
                }
                if near_noteblock {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::AllayDeliverCakeToNoteBlock,
                    );
                }
            }

            return true;
        }

        if self.owner.load().is_none() {
            self.owner.store(Some(player.gameprofile.id));
        }

        false
    }
}

impl CustomSound for AllayEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityAllayDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityAllayHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat;

    #[test]
    fn test_allay_parity_constants() {
        assert_eq!(AllayEntity::XP_REWARD, 2);
        assert_eq!(combat::ALLAY_MAX_HEALTH, 20.0);
        assert_eq!(combat::ALLAY_FLYING_SPEED, 0.1);
        assert_eq!(combat::ALLAY_BASE_MOVEMENT_SPEED, 0.1);
        assert_eq!(combat::ALLAY_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(combat::ALLAY_DUPLICATION_COOLDOWN_TICKS, 6000);
    }
}
