use super::*;
use crate::{
    details::{mtproto_dcenter::Dcenter, mtproto_serialized_request::RequestStore},
    mtproto_auth_key::KeyType,
    mtproto_dc_options::{DcOptions, Environment},
    session::{EnvironmentChange, MainCommand, ProxySettings, SessionEnvironment},
};
use std::sync::{
    atomic::{AtomicI64, AtomicU64, Ordering},
    Mutex,
};
use tg_base::timer::ManualClock;
use tokio::sync::mpsc;
#[derive(Clone)]
struct TestClock {
    time: ManualClock,
    unix: Arc<AtomicI64>,
    id: Arc<AtomicU64>,
    updates: Arc<Mutex<Vec<(i32, bool)>>>,
}
impl TestClock {
    fn new() -> Self {
        Self {
            time: ManualClock::new(100),
            unix: Arc::new(AtomicI64::new(1700000000)),
            id: Arc::new(AtomicU64::new(1700000000u64 << 32)),
            updates: Arc::new(Mutex::new(vec![])),
        }
    }
}
impl Clock for TestClock {
    fn now(&self) -> Time {
        Time {
            monotonic_ms: tg_base::timer::Clock::now(&self.time),
            unix_seconds: self.unix.load(Ordering::SeqCst),
        }
    }
    fn update_unix_time(&self, time: i32) {
        self.update_time(time, false)
    }
    fn next_message_id(&self) -> u64 {
        self.id.fetch_add(4, Ordering::SeqCst) + 4
    }
}
impl SessionClock for TestClock {
    fn local_unix_seconds(&self) -> i64 {
        1700000000
    }
    fn update_time(&self, time: i32, force: bool) {
        self.updates.lock().unwrap().push((time, force));
        self.unix.store(time as i64, Ordering::SeqCst);
    }
}
struct Entropy(u64);
impl Random for Entropy {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *byte = self.0 as u8;
        }
    }
}
struct Env {
    options: Mutex<DcOptions>,
    callbacks: Mutex<BTreeSet<i32>>,
    events: Mutex<Vec<InstanceEvent>>,
    changes: ChangeBus,
    destroyer: bool,
    proxy: Mutex<ProxySettings>,
}
impl Env {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            options: Mutex::new(DcOptions::new(Environment::Test)),
            callbacks: Mutex::new(BTreeSet::new()),
            events: Mutex::new(vec![]),
            changes: ChangeBus::default(),
            destroyer: false,
            proxy: Mutex::new(ProxySettings::default()),
        })
    }
}
impl SessionEnvironment for Env {
    fn dc_options(&self) -> DcOptions {
        self.options.lock().unwrap().clone()
    }
    fn is_test_mode(&self) -> bool {
        true
    }
    fn is_keys_destroyer(&self) -> bool {
        self.destroyer
    }
    fn device_model(&self) -> String {
        "Desktop".into()
    }
    fn system_version(&self) -> String {
        "Linux".into()
    }
    fn system_lang_code(&self) -> String {
        "en".into()
    }
    fn cloud_lang_code(&self) -> String {
        "ru".into()
    }
    fn lang_pack_name(&self) -> String {
        "tdesktop".into()
    }
    fn proxy_settings(&self) -> ProxySettings {
        self.proxy.lock().unwrap().clone()
    }
    fn has_callback(&self, id: i32) -> bool {
        self.callbacks.lock().unwrap().contains(&id)
    }
    fn subscribe(&self) -> mpsc::UnboundedReceiver<EnvironmentChange> {
        self.changes.subscribe()
    }
    fn emit(&self, event: InstanceEvent) {
        assert!(tg_runtime::main_queue().is_main_thread());
        self.events.lock().unwrap().push(event);
    }
}
fn main_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    tg_runtime::main_queue().bind_current_thread();
    tg_runtime::process_main_queue();
    guard
}
fn auth() -> Arc<AuthKey> {
    let mut key = AuthKey::new(KeyType::Temporary, 2, std::array::from_fn(|i| i as u8), 1);
    key.set_expires_at(1700086430);
    Arc::new(key)
}
struct Harness {
    p: SessionPrivate<TestClock, Entropy>,
    env: Arc<Env>,
    main: mpsc::UnboundedReceiver<MainCommand>,
    connection: u64,
    server_id: u64,
}
impl Harness {
    fn new() -> Self {
        let env = Env::new();
        let dc = Arc::new(Dcenter::new(2, None));
        let acquired = dc.acquire_key_creation(DcType::Regular);
        let persistent = Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1));
        assert!(dc.release_key_creation_on_done(acquired, auth(), Some(persistent)));
        let (tx, main) = mpsc::unbounded_channel();
        let data = SessionData::new(
            2,
            dc,
            env.clone(),
            Arc::new(Mutex::new(RequestStore::default())),
            tx,
        );
        data.refresh_options();
        let clock = TestClock::new();
        let host = TimerHost::new(Arc::new(clock.time.clone()));
        let mut p = SessionPrivate::new(
            data,
            clock,
            Entropy(0x123456789abcdef),
            host,
            ClientInfo::default(),
        );
        let actions = p.take_actions();
        let connection = actions
            .into_iter()
            .filter_map(|a| {
                if let TransportAction::Connect(spec) = a {
                    Some(spec)
                } else {
                    None
                }
            })
            .max_by_key(|s| s.priority)
            .unwrap()
            .id;
        p.on_connected(connection, true, "TCP".into(), 8000);
        Self {
            p,
            env,
            main,
            connection,
            server_id: (1700000000u64 << 32) | 1,
        }
    }
    fn ready() -> Self {
        let mut h = Self::new();
        h.p.try_to_send();
        let packet = h.sent().pop().expect("initial ping");
        let m = server_decrypt(&packet);
        assert_eq!(m.body[0] as u32, 0x7abe77ec);
        let body = vec![
            0x347773c5,
            m.msg_id as i32,
            (m.msg_id >> 32) as i32,
            m.body[1],
            m.body[2],
        ];
        h.reply(body, 100, 0);
        assert_eq!(h.p.get_state(), CONNECTED);
        h.p.take_actions();
        h.drain();
        h
    }
    fn drain(&mut self) -> Vec<MainCommand> {
        let mut commands = vec![];
        while let Ok(command) = self.main.try_recv() {
            if let MainCommand::Instance(event) = &command {
                crate::session::emit_on_main(self.env.clone(), event.clone());
            }
            commands.push(command);
        }
        commands
    }
    fn sent(&mut self) -> Vec<Buffer> {
        self.p
            .take_actions()
            .into_iter()
            .filter_map(|a| {
                if let TransportAction::Send(_, packet) = a {
                    Some(packet)
                } else {
                    None
                }
            })
            .collect()
    }
    fn request(&mut self, id: i32) -> (RequestHandle, u64) {
        self.env.callbacks.lock().unwrap().insert(id);
        let mut request = body_request(vec![0xc4f9186b_u32 as i32], self.p.host.now());
        request.request_id = id;
        let handle = self.p.data.register_request(request);
        self.p
            .data
            .queues
            .lock()
            .unwrap()
            .to_send
            .insert(id, handle);
        self.p.try_to_send();
        let msg = self
            .p
            .data
            .store
            .lock()
            .unwrap()
            .get(handle)
            .unwrap()
            .get_msg_id();
        (handle, msg)
    }
    fn reply(&mut self, body: Buffer, salt: u64, seq: u32) {
        self.server_id += 4;
        let packet = server_encrypt(&Message {
            salt,
            session_id: self.p.session_id(),
            msg_id: self.server_id,
            seq_no: seq,
            body,
        });
        self.p.on_packet(self.connection, packet, false);
    }
    fn handle(&mut self, body: &[i32], bad_time: bool) -> HandleResult {
        self.server_id += 4;
        self.p.handle_one_received(
            body,
            self.server_id,
            OuterInfo {
                outer_msg_id: self.server_id,
                server_salt: 100,
                server_time: 1700000000,
                bad_time,
            },
            &mut (32 * 1024 * 1024),
            0,
        )
    }
}
// The server derives wire keys from SHA256 slices, independently of AuthKey's
// schedule and the client's envelope functions. Only AES block primitives agree.
fn wire_keys(auth: &[u8; 256], mk: &[u8], x: usize) -> ([u8; 32], [u8; 32]) {
    let a = tg_base::openssl_help::sha256_parts(&[mk, &auth[x..x + 36]]);
    let b = tg_base::openssl_help::sha256_parts(&[&auth[40 + x..76 + x], mk]);
    (
        [&a[..8], &b[8..24], &a[24..32]]
            .concat()
            .try_into()
            .unwrap(),
        [&b[..8], &a[8..24], &b[24..32]]
            .concat()
            .try_into()
            .unwrap(),
    )
}
fn server_decrypt(packet: &[i32]) -> Message {
    server_decrypt_with(&std::array::from_fn(|i| i as u8), packet)
}
fn server_decrypt_with(auth: &[u8; 256], packet: &[i32]) -> Message {
    use aes::cipher::{BlockModeDecrypt, KeyIvInit};
    let bytes = tg_tl::primes_to_bytes(&packet[2..]);
    let mk = &bytes[8..24];
    let (k, iv) = wire_keys(auth, mk, 0);
    let mut plain = bytes[24..].to_vec();
    let mut cipher = ige::Decryptor::<aes::Aes256>::new((&k).into(), (&iv).into());
    for block in plain.chunks_exact_mut(16) {
        cipher.decrypt_block(block.try_into().unwrap());
    }
    assert_eq!(
        &tg_base::openssl_help::sha256_parts(&[&auth[88..120], &plain])[8..24],
        mk
    );
    let long = |at| u64::from_le_bytes(plain[at..at + 8].try_into().unwrap());
    let len = u32::from_le_bytes(plain[28..32].try_into().unwrap()) as usize;
    Message {
        salt: long(0),
        session_id: long(8),
        msg_id: long(16),
        seq_no: u32::from_le_bytes(plain[24..28].try_into().unwrap()),
        body: tg_tl::bytes_to_primes(&plain[32..32 + len]),
    }
}
fn server_encrypt(message: &Message) -> Buffer {
    server_encrypt_with(&std::array::from_fn(|i| i as u8), message)
}
fn server_encrypt_with(auth: &[u8; 256], message: &Message) -> Buffer {
    use aes::cipher::{BlockModeEncrypt, KeyIvInit};
    let mut bytes = vec![];
    for value in [message.salt, message.session_id, message.msg_id] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(message.seq_no.to_le_bytes());
    bytes.extend(((message.body.len() * 4) as u32).to_le_bytes());
    bytes.extend(tg_tl::primes_to_bytes(&message.body));
    let padding = 12 + (16 - (bytes.len() + 12) % 16) % 16;
    bytes.resize(bytes.len() + padding, 0xaa);
    let hash = tg_base::openssl_help::sha256_parts(&[&auth[96..128], &bytes]);
    let mk = &hash[8..24];
    let (k, iv) = wire_keys(auth, mk, 8);
    let mut cipher = ige::Encryptor::<aes::Aes256>::new((&k).into(), (&iv).into());
    for block in bytes.chunks_exact_mut(16) {
        cipher.encrypt_block(block.try_into().unwrap());
    }
    let id = &tg_base::openssl_help::sha1(auth)[12..20];
    tg_tl::bytes_to_primes(&[id, mk, &bytes].concat())
}
#[test]
fn initial_salt_ping_sequence_and_rpc_delivery() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (_, id) = h.request(1);
    let packet = h.sent().pop().unwrap();
    let msg = server_decrypt(&packet);
    assert_eq!(msg.msg_id, id);
    assert_eq!(msg.seq_no, 3);
    h.reply(
        vec![
            RPC_RESULT as i32,
            id as i32,
            (id >> 32) as i32,
            0x997275b5_u32 as i32,
        ],
        100,
        1,
    );
    let q = h.p.data.queues.lock().unwrap();
    assert_eq!(q.received.front().unwrap().request_id, 1);
    assert_eq!(q.received.front().unwrap().reply, [0x997275b5_u32 as i32]);
    assert!(!q.have_sent.contains_key(&id));
    drop(q);
    assert!(h.p.data.connection_inited());
    assert!(!h.p.ack.is_empty());
    assert!(h
        .drain()
        .iter()
        .any(|c| matches!(c, MainCommand::TryToReceive)));
}
#[test]
fn bad_msg_every_code_and_unknown_request() {
    let _main = main_lock();
    for code in [16, 17, 18, 19, 20, 32, 33, 34, 35, 48, 64, 999] {
        let mut h = Harness::ready();
        let (_, id) = h.request(1);
        h.sent();
        let old = h.p.session_id();
        h.reply(
            vec![0xa7eff811_u32 as i32, id as i32, (id >> 32) as i32, 3, code],
            100,
            1,
        );
        match code {
            16 | 17 | 64 => {
                assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&1));
                assert!(h
                    .p
                    .clock
                    .updates
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(_, force)| *force));
            }
            32 | 33 => {
                assert_ne!(h.p.session_id(), old);
                assert!(h.p.get_state() < 0);
            }
            _ => {
                assert_eq!(
                    h.p.data
                        .queues
                        .lock()
                        .unwrap()
                        .received
                        .front()
                        .unwrap()
                        .reply[0] as u32,
                    0x2144ca19
                );
            }
        }
    }
    let mut h = Harness::ready();
    assert_eq!(
        h.handle(&[0xa7eff811_u32 as i32, 7, 0, 0, 16], false),
        HandleResult::Success
    );
    assert_eq!(
        h.handle(&[0xa7eff811_u32 as i32, 7, 0, 0, 16], true),
        HandleResult::Ignored
    );
}
#[test]
fn salt_ack_resend_and_message_states() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (_, id) = h.request(1);
    h.sent();
    h.reply(
        vec![
            0xedab447b_u32 as i32,
            id as i32,
            (id >> 32) as i32,
            3,
            48,
            200,
            0,
        ],
        100,
        1,
    );
    assert_eq!(h.p.server_salt(), 200);
    assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&1));
    h.p.try_to_send();
    let sent = h.sent();
    assert_eq!(server_decrypt(&sent[0]).body[0] as u32, MSG_CONTAINER);
    assert!(h.p.data.queues.lock().unwrap().have_sent.contains_key(&id));
    h.reply(id_vector(0x62d6b459, &[id]), 200, 0);
    assert!(h.p.data.queues.lock().unwrap().have_sent.contains_key(&id));
    h.env.callbacks.lock().unwrap().clear();
    h.reply(id_vector(0x62d6b459, &[id]), 200, 0);
    assert!(!h.p.data.queues.lock().unwrap().have_sent.contains_key(&id));
    let (_, id2) = h.request(2);
    h.p.handle_msgs_states(&[id2], &[3]);
    assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&2));
    h.p.handle_msgs_states(&[id2], &[4]);
    assert!(h.p.resending.contains_key(&id2));
    h.env.callbacks.lock().unwrap().clear();
    h.p.handle_msgs_states(&[id2], &[4]);
    assert!(!h.p.resending.contains_key(&id2));
    h.p.handle_msgs_states(&[9], &[]);
}
#[test]
fn detailed_info_duplicate_old_and_new_session() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (_, id) = h.request(1);
    h.sent();
    let answer = 1700000000u64 << 32 | 101;
    h.reply(
        vec![
            0x276d3ec6,
            id as i32,
            (id >> 32) as i32,
            answer as i32,
            (answer >> 32) as i32,
            4,
            0,
        ],
        100,
        0,
    );
    assert_eq!(h.p.resend_ids, [answer]);
    let packet = server_encrypt(&Message {
        salt: 100,
        session_id: h.p.session_id(),
        msg_id: answer,
        seq_no: 1,
        body: vec![0x997275b5_u32 as i32],
    });
    h.p.on_packet(h.connection, packet.clone(), false);
    let received = h.p.data.queues.lock().unwrap().received.len();
    h.p.on_packet(h.connection, packet, false);
    assert_eq!(h.p.data.queues.lock().unwrap().received.len(), received);
    h.reply(
        vec![
            0x809db6df_u32 as i32,
            answer as i32,
            (answer >> 32) as i32,
            4,
            0,
        ],
        100,
        0,
    );
    assert!(h.p.ack.contains(&answer));
    h.reply(
        vec![
            0x9ec20908_u32 as i32,
            (id + 4) as i32,
            ((id + 4) >> 32) as i32,
            55,
            0,
            300,
            0,
        ],
        100,
        1,
    );
    assert_eq!(h.p.salt, 300);
    assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&1));
    assert_eq!(
        h.handle(&[0x809db6df_u32 as i32, 1, 0, 4, 0], true),
        HandleResult::Ignored
    );
}

