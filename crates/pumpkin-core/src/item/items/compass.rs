use std::any::Any;

use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{DataComponentImpl, LodestoneTarget, LodestoneTrackerImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

pub struct CompassItem;

impl CompassItem {
    pub const LODESTONE_COMPASS_TRANSLATION_KEY: &'static str = "item.minecraft.lodestone_compass";

    #[must_use]
    pub fn is_lodestone(block_id: pumpkin_data::BlockId) -> bool {
        block_id == pumpkin_data::BlockId::LODESTONE
    }

    #[must_use]
    pub fn is_compass(item_id: u16) -> bool {
        item_id == Item::COMPASS.id
    }

    #[must_use]
    pub fn is_recovery_compass(item_id: u16) -> bool {
        item_id == Item::RECOVERY_COMPASS.id
    }

    #[must_use]
    pub const fn should_replace_stack(creative: bool, count: u8) -> bool {
        !creative && count == 1
    }

    #[must_use]
    pub fn create_lodestone_tracker(dimension: &str, pos: &BlockPos) -> LodestoneTrackerImpl {
        LodestoneTrackerImpl {
            target: Some(LodestoneTarget {
                dimension: dimension.to_string(),
                x: pos.0.x,
                y: pos.0.y,
                z: pos.0.z,
            }),
            tracked: true,
        }
    }

    /// Vanilla LodestoneTracker.java:26-40:
    /// In the same dimension, verifies if the lodestone block still exists.
    /// If destroyed, resets target to None while keeping tracked: true.
    #[must_use]
    pub fn tick_tracker(
        tracker: &LodestoneTrackerImpl,
        current_dimension: &str,
        is_lodestone_at_pos: bool,
    ) -> LodestoneTrackerImpl {
        if tracker.tracked {
            if let Some(target) = &tracker.target {
                if target.dimension != current_dimension {
                    return tracker.clone();
                }
                if is_lodestone_at_pos {
                    return tracker.clone();
                }
                return LodestoneTrackerImpl {
                    target: None,
                    tracked: true,
                };
            }
        }
        tracker.clone()
    }
}

impl ItemMetadata for CompassItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::COMPASS.id, Item::RECOVERY_COMPASS.id])
    }
}

impl ItemBehaviour for CompassItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        if block.id == Block::LODESTONE.id && item.item.id == Item::COMPASS.id {
            let world = player.world();
            world.play_sound(
                Sound::ItemLodestoneCompassLock,
                SoundCategory::Players,
                &location.to_f64(),
            );

            let mut lodestone_compass = ItemStack::new(1, &Item::COMPASS);
            let tracker = LodestoneTrackerImpl {
                target: Some(LodestoneTarget {
                    dimension: world.dimension.minecraft_name.to_string(),
                    x: location.0.x,
                    y: location.0.y,
                    z: location.0.z,
                }),
                tracked: true,
            };
            lodestone_compass
                .patch
                .push((DataComponent::LodestoneTracker, Some(tracker.to_dyn())));
            lodestone_compass.patch.push((
                DataComponent::ItemName,
                Some(
                    pumpkin_data::data_component_impl::ItemNameImpl::Translation(
                        std::borrow::Cow::Borrowed("item.minecraft.lodestone_compass"),
                    )
                    .to_dyn(),
                ),
            ));

            let replace_existing =
                player.gamemode.load() != pumpkin_util::GameMode::Creative && item.item_count == 1;
            if replace_existing {
                *item = lodestone_compass;
            } else {
                item.decrement_unless_creative(player.gamemode.load(), 1);
                let was_added = player
                    .inventory
                    .insert_stack_anywhere(&mut lodestone_compass);
                if !was_added && !lodestone_compass.is_empty() {
                    world.drop_stack(&player.position().to_block_pos(), lodestone_compass);
                }
            }
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
