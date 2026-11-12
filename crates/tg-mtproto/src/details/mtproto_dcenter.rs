//! Thread-safe key ownership and creation exclusion; session callbacks are outside the lock.
use crate::{mtproto_auth_key::AuthKey, mtproto_dc_options::DcType};
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporaryKeyType {
    Regular,
    MediaCluster,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatingKeyType {
    None,
    Persistent,
    TemporaryRegular,
    TemporaryMediaCluster,
}
pub fn temporary_key_type_by_dc_type(kind: DcType) -> TemporaryKeyType {
    if kind == DcType::MediaCluster {
        TemporaryKeyType::MediaCluster
    } else {
        TemporaryKeyType::Regular
    }
}
fn index(kind: CreatingKeyType) -> usize {
    match kind {
        CreatingKeyType::Persistent | CreatingKeyType::TemporaryRegular => 0,
        CreatingKeyType::TemporaryMediaCluster => 1,
        CreatingKeyType::None => panic!("no creating key type"),
    }
}
struct Data {
    temporary: [Option<Arc<AuthKey>>; 2],
    persistent: Option<Arc<AuthKey>>,
    connection_inited: bool,
    creating: [bool; 2],
}
pub struct Dcenter {
    id: i32,
    data: Mutex<Data>,
}
impl Dcenter {
    pub fn new(id: i32, key: Option<Arc<AuthKey>>) -> Self {
        Self {
            id,
            data: Mutex::new(Data {
                temporary: [None, None],
                persistent: key,
                connection_inited: false,
                creating: [false; 2],
            }),
        }
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn get_persistent_key(&self) -> Option<Arc<AuthKey>> {
        self.data.lock().unwrap().persistent.clone()
    }
    pub fn get_temporary_key(&self, kind: TemporaryKeyType) -> Option<Arc<AuthKey>> {
        self.data.lock().unwrap().temporary[kind as usize].clone()
    }
    pub fn connection_inited(&self) -> bool {
        self.data.lock().unwrap().connection_inited
    }
    pub fn set_connection_inited(&self, value: bool) {
        self.data.lock().unwrap().connection_inited = value;
    }
    pub fn destroy_temporary_key(&self, id: u64) -> bool {
        let mut data = self.data.lock().unwrap();
        for key in &mut data.temporary {
            if key.as_ref().is_some_and(|k| k.key_id() == id) {
                *key = None;
                data.connection_inited = false;
                return true;
            }
        }
        false
    }
    pub fn destroy_confirmed_forgotten_key(&self, id: u64) -> bool {
        let mut data = self.data.lock().unwrap();
        if data.persistent.as_ref().is_none_or(|k| k.key_id() != id) {
            return false;
        }
        data.temporary = [None, None];
        data.persistent = None;
        data.connection_inited = false;
        true
    }
    pub fn acquire_key_creation(&self, kind: DcType) -> CreatingKeyType {
        let mut d = self.data.lock().unwrap();
        let key_type = temporary_key_type_by_dc_type(kind);
        let i = key_type as usize;
        if d.temporary[i].is_some() {
            return CreatingKeyType::None;
        }
        let (media, index) =
            if key_type == TemporaryKeyType::MediaCluster && d.temporary[0].is_some() {
                (true, i)
            } else {
                (false, 0)
            };
        if d.creating[index] {
            return CreatingKeyType::None;
        }
        d.creating[index] = true;
        if media {
            CreatingKeyType::TemporaryMediaCluster
        } else if kind != DcType::Cdn && d.persistent.is_none() {
            CreatingKeyType::Persistent
        } else {
            CreatingKeyType::TemporaryRegular
        }
    }
    pub fn release_key_creation_on_done(
        &self,
        kind: CreatingKeyType,
        temporary: Arc<AuthKey>,
        bound_persistent: Option<Arc<AuthKey>>,
    ) -> bool {
        let i = index(kind);
        let mut d = self.data.lock().unwrap();
        assert!(d.creating[i]);
        assert!(d.temporary[i].is_none());
        if kind == CreatingKeyType::Persistent {
            d.persistent = bound_persistent;
        } else if !match (&d.persistent, &bound_persistent) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        } {
            return false;
        }
        d.temporary[i] = Some(temporary);
        d.creating[i] = false;
        d.connection_inited = false;
        true
    }
    pub fn release_key_creation_on_fail(&self, kind: CreatingKeyType) {
        let i = index(kind);
        let mut d = self.data.lock().unwrap();
        assert!(d.creating[i]);
        assert!(d.temporary[i].is_none());
        d.creating[i] = false;
    }
}
