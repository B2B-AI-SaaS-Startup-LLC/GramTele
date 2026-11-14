use super::{data_peer_bot_command::*, data_peer_id::UserId};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum BotStatus {
    NoBots = -1,
    #[default]
    Unknown = 0,
    HasBots = 2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotCommands {
    pub user_id: UserId,
    pub commands: Vec<BotCommand>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatBotCommands(pub BTreeMap<UserId, Vec<BotCommand>>);
impl ChatBotCommands {
    pub fn update(&mut self, list: &[BotCommands]) -> bool {
        if list.is_empty() {
            let changed = !self.0.is_empty();
            self.0.clear();
            return changed;
        }
        let mut changed = false;
        for c in list {
            if c.commands.is_empty() {
                changed |= self.0.remove(&c.user_id).is_some();
            } else {
                let value = self.0.entry(c.user_id).or_default();
                if *value != c.commands {
                    changed = true;
                    *value = c.commands.clone();
                }
            }
        }
        changed
    }
}
pub fn bot_commands_from_tl(value: &tg_scheme::BotInfo) -> BotCommands {
    let tg_scheme::BotInfo::BotInfo(d) = value;
    BotCommands {
        user_id: UserId::new(d.vuser_id().unwrap_or_default()),
        commands: d
            .vcommands()
            .map_or_else(Vec::new, |v| v.iter().map(bot_command_from_tl).collect()),
    }
}
