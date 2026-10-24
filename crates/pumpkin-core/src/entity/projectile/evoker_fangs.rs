use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityStatus;
use pumpkin_data::sound::Sound;

use crate::{
    entity::{Entity, EntityBase},
    server::Server,
};

pub struct EvokerFangsEntity {
    pub entity: Entity,
    pub warmup_delay_ticks: AtomicI32,
    pub life_ticks: AtomicI32,
    pub owner_id: Option<i32>,
    pub sent_spike_event: AtomicBool,
}

impl EvokerFangsEntity {
    #[must_use]
    pub fn new(entity: Entity, warmup_ticks: u32, yaw: f32, owner_id: Option<i32>) -> Self {
        entity.set_rotation(yaw, 0.0);
        Self {
            entity,
            warmup_delay_ticks: AtomicI32::new(warmup_ticks as i32),
            life_ticks: AtomicI32::new(22),
            owner_id,
            sent_spike_event: AtomicBool::new(false),
        }
    }
}

impl EntityBase for EvokerFangsEntity {
    fn write_custom_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        nbt.put_int("Warmup", self.warmup_delay_ticks.load(Ordering::Relaxed));
    }

    fn read_custom_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        if let Some(warmup) = nbt.get_int("Warmup") {
            self.warmup_delay_ticks.store(warmup, Ordering::Relaxed);
        }
    }

    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        let world = entity.world.load();

        let warmup = self.warmup_delay_ticks.fetch_sub(1, Ordering::Relaxed) - 1;
        if warmup < 0 {
            if crate::entity::combat::is_evoker_fangs_attack_tick(warmup) {
                let bb = entity.bounding_box.load().expand(0.2, 0.0, 0.2);
                let candidates = world.get_all_at_box(&bb);

                let owner = self.owner_id.and_then(|id| world.get_entity_by_id(id));

                for cand in candidates {
                    let cand_ent = cand.get_entity();
                    if cand.is_spectator() || Some(cand_ent.entity_id) == self.owner_id {
                        continue;
                    }

                    if cand_ent.entity_id != entity.entity_id && cand.get_living_entity().is_some()
                    {
                        let damage_type = if owner.is_some() {
                            DamageType::INDIRECT_MAGIC
                        } else {
                            DamageType::MAGIC
                        };
                        let _ = cand.damage_with_context(
                            cand.as_ref(),
                            crate::entity::combat::get_evoker_fangs_damage(),
                            damage_type,
                            Some(entity.pos.load()),
                            Some(entity),
                            owner.as_deref(),
                        );
                    }
                }
            }

            if !self.sent_spike_event.swap(true, Ordering::SeqCst) {
                world.send_entity_status(entity, EntityStatus::StartAttacking, None);
                if !entity.silent.load(Ordering::Relaxed) {
                    let pitch = crate::entity::combat::calculate_evoker_fangs_sound_pitch(rand::random::<f32>());
                    world.play_sound_fine(
                        Sound::EntityEvokerFangsAttack,
                        pumpkin_data::sound::SoundCategory::Hostile,
                        &entity.pos.load(),
                        1.0,
                        pitch,
                    );
                }
            }

            let life = self.life_ticks.fetch_sub(1, Ordering::Relaxed) - 1;
            if life < 0 {
                entity.remove();
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