#[test]
fn container_limits_invoke_after_layer_and_sequence_wrap() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (first, id) = h.request(1);
    h.sent();
    let mut request = body_request(vec![0xc4f9186b_u32 as i32], h.p.host.now());
    request.request_id = 2;
    request.after = Some(first);
    request.needs_layer = true;
    let second = h.p.data.register_request(request);
    h.p.data.queues.lock().unwrap().to_send.insert(2, second);
    h.p.try_to_send();
    let msg = server_decrypt(&h.sent().pop().unwrap());
    assert_eq!(msg.body[0] as u32, 0xda9b0d0d);
    assert_eq!(msg.body[1], 229);
    assert!(msg
        .body
        .windows(3)
        .any(|w| w == [0xcb9f372d_u32 as i32, id as i32, (id >> 32) as i32]));
    h.p.requests_acked(&[id], true);
    let wrapped = h.p.wrap_invoke_after(second, false);
    assert_ne!(wrapped.body[0] as u32, 0xcb9f372d);
    h.p.counter = u32::MAX;
    assert_eq!(h.p.next_request_seq_number(true), u32::MAX);
    assert_eq!(h.p.next_request_seq_number(false), 0);
    let mut big_requests = vec![];
    for n in 3..8 {
        let mut r = body_request(vec![1; 9000], h.p.host.now());
        r.request_id = n;
        let handle = h.p.data.register_request(r);
        h.p.data.queues.lock().unwrap().to_send.insert(n, handle);
        big_requests.push(handle);
    }
    h.p.try_to_send();
    let sent = h.sent();
    assert_eq!(server_decrypt(&sent[0]).body[0] as u32, MSG_CONTAINER);
    assert_eq!(h.p.data.queues.lock().unwrap().to_send.len(), 3);
    assert!(h
        .drain()
        .iter()
        .any(|c| matches!(c, MainCommand::NeedToResumeAndSend)));
}
#[test]
fn gzip_container_budget_parse_errors_and_replay_window() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let fixture = include_bytes!("../../tests/fixtures/true.gz");
    let mut gzip = vec![GZIP_PACKED as i32];
    tg_tl::write_string(&mut gzip, fixture);
    assert_eq!(h.handle(&gzip, false), HandleResult::Success);
    assert_eq!(
        h.p.data
            .queues
            .lock()
            .unwrap()
            .received
            .back()
            .unwrap()
            .reply,
        [0x997275b5_u32 as i32]
    );
    assert_eq!(
        h.p.handle_one_received(
            &gzip,
            1,
            OuterInfo {
                outer_msg_id: 1,
                server_salt: 100,
                server_time: 1700000000,
                bad_time: false
            },
            &mut 1,
            0
        ),
        HandleResult::RestartConnection
    );
    assert_eq!(
        h.p.handle_one_received(
            &gzip,
            1,
            OuterInfo {
                outer_msg_id: 1,
                server_salt: 100,
                server_time: 1700000000,
                bad_time: false
            },
            &mut 400,
            64
        ),
        HandleResult::RestartConnection
    );
    for bad in [
        vec![],
        vec![GZIP_PACKED as i32],
        vec![MSG_CONTAINER as i32],
        vec![MSG_CONTAINER as i32, 1, 1, 0, 1, -4],
        vec![0x04deb57d],
        vec![0x8cc0d131_u32 as i32],
        vec![RPC_RESULT as i32],
        vec![0x9ec20908_u32 as i32],
        vec![0x347773c5],
        vec![0xa7eff811_u32 as i32],
    ] {
        assert_ne!(h.handle(&bad, false), HandleResult::Success, "{bad:?}");
    }
    assert_eq!(
        h.handle(&[0x997275b5_u32 as i32], true),
        HandleResult::ResetSession
    );
    let nested_id = h.server_id + 40;
    let mut body = vec![
        MSG_CONTAINER as i32,
        1,
        nested_id as i32,
        (nested_id >> 32) as i32,
        1,
        4,
        0x997275b5_u32 as i32,
    ];
    assert_eq!(h.handle(&body, false), HandleResult::Success);
    body[2] = 2;
    body[3] = 0;
    assert_eq!(h.handle(&body, false), HandleResult::RestartConnection);
    for n in 0..400 {
        h.p.received
            .register_msg_id((1800000000u64 << 32) | ((n + 1) * 4) | 1, true);
    }
    h.p.received.shrink();
    h.reply(vec![0x997275b5_u32 as i32], 100, 1);
    assert!(h.p.get_state() < 0);
}
#[test]
fn state_requests_reply_info_and_cancellation() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (_, id) = h.request(1);
    h.sent();
    h.p.state_ids.insert(id);
    h.p.try_to_send();
    h.sent();
    let (&state_id, _) = h.p.state_requests.first_key_value().unwrap();
    let mut info = vec![0x04deb57d, state_id as i32, (state_id >> 32) as i32];
    tg_tl::write_string(&mut info, &[3]);
    assert_eq!(h.handle(&info, true), HandleResult::Success);
    assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&1));
    let mut all = id_vector(0x8cc0d131, &[id]);
    tg_tl::write_string(&mut all, &[4]);
    assert_eq!(h.handle(&all, true), HandleResult::Ignored);
    assert_eq!(h.handle(&all, false), HandleResult::Success);
    let unknown = [0x04deb57d, 123, 0, 0];
    assert_eq!(h.handle(&unknown, false), HandleResult::Success);
    assert_eq!(h.handle(&unknown, true), HandleResult::Ignored);
    h.p.resend(id, 0);
    h.p.requests_acked(&[id], true);
    assert!(!h.p.resending.contains_key(&id));
}
#[test]
fn virtual_timer_backoff_connection_selection_and_time_adjustment() {
    let _main = main_lock();
    let mut h = Harness::new();
    h.p.try_to_send();
    h.sent();
    h.p.restart_now();
    assert_eq!(h.p.get_state(), -1);
    for expected in [2, 3, 1000, 2000, 4000] {
        let wait = h.p.retry_timeout;
        h.p.clock.time.advance(wait);
        h.p.process_timers();
        assert_eq!(h.p.retry_timeout, expected);
        assert_eq!(h.p.get_state(), CONNECTING);
        h.p.take_actions();
        h.p.restart();
    }
    h.p.retry_timeout = 64000;
    h.p.retry_by_timer();
    assert_eq!(h.p.retry_timeout, 64000);
    h.p.take_actions();
    h.p.clock.time.advance(1000);
    h.p.process_timers();
    assert_eq!(h.p.wait_connected, 2000);
    assert!(h
        .p
        .take_actions()
        .iter()
        .any(|a| matches!(a, TransportAction::TimedOut(_))));
    h.p.dc_options_changed();
    h.p.cdn_config_changed();
    let id = *h.p.candidates.first_key_value().unwrap().0;
    h.p.on_connected(id, true, "test".into(), 8000);
    h.p.on_received_some();
    h.p.timers.connected.cancel();
    h.p.timers.received.cancel();
    h.p.clock.time.advance(192000);
    h.p.process_timers();
    assert!(h.p.old_connection);
    h.p.on_sent_some(1024 * 1024);
    assert_eq!(h.p.timers.received.remaining_time(), 4000);
    h.p.on_received_some();
    h.p.on_sent_some(1024 * 1024);
    assert_eq!(h.p.timers.received.remaining_time(), 64000);
    h.p.host.adjust();
    assert!(h.p.next_deadline().is_some());
    h.p.clock.time.advance(64000);
    h.p.process_timers();
    assert!(h.p.wait_received >= 8000);
}
#[test]
fn virtual_sent_checks_container_expiry_and_ping_timeout() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (_, a) = h.request(1);
    h.sent();
    let mut request = body_request(vec![1], h.p.host.now());
    request.request_id = 2;
    request.force_send_in_container = true;
    let handle = h.p.data.register_request(request);
    h.p.data.queues.lock().unwrap().to_send.insert(2, handle);
    h.p.try_to_send();
    h.sent();
    assert!(h.p.timers.check.is_active());
    h.p.timers.received.cancel();
    h.p.timers.ping.cancel();
    h.p.clock.time.advance(10000);
    h.p.process_timers();
    assert!(h.p.state_ids.contains(&a));
    assert!(h
        .drain()
        .iter()
        .any(|c| matches!(c, MainCommand::SendAnything(1000))));
    h.p.timers.check.cancel();
    h.p.clock.time.advance(590000);
    h.p.process_timers();
    assert!(h.p.data.queues.lock().unwrap().to_send.contains_key(&2));
    h.p.ping_id = 10;
    h.p.ping_send_at = h.p.host.now() - 14999;
    h.p.send_ping_by_timer();
    assert!(h.p.get_state() < 0);
    h.p.ping_id = 0;
    h.p.send_ping_by_timer();
    assert!(h
        .drain()
        .iter()
        .any(|c| matches!(c, MainCommand::NeedToResumeAndSend)));
}

