use super::*;
use tg_tl::serialize;
fn test_instance() -> Instance {
    Instance::new(Mode::Normal, Fields::new(Environment::Test))
}
#[test]
fn instance_routing_callbacks_delays_and_sender_ownership() {
    let instance = test_instance();
    instance.start();
    assert_eq!(instance.main_dc_id(), 2);
    assert_eq!(instance.ensure_session(0), 2);
    assert_eq!(instance.ensure_session(20000), 20002);
    assert_eq!(instance.take_session_spawns().len(), 2);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sender = crate::sender::Sender::new(instance.clone());
    let out = calls.clone();
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done(move |dc| out.lock().unwrap().push(dc.c_nearest_dc().nearest_dc))
        .send();
    assert!(sender.pending(id));
    instance.process_callback(&Response {
        request_id: id,
        reply: serialize(&tg_scheme::mtp_nearest_dc(b"RU".to_vec(), 2, 4)),
        ..Response::default()
    });
    assert_eq!(*calls.lock().unwrap(), vec![4]);
    assert!(!sender.pending(id));
    assert!(!instance.has_callback(id));
    for prefix in ["PHONE", "FILE", "NETWORK", "USER"] {
        let id = instance.send(
            &tg_scheme::HelpGetConfig::new(),
            ResponseHandler::default(),
            20002,
            0,
            0,
            0,
        );
        let error = Error {
            code: 303,
            kind: format!("{prefix}_MIGRATE_4"),
            description: String::new(),
        };
        assert!(instance.on_error_default(
            &error,
            &Response {
                request_id: id,
                ..Response::default()
            }
        ));
        assert_eq!(instance.state.lock().unwrap().routes[&id], 20004);
        instance.cancel(id);
    }
    let id = sender.request(tg_scheme::HelpGetConfig::new()).send();
    let response = Response {
        request_id: id,
        ..Response::default()
    };
    let flood = Error {
        code: 420,
        kind: "FLOOD_WAIT_3".into(),
        description: String::new(),
    };
    assert!(instance.on_error_default(&flood, &response));
    assert_eq!(instance.next_delayed_deadline(), Some(3010));
    instance.check_delayed_requests(3009);
    assert_eq!(instance.next_delayed_deadline(), Some(3010));
    instance.check_delayed_requests(3010);
    assert_eq!(instance.next_delayed_deadline(), None);
    drop(sender);
    assert!(!instance.has_callback(id));
    assert!(!instance.state.lock().unwrap().requests.contains_key(&id));
    let ids: Vec<_> = (0..10).map(|_| get_next_request_id()).collect();
    assert!(ids.windows(2).all(|x| x[0] < x[1]));
    let mut pause = crate::facade::Pause::default();
    let mut unpaused = pause.unpaused();
    pause.pause();
    pause.pause();
    pause.unpause();
    assert!(pause.paused());
    assert!(unpaused.try_recv().is_err());
    pause.unpause();
    assert!(!pause.paused());
    assert!(unpaused.try_recv().is_ok());
}
#[test]
fn malformed_typed_reply_calls_fail_after_done_parser() {
    let instance = test_instance();
    let sender = crate::sender::Sender::new(instance.clone());
    let called = Arc::new(Mutex::new(Vec::new()));
    let out = called.clone();
    let id = sender
        .request(tg_scheme::HelpGetNearestDc::new())
        .done(|_| panic!("invalid reply was accepted"))
        .fail(move |error| out.lock().unwrap().push(error.kind.clone()))
        .send();
    instance.process_callback(&Response {
        request_id: id,
        reply: vec![1],
        ..Response::default()
    });
    assert_eq!(
        *called.lock().unwrap(),
        vec!["CLIENT_RESPONSE_PARSE_FAILED"]
    );
}
#[test]
fn guest_authorization_exports_imports_and_releases_waiters() {
    let instance = test_instance();
    let id = instance.send(
        &tg_scheme::HelpGetNearestDc::new(),
        ResponseHandler::default(),
        20001,
        0,
        0,
        0,
    );
    let response = Response {
        request_id: id,
        ..Response::default()
    };
    assert!(instance.on_error_default(
        &Error {
            code: 401,
            kind: "AUTH_KEY_UNREGISTERED".into(),
            description: String::new()
        },
        &response
    ));
    let export = *instance
        .state
        .lock()
        .unwrap()
        .exports
        .keys()
        .next()
        .unwrap();
    let result = tg_scheme::mtp_auth_exported_authorization(123, b"authorization".to_vec());
    instance.process_callback(&Response {
        request_id: export,
        reply: serialize(&result),
        ..Response::default()
    });
    let import = {
        let s = instance.state.lock().unwrap();
        *s.routes
            .iter()
            .find(|(r, dc)| **r != id && **dc == 20001)
            .unwrap()
            .0
    };
    instance.import_done(&Response {
        request_id: import,
        ..Response::default()
    });
    assert!(instance.state.lock().unwrap().waiters[&1].is_empty());
    assert_eq!(instance.state.lock().unwrap().routes[&id], 20001);
    instance.cancel(import);
    instance.cancel(id);
    instance.shutdown();
}
#[test]
fn config_enumeration_and_special_attempts_use_virtual_timers() {
    use crate::{config_loader::ConfigLoader, special_config_request::*};
    use std::rc::Rc;
    use tg_base::timer::{ManualClock, TimerHost};
    struct Random;
    impl crate::core_types::Random for Random {
        fn fill(&mut self, b: &mut [u8]) {
            b.fill(27);
        }
    }
    let clock = Arc::new(ManualClock::new(100));
    let host = TimerHost::new(clock.clone());
    let instance = test_instance();
    let mut loader = ConfigLoader::new(instance.clone(), "+7".into(), false, &host);
    loader.load();
    let first = instance.take_session_spawns();
    assert_eq!(first.front().unwrap().id, 2);
    clock.advance(7999);
    host.process();
    assert!(loader.process(&mut Random).is_empty());
    assert!(instance.take_session_spawns().is_empty());
    clock.advance(1);
    host.process();
    loader.process(&mut Random);
    let next = instance.take_session_spawns();
    assert_eq!(next.len(), 1);
    assert_ne!(next[0].id, 2);
    let endpoint = Endpoint {
        dc_id: 2,
        ip: "127.0.0.1".into(),
        port: 443,
        secret: vec![],
    };
    loader.add_special_endpoint(endpoint);
    clock.advance(1);
    host.process();
    loader.process(&mut Random);
    assert!(instance.take_session_spawns().iter().any(|x| x.id == 1002));
    loader.set_proxy_enabled(true);
    loader.set_phone("+44".into());
    let mut attempts = SpecialConfigRequest::new(true, false, &host, &mut Random);
    assert!(attempts.send_next_request().is_some());
    assert!(attempts.send_next_request().is_none());
    clock.advance(799);
    host.process();
    assert!(attempts.send_next_request().is_none());
    clock.advance(1);
    host.process();
    assert!(attempts.send_next_request().is_some());
    assert!(attempts.send_next_request().is_none());
    let _ = Rc::strong_count(&host);
}
#[test]
fn proxy_checker_candidates_completion_and_cancellation() {
    use crate::{
        connection_abstract::TransportError,
        mtproto_proxy_data::{ProxyData, Type},
        proxy_check::ProxyCheckers,
        session::{SessionTransport, TransportEvent},
        session_private::TransportAction,
    };
    #[derive(Default)]
    struct Capture(Vec<TransportAction>);
    impl SessionTransport for Capture {
        fn submit(&mut self, a: TransportAction) -> Result<(), TransportError> {
            self.0.push(a);
            Ok(())
        }
        async fn next_event(&mut self) -> Result<TransportEvent, TransportError> {
            std::future::pending().await
        }
    }
    let instance = test_instance();
    let mut transport = Capture::default();
    let mut check = ProxyCheckers::default();
    check
        .start_proxy_check(
            &instance,
            &ProxyData {
                kind: Type::Socks5,
                ..Default::default()
            },
            true,
            100,
            &mut transport,
        )
        .unwrap();
    assert!(check.has_proxy_checkers());
    let ids: Vec<_> = transport
        .0
        .iter()
        .filter_map(|a| {
            if let TransportAction::Connect(spec) = a {
                Some(spec.id)
            } else {
                None
            }
        })
        .collect();
    assert!(!ids.is_empty());
    assert_eq!(
        check.process_event(
            &TransportEvent::ProbeConnected {
                id: ids[0],
                tag: "TCP".into(),
                full_timeout: 8000,
                ping_time: 7
            },
            120
        ),
        Some((ids[0], Ok(7)))
    );
    assert_eq!(
        check.process_event(&TransportEvent::Error(ids[0], -444), 130),
        Some((ids[0], Err(())))
    );
    check.drop_proxy_checker(ids[0], &mut transport).unwrap();
    check.reset_proxy_checkers(&mut transport).unwrap();
    assert!(!check.has_proxy_checkers());
    check
        .start_proxy_check(
            &instance,
            &ProxyData {
                kind: Type::Web,
                ..Default::default()
            },
            false,
            200,
            &mut transport,
        )
        .unwrap();
    assert!(!check.has_proxy_checkers());
}
#[test]
fn resumable_file_chunks_limits_and_content_hash() {
    use crate::dedicated_file_loader::*;
    let path = std::env::temp_dir().join(format!("ayugram-dedicated-{}", get_next_request_id()));
    std::fs::create_dir_all(&path).unwrap();
    let file = path.join("file.bin");
    std::fs::write(&file, b"abcdef").unwrap();
    let mut output = AbstractDedicatedLoader::new(file.clone(), 4);
    output.validate_output().unwrap();
    assert_eq!(output.already_size(), 4);
    assert_eq!(std::fs::read(&file).unwrap(), b"abcd");
    output.write_chunk(b"efgh", 8).unwrap();
    assert_eq!(output.total_size(), 8);
    assert_eq!(
        tg_base::openssl_help::sha256(&std::fs::read(&file).unwrap()),
        tg_base::openssl_help::sha256(b"abcdefgh")
    );
    assert!(matches!(
        output.take_events().back(),
        Some(LoaderEvent::Ready(_))
    ));
    assert!(output.write_chunk(&[], MAX_FILE_SIZE + 1).is_err());
    assert!(!output.prefer_percent());
    std::fs::write(path.join("other"), b"x").unwrap();
    output.wipe_folder().unwrap();
    assert!(!path.join("other").exists());
    assert_eq!(
        Progress {
            already: 1,
            size: 2,
            percent: false
        },
        Progress {
            already: 1,
            size: 2,
            percent: true
        }
    );
    assert!(
        Progress::default()
            < Progress {
                already: 1,
                ..Default::default()
            }
    );
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn after_request_retains_identity_until_last_dependent_finishes() {
    let instance = test_instance();
    let first = instance.send(
        &tg_scheme::HelpGetConfig::new(),
        ResponseHandler::default(),
        0,
        0,
        0,
        0,
    );
    let second = instance.send(
        &tg_scheme::HelpGetConfig::new(),
        ResponseHandler::default(),
        0,
        0,
        first,
        0,
    );
    let (first_handle, second_handle) = {
        let s = instance.state.lock().unwrap();
        (s.requests[&first], s.requests[&second])
    };
    instance.unregister_request(first);
    assert!(instance.store.lock().unwrap().get(first_handle).is_some());
    assert_eq!(
        instance
            .store
            .lock()
            .unwrap()
            .get(second_handle)
            .unwrap()
            .after,
        Some(first_handle)
    );
    instance.unregister_request(second);
    assert!(instance.store.lock().unwrap().get(first_handle).is_none());
    assert!(instance.store.lock().unwrap().get(second_handle).is_none());
}
#[test]
fn key_write_identity_destroyer_slots_and_protocol_lifecycle() {
    use crate::mtproto_auth_key::KeyType;
    let instance = test_instance();
    let key = Arc::new(AuthKey::new(KeyType::Generated, 2, [9; 256], 1));
    instance.dc_persistent_key_changed(2, Some(key.clone()));
    assert_eq!(instance.get_keys_for_write()[0].key_id(), key.key_id());
    assert!(instance.take_notices().contains(&Notice::WriteKeys));
    instance.dc_persistent_key_changed(2, Some(key));
    assert!(!instance.take_notices().contains(&Notice::WriteKeys));
    instance.dc_persistent_key_changed(
        1002,
        Some(Arc::new(AuthKey::new(
            KeyType::Generated,
            1002,
            [9; 256],
            1,
        ))),
    );
    assert_eq!(instance.get_keys_for_write().len(), 1);
    instance.dc_persistent_key_changed(2, None);
    assert!(instance.get_keys_for_write().is_empty());
    let mut fields = Fields::new(Environment::Test);
    fields.main_dc_id = -1;
    let destroyer = Instance::new(Mode::KeysDestroyer, fields);
    destroyer.add_keys_for_destroy(vec![
        Arc::new(AuthKey::new(KeyType::Generated, 2, [1; 256], 1)),
        Arc::new(AuthKey::new(KeyType::Generated, 2, [2; 256], 1)),
    ]);
    let ids = destroyer
        .state
        .lock()
        .unwrap()
        .keys
        .keys()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![2560002, 2570002]);
    for id in ids {
        destroyer.process_event(InstanceEvent::KeyWasPossiblyDestroyed(id));
    }
    assert!(destroyer.get_keys_for_write().is_empty());
    assert!(destroyer.take_notices().contains(&Notice::AllKeysDestroyed));
    destroyer.shutdown();
}
#[test]
fn document_and_channel_helpers_preserve_exact_message_selection() {
    use crate::dedicated_file_loader::*;
    let document = tg_scheme::Document::Document(Arc::new(tg_scheme::DocumentData {
        id: 11,
        access_hash: 12,
        size: 13,
        dc_id: 2,
        attributes: vec![tg_scheme::mtp_document_attribute_filename(
            b"file.bin".to_vec(),
        )],
        ..Default::default()
    }));
    let media = tg_scheme::MessageMedia::MessageMediaDocument(Arc::new(
        tg_scheme::MessageMediaDocumentData {
            flags: tg_scheme::MessageMediaDocumentData::F_DOCUMENT,
            document: Some(document),
            ..Default::default()
        },
    ));
    let message = tg_scheme::Message::Message(Arc::new(tg_scheme::MessageData {
        id: 7,
        flags: tg_scheme::MessageData::F_MEDIA,
        media: Some(media),
        ..Default::default()
    }));
    let list =
        tg_scheme::MessagesMessages::MessagesMessages(Arc::new(tg_scheme::MessagesMessagesData {
            messages: vec![message],
            ..Default::default()
        }));
    assert!(get_messages_element(&list, 8).is_none());
    assert!(get_messages_element(&list, 0).is_some());
    let file = parse_file(&list, 7).unwrap();
    assert_eq!(
        (file.name, file.size, file.dc_id),
        ("file.bin".into(), 13, 2)
    );
    assert!(parse_file(&list, 8).is_none());
    let resolved = tg_scheme::ContactsResolvedPeer::ContactsResolvedPeer(Arc::new(
        tg_scheme::ContactsResolvedPeerData {
            peer: tg_scheme::mtp_peer_channel(42),
            chats: vec![tg_scheme::Chat::Channel(Arc::new(tg_scheme::ChannelData {
                id: 42,
                flags: tg_scheme::ChannelData::F_ACCESS_HASH,
                access_hash: Some(99),
                ..Default::default()
            }))],
            ..Default::default()
        },
    ));
    assert_eq!(
        tg_tl::serialize(&extract_channel(&resolved).unwrap()),
        tg_tl::serialize(&tg_scheme::mtp_input_channel(42, 99))
    );
}
#[test]
fn request_error_policies_and_numeric_suffixes() {
    for (policy, kind, code, called, delayed) in [
        (0, "FLOOD_WAIT_2", 420, false, true),
        (1, "FLOOD_WAIT_2", 420, true, false),
        (1, "INTERNAL", 500, false, true),
        (2, "INTERNAL", 500, true, false),
        (0, "BAD_REQUEST", 400, true, false),
    ] {
        let instance = test_instance();
        let sender = crate::sender::Sender::new(instance.clone());
        let done = Arc::new(Mutex::new(false));
        let callback = done.clone();
        let request = sender
            .request(tg_scheme::HelpGetConfig::new())
            .fail(move |_| *callback.lock().unwrap() = true);
        let request = match policy {
            1 => request.handle_flood_errors(),
            2 => request.handle_all_errors(),
            _ => request,
        };
        let id = request.send();
        instance.process_callback(&Response {
            request_id: id,
            reply: serialize(&tg_scheme::mtp_rpc_error(code, kind.as_bytes().to_vec())),
            ..Default::default()
        });
        assert_eq!(*done.lock().unwrap(), called);
        assert_eq!(instance.next_delayed_deadline().is_some(), delayed);
        instance.cancel(id);
    }
    assert_eq!(numeric_suffix("FLOOD_WAIT_+1", "FLOOD_WAIT_"), None);
    assert_eq!(
        numeric_suffix("FLOOD_WAIT_9999999999999999", "FLOOD_WAIT_"),
        Some(0)
    );
}
#[test]
fn proxy_dns_cache_preserves_and_prioritizes_address_order() {
    struct Random;
    impl crate::core_types::Random for Random {
        fn fill(&mut self, b: &mut [u8]) {
            b.fill(27);
        }
    }
    let instance = test_instance();
    instance.set_proxy_settings(ProxySettings {
        selected: crate::mtproto_proxy_data::ProxyData {
            kind: crate::mtproto_proxy_data::Type::Socks5,
            host: "proxy.example".into(),
            port: 1080,
            resolved_ips: vec!["10.0.0.2".into(), "10.0.0.9".into()],
            ..Default::default()
        },
        enabled: true,
        try_ipv6: true,
    });
    let mut events = instance.subscribe_proxy_domains();
    instance.apply_domain_ips(
        "proxy.example",
        &["10.0.0.1".into(), "10.0.0.2".into()],
        5000,
    );
    assert_eq!(
        instance.proxy_settings().selected.resolved_ips,
        vec!["10.0.0.2", "10.0.0.1"]
    );
    instance.set_good_proxy_domain("proxy.example", "10.0.0.1");
    assert_eq!(
        instance.proxy_settings().selected.resolved_ips,
        vec!["10.0.0.1", "10.0.0.2"]
    );
    instance.resolve_proxy_domain("proxy.example", 100, &mut Random);
    let mut requests = instance
        .poll_domain_requests(100, &mut Random)
        .into_iter()
        .collect::<Vec<_>>();
    assert!(!requests.is_empty());
    requests.sort_by_key(|r| !r.key.ipv6);
    for request in requests {
        let reply = if request.key.ipv6 {
            br#"{"Answer":[{"type":28,"data":"2001:db8::1","TTL":60}]}"#.as_slice()
        } else {
            br#"{"Answer":[{"type":1,"data":"10.0.0.3","TTL":60}]}"#.as_slice()
        };
        instance.domain_request_finished(&request, reply, 100);
    }
    assert!(events.try_recv().is_ok());
    assert!(instance
        .proxy_settings()
        .selected
        .resolved_ips
        .contains(&"10.0.0.3".into()));
}
#[test]
fn dedicated_high_level_helpers_send_and_release_callbacks() {
    use crate::dedicated_file_loader::*;
    let instance = test_instance();
    let lifetime = Arc::new(());
    let weak = Arc::new(WeakInstance::new(
        instance.clone(),
        Arc::downgrade(&lifetime),
    ));
    let result = Arc::new(Mutex::new(None));
    let out = result.clone();
    start_dedicated_loader(
        weak.clone(),
        Location {
            username: String::new(),
            channel_id: 42,
            access_hash: 99,
            post_id: 7,
        },
        move |file| *out.lock().unwrap() = Some(file.is_none()),
    );
    let id = *instance
        .state
        .lock()
        .unwrap()
        .requests
        .keys()
        .last()
        .unwrap();
    instance.process_callback(&Response {
        request_id: id,
        reply: serialize(&tg_scheme::MessagesMessages::default()),
        ..Default::default()
    });
    assert_eq!(*result.lock().unwrap(), Some(true));
    let failed = Arc::new(Mutex::new(false));
    let failure = failed.clone();
    resolve_channel(
        weak.clone(),
        "missing".into(),
        |_| panic!("missing channel accepted"),
        move || *failure.lock().unwrap() = true,
    );
    let id = *instance
        .state
        .lock()
        .unwrap()
        .requests
        .keys()
        .last()
        .unwrap();
    instance.process_callback(&Response {
        request_id: id,
        reply: serialize(&tg_scheme::ContactsResolvedPeer::default()),
        ..Default::default()
    });
    assert!(*failed.lock().unwrap());
    assert!(weak.valid());
    assert!(weak.session().upgrade().is_some());
    assert!(weak.instance().is_some());
    drop(weak);
    instance.shutdown();
}
#[test]
fn facade_shift_identifiers_and_ranges_match_source() {
    use crate::facade::*;
    assert_eq!(config_dc_id(2), 10002);
    assert_eq!(logout_dc_id(2), 20002);
    assert_eq!(updater_dc_id(2), 30002);
    assert_eq!(group_call_stream_dc_id(2), 60002);
    for index in 0..16 {
        let id = download_dc_id(2, index);
        assert!(is_download_dc_id(id));
        assert!(is_media_cluster_dc_id(id));
        assert!(!is_upload_dc_id(id));
        let upload = upload_dc_id(index);
        assert!(is_upload_dc_id(upload));
        assert!(!is_media_cluster_dc_id(upload));
    }
    assert!(is_cdn_dc(8));
    assert!(!is_cdn_dc(4));
    assert!(is_temporary_dc_id(11002));
    assert_eq!(get_real_id_from_temporary_dc_id(11002), 2);
    assert_eq!(get_real_id_from_temporary_dc_id(2), 0);
    assert_eq!(get_temporary_id_from_real_dc_id(2), 1002);
    assert_eq!(get_temporary_id_from_real_dc_id(1002), 0);
    assert_eq!(destroy_key_next_dc_id(2), 2560002);
    assert_eq!(destroy_key_next_dc_id(2560002), 2570002);
    assert_eq!(download_dc_id(2, -1), 150002);
    assert!(std::panic::catch_unwind(|| upload_dc_id(-1)).is_err());
    assert!(std::panic::catch_unwind(|| download_dc_id(2, 16)).is_err());
}
#[test]
fn duplicate_override_preserves_emplace_and_sender_wrap_cancellation() {
    let instance = test_instance();
    let called = Arc::new(Mutex::new(Vec::new()));
    let out = called.clone();
    let first = instance.send(
        &tg_scheme::HelpGetNearestDc::new(),
        ResponseHandler {
            done: Some(Box::new(move |_| {
                out.lock().unwrap().push(1);
                true
            })),
            fail: None,
        },
        0,
        0,
        0,
        0,
    );
    let old = instance.state.lock().unwrap().requests[&first];
    let out = called.clone();
    instance.send(
        &tg_scheme::HelpGetNearestDc::new(),
        ResponseHandler {
            done: Some(Box::new(move |_| {
                out.lock().unwrap().push(2);
                true
            })),
            fail: None,
        },
        0,
        0,
        0,
        first,
    );
    assert_eq!(instance.state.lock().unwrap().requests[&first], old);
    instance.process_callback(&Response {
        request_id: first,
        reply: vec![1],
        ..Default::default()
    });
    assert_eq!(*called.lock().unwrap(), vec![1]);
    let sender = crate::sender::Sender::new(instance.clone());
    let id = sender
        .request(tg_scheme::HelpGetConfig::new())
        .done_plain(|| panic!("duplicate ID was not cancelled"))
        .send();
    sender
        .request(tg_scheme::HelpGetConfig::new())
        .override_id(id)
        .done_plain(|| panic!("duplicate callback"))
        .send();
    assert!(!instance.has_callback(id));
    assert!(sender.pending(id));
    instance.shutdown();
}
#[test]
fn logout_and_destroy_rpc_callbacks_follow_source_cleanup() {
    let instance = test_instance();
    let done = Arc::new(Mutex::new(0));
    let callback = done.clone();
    instance.dc_persistent_key_changed(
        1,
        Some(Arc::new(AuthKey::new(
            crate::mtproto_auth_key::KeyType::Generated,
            1,
            [1; 256],
            1,
        ))),
    );
    instance.logout(move || *callback.lock().unwrap() += 1);
    let routes = instance.state.lock().unwrap().routes.clone();
    assert_eq!(routes.len(), 2);
    for (id, _) in routes {
        instance.process_callback(&Response {
            request_id: id,
            reply: vec![0x997275b5u32 as i32],
            ..Default::default()
        });
    }
    assert_eq!(*done.lock().unwrap(), 1);
    assert!(instance.state.lock().unwrap().routes.is_empty());
    let mut fields = Fields::new(Environment::Test);
    fields.main_dc_id = -1;
    let destroyer = Instance::new(Mode::KeysDestroyer, fields);
    let key = Arc::new(AuthKey::new(
        crate::mtproto_auth_key::KeyType::Generated,
        2,
        [2; 256],
        1,
    ));
    destroyer.add_keys_for_destroy(vec![key]);
    let logout = *destroyer
        .state
        .lock()
        .unwrap()
        .requests
        .keys()
        .next()
        .unwrap();
    destroyer.process_callback(&Response {
        request_id: logout,
        reply: vec![0x997275b5u32 as i32],
        ..Default::default()
    });
    let destroy = *destroyer
        .state
        .lock()
        .unwrap()
        .requests
        .keys()
        .next()
        .unwrap();
    destroyer.process_callback(&Response {
        request_id: destroy,
        reply: serialize(&tg_scheme::mtp_destroy_auth_key_ok()),
        ..Default::default()
    });
    let event = destroyer
        .environment
        .events
        .lock()
        .unwrap()
        .pop_front()
        .unwrap();
    destroyer.process_event(event);
    assert!(destroyer.get_keys_for_write().is_empty());
    instance.shutdown();
    destroyer.shutdown();
}
