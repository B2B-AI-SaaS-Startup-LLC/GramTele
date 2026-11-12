//! Port of base/options.{h,cpp}. Explicit registry instead of C++ static
//! registration; JSON and delayed persistence retain the original semantics.
use crate::timer::{Timer, TimerHost};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use tg_rpl::{EventStream, Producer};

#[derive(Clone)]
pub enum Scope {
    Platforms(u8),
    Predicate(Rc<dyn Fn() -> bool>),
}
pub const WINDOWS: u8 = 1 << 1;
pub const MACOS: u8 = 1 << 2;
pub const LINUX: u8 = 1 << 3;
impl Default for Scope {
    fn default() -> Self {
        Self::Platforms(WINDOWS | MACOS | LINUX)
    }
}
impl Scope {
    pub fn relevant(&self) -> bool {
        match self {
            Self::Predicate(f) => f(),
            Self::Platforms(flags) => {
                flags
                    & if cfg!(windows) {
                        WINDOWS
                    } else if cfg!(target_os = "macos") {
                        MACOS
                    } else {
                        LINUX
                    }
                    != 0
            }
        }
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for bool {}
    impl Sealed for i32 {}
    impl Sealed for String {}
}
pub trait OptionValue: sealed::Sealed + Clone + 'static {
    fn into_json(self) -> Value;
    fn from_json(value: &Value, strict: bool) -> Option<Self>;
}
impl OptionValue for bool {
    fn into_json(self) -> Value {
        self.into()
    }
    fn from_json(value: &Value, _: bool) -> Option<Self> {
        value.as_bool()
    }
}
impl OptionValue for i32 {
    fn into_json(self) -> Value {
        self.into()
    }
    fn from_json(value: &Value, strict: bool) -> Option<Self> {
        let value = value.as_f64()?;
        if value.fract() == 0.0 && value >= i32::MIN as f64 && value <= i32::MAX as f64 {
            Some(value as i32)
        } else if strict {
            None
        } else {
            Some(0)
        } // QJsonValue::toInt(default=0)
    }
}
impl OptionValue for String {
    fn into_json(self) -> Value {
        self.into()
    }
    fn from_json(value: &Value, _: bool) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}