#[test]
fn binding_success_failure_persistent_age_and_key_destruction() {
    let _main = main_lock();
    for response in [
        vec![tg_tl::ID_BOOL_TRUE as i32],
        tg_tl::serialize(&tg_scheme::mtp_rpc_error(500, b"UNKNOWN".to_vec())),
        tg_tl::serialize(&tg_scheme::mtp_rpc_error(
            400,
            b"ENCRYPTED_MESSAGE_INVALID".to_vec(),
        )),
    ] {
        let mut h = Harness::ready();
        let key = h.p.key.as_ref().unwrap().clone();
        assert!(h.p.data.dc.destroy_temporary_key(key.key_id()));
        assert_eq!(
            h.p.data.acquire_key_creation(DcType::Regular),
            CreatingKeyType::TemporaryRegular
        );
        let mut creator = BoundKeyCreator::new(DcKeyRequest {
            temporary_expires_in: 86400,
            persistent_needed: false,
        });
        creator.bind(h.p.data.get_persistent_key().unwrap());
        h.p.key_creator = Some(creator);
        h.p.try_to_send();
        let packet = h.sent().pop().unwrap();
        let message = server_decrypt(&packet);
        let bind = if message.body[0] as u32 == MSG_CONTAINER {
            let mut c = Cursor::new(&message.body[2..]);
            c.long().unwrap();
            c.int().unwrap();
            let n = c.int().unwrap() as usize / 4;
            c.take(n).unwrap().to_vec()
        } else {
            message.body
        };
        let request: tg_scheme::AuthBindTempAuthKey = tg_tl::deserialize_exact(&bind).unwrap();
        assert_eq!(
            request.perm_auth_key_id,
            h.p.data.get_persistent_key().unwrap().key_id()
        );
        let msg = h.p.bind_msg_id;
        assert_ne!(msg, 0);
        let result = h.p.handle_bind_response(msg, &response);
        if response[0] as u32 == 0x997275b5 {
            assert_eq!(result, HandleResult::Success);
            assert!(h.p.key_creator.is_none());
            assert!(h
                .p
                .data
                .get_temporary_key(TemporaryKeyType::Regular)
                .is_some());
        } else if response.len() > 4
            && crate::mtproto_response::Error::from_reply(&response).kind
                == "ENCRYPTED_MESSAGE_INVALID"
        {
            assert!(!h.p.destroy_old_enough_persistent_key());
            h.p.clock.time.advance(60001);
            assert!(h.p.destroy_old_enough_persistent_key());
        } else {
            assert_eq!(result, HandleResult::Success);
        }
        assert_eq!(
            h.p.handle_bind_response(999, &response),
            HandleResult::Ignored
        );
    }
    let mut h = Harness::ready();
    let old = h.p.key_id;
    h.p.on_error(h.connection, -404);
    assert!(h
        .p
        .data
        .get_temporary_key(TemporaryKeyType::Regular)
        .is_none());
    assert_ne!(h.p.key_id, old);
    h.p.on_error(h.connection, -444);
    h.drain();
    tg_runtime::process_main_queue();
    assert!(h
        .env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::BadConfigurationError)));
}
#[test]
fn connection_priority_failover_options_and_cdn_gating() {
    let _main = main_lock();
    let mut h = Harness::new();
    h.p.do_disconnect();
    h.p.connect_to_server(false);
    let lower =
        h.p.candidates
            .iter()
            .min_by_key(|(_, c)| c.spec.priority)
            .map(|(&id, _)| id)
            .unwrap();
    h.p.on_connected(lower, true, "IPv6".into(), 9000);
    assert!(h.p.connection.is_none());
    assert!(h.p.timers.better.is_active());
    h.p.clock.time.advance(2000);
    h.p.process_timers();
    assert_eq!(h.p.connection.as_ref().unwrap().spec.id, lower);
    assert_eq!(h.p.transport(), "IPv6");
    h.p.on_disconnected(lower);
    assert!(h.p.get_state() < 0);
    h.p.restart_now();
    h.p.clock.time.advance(1);
    h.p.process_timers();
    let id = *h.p.candidates.keys().next().unwrap();
    h.p.on_connected(id, false, "invalid".into(), 8000);
    assert!(h.p.get_state() < 0);
    let mut h = Harness::ready();
    h.p.data.dc.destroy_temporary_key(h.p.key_id);
    h.p.apply_auth_key(None);
    assert_eq!(h.p.key_id, 0);
    h.p.data.set_options(SessionOptions {
        use_ipv4: false,
        use_ipv6: false,
        use_tcp: false,
        use_http: false,
        ..SessionOptions::default()
    });
    h.p.connect_to_server(false);
    assert!(h
        .p
        .candidates
        .values()
        .all(|c| c.spec.endpoint.ip.contains(':') && c.spec.http));
    h.p.do_disconnect();
    h.env.options.lock().unwrap().construct_add_one(Endpoint {
        id: 9,
        flags: crate::mtproto_dc_options::CDN,
        ip: "127.0.0.1".into(),
        port: 443,
        secret: vec![],
    });
    h.p.real_dc_type = DcType::Cdn;
    h.p.current_dc_type = DcType::Cdn;
    h.p.request_cdn_config();
    h.drain();
    tg_runtime::process_main_queue();
    assert!(h
        .env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::RequestCdnConfig)));
}
#[test]
fn session_controller_main_queue_lifecycle_options_and_cancellation() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand, TransportEvent};
    let env = Env::new();
    let dc = Arc::new(Dcenter::new(2, None));
    let creation = dc.acquire_key_creation(DcType::Regular);
    assert!(dc.release_key_creation_on_done(
        creation,
        auth(),
        Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1)))
    ));
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, handle) = Session::new(
        2,
        dc.clone(),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        crate::session::SessionParameters {
            clock: clock.clone(),
            random: Entropy(0x123456),
            host: host.clone(),
            info: ClientInfo::default(),
        },
    );
    assert_eq!(session.get_dc_with_shift(), 2);
    assert_eq!(session.request_state(0), 1);
    let actions = session.take_transport_actions();
    let connection = actions
        .into_iter()
        .filter_map(|a| {
            if let TransportAction::Connect(c) = a {
                Some(c)
            } else {
                None
            }
        })
        .max_by_key(|c| c.priority)
        .unwrap()
        .id;
    session.on_transport_event(TransportEvent::Connected {
        id: connection,
        tag: "TCP".into(),
        full_timeout: 8000,
    });
    session.process();
    let packets = session.take_transport_actions();
    let packet = packets
        .into_iter()
        .find_map(|a| {
            if let TransportAction::Send(_, p) = a {
                Some(p)
            } else {
                None
            }
        })
        .unwrap();
    let message = server_decrypt(&packet);
    let reply = server_encrypt(&Message {
        salt: 100,
        session_id: message.session_id,
        msg_id: 1700000000u64 << 32 | 1,
        seq_no: 0,
        body: vec![
            0x347773c5,
            message.msg_id as i32,
            (message.msg_id >> 32) as i32,
            message.body[1],
            message.body[2],
        ],
    });
    session.on_transport_event(TransportEvent::Packet {
        id: connection,
        words: reply,
        need_http_wait: false,
    });
    session.process();
    assert_eq!(session.get_state(), CONNECTED);
    assert_eq!(session.transport(), "TCP");
    assert!(env.events.lock().unwrap().is_empty());
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::StateChange(2, CONNECTED))));
    let mut request = body_request(vec![1], 100);
    request.request_id = 5;
    let req = session.data.register_request(request);
    session.send_prepared(req, -1);
    assert_eq!(session.request_state(5), 2);
    session.cancel(5, 0);
    assert_eq!(session.request_state(5), 0);
    session.send_prepared(req, 30);
    session.process();
    clock.time.advance(10);
    session.send_anything(5);
    assert_eq!(host.next_deadline(), Some(115));
    clock.time.advance(5);
    session.process();
    assert_eq!(session.request_state(5), 0);
    assert!(!session.data.queues.lock().unwrap().have_sent.is_empty());
    let id = session
        .data
        .store
        .lock()
        .unwrap()
        .get(req)
        .unwrap()
        .get_msg_id();
    session.cancel(5, id);
    assert!(session.data.queues.lock().unwrap().have_sent.is_empty());
    handle.send(SessionCommand::PauseDelivery(true));
    session.process();
    session
        .data
        .queues
        .lock()
        .unwrap()
        .received
        .push_back(Response {
            reply: vec![1],
            request_id: 5,
            outer_msg_id: 1,
        });
    session.data.queue_try_to_receive();
    session.process();
    tg_runtime::process_main_queue();
    assert!(!env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::Callback(_))));
    handle.send(SessionCommand::Unpaused);
    session.process();
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::Callback(_))));
    session.notify_dc_connection_inited();
    assert!(session.connection_inited());
    session.set_connection_not_inited();
    assert!(!session.connection_inited());
    assert!(session.get_persistent_key().is_some());
    assert!(session
        .get_temporary_key(TemporaryKeyType::Regular)
        .is_some());
    env.changes.send(EnvironmentChange::DcOptions(2)).unwrap();
    session.process();
    session.stop();
    assert_eq!(session.get_state(), DISCONNECTED);
    assert!(session
        .take_transport_actions()
        .iter()
        .any(|a| matches!(a, TransportAction::Close(_))));
    session.need_to_resume_and_send();
    assert_eq!(session.get_state(), CONNECTING);
    session.re_init_connection();
    session.process();
    session.ping();
    session.kill();
    session.restart();
    session.send_anything(0);
    assert_eq!(session.get_state(), DISCONNECTED);
    session.data.queue_need_to_resume_and_send();
    assert!(!session.data.connection_inited());
}

