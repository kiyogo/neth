use core::f32;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::entity::combat::{
    calculate_painting_bounding_box, calculate_painting_center_pos,
    calculate_painting_drop_position, PAINTING_SURVIVAL_CHECK_INTERVAL,
};
use crate::entity::decoration::hanging;
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use pumpkin_data::BlockDirection;
use pumpkin_data::damage::DamageType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::painting_variant::PaintingVariant;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::version::JavaMinecraftVersion;

/// The world stores a painting facing as a horizontal value: 0 south, 1 west,
/// 2 north, 3 east (`Direction.get2DDataValue`). The entity data, and the spawn
/// packet built from it, use the 3D index instead: north 2, south 3, west 4,
/// east 5. Sending the raw horizontal value makes the client read 0 as down,
/// fail the horizontal check in `HangingEntity.setDirection` and disconnect.
const fn facing_from_horizontal(value: u8) -> BlockDirection {
    match value & 3 {
        1 => BlockDirection::West,
        2 => BlockDirection::North,
        3 => BlockDirection::East,
        _ => BlockDirection::South,
    }
}

/// Vanilla reads the field with `getByte`, which yields 0 (south) when it is
/// absent, so a partial entity file keeps a horizontal default.
fn facing_from_nbt(nbt: &NbtCompound) -> BlockDirection {
    facing_from_horizontal(nbt.get_byte("facing").unwrap_or(0) as u8)
}

/// Converts a facing to the horizontal value the world file stores, the inverse of `facing_from_horizontal`.
const fn facing_to_horizontal(direction: BlockDirection) -> u8 {
    match direction {
        BlockDirection::West => 1,
        BlockDirection::North => 2,
        BlockDirection::East => 3,
        _ => 0,
    }
}

/// The variant entity data is an `ID or X`: 0 means an inline definition, so a registry ID is sent as ID + 1.
const fn variant_metadata_value(variant: PaintingVariant) -> VarInt {
    VarInt(variant.id() as i32 + 1)
}

pub struct PaintingEntity {
    pub entity: Entity,
    variant_id: AtomicU32,
    survival_check_ticks: AtomicU32,
}

