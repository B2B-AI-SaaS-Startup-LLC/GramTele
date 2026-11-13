# Типизированные ID: блок ядра после lang/countries

Оригинал 0f2a31bb420de98f331990432265b131b8d836b5. Прочитаны полные
data_peer_id.cpp/.h и data_msg_id.h; проверены вызовы в serialize_peer,
main_session_settings и использование диапазонов истории. Это значения и
форматы, без Session/PeerData/History-заглушек и без изменений области Claude.

| Оригинал | Rust | Проверка |
|---|---|---|
| BareId, ChatIdType, UserId/ChatId/ChannelId/FakeChatId | u64, ChatIdType<const SHIFT: u8>, aliases | native P cases, compile-fail UserId/ChatId и reserved SHIFT |
| ChatIdType constructors from BareId/MTPlong | new + From<u64/u32/i64/i32> | native P kind 1–4/6–10, включая sign extension i32 |
| ChatIdType compare, bool/!, hash | derived Eq/Ord/Hash, is_zero/Not | native values; typed-key tests; hash is Rust's container contract |
| PeerIdZero/PeerIdHelper | Default/new; zero comparison → is_zero/Not | zero-kind tests; compile-fail relational comparison with integer |
| PeerId constructors, type mask, is/to | new/from_chat_id; is/to<const SHIFT> | native P cases: all 256 raw tags, overflow bare bits, fake/invalid kinds |
| PeerId compare, bool/!, hash | derived Eq/Ord/Hash, is_zero/Not | native M full keys, zero-kind and session/peer key tests |
| peerIsUser/Chat/Channel | peer_is_user/chat/channel | native P cases, including PeerId(0) classified as User |
| peerFromUser/Chat/Channel overloads | generic Into<typed ID>; const construction via PeerId::from_chat_id | native P cases and signed MTPint conversions |
| peerToUser/Chat/Channel/BareMTPInt | peer_to_user/chat/channel/bare_mtp_int | native P cases and lower-48-bit truncation |
| peerFromMTP/peerToMTP | peer_from_mtp/peer_to_mtp with actual tg_scheme::Peer | native boxed TL input/output bytes, including fallback peerUser(0) |
| DeserializePeerId/SerializePeerId | deserialize_peer_id/serialize_peer_id | all legacy tags, marker bit 55, high-byte preservation, Qt_5_1 BE bytes |
| MsgId ctor/compare/bool/!/hash | MsgId new/From<i64>, Eq/Ord/Hash, is_zero/Not | native M cases; signed order and key tests |
| MsgId unary - and + / - | Neg/Add/Sub | native M safe-domain arithmetic; explicit panic for source signed-overflow inputs |
| MsgId prefix/postfix ++ / -- | increment/post_increment/decrement/post_decrement | native M returns and post-call state |
| StoryId/BusinessShortcutId and all constants | i32 aliases, source-named UPPER_SNAKE_CASE constants | native C record, boundary M cases |
| IsClientMsgId/ClientMsgIndex/ClientMsgByIndex | is_client_msg_id/client_msg_index/client_msg_by_index | native M/I cases, precondition tests |
| IsStoryMsgId/StoryIdFromMsgId/StoryIdToMsgId | is_story_msg_id/story_id_from_msg_id/story_id_to_msg_id | native M/I cases, including allowed index >= ServerMaxStoryId |
| IsServerMsgId | is_server_msg_id | native M boundaries, i64 MIN/MAX |
| FullStoryId::valid/bool/compare/hash | valid/Not, Eq/Ord/Hash | native M zero/negative story cases, typed keys |
| MsgRange construction/equality | MsgRange new/Default/Eq | native M comparisons |
| FullMsgId ctor/ChannelId deleted ctor/bool/!/compare/hash | FullMsgId new/is_zero/Not, Eq/Ord/Hash | native M, compile-fail ChannelId argument, per-peer HashMap keys |
| GlobalMsgId compare/bool/! | GlobalMsgId, is_zero/Not, Eq/Ord | native M, per-session HashMap keys |
| Qt metatype declarations/QDebug | Rust owned types and Debug | no Qt metatype runtime or exact Qt debug-text ABI claimed |

`original_strong_ids_serialization_tl_and_message_operations` compares
1792 operations with an executable built from unchanged original headers,
data_peer_id.cpp and unchanged generated Peer methods: 1236 P, 549 M,
6 index conversions and one constants record. No native integer-ID adapter
substitutes the actual C++ strong types. The extract script copies only
three TypeCreator methods and the contiguous generated Peer implementation;
algorithm/writer bodies are not rewritten. Fixtures and reproduction are in
tests/reference-tools/data-ids/README.md.

Rust aliases are distinct const-generic types; the transparent newtype retains
one u64, with constructor/type-query assertions for reserved SHIFT. Implicit
C++ bool/zero conversions become explicit is_zero/Not. Rust generics cannot
call Into in const evaluation, so const callers use from_chat_id directly.
Hash output/iteration order and the raw layout of aggregate Full* structs are
not disk/wire formats and are not claimed to match Qt/std C++ numeric hashes.

Checked MsgId arithmetic rejects signed overflow, which is undefined in the
original C++; native reference deliberately avoids invoking those operations
outside their defined domain. There is no added cap on story_id_to_msg_id:
the source checks only >=0, and the native out-of-range conversion is retained.

Deferred from data_msg_id.h: MessageHighlightId, FullReplyTo and SuggestOptions
(including price and text/quote/entity comparisons). They need the actual
TextWithEntities and CreditsAmount being completed by Claude; no substitute
types or partial ready-looking definitions are added. Binding their final
contracts belongs to the next integration of this header.

Ledger: data_peer_id.cpp/.h done; data_msg_id.h partial. Existing storage-list
raw i64/u64 adapters are not migrated in this slice; MTProto packet MsgId=u64
also remains a different concept from the signed client message ID here.