struct DuplexTransport {
    events: mpsc::UnboundedReceiver<crate::session::TransportEvent>,
    event_tx: mpsc::UnboundedSender<crate::session::TransportEvent>,
    streams: mpsc::UnboundedSender<tokio::io::DuplexStream>,
    writers: BTreeMap<u64, mpsc::UnboundedSender<Buffer>>,
    tasks: BTreeMap<u64, tokio::task::JoinHandle<()>>,
}
impl DuplexTransport {
    fn new() -> (Self, mpsc::UnboundedReceiver<tokio::io::DuplexStream>) {
        let (tx, events) = mpsc::unbounded_channel();
        let (streams, rx) = mpsc::unbounded_channel();
        (
            Self {
                events,
                event_tx: tx,
                streams,
                writers: BTreeMap::new(),
                tasks: BTreeMap::new(),
            },
            rx,
        )
    }
}
impl crate::session::SessionTransport for DuplexTransport {
    fn submit(
        &mut self,
        action: TransportAction,
    ) -> Result<(), crate::connection_abstract::TransportError> {
        use crate::session::TransportEvent;
        match action {
            TransportAction::Connect(spec) => {
                if spec.http || spec.endpoint.ip.contains(':') {
                    return Ok(());
                }
                let (client, server) = tokio::io::duplex(128);
                self.streams.send(server).unwrap();
                let (tx, mut rx) = mpsc::unbounded_channel();
                self.writers.insert(spec.id, tx);
                let events = self.event_tx.clone();
                let id = spec.id;
                self.tasks.insert(
                    id,
                    tokio::spawn(async move {
                        let connection = crate::connection_tcp::TcpConnection::new(
                            client,
                            crate::connection_tcp::Protocol::Version0,
                            10002,
                            Entropy(0x13579),
                        )
                        .await
                        .unwrap();
                        let (mut reader, mut writer) = connection.into_split();
                        let _ = events.send(TransportEvent::Connected {
                            id,
                            tag: "duplex TCP".into(),
                            full_timeout: 8000,
                        });
                        let sending = async {
                            while let Some(packet) = rx.recv().await {
                                if writer.send_data(packet).await.is_err() {
                                    break;
                                }
                            }
                        };
                        let receiving = async {
                            loop {
                                match reader.receive().await {
                                    Ok(words) => {
                                        if events
                                            .send(TransportEvent::Packet {
                                                id,
                                                words,
                                                need_http_wait: false,
                                            })
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        let _ = events.send(TransportEvent::Disconnected(id));
                                        break;
                                    }
                                }
                            }
                        };
                        tokio::select! {_=sending=>{},_=receiving=>{}}
                    }),
                );
            }
            TransportAction::Send(id, packet) => {
                if let Some(writer) = self.writers.get(&id) {
                    let _ = writer.send(packet);
                }
            }
            TransportAction::Close(id) | TransportAction::TimedOut(id) => {
                self.writers.remove(&id);
                if let Some(task) = self.tasks.remove(&id) {
                    task.abort();
                }
            }
        }
        Ok(())
    }
    async fn next_event(
        &mut self,
    ) -> Result<crate::session::TransportEvent, crate::connection_abstract::TransportError> {
        self.events
            .recv()
            .await
            .ok_or(crate::connection_abstract::TransportError::Protocol(
                "test executor closed",
            ))
    }
}
impl Drop for DuplexTransport {
    fn drop(&mut self) {
        for (_, task) in std::mem::take(&mut self.tasks) {
            task.abort();
        }
    }
}
struct ServerTcp {
    stream: tokio::io::DuplexStream,
    receive: crate::mtproto_auth_key::CtrState,
    send: crate::mtproto_auth_key::CtrState,
}
impl ServerTcp {
    async fn new(mut stream: tokio::io::DuplexStream) -> Self {
        use tokio::io::AsyncReadExt;
        let mut header = [0; 64];
        stream.read_exact(&mut header).await.unwrap();
        let mut receive = crate::mtproto_auth_key::CtrState::new(
            &header[8..40].try_into().unwrap(),
            &header[40..56].try_into().unwrap(),
        );
        let mut reverse = header[8..56].to_vec();
        reverse.reverse();
        let send = crate::mtproto_auth_key::CtrState::new(
            &reverse[..32].try_into().unwrap(),
            &reverse[32..].try_into().unwrap(),
        );
        receive.apply(&mut header);
        assert_eq!(header[56..60], [0xef; 4]);
        Self {
            stream,
            receive,
            send,
        }
    }
    async fn read(&mut self) -> Buffer {
        use tokio::io::AsyncReadExt;
        let mut first = [0];
        self.stream.read_exact(&mut first).await.unwrap();
        self.receive.apply(&mut first);
        let count = if first[0] == 127 {
            let mut rest = [0; 3];
            self.stream.read_exact(&mut rest).await.unwrap();
            self.receive.apply(&mut rest);
            u32::from_le_bytes([rest[0], rest[1], rest[2], 0]) as usize
        } else {
            first[0] as usize
        };
        let mut body = vec![0; count * 4];
        self.stream.read_exact(&mut body).await.unwrap();
        self.receive.apply(&mut body);
        tg_tl::bytes_to_primes(&body)
    }
    async fn write(&mut self, words: &[i32]) {
        use tokio::io::AsyncWriteExt;
        let count = words.len();
        let mut frame = if count < 127 {
            vec![count as u8]
        } else {
            let mut bytes = vec![127];
            bytes.extend_from_slice(&(count as u32).to_le_bytes()[..3]);
            bytes
        };
        frame.extend(tg_tl::primes_to_bytes(words));
        self.send.apply(&mut frame);
        self.stream.write_all(&frame).await.unwrap();
    }
    async fn read_encrypted(&mut self) -> Message {
        let mut packet = vec![0, 0];
        packet.extend(self.read().await);
        server_decrypt(&packet)
    }
    async fn reply(&mut self, session: u64, id: u64, body: Buffer) {
        self.write(&server_encrypt(&Message {
            salt: 100,
            session_id: session,
            msg_id: id,
            seq_no: 1,
            body,
        }))
        .await;
    }
}
fn contained_requests(message: &Message) -> Vec<(u64, Buffer)> {
    if message.body[0] as u32 != MSG_CONTAINER {
        return vec![(message.msg_id, message.body.clone())];
    }
    let mut at = 2;
    let mut result = vec![];
    for _ in 0..message.body[1] {
        let id = message.body[at] as u32 as u64 | ((message.body[at + 1] as u32 as u64) << 32);
        let words = message.body[at + 3] as usize / 4;
        result.push((id, message.body[at + 4..at + 4 + words].to_vec()));
        at += 4 + words;
    }
    result
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn duplex_server_reconnect_resend_salt_ack_and_rpc_main_order() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand};
    let env = Env::new();
    env.callbacks.lock().unwrap().insert(77);
    let dc = Arc::new(Dcenter::new(2, None));
    let creation = dc.acquire_key_creation(DcType::Regular);
    dc.release_key_creation_on_done(
        creation,
        auth(),
        Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1))),
    );
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (session, handle) = Session::new(
        2,
        dc,
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        crate::session::SessionParameters {
            clock: clock.clone(),
            random: Entropy(0x2468),
            host,
            info: ClientInfo::default(),
        },
    );
    let mut request = body_request(vec![0xc4f9186b_u32 as i32], 100);
    request.request_id = 77;
    let request = session.data.register_request(request);
    handle.send(SessionCommand::Send {
        handle: request,
        can_wait: 0,
    });
    let (transport, mut streams) = DuplexTransport::new();
    let wake = Arc::new(tokio::sync::Notify::new());
    let waker = wake.clone();
    tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
    let script = async {
        let stream = streams.recv().await.unwrap();
        let mut peer = ServerTcp::new(stream).await;
        let ping = peer.read_encrypted().await;
        peer.reply(
            ping.session_id,
            (1700000000u64 << 32) | 1,
            vec![
                0x347773c5,
                ping.msg_id as i32,
                (ping.msg_id >> 32) as i32,
                ping.body[1],
                ping.body[2],
            ],
        )
        .await;
        let first = peer.read_encrypted().await;
        assert!(contained_requests(&first)
            .iter()
            .any(|(_, body)| body[0] as u32 == 0xc4f9186b));
        drop(peer);
        loop {
            tg_runtime::process_main_queue();
            if env
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,InstanceEvent::StateChange(2,state)if *state<0))
            {
                break;
            }
            wake.notified().await;
        }
        clock.time.advance(1);
        handle.wake();
        let stream = streams.recv().await.unwrap();
        let mut peer = ServerTcp::new(stream).await;
        let retry = peer.read_encrypted().await;
        let requests = contained_requests(&retry);
        let (request_id, _) = requests
            .iter()
            .find(|(_, body)| body[0] as u32 == 0xc4f9186b)
            .unwrap();
        let request_id = *request_id;
        let response = vec![
            RPC_RESULT as i32,
            request_id as i32,
            (request_id >> 32) as i32,
            0x997275b5_u32 as i32,
        ];
        peer.reply(retry.session_id, (1700000000u64 << 32) | 5, response)
            .await;
        loop {
            tg_runtime::process_main_queue();
            if env
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,InstanceEvent::Callback(r)if r.request_id==77))
            {
                break;
            }
            wake.notified().await;
        }
        let events = env.events.lock().unwrap();
        let callbacks: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let InstanceEvent::Callback(r) = e {
                    Some(r.request_id)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(callbacks, [77]);
        drop(events);
        handle.send(SessionCommand::Kill);
    };
    let (result, ()) = tokio::join!(session.run(transport), script);
    result.unwrap();
    tg_runtime::main_queue().set_waker(None);
    tg_runtime::process_main_queue();
}