impl PaintingEntity {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self::new_with_variant(entity, PaintingVariant::Alban)
    }

    #[must_use]
    pub const fn new_with_variant(entity: Entity, variant: PaintingVariant) -> Self {
        Self {
            entity,
            variant_id: AtomicU32::new(variant.id()),
            survival_check_ticks: AtomicU32::new(0),
        }
    }

    #[must_use]
    pub fn facing(&self) -> BlockDirection {
        BlockDirection::from_index(self.entity.data.load(Ordering::Relaxed) as u8)
            .unwrap_or(BlockDirection::South)
    }

    #[must_use]
    pub fn variant(&self) -> PaintingVariant {
        let id = self.variant_id.load(Ordering::Relaxed);
        PaintingVariant::all()
            .get(id as usize)
            .copied()
            .unwrap_or(PaintingVariant::Alban)
    }

    pub fn set_variant(&self, variant: PaintingVariant) {
        self.variant_id.store(variant.id(), Ordering::Relaxed);
        self.sync_variant();
        let facing = self.facing();
        let bbox = calculate_painting_bounding_box(
            self.entity.pos.load(),
            facing,
            variant.width(),
            variant.height(),
        );
        self.entity.bounding_box.store(bbox);
    }

    /// Vanilla `Painting.calculateBoundingBox`, around the already wall-shifted position.
    #[must_use]
    pub fn hanging_bounding_box(&self) -> BoundingBox {
        let facing = self.facing();
        let variant = self.variant();
        calculate_painting_bounding_box(
            self.entity.pos.load(),
            facing,
            variant.width(),
            variant.height(),
        )
    }

    /// Pushes the current variant into the tracked entity data so clients receive it.
    pub fn sync_variant(&self) {
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::painting::DATA_PAINTING_VARIANT_ID,
            variant_metadata_value(self.variant()),
        );
    }

    /// Calculates the exact floating-point center position for a painting of given width and height (in blocks),
    /// placed on `face` of `location`.
    #[must_use]
    pub fn calculate_center_pos(
        location: BlockPos,
        face: BlockDirection,
        width: u32,
        height: u32,
    ) -> Vector3<f64> {
        calculate_painting_center_pos(location, face, width, height)
    }

    /// Checks whether this painting survives in its current position on the wall.
    #[must_use]
    pub fn survives(&self) -> bool {
        let world = self.entity.world.load();
        let facing = self.facing();
        let location = self.entity.block_pos.load().offset(facing.opposite().to_offset());
        Self::painting_fits(&world, location, facing, self.variant())
            && !hanging::overlaps_hanging_entity(&world, &self.hanging_bounding_box(), self.entity.entity_id)
    }

    /// Checks if a painting of the specified variant fits on the wall at `location` facing `face`.
    /// The wall blocks behind the painting must be solid, and the blocks in front must not be solid.
    #[must_use]
    pub fn painting_fits(
        world: &crate::world::World,
        location: BlockPos,
        face: BlockDirection,
        variant: PaintingVariant,
    ) -> bool {
        let width = variant.width();
        let height = variant.height();
        let face_ccw = face.rotate_counter_clockwise();
        let ccw_offset = face_ccw.to_offset();
        let face_offset = face.to_offset();

        let k = -((width as i32 - 1) / 2);
        let l = -((height as i32 - 1) / 2);

        for x_idx in 0..width {
            for y_idx in 0..height {
                let wall_pos = BlockPos(Vector3::new(
                    location.0.x + ccw_offset.x * (x_idx as i32 + k),
                    location.0.y + (y_idx as i32 + l),
                    location.0.z + ccw_offset.z * (x_idx as i32 + k),
                ));
                let front_pos = wall_pos.offset(face_offset);

                let wall_state = world.get_block_state(&wall_pos);
                if !wall_state.is_solid() {
                    return false;
                }

                let front_state = world.get_block_state(&front_pos);
                if front_state.is_solid() {
                    return false;
                }
            }
        }
        true
    }

    /// Chooses a placeable painting variant that fits the wall at `location` facing `face`.
    /// Matches vanilla Minecraft: filters to variants that fit, selects the subset with
    /// the largest area (width * height), and randomly picks one from that subset.
    #[must_use]
    pub fn choose_variant(
        world: &crate::world::World,
        location: BlockPos,
        face: BlockDirection,
    ) -> Option<PaintingVariant> {
        use rand::seq::IndexedRandom;

        let mut fitting_variants = Vec::new();
        let mut max_area = 0;

        for &variant in PaintingVariant::all_placeable() {
            if Self::painting_fits(world, location, face, variant) {
                let area = variant.width() * variant.height();
                if area > max_area {
                    max_area = area;
                    fitting_variants.clear();
                    fitting_variants.push(variant);
                } else if area == max_area {
                    fitting_variants.push(variant);
                }
            }
        }

        let mut rng = rand::rng();
        fitting_variants.choose(&mut rng).copied()
    }

    /// Checks support and drops if invalid.
    pub fn check_support(&self) {
        if self.entity.is_removed() {
            return;
        }
        if !self.survives() {
            self.drop_and_remove(None);
        }
    }

    fn drop_and_remove(&self, caused_by: Option<&dyn EntityBase>) {
        let entity = &self.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        let facing = self.facing();
        let drop_pos = calculate_painting_drop_position(pos, facing);

        world.play_sound(
            Sound::EntityPaintingBreak,
            SoundCategory::Blocks,
            &pos,
        );
        world.emit_game_event(
            GameEvent::BlockChange.name(),
            pos,
        );

        let is_creative = caused_by.is_some_and(|c| {
            c.cast_any()
                .downcast_ref::<Player>()
                .is_some_and(Player::is_creative)
        });

        if !is_creative {
            let mut stack = ItemStack::new(1, &Item::PAINTING);
            if let Some(custom_name) = entity.custom_name.load().as_ref().clone() {
                stack.set_custom_name(custom_name.to_pretty_console());
            }
            world.drop_stack_at(drop_pos, stack);
        }

        entity.remove();
    }
}

