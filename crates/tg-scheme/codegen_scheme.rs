// Port of `Telegram/SourceFiles/codegen/scheme/codegen_scheme.py`: the
// scheme options passed to the generator. Included by `build.rs` and by
// the tests.

use tg_tl_gen::Config;

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

/// The `generate({...})` dictionary of codegen_scheme.py (the keys that
/// matter for the read-write section; `namespaces`, `prefixes`, `types`,
/// `sections`, `builtinInclude`, `optimizeSingleData` and `dumpToText` are
/// expressed by the shape of the generated Rust code).
pub fn config() -> Config {
    Config {
        // define some checked flag conversions
        // the key flag type should be a subset of the value flag type
        // with exact the same names, then the key flag can be implicitly
        // casted to the value flag type
        flag_inheritance: pairs(&[
            ("messageService", "message"),
            ("updateShortMessage", "message"),
            ("updateShortChatMessage", "message"),
            ("updateShortSentMessage", "message"),
            ("replyKeyboardHide", "replyKeyboardMarkup"),
            ("replyKeyboardForceReply", "replyKeyboardMarkup"),
            ("inputPeerNotifySettings", "peerNotifySettings"),
            ("peerNotifySettings", "inputPeerNotifySettings"),
            ("channelForbidden", "channel"),
            ("dialogFolder", "dialog"),
            ("dialogCommunity", "dialog"),
        ]),
        type_id_exceptions: strings(&[
            "channel#c88974ac",
            "ipPortSecret#37982646",
            "accessPointRule#4679b65f",
            "help.configSimple#5a592a6c",
            "messageReplies#81834865",
        ]),
        renamed_types: pairs(&[(
            "passwordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow",
            "passwordKdfAlgoModPow",
        )]),
        skip: strings(&[
            "int ? = Int;",
            "long ? = Long;",
            "double ? = Double;",
            "string ? = String;",
            "vector {t:Type} # [ t ] = Vector t;",
            "int128 4*[ int ] = Int128;",
            "int256 8*[ int ] = Int256;",
            "vector#1cb5c415 {t:Type} # [ t ] = Vector t;",
        ]),
        builtin: strings(&["int", "long", "double", "string", "bytes", "int128", "int256"]),
        builtin_templates: strings(&["vector", "flags"]),
        synonyms: pairs(&[("bytes", "string")]),
    }
}