#[test]
fn source_log_version_timezone_and_protocol_dc_rules() {
    let _main = main_lock();
    assert_eq!(log_ids_vector(&[]), "[]");
    assert_eq!(log_ids_vector(&[1, 2]), "[1, 1, 2]");
    for (arch, windows, distribution, expected) in [
        ("x86_64", false, AppDistribution::Plain, "7.2.9"),
        (
            "x86_64",
            true,
            AppDistribution::WindowsStore,
            "7.2.9 x64 Microsoft Store",
        ),
        ("i386", true, AppDistribution::Plain, "7.2.9"),
        (
            "arm64",
            false,
            AppDistribution::Flatpak,
            "7.2.9 arm64 Flatpak",
        ),
        (
            "arm64",
            false,
            AppDistribution::MacStore,
            "7.2.9 arm64 Mac App Store",
        ),
        ("x86_64", false, AppDistribution::Snap, "7.2.9 Snap"),
    ] {
        assert_eq!(
            compute_app_version("7.2.9", arch, windows, distribution),
            expected
        );
    }
    let mut h = Harness::ready();
    assert_eq!(h.p.get_shifted_dc_id(), 2);
    assert_eq!(h.p.get_protocol_dc_id(), 10002);
    h.p.current_dc_type = DcType::MediaCluster;
    assert_eq!(h.p.get_protocol_dc_id(), -10002);
    h.p.current_dc_type = DcType::Regular;
    for offset in [-50000, -45000, -1, 0, 449, 450, 50500, 60000] {
        h.p.info.local_utc_offset = offset;
        let params = h.p.prepare_init_params();
        let encoded = tg_tl::serialize(&params);
        assert!(tg_tl::deserialize_exact::<tg_scheme::JSONValue>(&encoded).is_ok());
    }
    h.p.info.local_utc_offset = 450;
    let result = h.p.prepare_init_params();
    assert_eq!(
        result.c_json_object().value[0]
            .c_json_object_value()
            .value
            .c_json_number()
            .value,
        900.
    );
}

async fn server_auth_exchange(peer: &mut ServerTcp, round: u64, retry: bool) -> [u8; 256] {
    use tg_base::openssl_help::{sha1, sha256, sha256_parts, BigNum};
    use tg_scheme::*;
    use tg_tl::{Deserialize, Reader};
    let mut fixture = include_bytes!("../../tests/fixtures/test-server-rsa.bin").as_slice();
    let mut parts = vec![];
    for _ in 0..3 {
        let n = u32::from_le_bytes(fixture[..4].try_into().unwrap()) as usize;
        parts.push(fixture[4..4 + n].to_vec());
        fixture = &fixture[4 + n..];
    }
    let public =
        crate::details::mtproto_rsa_public_key::RsaPublicKey::from_components(&parts[0], &parts[1])
            .unwrap();
    let private = BigNum::from_bytes(&parts[2]);
    let request = peer.read().await;
    assert_eq!(&request[..2], &[0, 0]);
    let pq = ReqPqMulti::read(&mut Reader::new(&request[5..])).unwrap();
    let nonce = pq.nonce;
    let server_nonce = tg_tl::Int128 {
        l: 0x8877665544332211 + round,
        h: 0x123456789abcdef,
    };
    let mut server_message_id = (1700000000u64 << 32) | 1;
    async fn response(peer: &mut ServerTcp, id: &mut u64, body: Buffer) {
        let mut packet = vec![
            0,
            0,
            *id as i32,
            (*id >> 32) as i32,
            (body.len() * 4) as i32,
        ];
        packet.extend(body);
        peer.write(&packet).await;
        *id += 4;
    }
    response(
        peer,
        &mut server_message_id,
        tg_tl::serialize(&mtp_res_pq(
            nonce,
            server_nonce,
            323u16.to_be_bytes().to_vec(),
            vec![public.fingerprint()],
        )),
    )
    .await;
    let request = peer.read().await;
    let dh = ReqDHParams::read(&mut Reader::new(&request[5..])).unwrap();
    assert_eq!(dh.nonce, nonce);
    assert_eq!(dh.server_nonce, server_nonce);
    let raw = BigNum::mod_exp(
        &BigNum::from_bytes(&dh.encrypted_data),
        &private,
        &BigNum::from_bytes(&parts[0]),
    );
    let mut padded = [0; 256];
    let raw = raw.get_bytes();
    padded[256 - raw.len()..].copy_from_slice(&raw);
    let hash = sha256(&padded[32..]);
    let key: [u8; 32] = std::array::from_fn(|i| padded[i] ^ hash[i]);
    crate::mtproto_auth_key::aes_ige_decrypt_raw(&mut padded[32..], &key, &[0; 32]).unwrap();
    let mut bytes = padded[32..224].to_vec();
    bytes.reverse();
    assert_eq!(
        &padded[224..],
        sha256_parts(&[key.as_slice(), bytes.as_slice()])
    );
    let words = tg_tl::bytes_to_primes(&bytes);
    let inner = PQInnerData::read(&mut Reader::new(&words)).unwrap();
    let new_nonce = if round == 0 {
        inner.c_p_q_inner_data_dc().new_nonce
    } else {
        let fields = inner.c_p_q_inner_data_temp_dc();
        assert_eq!(fields.expires_in, 86400);
        fields.new_nonce
    };
    let nonce_bytes = tg_tl::primes_to_bytes(&tg_tl::serialize(&new_nonce));
    let server_bytes = tg_tl::primes_to_bytes(&tg_tl::serialize(&server_nonce));
    let a = sha1(&[nonce_bytes.as_slice(), &server_bytes].concat());
    let b = sha1(&[server_bytes.as_slice(), &nonce_bytes].concat());
    let c = sha1(&[nonce_bytes.as_slice(), &nonce_bytes].concat());
    let aes: [u8; 32] = [&a[..], &b[..12]].concat().try_into().unwrap();
    let iv: [u8; 32] = [&b[12..], &c[..], &nonce_bytes[..4]]
        .concat()
        .try_into()
        .unwrap();
    let prime = BigNum::from_bytes(&crate::mtproto_dh_utils::GOOD_PRIME);
    let exponent = BigNum::from_bytes(&[0x55; 256]);
    let g_a = BigNum::mod_exp(&BigNum::from_word(3), &exponent, &prime).get_bytes();
    let data = tg_tl::primes_to_bytes(&tg_tl::serialize(&mtp_server_dh_inner_data(
        nonce,
        server_nonce,
        3,
        crate::mtproto_dh_utils::GOOD_PRIME.to_vec(),
        g_a,
        1700000000,
    )));
    let mut encrypted = sha1(&data).to_vec();
    encrypted.extend(data);
    encrypted.resize(encrypted.len().div_ceil(16) * 16, 0x55);
    crate::mtproto_auth_key::aes_ige_encrypt_raw(&mut encrypted, &aes, &iv).unwrap();
    response(
        peer,
        &mut server_message_id,
        tg_tl::serialize(&mtp_server_dh_params_ok(nonce, server_nonce, encrypted)),
    )
    .await;
    for attempt in 0..2 {
        let request = peer.read().await;
        let dh = SetClientDHParams::read(&mut Reader::new(&request[5..])).unwrap();
        let mut data = dh.encrypted_data;
        crate::mtproto_auth_key::aes_ige_decrypt_raw(&mut data, &aes, &iv).unwrap();
        let words = tg_tl::bytes_to_primes(&data[20..]);
        let mut cursor = Reader::new(&words);
        let inner = ClientDHInnerData::read(&mut cursor).unwrap();
        assert_eq!(&data[..20], sha1(&data[20..20 + cursor.position() * 4]));
        let inner = inner.c_client_dh_inner_data();
        let value = BigNum::mod_exp(&BigNum::from_bytes(&inner.g_b), &exponent, &prime).get_bytes();
        let mut auth = [0; 256];
        auth[256 - value.len()..].copy_from_slice(&value);
        let aux = sha1(&auth);
        let tag = if retry && attempt == 0 { 2 } else { 1 };
        let hash = sha1(&[nonce_bytes.as_slice(), &[tag], &aux[..8]].concat());
        let digest = tg_tl::Int128 {
            l: u64::from_le_bytes(hash[4..12].try_into().unwrap()),
            h: u64::from_le_bytes(hash[12..20].try_into().unwrap()),
        };
        let result = if tag == 2 {
            mtp_dh_gen_retry(nonce, server_nonce, digest)
        } else {
            mtp_dh_gen_ok(nonce, server_nonce, digest)
        };
        response(peer, &mut server_message_id, tg_tl::serialize(&result)).await;
        if tag == 1 {
            return auth;
        }
    }
    unreachable!("two handshake attempts")
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn duplex_server_creates_persistent_and_temp_key_retries_binds_and_delivers_rpc() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand};
    let env = Env::new();
    let pem = include_bytes!("../../tests/fixtures/test-server-rsa.pub").to_vec();
    env.options.lock().unwrap().set_cdn_config(
        tg_scheme::mtp_cdn_config(vec![tg_scheme::mtp_cdn_public_key(2, pem)]).c_cdn_config(),
    );
    env.callbacks.lock().unwrap().insert(88);
    let dc = Arc::new(Dcenter::new(2, None));
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (session, handle) = Session::new(
        2,
        dc.clone(),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        crate::session::SessionParameters {
            clock,
            random: Entropy(0x24682468),
            host,
            info: ClientInfo::default(),
        },
    );
    let mut request = body_request(vec![0xc4f9186b_u32 as i32], 100);
    request.request_id = 88;
    let request = session.data.register_request(request);
    handle.send(SessionCommand::Send {
        handle: request,
        can_wait: 0,
    });
    let (transport, mut streams) = DuplexTransport::new();
    let wake = Arc::new(tokio::sync::Notify::new());
    let waker = wake.clone();
    tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
    let script = async {
        let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
        let persistent = server_auth_exchange(&mut peer, 0, false).await;
        let temporary = server_auth_exchange(&mut peer, 1, true).await;
        let mut next_id = (1700000000u64 << 32) | 101;
        let mut bound = false;
        loop {
            let mut packet = vec![0, 0];
            packet.extend(peer.read().await);
            let message = server_decrypt_with(&temporary, &packet);
            for (id, body) in contained_requests(&message) {
                let result = match body[0] as u32 {
                    0xcdd42a05 => {
                        let bind: tg_scheme::AuthBindTempAuthKey =
                            tg_tl::deserialize_exact(&body).unwrap();
                        assert_eq!(
                            bind.perm_auth_key_id,
                            u64::from_le_bytes(
                                tg_base::openssl_help::sha1(&persistent)[12..20]
                                    .try_into()
                                    .unwrap()
                            )
                        );
                        server_check_bind(&persistent, &temporary, message.session_id, id, &bind);
                        bound = true;
                        Some(vec![
                            RPC_RESULT as i32,
                            id as i32,
                            (id >> 32) as i32,
                            0x997275b5_u32 as i32,
                        ])
                    }
                    0x7abe77ec | 0xf3427b8c => Some(vec![
                        0x347773c5,
                        id as i32,
                        (id >> 32) as i32,
                        body[1],
                        body[2],
                    ]),
                    0xc4f9186b => {
                        assert!(bound);
                        Some(vec![
                            RPC_RESULT as i32,
                            id as i32,
                            (id >> 32) as i32,
                            0x997275b5_u32 as i32,
                        ])
                    }
                    _ => None,
                };
                if let Some(body) = result {
                    let reply = server_encrypt_with(
                        &temporary,
                        &Message {
                            salt: 100,
                            session_id: message.session_id,
                            msg_id: next_id,
                            seq_no: 1,
                            body,
                        },
                    );
                    next_id += 4;
                    peer.write(&reply).await;
                }
            }
            if bound && dc.get_temporary_key(TemporaryKeyType::Regular).is_some() {
                break;
            }
        }
        loop {
            tg_runtime::process_main_queue();
            if env
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,InstanceEvent::Callback(r)if r.request_id==88))
            {
                break;
            }
            tokio::select! {_=wake.notified()=>{},packet=peer.read()=>{let mut words=vec![0,0];words.extend(packet);let message=server_decrypt_with(&temporary,&words);for(id,body)in contained_requests(&message){if body[0]as u32==0xc4f9186b{let reply=server_encrypt_with(&temporary,&Message{salt:100,session_id:message.session_id,msg_id:next_id,seq_no:1,body:vec![RPC_RESULT as i32,id as i32,(id>>32)as i32,0x997275b5_u32 as i32]});next_id+=4;peer.write(&reply).await;}}}}
        }
        assert_eq!(dc.get_persistent_key().unwrap().data(), &persistent);
        assert_eq!(
            dc.get_temporary_key(TemporaryKeyType::Regular)
                .unwrap()
                .data(),
            &temporary
        );
        assert_eq!(
            dc.get_temporary_key(TemporaryKeyType::Regular)
                .unwrap()
                .expires_at(),
            1700086430
        );
        handle.send(SessionCommand::Kill);
    };
    let (result, ()) = tokio::join!(session.run(transport), script);
    result.unwrap();
    tg_runtime::process_main_queue();
    tg_runtime::main_queue().set_waker(None);
}