impl EntityBase for PaintingEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let index = self.entity.data.load(Ordering::Relaxed) as u8;
        let direction = BlockDirection::from_index(index).unwrap_or(BlockDirection::South);
        nbt.put_byte("facing", facing_to_horizontal(direction) as i8);
        nbt.put_string("variant", self.variant().asset_id().to_string());
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        let facing = facing_from_nbt(nbt);
        self.entity
            .data
            .store(i32::from(facing.to_index()), Ordering::Relaxed);
        if let Some(variant_str) = nbt
            .get_string("variant")
            .or_else(|| nbt.get_string("Motive"))
            && let Some(variant) = PaintingVariant::from_name(variant_str)
        {
            self.set_variant(variant);
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn init_data_tracker(&self) {
        self.sync_variant();
    }

    /// Metadata sent right after the spawn packet, carrying the variant to the client.
    fn java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        let mut metadata = Vec::new();
        Metadata::new(
            pumpkin_data::tracked_data::painting::DATA_PAINTING_VARIANT_ID,
            variant_metadata_value(self.variant()),
        )
        .write(&mut metadata, &version)
        .ok()?;
        metadata.push(255);
        Some(metadata.into_boxed_slice())
    }

    fn set_variant_name(&self, name: &str) {
        if let Some(variant) = PaintingVariant::from_name(name) {
            self.set_variant(variant);
        }
    }

    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.drop_and_remove(Some(caller));
        true
    }

    fn can_hit(&self) -> bool {
        self.entity.is_alive()
    }

    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        self.entity.tick(caller, server);

        let ticks = self.survival_check_ticks.fetch_add(1, Ordering::Relaxed) + 1;
        if ticks >= PAINTING_SURVIVAL_CHECK_INTERVAL {
            self.survival_check_ticks.store(0, Ordering::Relaxed);
            self.check_support();
        }
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_and_3d_facings_round_trip() {
        for horizontal in 0u8..4 {
            let direction = facing_from_horizontal(horizontal);
            assert!(direction.is_horizontal(), "{direction:?} is vertical");
            assert_eq!(facing_to_horizontal(direction), horizontal);
        }
    }

    #[test]
    fn south_two_is_not_down_zero() {
        // The old bug: south (0 in the world file) was sent as index 0, which the
        // client reads as down and rejects.
        assert_eq!(facing_from_horizontal(0), BlockDirection::South);
        assert_eq!(facing_from_horizontal(0).to_index(), 3);
    }

    #[test]
    fn missing_facing_defaults_to_south() {
        let nbt = NbtCompound::new();
        assert_eq!(facing_from_nbt(&nbt), BlockDirection::South);
        assert_eq!(facing_from_nbt(&nbt).to_index(), 3);

        let mut with_facing = NbtCompound::new();
        with_facing.put_byte("facing", 1);
        assert_eq!(facing_from_nbt(&with_facing), BlockDirection::West);
    }

    #[test]
    fn out_of_range_values_wrap_into_horizontal_directions() {
        assert!(facing_from_horizontal(4).is_horizontal());
        assert!(facing_from_horizontal(255).is_horizontal());
    }

    #[test]
    fn calculate_center_pos_works() {
        let location = BlockPos(Vector3::new(10, 64, 20));

        // 1x1 painting facing North: placed against block at (10, 64, 20)
        let pos_1x1 = PaintingEntity::calculate_center_pos(location, BlockDirection::North, 1, 1);
        assert_eq!(pos_1x1.x, 10.5);
        assert_eq!(pos_1x1.y, 64.5);
        assert!((pos_1x1.z - 19.96875).abs() < 1e-6);

        // 2x1 painting facing North: width is 2, shifts along counter-clockwise (West = -X)
        let pos_2x1 = PaintingEntity::calculate_center_pos(location, BlockDirection::North, 2, 1);
        assert_eq!(pos_2x1.x, 10.0);
        assert_eq!(pos_2x1.y, 64.5);
        assert!((pos_2x1.z - 19.96875).abs() < 1e-6);

        // 2x2 painting facing North: height is 2, shifts +0.5 along Y
        let pos_2x2 = PaintingEntity::calculate_center_pos(location, BlockDirection::North, 2, 2);
        assert_eq!(pos_2x2.x, 10.0);
        assert_eq!(pos_2x2.y, 65.0);
        assert!((pos_2x2.z - 19.96875).abs() < 1e-6);

        // 4x4 painting facing North
        let pos_4x4 = PaintingEntity::calculate_center_pos(location, BlockDirection::North, 4, 4);
        assert_eq!(pos_4x4.x, 10.0);
        assert_eq!(pos_4x4.y, 65.0);
        assert!((pos_4x4.z - 19.96875).abs() < 1e-6);

        // 1x1 painting facing South: placed on south face of (10, 64, 20) -> target is (10, 64, 21)
        let pos_south = PaintingEntity::calculate_center_pos(location, BlockDirection::South, 1, 1);
        assert_eq!(pos_south.x, 10.5);
        assert_eq!(pos_south.y, 64.5);
        assert!((pos_south.z - 21.03125).abs() < 1e-6);
    }

    /// Names resolve to a variant with or without the `minecraft:` prefix.
    #[test]
    fn variant_resolution() {
        assert_eq!(
            PaintingVariant::from_name("minecraft:kebab"),
            Some(PaintingVariant::Kebab)
        );
        assert_eq!(
            PaintingVariant::from_name("kebab"),
            Some(PaintingVariant::Kebab)
        );
        assert_eq!(PaintingVariant::from_name("invalid"), None);
        assert_eq!(PaintingVariant::Kebab.asset_id(), "minecraft:kebab");
    }

    /// 0 tells the client that an inline variant definition follows, so the first variant is sent as 1.
    #[test]
    fn first_variant_is_not_sent_as_inline() {
        assert_eq!(variant_metadata_value(PaintingVariant::Alban).0, 1);
    }
}
