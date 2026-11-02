#[allow(clippy::wildcard_imports)]
use super::*;
use pumpkin_protocol::java::server::play::SSetCommandMinecart;
use pumpkin_util::GameMode;
use pumpkin_util::text::TextComponent;

use crate::entity::vehicle::minecart::MinecartEntity;

impl JavaClient {
    pub fn handle_set_command_minecart(&self, player: &Player, packet: &SSetCommandMinecart<'_>) {
        // Vanilla parity: ServerGamePacketListenerImpl.java:685-710
        // Requires creative mode and permission level >= 2 (canUseGameMasterBlocks).
        if player.permission_lvl.load() < PermissionLvl::Two
            || player.gamemode.load() != GameMode::Creative
        {
            player.send_system_message(&TextComponent::translate(
                pumpkin_data::translation::java::ADVMODE_NOTALLOWED,
                [],
            ));
            return;
        }

        let world = player.world();
        let Some(entity_base) = world.get_entity_by_id(packet.entity_id.0) else {
            return;
        };
        let Some(minecart) = entity_base.cast_any().downcast_ref::<MinecartEntity>() else {
            return;
        };
        let Some(cmd_cart) = minecart.command_block() else {
            return;
        };

        cmd_cart.set_command(minecart.get_entity(), packet.command.to_string());
        cmd_cart.set_track_output(minecart.get_entity(), packet.track_output);
        if !packet.track_output {
            cmd_cart.set_last_output(minecart.get_entity(), None);
        }

        let command_blocks_enabled = world.level_info.load().game_rules.command_blocks_work;
        if !packet.command.is_empty() {
            let key = if command_blocks_enabled {
                pumpkin_data::translation::java::ADVMODE_SETCOMMAND_SUCCESS
            } else {
                pumpkin_data::translation::java::ADVMODE_SETCOMMAND_DISABLED
            };
            player.send_system_message(&TextComponent::translate(
                key,
                [TextComponent::text(packet.command.to_string())],
            ));
        }

        debug!(
            "Player {} updated command minecart {} command to: {}",
            player.gameprofile.name,
            minecart.get_entity().entity_id,
            packet.command
        );
    }
}
