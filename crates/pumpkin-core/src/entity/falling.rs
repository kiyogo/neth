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
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
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
}

impl FallingEntity {
    pub const fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        Self {
            entity,
            block_state_id: AtomicU16::new(block_state_id.as_u16()),
        }
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

    /// Replaced the current Block and Spawns a new Falling one (synchronous)
    pub fn replace_spawn(world: &Arc<World>, position: BlockPos, block_state: BlockStateId) {
        // Replace the original block, TODO: use fluid state
        world.set_block_state(
            &position,
            Block::AIR.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );

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
    fn damage(&self, _caller: &dyn EntityBase, _amount: f32, _damage_type: DamageType) -> bool {
        false
    }

    fn get_gravity(&self) -> f64 {
        0.04
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
