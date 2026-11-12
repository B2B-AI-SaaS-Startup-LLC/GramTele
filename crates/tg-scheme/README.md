# Telegram TL, layer 229

`tg-scheme` generates Telegram API and MTProto types at build time. Inputs
`scheme/api.tl` and `scheme/mtproto.tl` are copied from AyuGram Desktop
`0f2a31bb420de98f331990432265b131b8d836b5`. Options from
`SourceFiles/codegen/scheme/codegen_scheme.py` live in `codegen_scheme.rs`.
Changing either input or the options reruns generation.

The generator is `tg-tl-gen`; serialization primitives are in `tg-tl`.
No Python or Qt is needed for generation or serialization.

| C++ | Rust |
|---|---|
| `MTPInputPeer` | `InputPeer` |
| `MTPDinputPeerUser` | `InputPeerUserData` |
| `MTP_inputPeerUser(id, hash)` | `mtp_input_peer_user(id, hash)` |
| `mtpc_inputPeerUser` | `InputPeerUserData::ID` |
| `peer.c_inputPeerUser()` | `peer.c_input_peer_user()` |
| `data.vuser_id()` | `data.vuser_id()` |
| `MTPmessages_GetHistory(...)` | `MessagesGetHistory::new(...)` |
| `ResponseType` | `tl::Function::Response` |
| `MTPBool`, `MTP_boolTrue()` | `bool`, `true` |
| `MTPstring`, `MTPbytes` | `Vec<u8>` |
| `MTPVector<T>` | `Vec<T>` |
| bare `vector<T>` | `tl::BareVec<T>` |
| `Flag::f_name` | `Data::F_NAME` |
| `flags` and `flags2` | typed `tl::Flags<Data>` with a `u64` value |
| `tl::details::type_owner` | `Arc<Data>` |

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
