use std::rc::Rc;
use std::sync::Arc;
use tg_base::options::{Descriptor, Registry};
use tg_base::timer::{ManualClock, TimerHost};
fn setup() -> (Registry, ManualClock, Rc<TimerHost>) {
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock.clone()));
    (Registry::new(&host), clock, host)
}
#[test]
fn qt_json_sample_and_delayed_write() {
    let (registry, clock, host) = setup();
    let toggle = registry.register(Descriptor::<bool>::new("downscale"));
    let limit = registry.register(Descriptor::<i32>::new("limit"));
    let text = registry.register(Descriptor::<String>::new("text"));
    let dir = std::env::temp_dir().join(format!("tg-options-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("experimental_options.json");
    let _ = std::fs::remove_file(&path);
    registry.init(&path);
    assert!(registry.deserialize(include_str!("fixtures/experimental_options.json")));
    assert!(toggle.value());
    assert_eq!(limit.value(), 17);
    assert_eq!(text.value(), "Привет\n\"test\"");
    assert!(!path.exists());
    clock.set(999);
    host.process();
    assert!(!path.exists());
    clock.set(1000);
    host.process();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        include_bytes!("fixtures/experimental_options.json")
    );
    assert_eq!(
        registry.serialize(),
        "{\"downscale\":true,\"limit\":17,\"text\":\"Привет\\n\\\"test\\\"\"}"
    );
    registry.reset();
    registry.flush().unwrap();
    assert!(!path.exists());
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn import_is_atomic_and_resets_missing_options() {
    let (registry, _, _) = setup();
    let flag = registry.register(Descriptor::<bool>::new("flag"));
    let number = registry.register(Descriptor::<i32>::new("number"));
    flag.set(true);
    number.set(4);
    assert!(!registry.deserialize("{\"flag\":false,\"number\":1.5}"));
    assert!(flag.value());
    assert_eq!(number.value(), 4);
    assert!(!registry.deserialize("[]"));
    assert!(registry.deserialize("{\"unknown\":999,\"number\":2}"));
    assert!(!flag.value());
    assert_eq!(number.value(), 2);
    assert!(!registry.deserialize("{\"number\":2147483648}"));
}
#[test]
fn unchanged_set_does_not_emit_change_event() {
    let (registry, _, _) = setup();
    let flag = registry.register(Descriptor::<bool>::new("flag"));
    let count = Rc::new(std::cell::Cell::new(0));
    let seen = count.clone();
    let _lifetime = flag
        .changes()
        .on_next_returning(move |_| seen.set(seen.get() + 1));
    flag.set(false);
    flag.set(true);
    flag.set(true);
    assert_eq!(count.get(), 1);
    assert_eq!(registry.lookup::<bool>("flag").value(), flag.value());
}

#[test]
fn read_retains_qt_lenient_integer_conversion_and_scope() {
    use tg_base::options::{Scope, LINUX, MACOS};
    let (registry, _, _) = setup();
    let number = registry.register(Descriptor::<i32>::new("number"));
    let mut descriptor = Descriptor::<bool>::new("linux-only");
    descriptor.scope = Scope::Platforms(LINUX);
    descriptor.restart_required = true;
    let linux = registry.register(descriptor);
    assert_eq!(linux.relevant(), cfg!(target_os = "linux"));
    assert!(linux.restart_required());
    assert_eq!(
        Scope::Platforms(MACOS).relevant(),
        cfg!(target_os = "macos")
    );
    assert!(Scope::Predicate(Rc::new(|| true)).relevant());
    let path = std::env::temp_dir().join(format!("tg-options-read-{}", std::process::id()));
    number.set(9);
    std::fs::write(&path, b"{\"number\": 2.5}").unwrap();
    registry.init(&path);
    assert_eq!(number.value(), 0);
    assert!(!registry.changed());
    std::fs::remove_file(path).unwrap();
}
