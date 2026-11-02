use std::sync::{
    Arc,
    Mutex as StdMutex,
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, Ordering},
};

use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::GameMode;
use pumpkin_util::PermissionLvl;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::text::TextComponent;

use crate::command::{CommandSender, CommandSource};
use crate::entity::Entity;
use crate::entity::player::Player;

/// Command Block Minecart implementation.
///
/// Matches vanilla `MinecartCommandBlock.java` and `BaseCommandBlock.java`:
/// - Cooldown: 4 ticks between activator rail triggers (`ACTIVATION_DELAY = 4`).
/// - Synchronized tracked data: `DATA_ID_COMMAND_NAME` (id 13) and `DATA_ID_LAST_OUTPUT` (id 14).
/// - Drops standard `Items.MINECART` on destruction (`MinecartCommandBlock.java:41-43`).
/// - Easter egg: "Searge" command outputs "#itzlipofutzli" with success count 1.
pub struct CommandBlockMinecart {
    command: StdMutex<String>,
    last_output: StdMutex<Option<TextComponent>>,
    track_output: AtomicBool,
    success_count: AtomicU32,
    update_last_execution: AtomicBool,
    last_execution: AtomicI64,
    last_activated: AtomicI32,
}

impl Default for CommandBlockMinecart {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBlockMinecart {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: StdMutex::new(String::new()),
            last_output: StdMutex::new(None),
            track_output: AtomicBool::new(true),
            success_count: AtomicU32::new(0),
            update_last_execution: AtomicBool::new(true),
            last_execution: AtomicI64::new(-1),
            last_activated: AtomicI32::new(0),
        }
    }

    pub fn init_data_tracker(&self, entity: &Entity) {
        let cmd = self.get_command();
        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_COMMAND_NAME,
            cmd,
        );
        let output = self
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| TextComponent::text(""));
        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_LAST_OUTPUT,
            output,
        );
    }

    #[must_use]
    pub fn get_command(&self) -> String {
        self.command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_command(&self, entity: &Entity, command: String) {
        *self
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = command.clone();
        // Vanilla: this.successCount = 0;
        self.success_count.store(0, Ordering::Relaxed);
        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_COMMAND_NAME,
            command,
        );
    }

    #[must_use]
    pub fn get_success_count(&self) -> u32 {
        self.success_count.load(Ordering::Relaxed)
    }

    pub fn set_success_count(&self, count: u32) {
        self.success_count.store(count, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_track_output(&self) -> bool {
        self.track_output.load(Ordering::Relaxed)
    }

    pub fn set_track_output(&self, entity: &Entity, track: bool) {
        self.track_output.store(track, Ordering::Relaxed);
        if !track {
            self.set_last_output(entity, None);
        }
    }

    #[must_use]
    pub fn get_last_output(&self) -> Option<TextComponent> {
        self.last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_last_output(&self, entity: &Entity, output: Option<TextComponent>) {
        *self
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = output.clone();
        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_LAST_OUTPUT,
            output.unwrap_or_else(|| TextComponent::text("")),
        );
    }

    /// Vanilla `MinecartCommandBlock.activateMinecart(level, xt, yt, zt, state)`:
    /// `if (state && this.tickCount - this.lastActivated >= 4)`
    pub fn activate(&self, entity: &Entity, state: bool, tick_count: i32) {
        if state
            && tick_count - self.last_activated.load(Ordering::Relaxed)
                >= crate::entity::combat::COMMAND_BLOCK_MINECART_ACTIVATION_DELAY
        {
            self.perform_command(entity);
            self.last_activated.store(tick_count, Ordering::Relaxed);
        }
    }

    /// Executes the command block minecart's command.
    ///
    /// Mirrors vanilla `BaseCommandBlock.performCommand(ServerLevel)`.
    pub fn perform_command(&self, entity: &Entity) -> bool {
        let world = entity.world.load();
        let game_time = world.level_time.try_lock().map_or(0, |t| t.world_age);
        if game_time == self.last_execution.load(Ordering::Relaxed) {
            return false;
        }

        let cmd = self.get_command();
        // Vanilla Searge easter egg
        if cmd.trim().eq_ignore_ascii_case("Searge") {
            self.set_last_output(entity, Some(TextComponent::text("#itzlipofutzli")));
            self.set_success_count(1);
            return true;
        }

        self.set_success_count(0);
        let command_blocks_work = world.level_info.load().game_rules.command_blocks_work;
        if command_blocks_work && !cmd.trim().is_empty() {
            if !self.track_output.load(Ordering::Relaxed) {
                self.set_last_output(entity, None);
            }
            if let Some(server) = world.server.upgrade() {
                let name = TextComponent::text("@");
                let source = CommandSource::new(
                    CommandSender::Dummy,
                    world.clone(),
                    None,
                    entity.pos.load(),
                    Vector2::new(entity.pitch.load(), entity.yaw.load()),
                    name.clone().get_text(),
                    name,
                    server.clone(),
                );
                server
                    .command_dispatcher
                    .load()
                    .handle_command(&source, &cmd);
                self.success_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        if self.update_last_execution.load(Ordering::Relaxed) {
            self.last_execution.store(game_time, Ordering::Relaxed);
        } else {
            self.last_execution.store(-1, Ordering::Relaxed);
        }
        true
    }

    /// Vanilla `MinecartCommandBlock.interact(player, hand, location)`:
    /// Only allows interaction if `player.canUseGameMasterBlocks()` (creative + op >= 2).
    #[must_use]
    pub fn interact(&self, _entity: &Entity, player: &Arc<Player>) -> bool {
        player.gamemode.load() == GameMode::Creative
            && player.permission_lvl.load() >= PermissionLvl::Two
    }

    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("Command", self.get_command());
        nbt.put_int("SuccessCount", self.get_success_count().cast_signed());
        nbt.put_bool("TrackOutput", self.is_track_output());
        if self.is_track_output() {
            if let Some(output) = self.get_last_output() {
                nbt.put_string("LastOutput", output.get_text());
            }
        }
        let update_last = self.update_last_execution.load(Ordering::Relaxed);
        nbt.put_bool("UpdateLastExecution", update_last);
        let last_exec = self.last_execution.load(Ordering::Relaxed);
        if update_last && last_exec != -1 {
            nbt.put_long("LastExecution", last_exec);
        }
    }

    pub fn read_nbt(&self, entity: &Entity, nbt: &NbtCompound) {
        let cmd = nbt.get_string("Command").unwrap_or("").to_string();
        let success = nbt.get_int("SuccessCount").unwrap_or(0).max(0) as u32;
        let track_output = nbt.get_bool("TrackOutput").unwrap_or(true);
        let last_output = if track_output {
            nbt.get_string("LastOutput")
                .map(|s| TextComponent::text(s.to_string()))
        } else {
            None
        };
        let update_last = nbt.get_bool("UpdateLastExecution").unwrap_or(true);
        let last_exec = if update_last {
            nbt.get_long("LastExecution").unwrap_or(-1)
        } else {
            -1
        };

        *self
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = cmd.clone();
        *self
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = last_output.clone();
        self.track_output.store(track_output, Ordering::Relaxed);
        self.success_count.store(success, Ordering::Relaxed);
        self.update_last_execution
            .store(update_last, Ordering::Relaxed);
        self.last_execution.store(last_exec, Ordering::Relaxed);

        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_COMMAND_NAME,
            cmd,
        );
        entity.set_synced_data(
            tracked_data::command_block_minecart::DATA_ID_LAST_OUTPUT,
            last_output.unwrap_or_else(|| TextComponent::text("")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_block_minecart_defaults() {
        let cart = CommandBlockMinecart::new();
        assert_eq!(cart.get_command(), "");
        assert_eq!(cart.get_success_count(), 0);
        assert!(cart.is_track_output());
        assert!(cart.get_last_output().is_none());
        assert_eq!(cart.last_execution.load(Ordering::Relaxed), -1);
        assert_eq!(cart.last_activated.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_command_block_minecart_nbt_roundtrip() {
        let cart = CommandBlockMinecart::new();
        *cart.command.lock().unwrap() = "say test_nbt".to_string();
        cart.success_count.store(3, Ordering::Relaxed);
        cart.track_output.store(false, Ordering::Relaxed);
        cart.last_execution.store(12345, Ordering::Relaxed);

        let mut nbt = NbtCompound::new();
        cart.write_nbt(&mut nbt);

        assert_eq!(nbt.get_string("Command"), Some("say test_nbt"));
        assert_eq!(nbt.get_int("SuccessCount"), Some(3));
        assert_eq!(nbt.get_bool("TrackOutput"), Some(false));
        assert_eq!(nbt.get_bool("UpdateLastExecution"), Some(true));
        assert_eq!(nbt.get_long("LastExecution"), Some(12345));
    }
}
