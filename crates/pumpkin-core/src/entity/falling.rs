use pumpkin_data::Block;
use pumpkin_data::BlockId;
use pumpkin_data::BlockState;
use pumpkin_data::BlockStateId;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::world::WorldEvent;
use pumpkin_protocol::bedrock::client::CUpdateBlock;
use pumpkin_protocol::java::client::play::CBlockUpdate;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use crossbeam::atomic::AtomicCell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU16, Ordering},
};

use crate::{
    block::blocks::falling::FallingBlock,
    entity::{Entity, EntityBase, living::LivingEntity},
    server::Server,
    world::World,
};

pub struct FallingEntity {
    entity: Entity,
    block_state_id: AtomicU16,
    fall_distance: AtomicCell<f32>,
    hurt_entities: AtomicBool,
    fall_damage_per_distance: AtomicCell<f32>,
    fall_damage_max: AtomicCell<f32>,
}

impl FallingEntity {
    pub fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        let block = Block::from_state_id(block_state_id);
        let is_anvil = block.has_tag(&tag::Block::MINECRAFT_ANVIL);
        let hurt_entities = is_anvil;
        let fall_damage_per_distance = if is_anvil {
            crate::entity::combat::ANVIL_FALL_DAMAGE_PER_DISTANCE
        } else {
            0.0
        };
        Self {
            entity,
            block_state_id: AtomicU16::new(block_state_id.as_u16()),
            fall_distance: AtomicCell::new(0.0),
            hurt_entities: AtomicBool::new(hurt_entities),
            fall_damage_per_distance: AtomicCell::new(fall_damage_per_distance),
            fall_damage_max: AtomicCell::new(crate::entity::combat::FALLING_BLOCK_DEFAULT_MAX_DAMAGE),
        }
    }

    /// Mirrors vanilla FallingBlockEntity.setHurtsEntities(float, int).
    pub fn set_hurts_entities(&self, damage_per_distance: f32, damage_max: f32) {
        self.hurt_entities.store(true, Ordering::Relaxed);
        self.fall_damage_per_distance.store(damage_per_distance);
        self.fall_damage_max.store(damage_max);
    }

    /// Sets the block state encoded in this entity's spawn packet.
    ///
    /// This must be called before the entity is spawned so clients render the
    /// requested block state when they receive its spawn packet.
    pub fn set_block_state(&self, block_state_id: BlockStateId) {
        self.block_state_id
            .store(block_state_id.as_u16(), Ordering::Relaxed);
        self.entity
            .data
            .store(i32::from(block_state_id.as_u16()), Ordering::Relaxed);
    }

    /// Replaces the current block with its fluid and spawns a falling one (synchronous)
    pub fn replace_spawn(world: &Arc<World>, position: BlockPos, block_state: BlockStateId) {
        let replacement = if block_state.is_waterlogged() {
            Block::WATER.default_state.id
        } else {
            Block::AIR.default_state.id
        };
        world.set_block_state(&position, replacement, BlockFlags::NOTIFY_ALL);
        let block_state = block_state
            .to_state()
            .set_waterlogged(false)
            .map_or(block_state, |state| state.id);

        let position = position.0.to_f64().add_raw(0.5, 0.0, 0.5);
        let entity = Entity::new(world.clone(), position, &EntityType::FALLING_BLOCK);
        let entity = Arc::new(Self::new(entity, block_state));
        entity.set_block_state(block_state);
        world.spawn_entity_non_save(entity);
    }
}

