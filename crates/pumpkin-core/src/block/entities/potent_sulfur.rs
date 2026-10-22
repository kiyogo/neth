use pumpkin_data::block_properties::{PotentSulfurLikeProperties, PotentSulfurState};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::Block;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};

use crate::world::World;

use super::BlockEntity;

pub struct PotentSulfurBlockEntity {
    pub position: BlockPos,
    pub waiting_countdown: AtomicI32,
    pub eruption_tick: AtomicI64,
}

impl BlockEntity for PotentSulfurBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let countdown = nbt.get_int("countdown").unwrap_or(-1);
        Self {
            position,
            waiting_countdown: AtomicI32::new(countdown),
            eruption_tick: AtomicI64::new(-1),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        // Vanilla Java PotentSulfurBlockEntity saves countdown unconditionally
        let countdown = self.waiting_countdown.load(Ordering::Relaxed);
        nbt.put_int("countdown", countdown);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_int(
            "countdown",
            self.waiting_countdown.load(Ordering::Relaxed),
        );
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn tick(&self, world: &Arc<World>) {
        let state_id = world.get_block_state_id(&self.position);
        let (block, _) = pumpkin_data::BlockState::from_id_with_block(state_id);
        if block != &Block::POTENT_SULFUR {
            return;
        }

        let mut props = PotentSulfurLikeProperties::from_state_id(state_id);
        match props.r#potent_sulfur_state {
            PotentSulfurState::Dry => {}
            PotentSulfurState::Wet => {
                // SERVER_NAUSEA_EFFECT_TICKER: every 10 ticks
                if world.get_world_age() % 10 == 0 {
                    self.tick_nausea_effect(world);
                }
            }
            PotentSulfurState::Dormant => {
                // SERVER_WAITING_COUNTDOWN_TICKER and SERVER_NAUSEA_EFFECT_TICKER
                if world.get_world_age() % 10 == 0 {
                    self.tick_nausea_effect(world);
                }
                if world.get_world_age() % 20 == 0 {
                    self.tick_waiting_countdown(world, &mut props);
                }
            }
            PotentSulfurState::Erupting => {
                // LAUNCH_ENTITY_TICKER every tick and SERVER_WAITING_COUNTDOWN_TICKER every 20 ticks
                self.tick_launch_entities(world);
                if world.get_world_age() % 20 == 0 {
                    self.tick_waiting_countdown(world, &mut props);
                }
            }
            PotentSulfurState::Continuous => {
                // LAUNCH_ENTITY_TICKER every tick
                self.tick_launch_entities(world);
            }
        }
    }
}

impl PotentSulfurBlockEntity {
    pub const ID: &'static str = "minecraft:potent_sulfur";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            waiting_countdown: AtomicI32::new(-1),
            eruption_tick: AtomicI64::new(-1),
        }
    }

    pub fn reset_countdown(&self) {
        self.waiting_countdown.store(-1, Ordering::Relaxed);
    }

    /// Finds the surface/water gas emission source block above this potent sulfur block.
    /// Scans up to 5 blocks above while waterlogged/water.
    pub fn find_noxious_gas_source_block(&self, world: &World) -> Option<BlockPos> {
        let max_y = self.position.0.y + 5;
        let mut curr = self.position.up();

        while curr.0.y <= max_y {
            let block = world.get_block(&curr);
            let is_water = block == &Block::WATER
                || world
                    .get_fluid(&curr)
                    .has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER);

            if !is_water {
                // If air or non-water passable, that's the surface
                if block == &Block::AIR {
                    return Some(curr);
                }
                break;
            }
            curr = curr.up();
        }

        None
    }

    /// Applies Nausea effect to living entities near the noxious gas source block.
    fn tick_nausea_effect(&self, world: &Arc<World>) {
        let Some(source_block) = self.find_noxious_gas_source_block(world) else {
            return;
        };

        let source_center = source_block.to_centered_f64();
        let entities = world.entities.load();
        for entity_base in entities.iter() {
            let entity = entity_base.get_entity();
            if !entity.is_alive() {
                continue;
            }

            let pos = entity.pos.load();
            if pos.squared_distance_to_vec(&source_center) <= 9.0 {
                if let Some(living) = entity_base.get_living_entity() {
                    living.add_effect(Effect {
                        effect_type: &StatusEffect::NAUSEA,
                        duration: 80,
                        amplifier: 0,
                        ambient: true,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    });
                }
            }
        }
    }

    /// Toggles state between Dormant and Erupting when waiting countdown expires.
    fn tick_waiting_countdown(&self, world: &Arc<World>, props: &mut PotentSulfurLikeProperties) {
        let Some(source_block) = self.find_noxious_gas_source_block(world) else {
            return;
        };

        let water_blocks = (source_block.0.y - self.position.0.y - 1).max(1);
        let mut countdown = self.waiting_countdown.load(Ordering::Relaxed);

        if countdown <= 0 {
            if props.r#potent_sulfur_state == PotentSulfurState::Dormant {
                countdown = 10 * (water_blocks - 1) + rand::random_range(15..=30);
            } else {
                countdown = (water_blocks - 1) + rand::random_range(1..=2);
            }
            self.waiting_countdown.store(countdown, Ordering::Relaxed);
        }

        if countdown > 0 {
            countdown -= 1;
            self.waiting_countdown.store(countdown, Ordering::Relaxed);
        }

        if countdown == 0 {
            let next_state = if props.r#potent_sulfur_state == PotentSulfurState::Dormant {
                PotentSulfurState::Erupting
            } else {
                PotentSulfurState::Dormant
            };

            props.r#potent_sulfur_state = next_state;
            world.set_block_state(
                &self.position,
                props.to_state_id(&Block::POTENT_SULFUR),
                BlockFlags::NOTIFY_ALL,
            );

            if next_state == PotentSulfurState::Erupting {
                world.play_sound(
                    Sound::BlockPotentSulfurGeyserEruption,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
        }
    }

    /// Launches non-flying entities upwards within the geyser water column.
    fn tick_launch_entities(&self, world: &Arc<World>) {
        let Some(source_block) = self.find_noxious_gas_source_block(world) else {
            return;
        };

        let water_blocks = (source_block.0.y - self.position.0.y - 1).max(1);
        let geyser_force_height = (6 * water_blocks).max(1);
        let center_x = self.position.0.x as f64 + 0.5;
        let center_z = self.position.0.z as f64 + 0.5;
        let min_y = self.position.0.y as f64 + 1.0;
        let max_y = min_y + geyser_force_height as f64;

        let entities = world.entities.load();
        for entity_base in entities.iter() {
            let entity = entity_base.get_entity();
            if !entity.is_alive() {
                continue;
            }

            let pos = entity.pos.load();
            if (pos.x - center_x).abs() <= 0.65
                && (pos.z - center_z).abs() <= 0.65
                && pos.y >= min_y
                && pos.y <= max_y
            {
                let velo = entity.velocity.load();
                let max_launch_speed = 0.3 + (water_blocks as f64) * 0.1;
                if velo.y < max_launch_speed {
                    let new_velo = Vector3::new(velo.x, (velo.y + 0.2).min(1.5), velo.z);
                    entity.velocity.store(new_velo);
                    entity.velocity_dirty.store(true, Ordering::Relaxed);
                    if let Some(living) = entity_base.get_living_entity() {
                        living.fall_distance.store(0.0);
                    }
                }
            }
        }
    }
}
