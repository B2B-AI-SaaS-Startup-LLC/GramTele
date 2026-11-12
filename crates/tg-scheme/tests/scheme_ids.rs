//! Checks the scheme as a whole: constructor ids against the CRC of their
//! lines, completeness of generation, and the C++ → Rust name mapping.

use std::collections::{HashMap, HashSet};

use tg_tl_gen::generate_tl::{clean_line, line_crc};
use tg_tl_gen::{Input, Names};

include!("../codegen_scheme.rs");

fn inputs() -> Vec<Input> {
    ["api.tl", "mtproto.tl"]
        .iter()
        .map(|name| Input {
            name: name.to_string(),
            text: std::fs::read_to_string(format!("{}/scheme/{name}", env!("CARGO_MANIFEST_DIR")))
                .unwrap(),
        })
        .collect()
}

/// Definition lines read with a simple independent scan: not a comment,
/// not blank, not a separator, not in the skip list.
fn definition_lines() -> Vec<String> {
    let skip = config().skip;
    let mut result = Vec::new();
    for input in inputs() {
        for line in input.text.lines() {
            let line = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            let line = line.trim();
            if line.is_empty() || line.starts_with("---") || skip.iter().any(|s| s == line) {
                continue;
            }
            result.push(line.to_string());
        }
    }
    result
}

#[test]
fn every_explicit_id_matches_its_crc() {
    let exceptions = config().type_id_exceptions;
    let mut checked = 0;
    let mut used_exceptions = Vec::new();
    for line in definition_lines() {
        let (head, result) = line.split_once('=').unwrap();
        let result = result.trim().trim_end_matches(';').trim();
        let name_end = head.find([' ', '#']).unwrap_or(head.len());
        let name = &head[..name_end];
        let (id, params) = match head[name_end..].strip_prefix('#') {
            Some(rest) => {
                let end = rest.find(' ').unwrap_or(rest.len());
                (Some(&rest[..end]), &rest[end..])
            }
            None => (None, &head[name_end..]),
        };
        let Some(id) = id else { continue };
        let crc = line_crc(&clean_line(name, params, result, &config().synonyms));
        let given = u32::from_str_radix(id, 16).unwrap();
        if crc != given {
            let key = format!("{name}#{}", id.trim_start_matches('0'));
            assert!(
                exceptions.contains(&key),
                "id mismatch: {line} (crc {crc:08x})"
            );
            used_exceptions.push(key);
        } else {
            checked += 1;
        }
    }
    assert!(checked > 2000, "{checked}");
    // The ids of these definitions differ from the CRC on purpose
    // (typeIdExceptions in codegen_scheme.py).
    used_exceptions.sort();
    assert_eq!(
        used_exceptions,
        [
            "accessPointRule#4679b65f",
            "help.configSimple#5a592a6c",
            "ipPortSecret#37982646"
        ]
    );
}

#[test]
fn every_definition_is_generated() {
    let lines = definition_lines();
    let schema = tg_tl_gen::parse(&inputs(), &config()).unwrap();
    assert!(schema.warnings.is_empty(), "{:?}", schema.warnings);
    assert!(schema.skipped_lines.is_empty());
    assert_eq!(schema.layer, 229);
    assert_eq!(schema.definition_lines, lines.len());
    assert_eq!(
        schema.constructors.len() + schema.functions.len(),
        lines.len()
    );
    assert_eq!(
        tg_scheme::meta::CONSTRUCTORS.len(),
        schema.constructors.len()
    );
    assert_eq!(tg_scheme::meta::FUNCTIONS.len(), schema.functions.len());
    assert_eq!(tg_scheme::meta::TYPES.len(), schema.types.len());
    assert_eq!(tg_scheme::CURRENT_LAYER, 229);
    for (def, meta) in schema
        .constructors
        .iter()
        .zip(tg_scheme::meta::CONSTRUCTORS)
    {
        assert_eq!(def.id, meta.id);
        assert_eq!(def.name, meta.name);
    }
    // Ids are unique.
    let mut ids = HashSet::new();
    for def in schema.constructors.iter().chain(&schema.functions) {
        assert!(ids.insert(def.id), "duplicate id {:08x}", def.id);
    }
    eprintln!(
        "lines {}, constructors {}, methods {}, types {}",
        lines.len(),
        schema.constructors.len(),
        schema.functions.len(),
        schema.types.len()
    );
}

#[test]
fn name_mapping_is_unambiguous() {
    let schema = tg_tl_gen::parse(&inputs(), &config()).unwrap();
    let names = Names::new(&schema);
    let mappings = names.mappings();
    let mut by_cpp: HashMap<&str, &str> = HashMap::new();
    for m in &mappings {
        if let Some(other) = by_cpp.insert(&m.cpp, &m.rust) {
            // The same C++ name may only appear twice with the same target.
            assert_eq!(other, m.rust, "{}", m.cpp);
        }
    }
    let expect = [
        ("MTPDuser", "UserData"),
        ("MTPUser", "User"),
        ("MTPuser", "User"),
        ("mtpc_user", "UserData::ID"),
        ("MTP_user", "mtp_user"),
        ("MTPUser::c_user", "User::c_user"),
        ("MTPInputPeer", "InputPeer"),
        ("MTP_inputPeerUser", "mtp_input_peer_user"),
        ("MTPmessages_GetHistory", "MessagesGetHistory"),
        ("MTPmessages_getHistory", "MessagesGetHistory"),
        ("mtpc_messages_getHistory", "MessagesGetHistory::ID"),
        ("MTPmessages_Messages", "MessagesMessages"),
        (
            "MTPDmessages_channelMessages",
            "MessagesChannelMessagesData",
        ),
        (
            "MTPmessages_Messages::c_messages_channelMessages",
            "MessagesMessages::c_messages_channel_messages",
        ),
        ("MTPDpasswordKdfAlgoModPow", "PasswordKdfAlgoModPowData"),
        ("MTPDserver_DH_params_ok", "ServerDHParamsOkData"),
        ("MTPRpc_drop_answer", "RpcDropAnswerMethod"),
        ("MTPBool", "bool"),
        ("mtpc_boolTrue", "tl::ID_BOOL_TRUE"),
        ("MTP_boolFalse", "false"),
        ("MTPInvokeWithLayer", "InvokeWithLayer"),
    ];
    for (cpp, rust) in expect {
        assert_eq!(by_cpp.get(cpp).copied(), Some(rust), "{cpp}");
    }
    // Every Rust type / data name is produced by exactly one C++ entity.
    tg_tl_gen::rust::check_names(&schema).unwrap();
}
