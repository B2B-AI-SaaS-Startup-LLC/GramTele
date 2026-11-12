use super::*;
use tg_tl::serialize;
fn test_instance() -> Instance {
    Instance::new(Mode::Normal, Fields::new(Environment::Test))
}
#[test]
fn instance_routing_callbacks_delays_and_sender_ownership() {
    tg_runtime::main_queue().bind_current_thread();
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
