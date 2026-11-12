use std::sync::Arc;
use tg_mtproto::{
    connection_resolving::{Action, ResolvingConnection},
    core_types::{Random, Time},
    details::{
        mtproto_dc_key_binder::{DcKeyBindState, DcKeyBinder},
        mtproto_dcenter::{CreatingKeyType, Dcenter, TemporaryKeyType},
        mtproto_domain_resolver::*,
        mtproto_serialized_request::SerializedRequest,
    },
    mtproto_auth_key::{AuthKey, KeyType},
    mtproto_config::Config,
    mtproto_dc_options::*,
    mtproto_proxy_data::{ProxyData, Type},
};
struct Entropy(u8);
impl Random for Entropy {
    fn fill(&mut self, data: &mut [u8]) {
        for b in data {
            *b = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}
#[test]
fn serialized_request_header_and_padding() {
    assert!(tg_tl::serialize(&SerializedRequest::default()).is_empty());
    let mut request = SerializedRequest::serialize(&tg_scheme::Ping::new(0x8877665544332211), 77);
    assert_eq!(request.words[7], 12);
    assert_eq!(request.last_sent_time, 77);
    request.set_msg_id(0xaabbccddeeff0010);
    request.set_seq_no(3);
    assert_eq!(request.get_msg_id(), 0xaabbccddeeff0010);
    assert_eq!(request.get_seq_no(), 3);
    assert!(request.need_ack());
    let body = request.body().to_vec();
    request.add_padding(false, &mut Entropy(15));
    assert_eq!(request.words.len() % 4, 0);
    assert!(request.words.len() * 4 >= (8 + body.len()) * 4 + 12);
    assert_eq!(request.body(), body);
    let len = request.words.len();
    request.add_padding(true, &mut Entropy(15));
    assert!(request.words.len() < len);
    assert_eq!(request.words.len() % 4, 0);
    assert_eq!(tg_tl::serialize(&request), body);
    for id in [
        0x73f1f8dc_u32,
        0x62d6b459,
        0x9299359f,
        0xa7eff811,
        0x8cc0d131,
        0x04deb57d,
        0x276d3ec6,
        0x809db6df,
    ] {
        request.words[8] = id as i32;
        assert!(!request.need_ack());
    }
}
#[test]
fn binding_request_decrypts_to_original_inner_layout() {
    use tg_tl::Deserialize;
    let persistent = Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1));
    let mut temp = AuthKey::new(KeyType::Temporary, 2, [4; 256], 2);
    temp.set_expires_at(1700100000);
    let mut bound = tg_mtproto::details::mtproto_bound_key_creator::BoundKeyCreator::new(
        tg_mtproto::details::mtproto_dc_key_creator::DcKeyRequest {
            temporary_expires_in: 86400,
            persistent_needed: false,
        },
    );
    let first = bound.start(2, 10002, &mut Entropy(1)).unwrap();
    assert!(tg_tl::deserialize_exact::<tg_scheme::ReqPqMulti>(&first).is_ok());
    bound.stop();
    assert!(bound.creator.is_none());
    bound.bind(persistent.clone());
    assert!(bound.ready_to_bind());
    bound.restart_binder();
    assert!(Arc::ptr_eq(&bound.bind_persistent_key(), &persistent));
    let binder = DcKeyBinder::new(persistent.clone());
    let request = binder.prepare_request(
        &temp,
        0x1234567812345678,
        0x1000000000000004,
        44,
        &mut Entropy(1),
    );
    assert_eq!(request.get_msg_id(), 0x1000000000000004);
    let body = tg_tl::deserialize_exact::<tg_scheme::AuthBindTempAuthKey>(request.body()).unwrap();
    assert_eq!(body.perm_auth_key_id, persistent.key_id());
    let encrypted = body.encrypted_message;
    assert_eq!(
        u64::from_le_bytes(encrypted[..8].try_into().unwrap()),
        persistent.key_id()
    );
    let msg = encrypted[8..24].try_into().unwrap();
    let (k, iv) = persistent.prepare_aes_oldmtp(&msg, true);
    let mut bytes = encrypted[24..].to_vec();
    tg_mtproto::mtproto_auth_key::aes_ige_decrypt_raw(&mut bytes, &k, &iv).unwrap();
    let words = tg_tl::bytes_to_primes(&bytes);
    assert_eq!(words[6], 0);
    let size = words[7] as usize / 4;
    assert_eq!(
        &tg_base::openssl_help::sha1(&bytes[..(8 + size) * 4])[4..],
        &msg
    );
    let inner = tg_scheme::BindAuthKeyInner::read(&mut tg_tl::Reader::new(&words[8..])).unwrap();
    let inner = inner.c_bind_auth_key_inner();
    assert_eq!(inner.nonce, body.nonce);
    assert_eq!(inner.temp_auth_key_id, temp.key_id());
    assert_eq!(inner.perm_auth_key_id, persistent.key_id());
    assert_eq!(inner.expires_at, temp.expires_at() as i32);
    assert_eq!(
        binder.handle_response(&tg_tl::serialize(&true)),
        DcKeyBindState::Success
    );
    assert_eq!(
        binder.handle_response(&tg_tl::serialize(&tg_scheme::mtp_rpc_error(
            400,
            b"ENCRYPTED_MESSAGE_INVALID".to_vec()
        ))),
        DcKeyBindState::DefinitelyDestroyed
    );
    assert_eq!(
        binder.handle_response(&tg_tl::serialize(&tg_scheme::mtp_rpc_error(
            500,
            b"ERROR".to_vec()
        ))),
        DcKeyBindState::Failed
    );
    assert_eq!(binder.handle_response(&[]), DcKeyBindState::Failed);
}
#[test]
fn concurrent_key_creation_has_one_owner() {
    let center = Arc::new(Dcenter::new(2, None));
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let center = center.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                center.acquire_key_creation(DcType::Regular)
            })
        })
        .collect();
    let values: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        values
            .iter()
            .filter(|&&v| v == CreatingKeyType::Persistent)
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|&&v| v == CreatingKeyType::None)
            .count(),
        7
    );
    center.release_key_creation_on_fail(CreatingKeyType::Persistent);
    let kind = center.acquire_key_creation(DcType::Regular);
    assert_eq!(kind, CreatingKeyType::Persistent);
    let persistent = Arc::new(AuthKey::new(KeyType::Generated, 2, [1; 256], 1));
    let temporary = Arc::new(AuthKey::new(KeyType::Temporary, 2, [2; 256], 2));
    assert!(center.release_key_creation_on_done(kind, temporary.clone(), Some(persistent.clone())));
    assert_eq!(
        center.get_persistent_key().unwrap().key_id(),
        persistent.key_id()
    );
    assert_eq!(
        center
            .get_temporary_key(TemporaryKeyType::Regular)
            .unwrap()
            .key_id(),
        temporary.key_id()
    );
    assert_eq!(
        center.acquire_key_creation(DcType::MediaCluster),
        CreatingKeyType::TemporaryMediaCluster
    );
    center.release_key_creation_on_fail(CreatingKeyType::TemporaryMediaCluster);
    center.set_connection_inited(true);
    assert!(center.destroy_temporary_key(temporary.key_id()));
    assert!(!center.connection_inited());
    assert!(center.destroy_confirmed_forgotten_key(persistent.key_id()));
    assert!(center.get_persistent_key().is_none());
}
#[test]
fn endpoint_filtering_duplicate_policy_and_persisted_limits() {
    let mut options = DcOptions::new(Environment::Test);
    options.construct_add_one(Endpoint {
        id: 20,
        flags: 0,
        ip: "1.2.3.4".into(),
        port: 80,
        secret: Vec::new(),
    });
    options.construct_add_one(Endpoint {
        id: 20,
        flags: STATIC | MEDIA_ONLY | SECRET,
        ip: "5.6.7.8".into(),
        port: 443,
        secret: vec![1; 16],
    });
    options.construct_add_one(Endpoint {
        id: 20,
        flags: CDN,
        ip: "1.2.3.4".into(),
        port: 80,
        secret: Vec::new(),
    });
    assert_eq!(options.lookup(20, DcType::Regular, false)[0][0].len(), 1);
    let media = options.lookup(20, DcType::MediaCluster, false);
    assert_eq!(media[0][0].len(), 1);
    assert_eq!(media[0][0][0].ip, "5.6.7.8");
    assert_eq!(media[0][1].len(), 1);
    let variants = options.lookup(20, DcType::Regular, true);
    assert_eq!(variants[0][0][0].flags, 0);
    assert_eq!(options.dc_type(0x10 * 10000 + 20), DcType::MediaCluster);
    assert_eq!(options.dc_type(1002), DcType::Temporary);
    let mut secret = vec![0xee];
    secret.extend([1; 16]);
    secret.extend(b"long-domain-name.example");
    assert!(DcOptions::validate_secret(&secret));
    options.construct_add_one(Endpoint {
        id: 21,
        flags: SECRET,
        ip: "1.2.3.4".into(),
        port: 443,
        secret,
    });
    let bytes = options.serialize();
    assert!(!DcOptions::new(Environment::Test).construct_from_serialized(&bytes));
}
#[test]
fn config_legacy_suffixes_and_malformed_inputs() {
    let config = Config::new(Environment::Test);
    let bytes = config.serialize();
    for n in 0..bytes.len() {
        let _ = Config::from_serialized(&bytes[..n]);
    }
    let mut wrong = bytes.clone();
    wrong[3] = 2;
    assert!(Config::from_serialized(&wrong).is_none());
    let mut wrong = bytes.clone();
    wrong[7] = 2;
    assert!(Config::from_serialized(&wrong).is_none());
    let data = tg_scheme::ConfigData {
        test_mode: true,
        chat_size_max: 1234,
        rating_e_decay: -1,
        ..Default::default()
    };
    let mut applied = Config::new(Environment::Test);
    assert!(applied.apply(&data));
    assert_eq!(applied.fields.chat_size_max, 1234);
    assert_eq!(applied.fields.rating_decay, 2419200);
    assert!(!Config::new(Environment::Production).apply(&data));
}
#[test]
fn dns_json_type_filter_and_ttl() {
    let bytes=br#"{"Answer":[null,{"data":"1.2.3.4","TTL":4,"type":1},{"data":"::1","TTL":9999,"type":28},{"data":3},{"data":"txt","TTL":"bad","type":16}]}"#;
    let parsed = parse_dns_response(bytes, None);
    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parse_dns_response(bytes, Some(1)),
        vec![DnsEntry {
            data: "1.2.3.4".into(),
            ttl: 4
        }]
    );
    for bad in [b"not json".as_slice(), b"[]", b"{}", br#"{"Answer":{}}"#] {
        assert!(parse_dns_response(bad, None).is_empty());
    }
}
#[test]
fn dns_virtual_stagger_cache_and_combined_result() {
    let mut resolver = DomainResolver::default();
    let mut random = Entropy(24);
    let requests = resolver.resolve("proxy.example", 1000, &mut random);
    assert_eq!(requests.len(), 2);
    assert_eq!(resolver.next_deadline(), Some(1800));
    assert!(resolver.poll(1799, &mut random).is_empty());
    assert_eq!(resolver.poll(1800, &mut random).len(), 2);
    let v4 = requests.iter().find(|r| !r.key.ipv6).unwrap();
    let v6 = requests.iter().find(|r| r.key.ipv6).unwrap();
    resolver.request_finished(v6, br#"{"Answer":[{"data":"::1","TTL":20}]}"#, 2000);
    assert!(resolver.resolved.is_empty());
    resolver.request_finished(v4, br#"{"Answer":[{"data":"1.2.3.4","TTL":1}]}"#, 2000);
    let result = resolver.resolved.pop_front().unwrap();
    assert_eq!(result.ips, vec!["1.2.3.4", "::1"]);
    assert_eq!(result.expire_at, 12000);
    assert!(!resolver.is_request_active(v4));
    assert!(resolver.poll(10000, &mut random).is_empty());
    assert!(resolver
        .resolve("proxy.example", 11000, &mut random)
        .is_empty());
    assert_eq!(
        resolver.resolve("proxy.example", 13000, &mut random).len(),
        1
    );
}
#[test]
fn resolver_virtual_deadlines_and_ip_changes() {
    let proxy = ProxyData {
        kind: Type::Mtproto,
        host: "proxy.example".into(),
        port: 443,
        password: "000102030405060708090a0b0c0d0e0f".into(),
        resolved_ips: vec!["1.2.3.4".into(), "5.6.7.8".into()],
        ..Default::default()
    };
    let mut c = ResolvingConnection::new(proxy, 100);
    assert_eq!(c.full_connect_timeout(), 8000);
    assert_eq!(c.deadline, Some(4100));
    assert!(matches!(
        c.actions.pop_back(),
        Some(Action::ReplaceChild(_))
    ));
    c.actions.clear();
    c.poll(4099);
    assert!(c.actions.is_empty());
    c.poll(4100);
    assert_eq!(c.ip_index, 1);
    c.handle_connected();
    assert_eq!(c.deadline, None);
    c.poll(10000);
    assert!(c.connected);
    c.domain_resolved("other", &[], 0, 10000);
    assert_eq!(c.proxy.resolved_ips.len(), 2);
    c.domain_resolved("proxy.example", &["9.8.7.6".into()], 20000, 10000);
    assert_eq!(c.ip_index, 0);
}
#[test]
fn error_response_qt_regex_and_local_values() {
    use tg_mtproto::mtproto_response::Error;
    let error = Error::from_rpc_error(&tg_scheme::mtp_rpc_error(420, b"FLOOD_WAIT_12".to_vec()));
    assert!(error.is_flood_error());
    assert!(error.is_temporary_error());
    let error = Error::from_rpc_error(&tg_scheme::mtp_rpc_error(
        400,
        b"BAD: first\nsecond".to_vec(),
    ));
    assert_eq!(error.kind, "BAD");
    assert_eq!(error.description, "first\nsecond");
    assert_eq!(Error::from_reply(&[]).kind, "CLIENT_RESPONSE_PARSE_FAILED");
    assert_eq!(
        Error::from_rpc_error(&tg_scheme::mtp_rpc_error(500, b"bad text".to_vec())).kind,
        "INTERNAL_SERVER_ERROR"
    );
    assert_eq!(
        Error::from_rpc_error(&tg_scheme::mtp_rpc_error(400, b"bad text".to_vec())).kind,
        "CLIENT_BAD_RPC_ERROR"
    );
    let _ = Time::default();
}
#[test]
fn request_identity_is_shared_by_handles() {
    use tg_mtproto::details::mtproto_serialized_request::RequestStore;
    let mut store = RequestStore::default();
    let a = store.insert(SerializedRequest::serialize(&tg_scheme::Ping::new(7), 0));
    let b = a;
    store.get_mut(a).unwrap().set_msg_id(1234);
    assert_eq!(store.get(b).unwrap().get_msg_id(), 1234);
    let mut request = SerializedRequest::serialize(&tg_scheme::Ping::new(8), 0);
    request.after = Some(a);
    let next = store.insert(request);
    assert_eq!(store.get(next).unwrap().after, Some(b));
    store.remove(a);
    assert!(store.get(b).is_none());
}
#[test]
fn web_socket_stream_window_and_drop() {
    use std::sync::Mutex;
    use tg_mtproto::details::{
        mtproto_abstract_socket::SocketEvent,
        mtproto_web_proxy_socket::{WebProxySocket, WebTransport},
    };
    struct Transport(Arc<Mutex<Vec<String>>>);
    impl WebTransport for Transport {
        fn register_stream(&mut self, id: u32) {
            self.0.lock().unwrap().push(format!("register {id}"));
        }
        fn close_stream(&mut self, id: u32) {
            self.0.lock().unwrap().push(format!("close {id}"));
        }
        fn send_data(&mut self, id: u32, data: Vec<u8>) {
            self.0.lock().unwrap().push(format!("send {id} {data:?}"));
        }
        fn grant_window(&mut self, id: u32, size: usize) {
            self.0.lock().unwrap().push(format!("grant {id} {size}"));
        }
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    {
        let mut socket = WebProxySocket::new(7, Transport(calls.clone()));
        socket.connect_to_host();
        socket.transport_connected();
        assert_eq!(socket.events.pop_front(), Some(SocketEvent::Connected));
        socket.transport_data(&[1, 2, 3]);
        assert_eq!(socket.events.pop_front(), Some(SocketEvent::ReadyRead));
        let mut output = [0; 2];
        assert_eq!(socket.read(&mut output), 2);
        assert_eq!(output, [1, 2]);
        assert!(socket.has_bytes_available());
        assert_eq!(socket.read(&mut output), 1);
        assert!(!socket.has_bytes_available());
        socket.write(&[8], &[9]);
        socket.transport_disconnected();
        socket.transport_disconnected();
        assert_eq!(socket.events.len(), 1);
    }
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "register 7",
            "grant 7 2",
            "grant 7 1",
            "send 7 [8, 9]",
            "close 7"
        ]
    );
}
