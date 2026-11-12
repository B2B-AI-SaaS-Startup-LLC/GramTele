# Telegram TL, layer 229

`tg-scheme` generates Telegram API and MTProto types at build time. Inputs
`scheme/api.tl` and `scheme/mtproto.tl` are copied from AyuGram Desktop
`0f2a31bb420de98f331990432265b131b8d836b5`. Options from
`SourceFiles/codegen/scheme/codegen_scheme.py` live in `codegen_scheme.rs`.
Changing either input or the options reruns generation.

The generator is `tg-tl-gen`; serialization primitives are in `tg-tl`.
No Python or Qt is needed for generation or serialization.

The examples below are adaptations of actual client expressions. Paths and
line numbers refer to `Telegram/SourceFiles` at the source revision above;
variables such as `data`, `peer`, and `history` have the corresponding TL type.
Import `tg_scheme::*` and `tg_scheme::tl::Function` for Rust examples.

| C++ expression | Rust equivalent | Client source |
|---|---|---|
| `MTPInputPeer` | `InputPeer` | `data/data_peer.cpp:2074` |
| `MTP_inputPeerUser(data.vuser_id(), data.vaccess_hash())` | `mtp_input_peer_user(data.vuser_id(), data.vaccess_hash())` | `data/data_peer.cpp:2074` |
| `MTP_inputPeerUserFromMessage(...)` | `mtp_input_peer_user_from_message(peer, msg_id, user_id)` | `data/data_peer.cpp:2076` |
| `MTP_int(0)` | `0i32` | `data/data_histories.cpp:566` |
| `MTP_int(2)` (limit) | `2i32` | `data/data_histories.cpp:569` |
| `MTP_long(0)` (hash) | `0u64` | `data/data_histories.cpp:572` |
| `MTPmessages_GetHistory(peer, 0, 0, 0, 2, 0, 0, 0)` | `MessagesGetHistory::new(peer, 0, 0, 0, 2, 0, 0, 0)` | `data/data_histories.cpp:564–572` |
| `const MTPmessages_Messages &result` | `&<MessagesGetHistory as Function>::Response` (alias `MessagesMessages`) | `data/data_histories.cpp:573` |
| `api().request(request).done(...).send()` | Construct and serialize the `Function`; sending/receiving awaits the MTProto request dispatcher (stage 2) | `data/data_histories.cpp:564–584` |
| `MTP_messages_messages(messages, topics, chats, users)` | `mtp_messages_messages(messages, topics, chats, users)` | `data/data_histories.cpp:579` |
| `MTP_vector<MTPMessage>(0)` | `Vec::<Message>::new()` | `data/data_histories.cpp:580` |
| `MTP_vector<MTPForumTopic>(0)` | `Vec::<ForumTopic>::new()` | `data/data_histories.cpp:581` |
| `MTP_vector<MTPChat>(0)` | `Vec::<Chat>::new()` | `data/data_histories.cpp:582` |
| `MTP_vector<MTPUser>(0)` | `Vec::<User>::new()` | `data/data_histories.cpp:583` |
| `MTP_string(_query)` | `query.as_bytes().to_vec()` | `api/api_peer_search.cpp:71` |
| `MTPcontacts_GetSponsoredPeers(MTP_string(_query))` | `ContactsGetSponsoredPeers::new(query.as_bytes().to_vec())` | `api/api_peer_search.cpp:97` |
| `result.match(empty_lambda, peers_lambda)` | `match result { ContactsSponsoredPeers::ContactsSponsoredPeersEmpty(_) => {}, ContactsSponsoredPeers::ContactsSponsoredPeers(data) => { /* data */ } }` | `api/api_peer_search.cpp:106` |
| `data.vusers()` | `data.vusers()` (`&Vec<User>`) | `api/api_peer_search.cpp:109` |
| `data.vchats()` | `data.vchats()` (`&Vec<Chat>`) | `api/api_peer_search.cpp:110` |
| `status.match(...)` | `match status { UserStatus::UserStatusOnline(data) => { /* data.vexpires() */ }, _ => {} }` | `data/data_user.cpp:122–141` |
| `data.vexpires().v` | `data.vexpires()` (`i32`) | `data/data_user.cpp:136` |
| `qs(data.vfirst_name().value_or_empty())` | `String::from_utf8_lossy(data.vfirst_name().map_or(&[], Vec::as_slice))` | `data/data_session.cpp:825` |
| `qs(data.vusername().value_or_empty())` | `String::from_utf8_lossy(data.vusername().map_or(&[], Vec::as_slice))` | `data/data_session.cpp:838` |
| `data.is_self()` | `data.is_self()` (flag without payload) | `data/data_session.cpp:850` |
| `if (const auto photo = data.vphoto())` | `if let Some(photo) = data.vphoto() { /* photo */ }` | `data/data_session.cpp:877` |
| `MTP_bytes(replyTo.pollOption)` | `reply_to.poll_option.clone()` (`Vec<u8>`, no text conversion) | `data/data_histories.cpp:121` |
| `MTP_string(url)` | `url.as_bytes().to_vec()` | `data/data_histories.cpp:141` |