#[test]
fn session_data_options_keys_and_notifications() {
    let _main = main_lock();
    let env = Env::new();
    let dc = Arc::new(Dcenter::new(2, None));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let data = SessionData::new(
        2,
        dc.clone(),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    assert!(data.options().use_tcp);
    data.refresh_options();
    assert_eq!(data.options().system_lang_code, "en");
    assert!(!data.options().use_ipv6);
    data.queue_try_to_receive();
    data.queue_need_to_resume_and_send();
    data.queue_connection_state_change(2);
    data.queue_reset_done();
    data.queue_send_anything(5);
    for _ in 0..5 {
        rx.try_recv().unwrap();
    }
    assert!(rx.try_recv().is_err());
    assert_eq!(
        data.acquire_key_creation(DcType::Regular),
        CreatingKeyType::Persistent
    );
    data.release_key_creation_on_fail();
    assert_eq!(
        data.acquire_key_creation(DcType::Regular),
        CreatingKeyType::Persistent
    );
    let permanent = Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1));
    assert!(data.release_key_creation_on_done(auth(), permanent.clone()));
    assert!(Arc::ptr_eq(&data.get_persistent_key().unwrap(), &permanent));
    data.notify_connection_inited(&SessionOptions {
        cloud_lang_code: "different".into(),
        ..SessionOptions::default()
    });
    assert!(data.connection_inited());
    data.destroy_temporary_key(auth().key_id());
    assert!(data.get_temporary_key(TemporaryKeyType::Regular).is_none());
    assert_eq!(
        data.acquire_key_creation(DcType::MediaCluster),
        CreatingKeyType::TemporaryRegular
    );
    data.release_key_creation_on_fail();
    dispatch_instances(&mut rx, env.clone());
    data.detach();
    data.queue_reset_done();
    assert!(rx.try_recv().is_err());
    assert!(!data.connection_inited());
    assert!(data.get_persistent_key().is_none());
    assert!(data.get_temporary_key(TemporaryKeyType::Regular).is_none());
    assert_eq!(
        data.acquire_key_creation(DcType::Regular),
        CreatingKeyType::None
    );
    assert!(!data.release_key_creation_on_done(auth(), permanent));
    let dc = Arc::new(Dcenter::new(9, None));
    let (tx, _) = mpsc::unbounded_channel();
    let cdn = SessionData::new(
        9,
        dc,
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    assert_eq!(
        cdn.acquire_key_creation(DcType::Cdn),
        CreatingKeyType::TemporaryRegular
    );
    assert!(cdn.release_cdn_key_creation_on_done(auth()));
    assert!(cdn.get_persistent_key().is_none());
    assert!(cdn.get_temporary_key(TemporaryKeyType::Regular).is_some());
    use crate::session::{Session, SessionParameters};
    let dc = Arc::new(Dcenter::new(2, None));
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, _) = Session::new(
        2,
        dc,
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        SessionParameters {
            clock,
            random: Entropy(55),
            host,
            info: ClientInfo::default(),
        },
    );
    session.stop();
    assert_eq!(
        session.acquire_key_creation(DcType::Regular),
        CreatingKeyType::Persistent
    );
    session.release_key_creation_on_fail();
    let permanent = Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1));
    assert_eq!(
        session.acquire_key_creation(DcType::Regular),
        CreatingKeyType::Persistent
    );
    assert!(session.release_key_creation_on_done(auth(), permanent));
    session.destroy_temporary_key(auth().key_id());
    assert_eq!(
        session.acquire_key_creation(DcType::Cdn),
        CreatingKeyType::TemporaryRegular
    );
    session.release_key_creation_on_fail();
    session.process();
    dispatch_instances(&mut rx, env.clone());
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::DcPersistentKeyChanged(2, _))));
}
#[test]
fn key_creation_variants_cdn_and_data_owner_contract() {
    let _main = main_lock();
    use crate::mtproto_dc_options::{CDN, MEDIA_ONLY};
    let mut h = Harness::ready();
    assert!(!h.p.real_dc_type_changed());
    h.p.real_dc_type = DcType::Cdn;
    assert!(h.p.real_dc_type_changed());
    h.env.options.lock().unwrap().construct_add_one(Endpoint {
        id: 2,
        flags: MEDIA_ONLY,
        ip: "192.0.2.1".into(),
        port: 443,
        secret: vec![],
    });
    let (tx, _rx) = mpsc::unbounded_channel();
    let data = SessionData::new(
        160002,
        h.p.data.dc.clone(),
        h.env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    data.refresh_options();
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let mut media = SessionPrivate::new(data, clock, Entropy(44), host, ClientInfo::default());
    assert_eq!(media.real_dc_type, DcType::MediaCluster);
    assert!(media.no_media_key_with_existing_regular_key());
    assert_eq!(media.try_acquire_key_creation(), DcType::MediaCluster);
    let id = *media.candidates.keys().next().unwrap();
    media.on_connected(id, true, "media".into(), 8000);
    assert!(media.key_creator.as_ref().unwrap().creator.is_some());
    media.key_creation_failed(DcKeyError::Invalid("test refusal"));
    assert!(media.get_state() < 0);
    let env = Env::new();
    env.options.lock().unwrap().construct_add_one(Endpoint {
        id: 9,
        flags: CDN,
        ip: "192.0.2.9".into(),
        port: 443,
        secret: vec![],
    });
    let (tx, mut rx) = mpsc::unbounded_channel();
    let data = SessionData::new(
        9,
        Arc::new(Dcenter::new(9, None)),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    data.refresh_options();
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let mut cdn = SessionPrivate::new(data, clock, Entropy(33), host, ClientInfo::default());
    assert!(cdn.candidates.is_empty());
    dispatch_instances(&mut rx, env.clone());
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::RequestCdnConfig)));
    cdn.key_creation_failed(DcKeyError::UnknownPublicKey);
    dispatch_instances(&mut rx, env.clone());
    tg_runtime::process_main_queue();
    assert!(cdn.key_creator.is_none());
    let mut env = Env::new();
    Arc::get_mut(&mut env).unwrap().destroyer = true;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let persistent = Arc::new(AuthKey::new(KeyType::ReadFromFile, 2, [2; 256], 0));
    let data = SessionData::new(
        2,
        Arc::new(Dcenter::new(2, Some(persistent.clone()))),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    data.refresh_options();
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let mut destroyer = SessionPrivate::new(data, clock, Entropy(22), host, ClientInfo::default());
    let id = *destroyer.candidates.keys().next().unwrap();
    destroyer.on_connected(id, true, "destroyer".into(), 8000);
    assert_eq!(destroyer.key_id, persistent.key_id());
    destroyer.update_auth_key();
    destroyer.destroy_temporary_key();
    dispatch_instances(&mut rx, env.clone());
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::KeyWasPossiblyDestroyed(2))));
}
#[test]
fn identity_replacement_special_ids_rpc_gzip_and_bad_time_paths() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let (handle, id) = h.request(1);
    h.sent();
    let sid = h.p.session_id();
    h.p.set_current_key_id(h.p.key_id);
    assert_eq!(h.p.session_id(), sid);
    h.p.bind_msg_id = 123;
    h.p.clear_special_msg_id(123);
    assert_eq!(h.p.bind_msg_id, 0);
    let ping = h.p.ping_msg_id;
    h.p.clear_special_msg_id(ping);
    assert_eq!(h.p.ping_id, 0);
    assert_eq!(h.p.ping_msg_id, 0);
    let new = h.p.replace_msg_id(handle, id + 400);
    assert_eq!(new, id + 400);
    assert!(h.p.data.queues.lock().unwrap().have_sent.contains_key(&new));
    assert_eq!(
        h.p.data
            .store
            .lock()
            .unwrap()
            .get(handle)
            .unwrap()
            .get_msg_id(),
        new
    );
    let mut body = vec![
        RPC_RESULT as i32,
        new as i32,
        (new >> 32) as i32,
        GZIP_PACKED as i32,
    ];
    tg_tl::write_string(&mut body, include_bytes!("../../tests/fixtures/true.gz"));
    h.reply(body, 100, 1);
    assert_eq!(
        h.p.data
            .queues
            .lock()
            .unwrap()
            .received
            .back()
            .unwrap()
            .reply,
        [0x997275b5_u32 as i32]
    );
    let (_, id) = h.request(2);
    h.sent();
    assert_eq!(
        h.handle(&[0xedab447b_u32 as i32, 999, 0, 1, 48, 2, 0], true),
        HandleResult::Ignored
    );
    assert_eq!(
        h.handle(
            &[0xa7eff811_u32 as i32, id as i32, (id >> 32) as i32, 1, 32],
            true
        ),
        HandleResult::ResetSession
    );
    h.p.bind_msg_id = 50;
    assert_eq!(
        h.handle(
            &[0xa7eff811_u32 as i32, id as i32, (id >> 32) as i32, 1, 32],
            false
        ),
        HandleResult::RestartConnection
    );
    assert_eq!(
        h.handle(
            &[0xa7eff811_u32 as i32, id as i32, (id >> 32) as i32, 1, 18],
            false
        ),
        HandleResult::RestartConnection
    );
    assert_eq!(
        h.handle(
            &[
                0xedab447b_u32 as i32,
                id as i32,
                (id >> 32) as i32,
                1,
                48,
                2,
                0
            ],
            false
        ),
        HandleResult::RestartConnection
    );
    h.p.bind_msg_id = 0;
    assert_eq!(
        h.handle(&[0x347773c5, 999, 0, 3, 0], false),
        HandleResult::Ignored
    );
    for constructor in [0xda69fb52_u32, 0x7d861a08, 0xae500895] {
        let mut body = vec![constructor as i32];
        body.extend([0x1cb5c415, 0]);
        assert_eq!(h.handle(&body, false), HandleResult::Success);
    }
    h.p.current_dc_type = DcType::Cdn;
    let received = h.p.data.queues.lock().unwrap().received.len();
    assert_eq!(
        h.handle(&[0x997275b5_u32 as i32], false),
        HandleResult::Success
    );
    assert_eq!(h.p.data.queues.lock().unwrap().received.len(), received);
}

