#![cfg(target_os = "linux")]
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tg_base::single_instance::*;

#[test]
fn qt_wire_helper_process() {
    let Ok(path) = std::env::var("TG_BASE_QT_WIRE_SOCKET") else {
        return;
    };
    let mut socket = UnixStream::connect(path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // Independent wire fixture, split across writes like QLocalSocket packets.
    socket.write_all(b"SEVM").unwrap();
    socket.write_all(b"TE8=;").unwrap();
    let mut response = Vec::new();
    let mut byte = [0];
    while !response.ends_with(b";") {
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }
    assert_eq!(response, b"UElEOjA7V05EOjA7;");
}

#[test]
fn primary_and_secondary_exchange_and_cleanup() {
    let dir = std::env::temp_dir().join(format!("tg-single-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{}/", dir.display());
    let primary = SingleInstance::start_in("AyuGram-test", &path, &dir).unwrap();
    assert!(primary.is_primary());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let lifetime = primary
        .commands()
        .on_next_returning(move |message| output.borrow_mut().push(message));
    let socket_path = dir.join(name_for_path("AyuGram-test", &path));
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "qt_wire_helper_process"])
        .env("TG_BASE_QT_WIRE_SOCKET", &socket_path)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while seen.borrow().is_empty() {
        assert!(Instant::now() < deadline);
        primary.process().unwrap();
        std::thread::yield_now();
    }
    let message = seen.borrow()[0].clone();
    assert_eq!(message.data, b"HELLO");
    primary.reply(message.id, None).unwrap();
    assert!(child.wait().unwrap().success());
    let secondary = SingleInstance::start_in("AyuGram-test", &path, &dir).unwrap();
    assert!(!secondary.is_primary());
    secondary.send(b"OPEN:tg://test").unwrap();
    while seen.borrow().len() < 2 {
        assert!(Instant::now() < deadline);
        primary.process().unwrap();
    }
    primary
        .reply(
            seen.borrow()[1].id,
            Some(Reply {
                pid: 123,
                window: 456,
            }),
        )
        .unwrap();
    assert_eq!(
        secondary.poll_reply().unwrap(),
        Some(Reply {
            pid: 123,
            window: 456
        })
    );
    drop(secondary);
    drop(lifetime);
    drop(primary);
    assert!(!socket_path.exists());
    assert!(!dir.join("_single_instance.tmp").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn protocol_fixtures() {
    assert_eq!(name_for_path("AyuGram", "/tmp/data/"), "S3j1kggjz94=");
    assert_eq!(encode_message(b"HELLO"), b"SEVMTE8=;");
    assert_eq!(decode_message(b"SEVMTE8="), None);
    assert_eq!(decode_message(b"SEVMTE8=;"), Some(b"HELLO".to_vec()));
    assert_eq!(encode_message(&[0xff, 0xfe]), b"__4=;");
    assert_eq!(decode_message(b"__4=;"), Some(vec![0xff, 0xfe]));
    assert_eq!(decode_message(b";"), Some(Vec::new()));
}
