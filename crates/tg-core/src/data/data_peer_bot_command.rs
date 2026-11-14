//! Real BotCommand value, independent of bot/user owners.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
    pub ephemeral: bool,
}
pub fn bot_command_from_tl(value: &tg_scheme::BotCommand) -> BotCommand {
    let tg_scheme::BotCommand::BotCommand(d) = value;
    BotCommand {
        command: String::from_utf8_lossy(&d.command).into(),
        description: String::from_utf8_lossy(&d.description).into(),
        ephemeral: d.is_ephemeral(),
    }
}
