use super::*;
use crate::mtproto_response::Error;
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
#[test]
fn weak_sender_lifetimes_and_unavailable_callbacks_are_guarded() {
    let _main = main_lock();
    use crate::dedicated_file_loader::WeakInstance;
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let owner = Arc::new(());
    let weak = WeakInstance::new(instance.clone(), Arc::downgrade(&owner));
    let failed = Arc::new(Mutex::new(Vec::new()));
    let errors = failed.clone();
    weak.send(
        tg_scheme::HelpGetConfig::new(),
        |_| {},
        move |error| errors.lock().unwrap().push(error.kind.clone()),
        0,
    );
    drop(owner);
    weak.process();
    assert_eq!(*failed.lock().unwrap(), vec!["CLIENT_UNAVAILABLE"]);
    assert!(!weak.valid());
    drop(weak);
    let absent = WeakInstance::new(instance.clone(), std::sync::Weak::new());
    absent.send(
        tg_scheme::HelpGetConfig::new(),
        |_| {},
        |_| panic!("callback outlived WeakInstance"),
        0,
    );
    drop(absent);
    tg_runtime::process_main_queue();
    let copy = WeakInstance::new(instance.clone(), std::sync::Weak::new());
    let called = Arc::new(Mutex::new(false));
    let out = called.clone();
    copy.send(
        tg_scheme::HelpGetConfig::new(),
        |_| {},
        move |_| *out.lock().unwrap() = true,
        0,
    );
    tg_runtime::process_main_queue();
    assert!(*called.lock().unwrap());
    let store = Arc::downgrade(&instance.store);
    let queue = Arc::new(Mutex::new(VecDeque::<Box<dyn FnOnce() + Send>>::new()));
    let jobs = queue.clone();
    let concurrent = crate::mtproto_concurrent_sender::ConcurrentSender::new(
        instance.clone(),
        Arc::new(move |job| jobs.lock().unwrap().push_back(job)),
    );
    drop(copy);
    drop(instance);
    assert!(store.upgrade().is_none());
    let id = concurrent
        .request(tg_scheme::HelpGetConfig::new())
        .done_plain(|| panic!("dead Instance callback"))
        .send();
    assert!(concurrent.pending(id));
    tg_runtime::process_main_queue();
    drop(concurrent);
    tg_runtime::process_main_queue();
}
#[test]
fn observer_streams_config_time_and_global_errors_follow_source() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    instance.start();
    let mut dc = instance.main_dc_id_value();
    assert_eq!(dc.try_recv().unwrap(), 2);
    instance.suggest_main_dc_id(3);
    assert_eq!(dc.try_recv().unwrap(), 3);
    instance.suggest_main_dc_id(1);
    assert!(dc.try_recv().is_err());
    let mut writes = instance.write_keys_requests();
    let mut temporary = instance.temporary_key_changes();
    instance.dc_persistent_key_changed(
        3,
        Some(Arc::new(AuthKey::new(KeyType::Generated, 3, [1; 256], 1))),
    );
    assert_eq!(temporary.try_recv().unwrap(), 3);
    assert!(writes.try_recv().is_ok());
    let mut restarts = instance.restarts_by_timeout();
    instance.emit(InstanceEvent::RestartedByTimeout(3));
    let mut frozen = instance.frozen_error_received();
    let mut premium = instance.non_premium_delayed_requests();
    let id = instance.send(
        &tg_scheme::HelpGetConfig::new(),
        Default::default(),
        0,
        0,
        0,
        0,
    );
    let response = Response {
        request_id: id,
        ..Default::default()
    };
    assert!(instance.on_error_default(
        &Error {
            code: 420,
            kind: "FLOOD_PREMIUM_WAIT_0".into(),
            description: String::new()
        },
        &response
    ));
    instance.emit(InstanceEvent::Callback(Response {
        request_id: 0,
        reply: vec![],
        ..Default::default()
    }));
    instance.process(100);
    assert_eq!(restarts.try_recv().unwrap(), 3);
    assert_eq!(premium.try_recv().unwrap(), id);
    instance.on_error_default(
        &Error {
            code: 400,
            kind: "FROZEN_METHOD_INVALID".into(),
            description: String::new(),
        },
        &response,
    );
    assert!(frozen.try_recv().is_ok());
    let mut states = Vec::new();
    let state = Arc::new(Mutex::new(Vec::new()));
    let shared = state.clone();
    instance.set_state_changed_handler(Some(Box::new(move |dc, value| {
        shared.lock().unwrap().push((dc, value))
    })));
    let shared = state.clone();
    instance.set_session_reset_handler(Some(Box::new(move |dc| {
        shared.lock().unwrap().push((dc, 99))
    })));
    instance.emit(InstanceEvent::StateChange(3, 2));
    instance.emit(InstanceEvent::SessionReset(3));
    instance.process(100);
    states.extend(state.lock().unwrap().iter().copied());
    assert_eq!(states, vec![(3, 2), (3, 99)]);
    instance.clear_global_handlers();
    instance.cancel(id);
    instance.set_languages("ru".into(), "ru".into(), "tdesktop".into());
    assert_eq!(instance.system_lang_code(), "ru");
    assert_eq!(instance.cloud_lang_code(), "ru");
    assert_eq!(instance.lang_pack_name(), "tdesktop");
    let _ = instance.environment();
    let _ = instance.config_values();
    instance.set_user_phone("+7".into());
    assert_eq!(instance.user_phone(), "+7");
    instance.shutdown();
}
#[test]
fn nested_global_pause_defers_real_session_delivery_until_unpaused() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let sender = Sender::new(instance.clone());
    let called = Arc::new(Mutex::new(false));
    let out = called.clone();
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done(move |_| *out.lock().unwrap() = true)
        .send();
    let spawn = instance.take_session_spawns().pop_front().unwrap();
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, handle) = crate::session::Session::new(
        spawn.id,
        spawn.dc,
        instance.session_environment(),
        instance.store.clone(),
        crate::session::SessionParameters {
            clock,
            random: Entropy(23),
            host,
            info: ClientInfo::default(),
        },
    );
    instance.attach_session(2, handle);
    session
        .data
        .queues
        .lock()
        .unwrap()
        .received
        .push_back(Response {
            request_id: id,
            reply: tg_tl::serialize(&tg_scheme::mtp_nearest_dc(b"XX".to_vec(), 2, 1)),
            ..Default::default()
        });
    crate::facade::pause();
    crate::facade::pause();
    session.try_to_receive();
    tg_runtime::process_main_queue();
    instance.process(100);
    let before = *called.lock().unwrap();
    crate::facade::unpause();
    instance.process(100);
    session.process();
    let middle = *called.lock().unwrap();
    crate::facade::unpause();
    instance.process(100);
    session.process();
    tg_runtime::process_main_queue();
    instance.process(100);
    assert!(!before && !middle);
    assert!(*called.lock().unwrap());
    assert!(!crate::facade::paused());
    instance.shutdown();
}
#[test]
fn tls_timeout_notifies_time_owner_before_candidates_are_dropped() {
    let _main = main_lock();
    let mut h = Harness::new();
    h.p.destroy_all_connections();
    h.p.connect_to_server(false);
    let mut count = 0;
    for candidate in h.p.candidates.values_mut() {
        if !candidate.spec.http {
            candidate.spec.endpoint.secret = vec![0xee; 21];
            count += 1;
        }
    }
    assert!(count > 0);
    h.p.clock.time.advance(1000);
    h.p.process_timers();
    h.drain();
    tg_runtime::process_main_queue();
    assert_eq!(
        h.env
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, InstanceEvent::SyncHttpUnixtime))
            .count(),
        count
    );
}
#[test]
fn configuration_expiry_cdn_and_protocol_layer_use_virtual_time() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    instance.process_at(Time {
        monotonic_ms: 0,
        unix_seconds: 1700000000,
    });
    let config = tg_scheme::Config::Config(Arc::new(tg_scheme::ConfigData {
        date: 1700000000,
        expires: 1700007200,
        ..Default::default()
    }));
    instance.apply_config(&config, false);
    assert_eq!(instance.request_config_if_expired(), Some(3600000));
    instance.take_notices();
    instance.request_config_if_old();
    assert!(instance.take_notices().is_empty());
    instance.process_at(Time {
        monotonic_ms: 3600000,
        unix_seconds: 1700003600,
    });
    assert_eq!(instance.request_config_if_expired(), Some(3600000));
    instance.process_at(Time {
        monotonic_ms: 7200000,
        unix_seconds: 1700007200,
    });
    assert_eq!(instance.request_config_if_expired(), None);
    assert!(instance
        .take_notices()
        .contains(&crate::mtp_instance::Notice::ConfigNeeded));
    let mut changes = instance.subscribe();
    let id = instance.request_cdn_config();
    assert_ne!(id, 0);
    assert_eq!(instance.request_cdn_config(), 0);
    instance.process_callback(&Response {
        request_id: id,
        reply: tg_tl::serialize(&tg_scheme::mtp_cdn_config(vec![])),
        ..Default::default()
    });
    assert!(matches!(
        changes.try_recv(),
        Ok(EnvironmentChange::CdnConfig)
    ));
    assert!(instance
        .take_notices()
        .contains(&crate::mtp_instance::Notice::WriteSettings));
    let id = instance.send_protocol_message(2, &tg_scheme::DestroyAuthKey::new());
    assert!(!instance.session_environment().has_callback(id));
    instance.with_config(|config| assert!(config.is_test_mode()));
    instance.shutdown();
}
#[test]
fn forced_http_policy_survives_refresh_and_restart() {
    let _main = main_lock();
    let instance = Instance::new(
        Mode::Normal,
        Fields::new(crate::mtproto_dc_options::Environment::Test),
    );
    let clock = TestClock::new();
    let host = TimerHost::new(Arc::new(clock.time.clone()));
    let (mut session, _handle) = crate::session::Session::new(
        2,
        Arc::new(Dcenter::new(2, None)),
        instance.session_environment(),
        instance.store.clone(),
        crate::session::SessionParameters {
            clock: clock.clone(),
            random: Entropy(31),
            host,
            info: ClientInfo::default(),
        },
    );
    session.take_transport_actions();
    session.set_protocol_policy(false, true, false);
    for _ in 0..2 {
        clock.time.advance(1);
        session.process();
        let connects = session
            .take_transport_actions()
            .into_iter()
            .filter_map(|action| {
                if let TransportAction::Connect(spec) = action {
                    Some(spec)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(!connects.is_empty());
        assert!(connects.iter().all(|spec| spec.http));
        assert!(!session.data.options().use_tcp);
        session.restart();
    }
    instance.shutdown();
}
#[test]
fn instance_controls_main_download_sessions_and_metadata() {
    let _main = main_lock();
    let mut fields = Fields::new(crate::mtproto_dc_options::Environment::Test);
    fields.device_model = "Default".into();
    fields.system_version = "Test OS".into();
    let instance = Instance::new(Mode::Normal, fields);
    instance.start();
    instance.ensure_session(crate::facade::download_dc_id(2, 0));
    let dc = instance.get_dc_by_id(2);
    assert!(Arc::ptr_eq(&dc, &instance.get_dc_by_id(160002)));
    let clock = TestClock::new();
    let mut sessions = Vec::new();
    for spawn in instance.take_session_spawns() {
        let host = TimerHost::new(Arc::new(clock.time.clone()));
        let (session, handle) = crate::session::Session::new(
            spawn.id,
            spawn.dc,
            instance.session_environment(),
            instance.store.clone(),
            crate::session::SessionParameters {
                clock: clock.clone(),
                random: Entropy(spawn.id as u64),
                host,
                info: ClientInfo::default(),
            },
        );
        instance.attach_session(spawn.id, handle);
        sessions.push(session);
    }
    assert_eq!(sessions.len(), 2);
    instance.stop_session(0);
    instance.stop_session(2);
    for s in &mut sessions {
        s.process();
    }
    assert_eq!(instance.dcstate(0), CONNECTING);
    instance.stop_session(160002);
    for s in &mut sessions {
        s.process();
    }
    assert_eq!(instance.dcstate(160002), DISCONNECTED);
    instance.restart(Some(2));
    for s in &mut sessions {
        s.process();
    }
    assert!(instance.dcstate(2) < 0);
    instance.restart(None);
    for s in &mut sessions {
        s.process();
    }
    instance.set_device_model("Custom".into());
    assert_eq!(instance.device_model(), "Custom");
    instance.set_device_model(String::new());
    assert_eq!(instance.device_model(), "Default");
    assert_eq!(instance.system_version(), "Test OS");
    instance.re_init_connection(2);
    instance.ping();
    instance.send_anything(2, 0);
    for s in &mut sessions {
        s.process();
    }
    instance.kill_session(160002);
    assert!(instance.session(160002).is_none());
    assert!(std::panic::catch_unwind(|| instance.kill_session(2)).is_err());
    instance.shutdown();
}
