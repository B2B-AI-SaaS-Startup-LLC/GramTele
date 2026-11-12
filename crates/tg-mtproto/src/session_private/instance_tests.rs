use super::*;
use crate::{
    mtp_instance::{Fields, Instance, Mode},
    sender::Sender,
};
#[tokio::test(flavor = "current_thread")]
async fn instance_two_dcs_use_real_sessions_and_independent_tcp_server() {
    multi_dc_scenario(false).await;
}
#[tokio::test(flavor = "current_thread")]
async fn guest_authorization_crosses_two_real_sessions_in_memory() {
    multi_dc_scenario(true).await;
}
#[allow(clippy::await_holding_lock)]
async fn multi_dc_scenario(guest_auth: bool) {
    let _main = main_lock();
    tokio::task::LocalSet::new()
        .run_until(async {
            let instance = Instance::new(
                Mode::Normal,
                Fields::new(crate::mtproto_dc_options::Environment::Test),
            );
            instance.start();
            let sender = Sender::new(instance.clone());
            let replies = Arc::new(Mutex::new(Vec::new()));
            for dc in [2, 1] {
                let replies = replies.clone();
                sender
                    .request(tg_scheme::HelpGetNearestDc::new())
                    .to_dc(dc)
                    .done(move |result| replies.lock().unwrap().push(result.c_nearest_dc().this_dc))
                    .send();
            }
            let clock = TestClock::new();
            let timer_host = TimerHost::new(Arc::new(clock.time.clone()));
            let instance_timers =
                crate::mtp_instance::InstanceTimers::new(instance.clone(), timer_host);
            let wake = Arc::new(tokio::sync::Notify::new());
            let waker = wake.clone();
            tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
            let mut sessions = Vec::new();
            let mut servers = Vec::new();
            for spawn in instance.take_session_spawns() {
                let dc = spawn.id;
                let creating = spawn.dc.acquire_key_creation(DcType::Regular);
                assert!(spawn.dc.release_key_creation_on_done(
                    creating,
                    auth(),
                    Some(Arc::new(AuthKey::new(KeyType::Generated, dc, [3; 256], 1)))
                ));
                let host = TimerHost::new(Arc::new(clock.time.clone()));
                let (session, handle) = crate::session::Session::new(
                    dc,
                    spawn.dc,
                    instance.session_environment(),
                    instance.store.clone(),
                    crate::session::SessionParameters {
                        clock: clock.clone(),
                        random: Entropy(dc as u64 + 123),
                        host,
                        info: ClientInfo::default(),
                    },
                );
                instance.attach_session(dc, handle);
                let (transport, mut streams) = DuplexTransport::new();
                sessions.push(tokio::task::spawn_local(session.run(transport)));
                servers.push(tokio::task::spawn_local(async move {
                    let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
                    let mut counter = 1;
                    let mut attempts = 0;
                    let mut authorized = false;
                    loop {
                        let message = peer.read_encrypted().await;
                        for (id, body) in contained_requests(&message) {
                            let response =
                                if matches!(body[0] as u32, 0x347773c5 | 0xf3427b8c | 0x7abe77ec) {
                                    vec![0x347773c5, id as i32, (id >> 32) as i32, body[1], body[2]]
                                } else if body.contains(&(tg_scheme::HelpGetNearestDc::ID as i32)) {
                                    let mut reply =
                                        vec![RPC_RESULT as i32, id as i32, (id >> 32) as i32];
                                    attempts += 1;
                                    if guest_auth && dc == 1 && !authorized {
                                        reply.extend(tg_tl::serialize(&tg_scheme::mtp_rpc_error(
                                            401,
                                            b"AUTH_KEY_UNREGISTERED".to_vec(),
                                        )));
                                    } else if !guest_auth && dc == 1 {
                                        reply.extend(tg_tl::serialize(&tg_scheme::mtp_rpc_error(
                                            303,
                                            b"NETWORK_MIGRATE_2".to_vec(),
                                        )));
                                    } else if !guest_auth && attempts == 1 {
                                        reply.extend(tg_tl::serialize(&tg_scheme::mtp_rpc_error(
                                            420,
                                            b"FLOOD_WAIT_0".to_vec(),
                                        )));
                                    } else {
                                        reply.extend(tg_tl::serialize(&tg_scheme::mtp_nearest_dc(
                                            b"XX".to_vec(),
                                            dc,
                                            dc,
                                        )));
                                    }
                                    reply
                                } else if body
                                    .contains(&(tg_scheme::AuthExportAuthorization::ID as i32))
                                {
                                    assert_eq!(dc, 2);
                                    assert!(body.ends_with(&tg_tl::serialize(
                                        &tg_scheme::AuthExportAuthorization::new(1)
                                    )));
                                    let mut reply =
                                        vec![RPC_RESULT as i32, id as i32, (id >> 32) as i32];
                                    reply.extend(tg_tl::serialize(
                                        &tg_scheme::mtp_auth_exported_authorization(
                                            123,
                                            b"token".to_vec(),
                                        ),
                                    ));
                                    reply
                                } else if body
                                    .contains(&(tg_scheme::AuthImportAuthorization::ID as i32))
                                {
                                    assert_eq!(dc, 1);
                                    assert!(body.ends_with(&tg_tl::serialize(
                                        &tg_scheme::AuthImportAuthorization::new(
                                            123,
                                            b"token".to_vec()
                                        )
                                    )));
                                    authorized = true;
                                    let mut reply =
                                        vec![RPC_RESULT as i32, id as i32, (id >> 32) as i32];
                                    reply.extend(tg_tl::serialize(
                                        &tg_scheme::mtp_auth_authorization_sign_up_required(
                                            tg_tl::Flags::default(),
                                            None,
                                        ),
                                    ));
                                    reply
                                } else {
                                    continue;
                                };
                            peer.reply(
                                message.session_id,
                                (1700000000u64 << 32) | counter,
                                response,
                            )
                            .await;
                            counter += 2;
                        }
                    }
                }));
            }
            loop {
                tg_runtime::process_main_queue();
                instance.process(clock.now().monotonic_ms);
                instance_timers.process();
                if let Some(deadline) = instance.next_delayed_deadline() {
                    clock
                        .time
                        .advance((deadline - clock.now().monotonic_ms).max(0));
                    instance_timers.process();
                }
                if replies.lock().unwrap().len() == 2 {
                    break;
                }
                wake.notified().await;
            }
            let mut got = replies.lock().unwrap().clone();
            got.sort();
            assert_eq!(got, if guest_auth { vec![1, 2] } else { vec![2, 2] });
            assert_eq!(instance.get_keys_for_write().len(), 0);
            for task in sessions.into_iter().chain(servers) {
                task.abort();
            }
            instance.shutdown();
            tg_runtime::main_queue().set_waker(None);
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
#[allow(clippy::await_holding_lock)]
async fn dedicated_upload_get_file_uses_session_and_checks_downloaded_hash() {
    let _main = main_lock();
    tokio::task::LocalSet::new()
        .run_until(async {
            use crate::dedicated_file_loader::{
                DedicatedLoader, FileDescription, LoaderEvent, CHUNK_SIZE,
            };
            let instance = Instance::new(
                Mode::Normal,
                Fields::new(crate::mtproto_dc_options::Environment::Test),
            );
            let clock = TestClock::new();
            let host = TimerHost::new(Arc::new(clock.time.clone()));
            let folder = std::env::temp_dir().join(format!(
                "ayugram-session-download-{}",
                crate::mtp_instance::get_next_request_id()
            ));
            std::fs::create_dir_all(&folder).unwrap();
            let size = CHUNK_SIZE + 3;
            let mut loader = DedicatedLoader::new(
                instance.clone(),
                &folder,
                FileDescription {
                    name: "download.bin".into(),
                    size: size as u64,
                    dc_id: 2,
                    location: tg_scheme::mtp_input_document_file_location(1, 2, vec![], vec![]),
                },
                &host,
            );
            loader.start_loading().unwrap();
            let spawn = instance.take_session_spawns().pop_front().unwrap();
            let creating = spawn.dc.acquire_key_creation(DcType::Regular);
            assert!(spawn.dc.release_key_creation_on_done(
                creating,
                auth(),
                Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1)))
            ));
            let (session, handle) = crate::session::Session::new(
                spawn.id,
                spawn.dc,
                instance.session_environment(),
                instance.store.clone(),
                crate::session::SessionParameters {
                    clock: clock.clone(),
                    random: Entropy(37),
                    host: host.clone(),
                    info: ClientInfo::default(),
                },
            );
            instance.attach_session(spawn.id, handle.clone());
            let (transport, mut streams) = DuplexTransport::new();
            let task = tokio::task::spawn_local(session.run(transport));
            let server = tokio::task::spawn_local(async move {
                let mut peer = ServerTcp::new(streams.recv().await.unwrap()).await;
                let mut counter = 1;
                loop {
                    let message = peer.read_encrypted().await;
                    for (id, body) in contained_requests(&message) {
                        let reply = if matches!(body[0] as u32, 0xf3427b8c | 0x7abe77ec) {
                            vec![0x347773c5, id as i32, (id >> 32) as i32, body[1], body[2]]
                        } else if body.contains(&(tg_scheme::UploadGetFile::ID as i32)) {
                            let offset = body[body.len() - 3] as u32 as usize;
                            assert!(offset < size);
                            let count = CHUNK_SIZE.min(size - offset);
                            let bytes = (offset..offset + count).map(|i| (i % 251) as u8).collect();
                            let result = tg_scheme::mtp_upload_file(
                                tg_scheme::mtp_storage_file_unknown(),
                                0,
                                bytes,
                            );
                            let mut reply = vec![RPC_RESULT as i32, id as i32, (id >> 32) as i32];
                            reply.extend(tg_tl::serialize(&result));
                            reply
                        } else {
                            continue;
                        };
                        peer.reply(message.session_id, (1700000000u64 << 32) | counter, reply)
                            .await;
                        counter += 2;
                    }
                }
            });
            let wake = Arc::new(tokio::sync::Notify::new());
            let waker = wake.clone();
            tg_runtime::main_queue().set_waker(Some(Arc::new(move || waker.notify_one())));
            loop {
                tg_runtime::process_main_queue();
                instance.process(clock.now().monotonic_ms);
                host.process();
                loader.process();
                let events = loader.output.take_events();
                assert!(!events
                    .iter()
                    .any(|event| matches!(event, LoaderEvent::Failed(_))));
                if events
                    .iter()
                    .any(|event| matches!(event, LoaderEvent::Ready(_)))
                {
                    break;
                }
                wake.notified().await;
            }
            let actual = std::fs::read(folder.join("download.bin")).unwrap();
            let expected: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
            assert_eq!(actual.len(), size);
            assert_eq!(
                tg_base::openssl_help::sha256(&actual),
                tg_base::openssl_help::sha256(&expected)
            );
            task.abort();
            server.abort();
            instance.shutdown();
            std::fs::remove_dir_all(folder).unwrap();
            tg_runtime::main_queue().set_waker(None);
        })
        .await;
}
#[test]
fn concurrent_sender_dispatch_lifetime_and_typed_callback_forms() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let jobs = Arc::new(Mutex::new(VecDeque::<Box<dyn FnOnce() + Send>>::new()));
    let runner = jobs.clone();
    let sender = crate::mtproto_concurrent_sender::ConcurrentSender::new(
        instance.clone(),
        Arc::new(move |job| runner.lock().unwrap().push_back(job)),
    );
    let received = Arc::new(Mutex::new(Vec::new()));
    let got = received.clone();
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done_with_id(move |result, id| {
            got.lock()
                .unwrap()
                .push((id, result.c_nearest_dc().nearest_dc))
        })
        .send();
    assert!(!instance.has_callback(id));
    tg_runtime::process_main_queue();
    assert!(instance.has_callback(id));
    instance.process_callback(&Response {
        request_id: id,
        reply: tg_tl::serialize(&tg_scheme::mtp_nearest_dc(b"XX".to_vec(), 2, 1)),
        ..Default::default()
    });
    assert!(received.lock().unwrap().is_empty());
    assert!(sender.pending(id));
    jobs.lock().unwrap().pop_front().unwrap()();
    assert_eq!(*received.lock().unwrap(), vec![(id, 1)]);
    assert!(!sender.pending(id));
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done_plain(|| panic!("cancelled callback"))
        .send();
    sender.cancel(id);
    tg_runtime::process_main_queue();
    assert!(!instance.has_callback(id));
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done_plain(|| panic!("detached callback"))
        .send();
    tg_runtime::process_main_queue();
    sender.detach(id);
    instance.process_callback(&Response {
        request_id: id,
        reply: tg_tl::serialize(&tg_scheme::mtp_nearest_dc(b"XX".to_vec(), 2, 1)),
        ..Default::default()
    });
    jobs.lock().unwrap().pop_front().unwrap()();
    let normal = Sender::new(instance.clone());
    let id = normal
        .request(tg_scheme::HelpGetConfig::new())
        .after_delay(3)
        .after_request(0)
        .override_id(crate::mtp_instance::get_next_request_id())
        .done_with_response(|_, _| {})
        .fail_with_id(|_, _| {})
        .handle_all_errors()
        .to_dc(2)
        .send();
    assert!(normal.pending(id));
    normal.sent_request(id).cancel();
    assert!(!normal.pending(id));
    let done = Arc::new(Mutex::new(false));
    let callback = done.clone();
    let id = normal
        .request(tg_scheme::HelpGetConfig::new())
        .fail(move |_| *callback.lock().unwrap() = true)
        .handle_flood_errors()
        .send();
    instance.process_callback(&Response {
        request_id: id,
        reply: tg_tl::serialize(&tg_scheme::mtp_rpc_error(420, b"FLOOD_WAIT_5".to_vec())),
        ..Default::default()
    });
    assert!(*done.lock().unwrap());
    assert!(instance.next_delayed_deadline().is_none());
    instance.shutdown();
}
#[test]
fn environment_bridge_has_no_ownership_cycle_and_handler_replacement_survives() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let values = Arc::new(Mutex::new(Vec::new()));
    let first = values.clone();
    let owner = instance.clone();
    instance.set_updates_handler(Some(Box::new(move |_| {
        first.lock().unwrap().push(1);
        let next = first.clone();
        owner.set_updates_handler(Some(Box::new(move |_| next.lock().unwrap().push(2))));
    })));
    instance.emit(InstanceEvent::Update(Response::default()));
    instance.emit(InstanceEvent::Update(Response::default()));
    instance.process(100);
    assert_eq!(*values.lock().unwrap(), vec![1, 2]);
    instance.clear_global_handlers();
    let bridge = instance.session_environment();
    let weak = Arc::downgrade(&instance.store);
    drop(instance);
    assert!(weak.upgrade().is_none());
    assert_eq!(bridge.device_model(), "");
}
#[test]
fn delayed_initial_request_keeps_deadline_and_send_delayed_flushes() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let clock = TestClock::new();
    instance.process(clock.now().monotonic_ms);
    let sender = Sender::new(instance.clone());
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .after_delay(100)
        .send();
    clock.time.advance(30);
    instance.process(clock.now().monotonic_ms);
    let spawn = instance.take_session_spawns().pop_front().unwrap();
    let creating = spawn.dc.acquire_key_creation(DcType::Regular);
    spawn.dc.release_key_creation_on_done(
        creating,
        auth(),
        Some(Arc::new(AuthKey::new(KeyType::Generated, 2, [3; 256], 1))),
    );
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, handle) = crate::session::Session::new(
        2,
        spawn.dc,
        instance.session_environment(),
        instance.store.clone(),
        crate::session::SessionParameters {
            clock: clock.clone(),
            random: Entropy(31),
            host: host.clone(),
            info: ClientInfo::default(),
        },
    );
    let candidate = session
        .take_transport_actions()
        .into_iter()
        .filter_map(|action| {
            if let TransportAction::Connect(spec) = action {
                Some(spec)
            } else {
                None
            }
        })
        .max_by_key(|spec| spec.priority)
        .unwrap()
        .id;
    session.on_transport_event(crate::session::TransportEvent::Connected {
        id: candidate,
        tag: "TCP".into(),
        full_timeout: 8000,
    });
    session.process();
    let ping = session
        .take_transport_actions()
        .into_iter()
        .find_map(|action| {
            if let TransportAction::Send(_, packet) = action {
                Some(server_decrypt(&packet))
            } else {
                None
            }
        })
        .unwrap();
    let pong = server_encrypt(&Message {
        salt: 100,
        session_id: ping.session_id,
        msg_id: (1700000000u64 << 32) | 1,
        seq_no: 0,
        body: vec![
            0x347773c5,
            ping.msg_id as i32,
            (ping.msg_id >> 32) as i32,
            ping.body[1],
            ping.body[2],
        ],
    });
    session.on_transport_event(crate::session::TransportEvent::Packet {
        id: candidate,
        words: pong,
        need_http_wait: false,
    });
    session.process();
    session.take_transport_actions();
    instance.attach_session(2, handle);
    session.process();
    assert_eq!(host.next_deadline(), Some(200));
    assert_eq!(session.request_state(id), 2);
    sender.request_send_delayed();
    session.process();
    let packets = session.take_transport_actions();
    assert!(packets
        .into_iter()
        .any(|action| if let TransportAction::Send(_, packet) = action {
            server_decrypt(&packet)
                .body
                .contains(&(tg_scheme::HelpGetNearestDc::ID as i32))
        } else {
            false
        }));
    instance.shutdown();
}