impl EntityBase for FallingEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.velocity.store(velo);

        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);

        // Preserve collision and block-interaction velocity changes (for example,
        // cobweb slowdown) instead of overwriting them with the pre-move velocity.
        let velo = entity.velocity.load();
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
            let fall_dist = self.fall_distance.swap(0.0);
            if self.hurt_entities.load(Ordering::Relaxed) && fall_dist > 1.0 {
                let damage = crate::entity::combat::calculate_falling_block_damage(
                    fall_dist,
                    self.fall_damage_per_distance.load(),
                    self.fall_damage_max.load(),
                );
                if damage > 0.0 {
                    let world = entity.world.load();
                    let bb = entity.bounding_box.load();
                    let victims = world.get_entities_at_box(&bb);
                    for victim in victims {
                        if victim.get_entity().entity_id != entity.entity_id {
                            let _ = victim.damage_with_context(
                                caller,
                                damage,
                                DamageType::FALLING_ANVIL,
                                Some(entity.pos.load()),
                                Some(caller),
                                None,
                            );
                        }
                    }
                }
            }
            let world = entity.world.load();
            let landing_pos = self.entity.block_pos.load();
            let mut state_id =
                BlockStateId::new_or_air(self.block_state_id.load(Ordering::Relaxed));
            let block = Block::from_state_id(state_id);

            if shatters_on_landing(block) {
                world.sync_world_event(
                    WorldEvent::ParticlesDestroyBlock,
                    landing_pos,
                    i32::from(state_id.as_u16()),
                );
                if let Some(sound) = shatter_sound(block) {
                    world.play_sound(sound, SoundCategory::Blocks, &landing_pos.to_f64());
                }
                self.entity.remove();
                return;
            }

            // FallingBlockEntity.tick waits for a moving piston instead of replacing it.
            if world.get_block(&landing_pos) != &Block::MOVING_PISTON {
                if block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS)
                    && FallingBlock::should_solidify(&**world, &landing_pos)
                    && let Some(name) = block.name.strip_suffix("_powder")
                    && let Some(concrete) = Block::from_name(name)
                {
                    state_id = concrete.default_state.id;
                }
                let landed_block = Block::from_state_id(state_id);
                let can_land = world.get_block_state(&landing_pos).replaceable()
                    && world.block_registry.can_place_at(
                        None,
                        Some(&**world),
                        &**world,
                        None,
                        landed_block,
                        BlockState::from_id(state_id),
                        &landing_pos,
                        None,
                        None,
                    );
                if can_land {
                    let mut event = crate::plugin::api::events::entity::entity_change_block::EntityChangeBlockEvent::new(
                        entity.entity_id,
                        landing_pos,
                        format!("minecraft:{}", landed_block.name),
                    );
                    if let Some(server) = world.server.upgrade() {
                        server.plugin_manager.fire_blocking(&server, &mut event);
                    }
                    if !event.cancelled {
                        if world.is_water_source(&landing_pos)
                            && let Some(waterlogged) = state_id.to_state().set_waterlogged(true)
                        {
                            state_id = waterlogged.id;
                        }
                        world.set_block_state(&landing_pos, state_id, BlockFlags::NOTIFY_ALL);
                        // block updates to watchers before the despawn, else a invisible block gap until the tick flush.
                        let placed = world.get_block_state_id(&landing_pos);
                        world.send_to_tracking_players_editioned(
                            entity,
                            &CBlockUpdate::new(landing_pos, i32::from(placed.as_u16()).into()),
                            &CUpdateBlock::new(landing_pos, BlockState::to_be_network_id(placed)),
                        );
                    }
                } else if world.level_info.load().game_rules.entity_drops
                    && let Some(item) = Item::from_registry_key(landed_block.name)
                {
                    world.drop_stack(&landing_pos, ItemStack::new(1, item));
                }
                self.entity.remove();
            }
        }

        if !entity.on_ground.load(Ordering::Relaxed) && velo.y < 0.0 {
            self.fall_distance.store(self.fall_distance.load() + (-velo.y as f32));
        }

        entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));
    }

    fn init_data_tracker(&self) {
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::falling_block::START_POS,
            self.entity.block_pos.load(),
        );
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn can_hit(&self) -> bool {
        !self.entity.is_removed()
    }

    fn damage(&self, _caller: &dyn EntityBase, _amount: f32, _damage_type: DamageType) -> bool {
        false
    }

    fn get_gravity(&self) -> f64 {
        crate::entity::combat::FALLING_BLOCK_DEFAULT_GRAVITY
    }

    // TODO: Bedrock spawn metadata lacks the block variant (renders grey while falling)
    fn bedrock_y_offset(&self) -> f64 {
        0.49
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn shatters_on_landing(block: &Block) -> bool {
    block.id == BlockId::SUSPICIOUS_SAND || block.id == BlockId::SUSPICIOUS_GRAVEL
}

fn shatter_sound(block: &Block) -> Option<Sound> {
    if block.id == BlockId::SUSPICIOUS_SAND {
        Some(Sound::BlockSuspiciousSandBreak)
    } else if block.id == BlockId::SUSPICIOUS_GRAVEL {
        Some(Sound::BlockSuspiciousGravelBreak)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspicious_blocks_shatter_with_their_sound() {
        assert!(shatters_on_landing(&Block::SUSPICIOUS_SAND));
        assert!(shatters_on_landing(&Block::SUSPICIOUS_GRAVEL));
        assert_eq!(
            shatter_sound(&Block::SUSPICIOUS_SAND),
            Some(Sound::BlockSuspiciousSandBreak)
        );
        assert_eq!(
            shatter_sound(&Block::SUSPICIOUS_GRAVEL),
            Some(Sound::BlockSuspiciousGravelBreak)
        );
    }

    #[test]
    fn ordinary_falling_blocks_place_themselves() {
        for block in [
            &Block::SAND,
            &Block::GRAVEL,
            &Block::RED_SAND,
            &Block::WHITE_CONCRETE_POWDER,
        ] {
            assert!(!shatters_on_landing(block));
            assert_eq!(shatter_sound(block), None);
        }
    }
}