pub struct Descriptor<T> {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_value: T,
    pub scope: Scope,
    pub restart_required: bool,
}
impl<T: Default> Descriptor<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            description: String::new(),
            default_value: T::default(),
            scope: Scope::default(),
            restart_required: false,
        }
    }
}
struct Item {
    id: String,
    name: String,
    description: String,
    scope: Scope,
    restart_required: bool,
    default: Value,
    value: RefCell<Value>,
    changes: EventStream<()>,
    parse: fn(&Value, bool) -> Option<Value>,
}
struct Inner {
    map: RefCell<BTreeMap<String, Rc<Item>>>,
    path: RefCell<Option<PathBuf>>,
    scheduled: Cell<bool>,
    timer: Timer,
}
impl Inner {
    fn serialize(&self, pretty: bool) -> String {
        let items: Vec<_> = self.map.borrow().values().cloned().collect();
        let mut values: Vec<_> = items
            .iter()
            .filter_map(|item| {
                let value = item.value.borrow().clone();
                (value != item.default).then(|| (item.id.clone(), value))
            })
            .collect();
        values.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        if !pretty {
            return format!(
                "{{{}}}",
                values
                    .iter()
                    .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        if values.is_empty() {
            return "{\n}\n".into();
        }
        format!(
            "{{\n{}\n}}\n",
            values
                .iter()
                .map(|(k, v)| format!("    {}: {}", serde_json::to_string(k).unwrap(), v))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
    fn write(&self) -> std::io::Result<()> {
        let Some(path) = self.path.borrow().clone() else {
            return Ok(());
        };
        if !self.scheduled.replace(false) {
            return Ok(());
        }
        self.timer.cancel();
        if self.serialize(false) == "{}" {
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        } else {
            std::fs::write(path, self.serialize(true))
        }
    }
    fn schedule(&self) {
        if self.path.borrow().is_some() && !self.scheduled.replace(true) {
            self.timer.call_once(1000);
        }
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.write();
    }
}

#[derive(Clone)]
pub struct Registry(Rc<Inner>);
impl Registry {
    pub fn new(host: &Rc<TimerHost>) -> Self {
        Self(Rc::new_cyclic(|weak: &Weak<Inner>| {
            let weak = weak.clone();
            Inner {
                map: RefCell::new(BTreeMap::new()),
                path: RefCell::new(None),
                scheduled: Cell::new(false),
                timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(inner) = weak.upgrade() {
                            if let Err(e) = inner.write() {
                                crate::log!("Experimental: Could not write: {e}");
                            }
                        }
                    })),
                ),
            }
        }))
    }
    pub fn register<T: OptionValue>(&self, fields: Descriptor<T>) -> OptionHandle<T> {
        let default = fields.default_value.into_json();
        let item = Rc::new(Item {
            id: fields.id,
            name: fields.name,
            description: fields.description,
            scope: fields.scope,
            restart_required: fields.restart_required,
            value: RefCell::new(default.clone()),
            default,
            changes: EventStream::new(),
            parse: |v, s| T::from_json(v, s).map(T::into_json),
        });
        assert!(
            !self.0.map.borrow().contains_key(&item.id),
            "duplicate option id"
        );
        self.0
            .map
            .borrow_mut()
            .insert(item.id.clone(), item.clone());
        OptionHandle {
            item,
            registry: Rc::downgrade(&self.0),
            marker: std::marker::PhantomData,
        }
    }
    pub fn lookup<T: OptionValue>(&self, id: &str) -> OptionHandle<T> {
        let item = self
            .0
            .map
            .borrow()
            .get(id)
            .expect("unknown option id")
            .clone();
        assert!(
            T::from_json(&item.default, true).is_some(),
            "wrong option type"
        );
        OptionHandle {
            item,
            registry: Rc::downgrade(&self.0),
            marker: std::marker::PhantomData,
        }
    }
    pub fn changed(&self) -> bool {
        self.0
            .map
            .borrow()
            .values()
            .any(|i| *i.value.borrow() != i.default)
    }
    pub fn serialize(&self) -> String {
        self.0.serialize(false)
    }
    pub fn reset(&self) {
        let items: Vec<_> = self.0.map.borrow().values().cloned().collect();
        for item in items {
            if *item.value.borrow() != item.default {
                self.set(&item, item.default.clone());
            }
        }
    }
    fn set(&self, item: &Item, value: Value) {
        let changed = *item.value.borrow() != value;
        *item.value.borrow_mut() = value;
        if changed {
            item.changes.fire(());
        }
        self.0.schedule();
    }
    /// Import validates everything first, then resets and applies. Unknown ids ignored.
    pub fn deserialize(&self, json: &str) -> bool {
        let Ok(Value::Object(object)) = serde_json::from_str(json) else {
            return false;
        };
        let mut imported = Vec::new();
        for (id, value) in object {
            let Some(item) = self.0.map.borrow().get(&id).cloned() else {
                continue;
            };
            let Some(parsed) = (item.parse)(&value, true) else {
                return false;
            };
            imported.push((item, parsed));
        }
        self.reset();
        for (item, value) in imported {
            self.set(&item, value);
        }
        true
    }
    /// File read is lenient and does not reset unspecified options.
    pub fn init(&self, path: &Path) {
        assert!(
            self.0.path.borrow().is_none(),
            "options already initialized"
        );
        if path.as_os_str().is_empty() {
            return;
        }
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(Value::Object(object)) = serde_json::from_slice(&bytes) {
                for (id, value) in object {
                    let item = self.0.map.borrow().get(&id).cloned();
                    if let Some(item) = item {
                        if let Some(value) = (item.parse)(&value, false) {
                            self.set(&item, value);
                        }
                    }
                }
            }
        }
        *self.0.path.borrow_mut() = Some(path.to_owned());
    }
    pub fn flush(&self) -> std::io::Result<()> {
        self.0.write()
    }
}

pub struct OptionHandle<T: OptionValue> {
    item: Rc<Item>,
    registry: Weak<Inner>,
    marker: std::marker::PhantomData<T>,
}
impl<T: OptionValue> OptionHandle<T> {
    pub fn set(&self, value: T) {
        let registry = Registry(self.registry.upgrade().expect("option registry destroyed"));
        registry.set(&self.item, value.into_json());
    }
    pub fn value(&self) -> T {
        T::from_json(&self.item.value.borrow(), true).unwrap()
    }
    pub fn default_value(&self) -> T {
        T::from_json(&self.item.default, true).unwrap()
    }
    pub fn changes(&self) -> Producer {
        self.item.changes.events()
    }
    pub fn id(&self) -> &str {
        &self.item.id
    }
    pub fn name(&self) -> &str {
        &self.item.name
    }
    pub fn description(&self) -> &str {
        &self.item.description
    }
    pub fn scope(&self) -> Scope {
        self.item.scope.clone()
    }
    pub fn relevant(&self) -> bool {
        self.item.scope.relevant()
    }
    pub fn restart_required(&self) -> bool {
        self.item.restart_required
    }
}