fn server_check_bind(
    persistent: &[u8; 256],
    temporary: &[u8; 256],
    session: u64,
    message_id: u64,
    bind: &tg_scheme::AuthBindTempAuthKey,
) {
    use aes::cipher::{BlockModeDecrypt, KeyIvInit};
    use tg_base::openssl_help::sha1;
    let encrypted = &bind.encrypted_message;
    let mk = &encrypted[8..24];
    let a = sha1(&[mk, &persistent[..32]].concat());
    let b = sha1(&[&persistent[32..48], mk, &persistent[48..64]].concat());
    let c = sha1(&[&persistent[64..96], mk].concat());
    let d = sha1(&[mk, &persistent[96..128]].concat());
    let key: [u8; 32] = [&a[..8], &b[8..20], &c[4..16]].concat().try_into().unwrap();
    let iv: [u8; 32] = [&a[8..20], &b[..8], &c[16..20], &d[..8]]
        .concat()
        .try_into()
        .unwrap();
    let mut plain = encrypted[24..].to_vec();
    let mut cipher = ige::Decryptor::<aes::Aes256>::new((&key).into(), (&iv).into());
    for block in plain.chunks_exact_mut(16) {
        cipher.decrypt_block(block.try_into().unwrap());
    }
    let length = u32::from_le_bytes(plain[28..32].try_into().unwrap()) as usize;
    assert_eq!(&sha1(&plain[..32 + length])[4..20], mk);
    assert_eq!(
        u64::from_le_bytes(plain[16..24].try_into().unwrap()),
        message_id
    );
    let inner: tg_scheme::BindAuthKeyInner =
        tg_tl::deserialize_exact(&tg_tl::bytes_to_primes(&plain[32..32 + length])).unwrap();
    let inner = inner.c_bind_auth_key_inner();
    assert_eq!(inner.nonce, bind.nonce);
    assert_eq!(inner.temp_session_id, session);
    assert_eq!(
        inner.temp_auth_key_id,
        u64::from_le_bytes(sha1(temporary)[12..20].try_into().unwrap())
    );
    assert_eq!(inner.perm_auth_key_id, bind.perm_auth_key_id);
    assert_eq!(inner.expires_at, bind.expires_at);
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn duplex_server_cdn_key_without_permanent_binding() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand, SessionParameters};
    let env = Env::new();
    {
        let mut options = env.options.lock().unwrap();
        options.construct_add_one(Endpoint {
            id: 9,
            flags: crate::mtproto_dc_options::CDN,
            ip: "192.0.2.9".into(),
            port: 443,
            secret: vec![],
        });
        options.set_cdn_config(
            tg_scheme::mtp_cdn_config(vec![tg_scheme::mtp_cdn_public_key(
                9,
                include_bytes!("../../tests/fixtures/test-server-rsa.pub").to_vec(),
            )])
            .c_cdn_config(),
        );
    }
    env.callbacks.lock().unwrap().insert(99);
    let dc = Arc::new(Dcenter::new(9, None));
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (session, handle) = Session::new(
        9,
        dc.clone(),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        SessionParameters {
            clock,
            random: Entropy(0x998877),
            host,
            info: ClientInfo::default(),
        },
    );
    let mut request = body_request(vec![0xc4f9186b_u32 as i32], 100);
    request.request_id = 99;
    let request = session.data.register_request(request);
    handle.send(SessionCommand::Send {
        handle: request,
        can_wait: 0,
    });
    let (transport, mut streams) = DuplexTransport::new();
    let wake = Arc::new(tokio::sync::Notify::new());
    let waker = wake.clone();
    tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
    let script = async {
        let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
        let key = server_auth_exchange(&mut peer, 1, false).await;
        let mut next_id = (1700000000u64 << 32) | 201;
        let mut answered = false;
        while !answered {
            let mut packet = vec![0, 0];
            packet.extend(peer.read().await);
            let message = server_decrypt_with(&key, &packet);
            for (id, body) in contained_requests(&message) {
                assert_ne!(body[0] as u32, 0xcdd42a05);
                let reply = match body[0] as u32 {
                    0x7abe77ec | 0xf3427b8c => Some(vec![
                        0x347773c5,
                        id as i32,
                        (id >> 32) as i32,
                        body[1],
                        body[2],
                    ]),
                    0xc4f9186b => {
                        answered = true;
                        Some(vec![
                            RPC_RESULT as i32,
                            id as i32,
                            (id >> 32) as i32,
                            0x997275b5_u32 as i32,
                        ])
                    }
                    _ => None,
                };
                if let Some(body) = reply {
                    let packet = server_encrypt_with(
                        &key,
                        &Message {
                            salt: 100,
                            session_id: message.session_id,
                            msg_id: next_id,
                            seq_no: 1,
                            body,
                        },
                    );
                    next_id += 4;
                    peer.write(&packet).await;
                }
            }
        }
        loop {
            tg_runtime::process_main_queue();
            if env
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e,InstanceEvent::Callback(r)if r.request_id==99))
            {
                break;
            }
            wake.notified().await;
        }
        assert!(dc.get_persistent_key().is_none());
        assert_eq!(
            dc.get_temporary_key(TemporaryKeyType::Regular)
                .unwrap()
                .data(),
            &key
        );
        handle.send(SessionCommand::Kill);
    };
    let (result, ()) = tokio::join!(session.run(transport), script);
    result.unwrap();
    tg_runtime::process_main_queue();
    tg_runtime::main_queue().set_waker(None);
}

fn dispatch_instances(receiver: &mut mpsc::UnboundedReceiver<MainCommand>, environment: Arc<Env>) {
    while let Ok(command) = receiver.try_recv() {
        if let MainCommand::Instance(event) = command {
            crate::session::emit_on_main(environment.clone(), event);
        }
    }
}

#[test]
fn remaining_owner_proxy_timeout_and_clock_contracts() {
    let _main = main_lock();
    use crate::session::{Session, SessionParameters};
    let env = Env::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let dc = Arc::new(Dcenter::new(9, None));
    let data = SessionData::new(
        9,
        dc.clone(),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        tx,
    );
    for kind in [
        Type::None,
        Type::Socks5,
        Type::Http,
        Type::Mtproto,
        Type::Web,
    ] {
        *env.proxy.lock().unwrap() = ProxySettings {
            selected: crate::mtproto_proxy_data::ProxyData {
                kind,
                host: "proxy.example.com".into(),
                port: 1080,
                ..Default::default()
            },
            enabled: true,
            try_ipv6: true,
        };
        data.refresh_options();
        let options = data.options();
        assert_eq!(options.use_tcp, kind != Type::Http);
        assert_eq!(options.use_http, !matches!(kind, Type::Mtproto | Type::Web));
        assert!(options.use_ipv4 && options.use_ipv6);
        assert_eq!(options.proxy.kind, kind);
    }
    *env.proxy.lock().unwrap() = ProxySettings::default();
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, _) = Session::new(
        9,
        dc,
        env.clone(),
        data.store.clone(),
        SessionParameters {
            clock,
            random: Entropy(79),
            host,
            info: ClientInfo::default(),
        },
    );
    session.stop();
    assert_eq!(
        session.acquire_key_creation(DcType::Cdn),
        CreatingKeyType::TemporaryRegular
    );
    assert!(session.release_cdn_key_creation_on_done(auth()));
    session.process();
    dispatch_instances(&mut rx, env.clone());
    tg_runtime::process_main_queue();
    assert!(env
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstanceEvent::DcTemporaryKeyChanged(9))));
    let mut h = Harness::ready();
    h.p.info.local_utc_offset = 3600;
    h.p.clock.unix.store(1700000900, Ordering::SeqCst);
    let params = h.p.prepare_init_params();
    assert_eq!(
        params.c_json_object().value[0]
            .c_json_object_value()
            .value
            .c_json_number()
            .value,
        2700.
    );
    h.p.clock.unix.store(1700000000, Ordering::SeqCst);
    h.p.do_disconnect();
    h.p.connect_to_server(false);
    let id = *h.p.candidates.keys().next().unwrap();
    h.p.update_connect_timeout(id, 12000);
    h.p.wait_connected = 8000;
    h.p.wait_connected_failed();
    assert_eq!(h.p.wait_connected, 12000);
    let mut h = Harness::ready();
    let id = h.p.key_id;
    assert_eq!(
        h.p.data.dc.acquire_key_creation(DcType::Regular),
        CreatingKeyType::None
    );
    h.p.set_current_key_id(id);
    assert_eq!(h.p.key_id, id);
    h.p.on_error(h.connection, -429);
    assert!(h.p.get_state() < 0);
}

#[derive(Default)]
struct ChangeBus(Mutex<Vec<mpsc::UnboundedSender<EnvironmentChange>>>);
impl ChangeBus {
    fn subscribe(&self) -> mpsc::UnboundedReceiver<EnvironmentChange> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.0.lock().unwrap().push(tx);
        rx
    }
    fn send(
        &self,
        event: EnvironmentChange,
    ) -> Result<usize, mpsc::error::SendError<EnvironmentChange>> {
        let mut subscribers = self.0.lock().unwrap();
        subscribers.retain(|sender| sender.send(event.clone()).is_ok());
        if subscribers.is_empty() {
            Err(mpsc::error::SendError(event))
        } else {
            Ok(subscribers.len())
        }
    }
}