This table covers construction, enum dispatch, required/conditional fields,
flags, vectors, text versus arbitrary bytes, methods and their response types.
It does not imply that the client request dispatcher has already been ported.

Fields are public. Constructor getters panic on the wrong variant, like the
original's precondition. Use Rust `match` for visitors. Conditional getters
return `Option`; flag bits control serialization even if the stored optional
value differs. A set bit with no payload writes its default value.
`Debug` derives are enabled with feature `debug`.

AyuGram patches are preserved: `is_noforwards()` and `is_restricted()` return
false for patched constructors; `is_ayu_noforwards()` and
`is_ayu_restricted()` expose the actual bits.

## Validation

- `reference_bytes.rs`: hand-written expected wire bytes for primitives,
  string lengths around 253/254, vectors, nested constructors, flags,
  `flags2`, generic requests, errors and AyuGram hooks.
- `scheme_ids.rs`: CRCs (including upstream exceptions), definition counts,
  uniqueness and C++ name mappings across both complete schemas.
- `all_definitions.rs`: wire samples built from metadata for every constructor
  and method, with optional fields absent and present. These complement the
  independent byte fixtures; metadata and generated code share the parser.
- `dump_to_text.rs`: expected text from the original generator's stage machine,
  indentation, conditional fields and decoding failures.

## Scope and differences

The read/write generator and schema-specific text dumper are implemented.
The external API conversion backend of `generate_tl.py` is not ported: the
AyuGram scheme invocation does not use it. Text dumping delegates primitives,
vector traversal, layer wrappers and gzip to the `DumpCore` interface; the
MTProto implementation remains for stage 2.

`tl::count_length` serializes to a temporary buffer instead of using a counting
accumulator. Strings remain arbitrary bytes; text conversion uses UTF-8 with
replacement for invalid sequences. Invalid reads return `Result` instead of
`false`, with a partially advanced cursor permitted as in C++.

Defaults select the first constructor; C++ can represent an uninitialized
multi-constructor value. Single dataless constructors preserve the original
quirk of accepting an arbitrary constructor ID when reading. Unsupported
schema constructs that would produce invalid read/write C++ are rejected
explicitly; see `tg-tl-gen/src/generate_tl.rs` for the list.

## Clean build measurements

Measured in the Nix shell on 2026-11-12, x86_64 Linux, rustc 1.95.0
(`59807616e`, LLVM 21.1.8), cargo 1.95.0. Each command used a different,
initially absent target directory; Cargo's download cache was already warm.
No other compilation ran concurrently. Generation and all dependencies of
`tg-scheme` are included; these are wall times, not incremental rebuild times.

| Command | Wall time | User / system CPU |
|---|---:|---:|
| `cargo build -p tg-scheme --target-dir /tmp/ayugram-tl-clean-debug-20261112` | 82.927 s | 133.074 / 6.612 s |
| `cargo build -p tg-scheme --release --target-dir /tmp/ayugram-tl-clean-release-20261112` | 194.744 s | 278.217 / 4.723 s |

Both commands completed successfully. The target directories were isolated
from the workspace build; repeat on the target machine for capacity planning.
