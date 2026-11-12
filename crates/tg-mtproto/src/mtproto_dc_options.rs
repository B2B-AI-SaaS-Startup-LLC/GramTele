//! Original endpoint ordering, filtering, RSA selection, and tdata representation.
use crate::{
    core_types::{bare_dc_id, get_dc_id_shift},
    details::{
        builtins::*,
        mtproto_rsa_public_key::RsaPublicKey,
        qt_stream::{Reader, Writer},
    },
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment {
    Production,
    Test,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcType {
    Regular,
    Temporary,
    MediaCluster,
    Cdn,
}
pub const IPV6: u32 = 1;
pub const MEDIA_ONLY: u32 = 2;
pub const TCPO_ONLY: u32 = 4;
pub const CDN: u32 = 8;
pub const STATIC: u32 = 16;
pub const SECRET: u32 = 1024;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub id: i32,
    pub flags: u32,
    pub ip: String,
    pub port: i32,
    pub secret: Vec<u8>,
}
#[derive(Clone)]
pub struct DcOptions {
    environment: Environment,
    data: BTreeMap<i32, Vec<Endpoint>>,
    cdn_dc_ids: BTreeSet<i32>,
    public_keys: BTreeMap<u64, RsaPublicKey>,
    cdn_public_keys: BTreeMap<i32, BTreeMap<u64, RsaPublicKey>>,
    immutable: bool,
    pub changed: Vec<i32>,
    pub cdn_config_changed: u64,
}
pub type Variants = [[Vec<Endpoint>; 2]; 2];
impl DcOptions {
    pub fn new(environment: Environment) -> Self {
        let mut result = Self {
            environment,
            data: Default::default(),
            cdn_dc_ids: Default::default(),
            public_keys: Default::default(),
            cdn_public_keys: Default::default(),
            immutable: false,
            changed: Vec::new(),
            cdn_config_changed: 0,
        };
        result.construct_from_built_in();
        result
    }
    pub fn environment(&self) -> Environment {
        self.environment
    }
    pub fn is_test_mode(&self) -> bool {
        self.environment == Environment::Test
    }
    pub fn validate_secret(secret: &[u8]) -> bool {
        secret.is_empty()
            || secret.len() == 16
            || secret.len() == 17 && secret[0] == 0xdd
            || secret.len() >= 21 && secret[0] == 0xee
    }
    pub fn construct_from_built_in(&mut self) {
        self.data.clear();
        let (v4, v6, pem) = if self.is_test_mode() {
            (DCS_TEST, DCS_IPV6_TEST, TEST_RSA_PEM)
        } else {
            (DCS, DCS_IPV6, RSA_PEM)
        };
        let key = RsaPublicKey::from_pem(pem).expect("original built-in RSA key");
        self.public_keys.insert(key.fingerprint(), key);
        for &(id, ip, port) in v4 {
            self.apply_one(Endpoint {
                id,
                flags: STATIC,
                ip: ip.into(),
                port,
                secret: Vec::new(),
            });
        }
        for &(id, ip, port) in v6 {
            self.apply_one(Endpoint {
                id,
                flags: STATIC | IPV6,
                ip: ip.into(),
                port,
                secret: Vec::new(),
            });
        }
        self.compute_cdn_dc_ids();
    }
    fn apply_one(&mut self, endpoint: Endpoint) -> bool {
        let list = self.data.entry(endpoint.id).or_default();
        if list
            .iter()
            .any(|old| old.ip == endpoint.ip && old.port == endpoint.port)
        {
            return false;
        }
        list.push(endpoint);
        true
    }
    pub fn construct_add_one(&mut self, mut endpoint: Endpoint) {
        endpoint.id = bare_dc_id(endpoint.id);
        self.apply_one(endpoint);
        self.compute_cdn_dc_ids();
    }
    pub fn serialize(&self) -> Vec<u8> {
        if self.immutable {
            return Self::new(self.environment).serialize();
        }
        let mut out = Writer::default();
        out.i32(-2);
        out.i32(
            self.data
                .iter()
                .filter(|(id, _)| !is_temporary_dc_id(**id))
                .map(|(_, v)| v.len())
                .sum::<usize>() as i32,
        );
        for (&id, list) in &self.data {
            if is_temporary_dc_id(id) {
                continue;
            }
            for e in list {
                out.i32(e.id);
                out.i32(e.flags as i32);
                out.i32(e.port);
                out.bytes(e.ip.as_bytes());
                out.bytes(&e.secret);
            }
        }
        out.i32(
            self.cdn_public_keys
                .values()
                .map(BTreeMap::len)
                .sum::<usize>() as i32,
        );
        for (&id, keys) in &self.cdn_public_keys {
            for key in keys.values() {
                out.i32(id);
                out.bytes(key.get_n());
                out.bytes(key.get_e());
            }
        }
        out.0
    }
    pub fn construct_from_serialized(&mut self, bytes: &[u8]) -> bool {
        let result = self.read_serialized(&mut Reader::new(bytes)).is_some();
        self.compute_cdn_dc_ids();
        result
    }
    fn read_serialized(&mut self, input: &mut Reader<'_>) -> Option<()> {
        let first = input.i32()?;
        let version = if first < 0 { first.checked_neg()? } else { 0 };
        let count = if version > 0 { input.i32()? } else { first };
        if count < 0 {
            return None;
        }
        self.data.clear();
        for _ in 0..count {
            let id = input.i32()?;
            let flags = input.i32()? as u32;
            let port = input.i32()?;
            let len = input.i32()?;
            if !(1..=45).contains(&len) {
                return None;
            }
            let ip = String::from_utf8(input.raw(len as usize)?.to_vec()).ok()?;
            let secret = if version > 0 {
                let len = input.i32()?;
                if !(0..=32).contains(&len) {
                    return None;
                }
                input.raw(len as usize)?.to_vec()
            } else {
                Vec::new()
            };
            self.apply_one(Endpoint {
                id,
                flags,
                ip,
                port,
                secret,
            });
        }
        if !input.at_end() && version > 1 {
            let count = input.i32()?;
            if count < 0 {
                return None;
            }
            for _ in 0..count {
                let id = input.i32()?;
                let n = input.bytes()?;
                let e = input.bytes()?;
                let key = RsaPublicKey::from_components(&n, &e)?;
                self.cdn_public_keys
                    .entry(id)
                    .or_default()
                    .entry(key.fingerprint())
                    .or_insert(key);
            }
        }
        Some(())
    }
    pub fn config_enum_dc_ids(&self) -> Vec<i32> {
        self.data
            .iter()
            .filter(|(id, list)| !is_temporary_dc_id(**id) && list[0].flags & CDN == 0)
            .map(|(&id, _)| id)
            .collect()
    }
    pub fn dc_type(&self, id: i32) -> DcType {
        if is_temporary_dc_id(id) {
            DcType::Temporary
        } else if self.cdn_dc_ids.contains(&bare_dc_id(id)) {
            DcType::Cdn
        } else if is_media_cluster_dc_id(id)
            && self
                .data
                .get(&bare_dc_id(id))
                .is_some_and(|list| list.iter().any(|e| e.flags & MEDIA_ONLY != 0))
        {
            DcType::MediaCluster
        } else {
            DcType::Regular
        }
    }
    pub fn lookup(&self, id: i32, kind: DcType, through_proxy: bool) -> Variants {
        let mut result: Variants = Default::default();
        if let Some(list) = self.data.get(&id) {
            for e in list {
                if kind == DcType::Cdn && e.flags & CDN == 0
                    || kind != DcType::MediaCluster && e.flags & MEDIA_ONLY != 0
                    || !Self::validate_secret(&e.secret)
                {
                    continue;
                }
                let address = usize::from(e.flags & IPV6 != 0);
                result[address][0].push(e.clone());
                if e.flags & (TCPO_ONLY | SECRET) == 0 {
                    result[address][1].push(e.clone());
                }
            }
        }
        if kind == DcType::MediaCluster {
            filter_if_has_with_flag(&mut result, MEDIA_ONLY);
        }
        if through_proxy {
            filter_if_has_with_flag(&mut result, STATIC);
        }
        result
    }
    pub fn public_keys(&self, id: i32) -> Vec<RsaPublicKey> {
        self.cdn_public_keys
            .get(&id)
            .unwrap_or(&self.public_keys)
            .values()
            .cloned()
            .collect()
    }
    pub fn get_dc_rsa_key(&self, id: i32, fingerprints: &[u64]) -> Option<RsaPublicKey> {
        let keys = self.cdn_public_keys.get(&id).unwrap_or(&self.public_keys);
        fingerprints.iter().find_map(|fp| keys.get(fp)).cloned()
    }
    pub fn has_cdn_keys_for_dc(&self, id: i32) -> bool {
        self.cdn_public_keys.contains_key(&id)
    }
    fn compute_cdn_dc_ids(&mut self) {
        self.cdn_dc_ids = self
            .data
            .iter()
            .filter(|(_, v)| v[0].flags & CDN != 0)
            .map(|(&id, _)| bare_dc_id(id))
            .collect();
    }
    pub fn set_cdn_config(&mut self, config: &tg_scheme::CdnConfigData) {
        self.cdn_public_keys.clear();
        for k in &config.public_keys {
            let k = k.c_cdn_public_key();
            if let Ok(pem) = std::str::from_utf8(&k.public_key) {
                if let Some(key) = RsaPublicKey::from_pem(pem) {
                    self.cdn_public_keys
                        .entry(k.dc_id)
                        .or_default()
                        .entry(key.fingerprint())
                        .or_insert(key);
                }
            }
        }
        self.cdn_config_changed += 1;
    }
    pub fn set_from_list(&mut self, options: &[tg_scheme::DcOption]) {
        self.process_from_list(options, true);
    }
    pub fn add_from_list(&mut self, options: &[tg_scheme::DcOption]) {
        self.process_from_list(options, false);
    }
    fn process_from_list(&mut self, options: &[tg_scheme::DcOption], overwrite: bool) {
        if options.is_empty() || self.immutable {
            return;
        }
        let old = self.data.clone();
        if overwrite {
            self.data.clear();
        }
        for option in options {
            let d = option.c_dc_option();
            self.apply_one(Endpoint {
                id: d.id,
                flags: d.flags.value() as u32,
                ip: String::from_utf8_lossy(&d.ip_address).into_owned(),
                port: d.port,
                secret: d.secret.clone().unwrap_or_default(),
            });
        }
        let ids: BTreeSet<_> = old.keys().chain(self.data.keys()).copied().collect();
        let difference: Vec<_> = ids
            .into_iter()
            .filter(|id| !same_endpoints(old.get(id), self.data.get(id)))
            .collect();
        if difference.is_empty() {
            self.data = old;
        } else {
            self.changed.extend(difference);
        }
        self.compute_cdn_dc_ids();
    }
    pub fn add_from_other(&mut self, other: Self) {
        if self.immutable || other.data.is_empty() {
            return;
        }
        for (id, list) in other.data {
            let mut changed = false;
            for e in list {
                changed |= self.apply_one(e);
            }
            if changed {
                self.changed.push(id);
            }
        }
        for (id, keys) in other.cdn_public_keys {
            self.cdn_public_keys.entry(id).or_default().extend(keys);
        }
        self.compute_cdn_dc_ids();
    }
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> bool {
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        let mut data = BTreeMap::new();
        for line in text.lines() {
            let c: Vec<_> = line
                .split_whitespace()
                .take_while(|v| !v.starts_with('#'))
                .collect();
            if c.is_empty() {
                continue;
            }
            if c.len() < 3 {
                return false;
            }
            let Ok(id) = c[0].parse::<i32>() else {
                return false;
            };
            let Ok(ip) = c[1].parse::<std::net::IpAddr>() else {
                return false;
            };
            let Ok(port) = c[2].parse::<i32>() else {
                return false;
            };
            if !(1..10000).contains(&id) || port <= 0 {
                return false;
            }
            let mut flags = if ip.is_ipv6() { IPV6 } else { 0 };
            for &option in &c[3..] {
                match option {
                    "tcpo_only" => flags |= TCPO_ONLY,
                    "media_only" => flags |= MEDIA_ONLY,
                    _ => return false,
                }
            }
            data.entry(id).or_insert_with(Vec::new).push(Endpoint {
                id,
                ip: c[1].into(),
                port,
                flags,
                secret: Vec::new(),
            });
        }
        if data.is_empty() {
            return false;
        }
        let changed: BTreeSet<_> = self.data.keys().chain(data.keys()).copied().collect();
        self.data = data;
        self.changed.extend(changed);
        self.immutable = true;
        self.compute_cdn_dc_ids();
        true
    }
    pub fn write_to_file(&self, path: impl AsRef<std::path::Path>) -> bool {
        let mut text = String::new();
        for e in self.data.values().flatten() {
            text += &format!(
                "{} {} {}{}{}\n",
                e.id,
                e.ip,
                e.port,
                if e.flags & TCPO_ONLY != 0 {
                    " tcpo_only"
                } else {
                    ""
                },
                if e.flags & MEDIA_ONLY != 0 {
                    " media_only"
                } else {
                    ""
                }
            );
        }
        std::fs::write(path, text).is_ok()
    }
}
fn same_endpoints(a: Option<&Vec<Endpoint>>, b: Option<&Vec<Endpoint>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|x| b.iter().any(|y| x.ip == y.ip && x.port == y.port))
        }
        _ => false,
    }
}
fn filter_if_has_with_flag(variants: &mut Variants, flag: u32) {
    for list in variants.iter_mut().flatten() {
        if list.iter().any(|e| e.flags & flag != 0) {
            list.retain(|e| e.flags & flag != 0);
        }
    }
}
pub fn is_temporary_dc_id(id: i32) -> bool {
    bare_dc_id(id) >= 1000
}
fn is_media_cluster_dc_id(id: i32) -> bool {
    let shift = get_dc_id_shift(id);
    (0x10..0x20).contains(&shift) || matches!(shift, 3 | 5 | 6)
}
