//! Port of `lang/lang_hardcoded.h`: texts that are not translated.

pub fn faved_set_title() -> String {
    "Favorite stickers".into()
}

pub fn call_error_incompatible() -> String {
    "{user}'s app is using an incompatible protocol. They need to update their app before you can call them.".into()
}

pub fn server_error() -> String {
    "Internal server error.".into()
}

pub fn clear_path_failed() -> String {
    "Clear failed :(".into()
}

pub fn proxy_config_error() -> String {
    "The proxy you are using is not configured correctly and will be disabled. Please find another one.".into()
}

pub fn no_authorization_bot() -> String {
    "Could not get authorization bot.".into()
}

pub fn secure_save_error() -> String {
    "Error saving value.".into()
}

pub fn secure_accept_error() -> String {
    "Error accepting form.".into()
}

pub fn passport_corrupted() -> String {
    "It seems your Telegram Passport data was corrupted.\n\nYou can reset your Telegram Passport and restart this authorization.".into()
}

pub fn passport_corrupted_change() -> String {
    "It seems your Telegram Passport data was corrupted.\n\nYou can reset your Telegram Passport and change your cloud password.".into()
}

pub fn passport_corrupted_reset() -> String {
    "Reset".into()
}

pub fn passport_corrupted_reset_sure() -> String {
    "Are you sure you want to reset your Telegram Passport data?".into()
}

pub fn unknown_secure_scan_error() -> String {
    "Unknown scan read error.".into()
}

pub fn email_confirmation_expired() -> String {
    "This email confirmation has expired. Please setup two-step verification once again.".into()
}

pub fn autostart_enable_error() -> String {
    "Could not register for autostart.".into()
}