#[test]
fn bounded_ack_cache_service_ownership_and_registered_preferences() {
    let _main = main_lock();
    let mut h = Harness::ready();
    let mut ids = vec![];
    for id in 1..=401 {
        let mut request = body_request(vec![1], 100);
        request.request_id = id;
        request.set_msg_id((1700000000u64 << 32) + id as u64 * 4);
        let handle = h.p.data.register_request(request);
        let msg = (1700000000u64 << 32) + id as u64 * 4;
        h.p.data
            .queues
            .lock()
            .unwrap()
            .have_sent
            .insert(msg, handle);
        ids.push(msg);
    }
    h.p.requests_acked(&ids, false);
    assert_eq!(h.p.acked.len(), 400);
    assert!(!h.p.acked.contains_key(&ids[0]));
    h.p.state_ids.insert(ids[400]);
    h.p.try_to_send();
    let handle = *h.p.state_requests.values().next().unwrap();
    let store = h.p.data.store.clone();
    drop(h.p);
    assert!(store.lock().unwrap().get(handle).is_none());
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock));
    let registry = tg_base::options::Registry::new(&host);
    let option = register_options(&registry);
    assert!(!option.value());
    assert_eq!(option.id(), "prefer-ipv6");
    assert!(option
        .description()
        .contains("\"Try connecting through IPv6\""));
    option.set(true);
    assert!(registry.lookup::<bool>(OPTION_PREFER_IPV6).value());
    let env = Env::new();
    let (mut session, handle) = crate::session::Session::new(
        2,
        Arc::new(Dcenter::new(2, None)),
        env.clone(),
        Arc::new(Mutex::new(RequestStore::default())),
        crate::session::SessionParameters {
            clock: TestClock::new(),
            random: Entropy(4),
            host: host.clone(),
            info: ClientInfo::default(),
        },
    );
    session.take_transport_actions();
    session.stop();
    handle.send(crate::session::SessionCommand::ClientInfo(ClientInfo {
        prefer_ipv6: option.value(),
        ..ClientInfo::default()
    }));
    session.process();
    session.need_to_resume_and_send();
    let actions = session.take_transport_actions();
    let ips = actions
        .into_iter()
        .filter_map(|action| {
            if let TransportAction::Connect(spec) = action {
                Some(spec)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(!ips.is_empty());
    let mut receiver = env.subscribe();
    for id in 0..128 {
        env.changes
            .send(EnvironmentChange::TemporaryKey(id))
            .unwrap();
    }
    for id in 0..128 {
        assert!(
            matches!(receiver.try_recv(),Ok(EnvironmentChange::TemporaryKey(value))if value==id)
        );
    }
    session.process();
    assert_eq!(session.get_state(), CONNECTING);
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn duplex_server_bad_messages_salt_new_session_gzip_and_containers() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand, SessionParameters};
    for scenario in [
        16, 17, 18, 19, 20, 32, 33, 34, 35, 48, 64, 999, 1000, 1001, 1002, 1003,
    ] {
        let env = Env::new();
        env.callbacks.lock().unwrap().insert(77);
        let dc = Arc::new(Dcenter::new(2, None));
        let creation = dc.acquire_key_creation(DcType::Regular);
        dc.release_key_creation_on_done(
            creation,
            auth(),
            Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1))),
        );
        let clock = TestClock::new();
        let host = TimerHost::new(Arc::new(clock.time.clone()));
        let (session, handle) = Session::new(
            2,
            dc,
            env.clone(),
            Arc::new(Mutex::new(RequestStore::default())),
            SessionParameters {
                clock: clock.clone(),
                random: Entropy(0x2468),
                host,
                info: ClientInfo::default(),
            },
        );
        let mut request = body_request(vec![0xc4f9186b_u32 as i32], 100);
        request.request_id = 77;
        request.force_send_in_container = true;
        let request = session.data.register_request(request);
        handle.send(SessionCommand::Send {
            handle: request,
            can_wait: 0,
        });
        let (transport, mut streams) = DuplexTransport::new();
        let wake = Arc::new(tokio::sync::Notify::new());
        let waker = wake.clone();
        tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
        let script = async {
            let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
            let ping = peer.read_encrypted().await;
            peer.reply(
                ping.session_id,
                (1700000000u64 << 32) | 1,
                vec![
                    0x347773c5,
                    ping.msg_id as i32,
                    (ping.msg_id >> 32) as i32,
                    ping.body[1],
                    ping.body[2],
                ],
            )
            .await;
            let message = peer.read_encrypted().await;
            let (id, _) = contained_requests(&message)
                .into_iter()
                .find(|(_, body)| body[0] as u32 == 0xc4f9186b)
                .unwrap();
            let bad_id = if scenario == 64 { message.msg_id } else { id };
            let salt = if scenario == 1000 { 200 } else { 100 };
            let direct = matches!(scenario, 1002 | 1003);
            let fatal = !matches!(scenario, 16 | 17 | 32 | 33 | 64 | 1000 | 1001 | 1002 | 1003);
            let mut body = if scenario == 1000 {
                vec![
                    0xedab447b_u32 as i32,
                    id as i32,
                    (id >> 32) as i32,
                    3,
                    48,
                    salt as i32,
                    0,
                ]
            } else if scenario == 1001 {
                vec![
                    0x9ec20908_u32 as i32,
                    (id + 4) as i32,
                    ((id + 4) >> 32) as i32,
                    7,
                    0,
                    100,
                    0,
                ]
            } else if direct {
                let mut reply = vec![RPC_RESULT as i32, id as i32, (id >> 32) as i32];
                if scenario == 1002 {
                    reply.push(GZIP_PACKED as i32);
                    tg_tl::write_string(&mut reply, include_bytes!("../../tests/fixtures/true.gz"));
                } else {
                    reply.push(0x997275b5_u32 as i32);
                    let mut outer = vec![
                        MSG_CONTAINER as i32,
                        1,
                        33,
                        1700000000,
                        1,
                        (reply.len() * 4) as i32,
                    ];
                    outer.extend(reply);
                    reply = outer;
                }
                reply
            } else {
                vec![
                    0xa7eff811_u32 as i32,
                    bad_id as i32,
                    (bad_id >> 32) as i32,
                    3,
                    scenario,
                ]
            };
            peer.reply(
                message.session_id,
                (1700000000u64 << 32) | 101,
                std::mem::take(&mut body),
            )
            .await;
            if !fatal && !direct {
                if matches!(scenario, 32 | 33) {
                    loop {
                        tg_runtime::process_main_queue();
                        if env
                            .events
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|e| matches!(e,InstanceEvent::StateChange(2,state)if *state<0))
                        {
                            break;
                        }
                        wake.notified().await;
                    }
                    clock.time.advance(1);
                    handle.wake();
                    peer = ServerTcp::new(streams.recv().await.unwrap()).await;
                } else if scenario == 1001 {
                    loop {
                        tg_runtime::process_main_queue();
                        if env.events.lock().unwrap().iter().any(|e|matches!(e,InstanceEvent::Update(r)if r.reply.first().is_some_and(|id|*id as u32==0x9ec20908))){break;}
                        wake.notified().await;
                    }
                    clock.time.advance(10);
                    handle.wake();
                }
                let resent = peer.read_encrypted().await;
                assert_eq!(resent.salt, salt);
                if matches!(scenario, 32 | 33) {
                    assert_ne!(resent.session_id, message.session_id);
                }
                let (retry_id, _) = contained_requests(&resent)
                    .into_iter()
                    .find(|(_, body)| body[0] as u32 == 0xc4f9186b)
                    .unwrap();
                let packet = server_encrypt(&Message {
                    salt,
                    session_id: resent.session_id,
                    msg_id: (1700000000u64 << 32) | 105,
                    seq_no: 1,
                    body: vec![
                        RPC_RESULT as i32,
                        retry_id as i32,
                        (retry_id >> 32) as i32,
                        0x997275b5_u32 as i32,
                    ],
                });
                peer.write(&packet).await;
            }
            loop {
                tg_runtime::process_main_queue();
                if env
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|e| matches!(e,InstanceEvent::Callback(r)if r.request_id==77))
                {
                    break;
                }
                wake.notified().await;
            }
            let events = env.events.lock().unwrap();
            let callbacks: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let InstanceEvent::Callback(r) = e {
                        Some(r)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(callbacks.len(), 1, "scenario {scenario}");
            assert_eq!(
                callbacks[0].reply[0] as u32,
                if fatal { 0x2144ca19 } else { 0x997275b5 }
            );
            drop(events);
            handle.send(SessionCommand::Kill);
        };
        let (result, ()) = tokio::join!(session.run(transport), script);
        result.unwrap();
        tg_runtime::process_main_queue();
        tg_runtime::main_queue().set_waker(None);
    }
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn duplex_server_state_requests_resend_requests_and_acks() {
    let _main = main_lock();
    use crate::session::{Session, SessionCommand, SessionParameters};
    for resend_answer in [false, true] {
        let env = Env::new();
        env.callbacks.lock().unwrap().insert(77);
        let dc = Arc::new(Dcenter::new(2, None));
        let creation = dc.acquire_key_creation(DcType::Regular);
        dc.release_key_creation_on_done(
            creation,
            auth(),
            Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1))),
        );
        let clock = TestClock::new();
        let host = TimerHost::new(Arc::new(clock.time.clone()));
        let (session, handle) = Session::new(
            2,
            dc,
            env.clone(),
            Arc::new(Mutex::new(RequestStore::default())),
            SessionParameters {
                clock: clock.clone(),
                random: Entropy(0x2468),
                host,
                info: ClientInfo::default(),
            },
        );
        let mut request = body_request(vec![0xc4f9186b_u32 as i32], 100);
        request.request_id = 77;
        request.force_send_in_container = true;
        let request = session.data.register_request(request);
        handle.send(SessionCommand::Send {
            handle: request,
            can_wait: 0,
        });
        let (transport, mut streams) = DuplexTransport::new();
        let wake = Arc::new(tokio::sync::Notify::new());
        let waker = wake.clone();
        tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
        fn marker(first: Buffer, low: i32) -> Buffer {
            let mut body = vec![
                MSG_CONTAINER as i32,
                2,
                low,
                1700000000,
                0,
                (first.len() * 4) as i32,
            ];
            body.extend(first);
            body.extend([low + 4, 1700000000, 0, 4, 0x997275b5_u32 as i32]);
            body
        }
        let script = async {
            let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
            let ping = peer.read_encrypted().await;
            peer.reply(
                ping.session_id,
                (1700000000u64 << 32) | 1,
                vec![
                    0x347773c5,
                    ping.msg_id as i32,
                    (ping.msg_id >> 32) as i32,
                    ping.body[1],
                    ping.body[2],
                ],
            )
            .await;
            let message = peer.read_encrypted().await;
            let (id, _) = contained_requests(&message)
                .into_iter()
                .find(|(_, body)| body[0] as u32 == 0xc4f9186b)
                .unwrap();
            let answer = (1700000000u64 << 32) | 901;
            let first = if resend_answer {
                vec![
                    0x276d3ec6,
                    id as i32,
                    (id >> 32) as i32,
                    answer as i32,
                    (answer >> 32) as i32,
                    4,
                    0,
                ]
            } else {
                id_vector(0x62d6b459, &[id])
            };
            peer.reply(
                message.session_id,
                (1700000000u64 << 32) | 101,
                marker(first, 33),
            )
            .await;
            loop {
                tg_runtime::process_main_queue();
                if env
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|e| matches!(e, InstanceEvent::Update(_)))
                {
                    break;
                }
                wake.notified().await;
            }
            clock.time.advance(10000);
            handle.wake();
            let state = peer.read_encrypted().await;
            let records = contained_requests(&state);
            let constructor = if resend_answer {
                0x7d861a08
            } else {
                0xda69fb52
            };
            let (state_id, body) = records
                .iter()
                .find(|(_, body)| body[0] as u32 == constructor)
                .unwrap();
            let mut cursor = Cursor::new(&body[1..]);
            assert_eq!(
                cursor.ids().unwrap(),
                [if resend_answer { answer } else { id }]
            );
            let final_id = if resend_answer {
                answer
            } else {
                (1700000000u64 << 32) | 109
            };
            let mut rpc_id = id;
            if !resend_answer {
                let mut info = vec![0x04deb57d, *state_id as i32, (*state_id >> 32) as i32];
                tg_tl::write_string(&mut info, &[3]);
                peer.reply(
                    message.session_id,
                    (1700000000u64 << 32) | 105,
                    marker(info, 41),
                )
                .await;
                loop {
                    tg_runtime::process_main_queue();
                    if env
                        .events
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|e| matches!(e, InstanceEvent::Update(_)))
                        .count()
                        >= 2
                    {
                        break;
                    }
                    wake.notified().await;
                }
                clock.time.advance(10);
                handle.wake();
                let retry = peer.read_encrypted().await;
                rpc_id = contained_requests(&retry)
                    .into_iter()
                    .find(|(_, body)| body[0] as u32 == 0xc4f9186b)
                    .unwrap()
                    .0;
            }
            peer.reply(
                message.session_id,
                final_id,
                vec![
                    RPC_RESULT as i32,
                    rpc_id as i32,
                    (rpc_id >> 32) as i32,
                    0x997275b5_u32 as i32,
                ],
            )
            .await;
            loop {
                tg_runtime::process_main_queue();
                if env
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|e| matches!(e,InstanceEvent::Callback(r)if r.request_id==77))
                {
                    break;
                }
                wake.notified().await;
            }
            handle.send(SessionCommand::Kill);
        };
        let (result, ()) = tokio::join!(session.run(transport), script);
        result.unwrap();
        tg_runtime::process_main_queue();
        tg_runtime::main_queue().set_waker(None);
    }
}
