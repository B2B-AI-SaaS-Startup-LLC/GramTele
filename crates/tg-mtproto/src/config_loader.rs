//! Config DC enumeration and temporary endpoint scheduling on base::Timer.
use crate::{
    mtp_instance::Instance,
    mtproto_response::{Error, ResponseHandler},
    special_config_request::Endpoint,
};
use std::{
    cell::Cell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
};
use tg_base::timer::{Timer, TimerHost};
use tg_tl::Deserialize;
pub struct ConfigLoader {
    instance: Instance,
    current: i32,
    request: i32,
    special_current: i32,
    special_request: i32,
    endpoints: VecDeque<Endpoint>,
    tried: Vec<Endpoint>,
    phone: String,
    proxy: bool,
    enumerate_due: Rc<Cell<bool>>,
    special_due: Rc<Cell<bool>>,
    enumerate_timer: Timer,
    special_timer: Timer,
    results: Arc<Mutex<VecDeque<Result<(bool, tg_scheme::Config), Error>>>>,
}
impl ConfigLoader {
    pub fn new(instance: Instance, phone: String, proxy: bool, host: &Rc<TimerHost>) -> Self {
        let enumerate_due = Rc::new(Cell::new(false));
        let special_due = Rc::new(Cell::new(false));
        let e = enumerate_due.clone();
        let s = special_due.clone();
        Self {
            instance,
            current: 0,
            request: 0,
            special_current: 0,
            special_request: 0,
            endpoints: VecDeque::new(),
            tried: vec![],
            phone,
            proxy,
            enumerate_due,
            special_due,
            enumerate_timer: Timer::in_host(host, Some(Rc::new(move || e.set(true)))),
            special_timer: Timer::in_host(host, Some(Rc::new(move || s.set(true)))),
            results: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    pub fn load(&mut self) {
        if !crate::session::SessionEnvironment::is_keys_destroyer(&self.instance) {
            self.send_request(self.instance.main_dc_id(), false);
            self.enumerate_timer.call_once(8000);
        } else {
            self.current = *crate::session::SessionEnvironment::dc_options(&self.instance)
                .config_enum_dc_ids()
                .first()
                .expect("no config DCs");
            self.enumerate();
        }
    }
    pub fn send_request(&self, dc: i32, special: bool) -> i32 {
        let done = self.results.clone();
        let fail = self.results.clone();
        self.instance.send(
            &tg_scheme::HelpGetConfig::new(),
            ResponseHandler {
                done: Some(Box::new(move |response| {
                    let mut reader = tg_tl::Reader::new(&response.reply);
                    let Ok(config) = tg_scheme::Config::read(&mut reader) else {
                        return false;
                    };
                    done.lock().unwrap().push_back(Ok((special, config)));
                    true
                })),
                fail: Some(Box::new(move |error, _| {
                    fail.lock().unwrap().push_back(Err(error.clone()));
                    false
                })),
            },
            dc,
            0,
            0,
            0,
        )
    }
    pub fn terminate_request(&mut self) {
        if self.request != 0 {
            self.instance.cancel(std::mem::take(&mut self.request));
        }
        if self.current != 0 {
            self.instance
                .kill_session(crate::facade::config_dc_id(self.current));
        }
    }
    pub fn terminate_special_request(&mut self) {
        if self.special_request != 0 {
            self.instance
                .cancel(std::mem::take(&mut self.special_request));
        }
        if self.special_current != 0 {
            self.instance.kill_session(self.special_current);
        }
    }
    pub fn enumerate(&mut self) {
        self.terminate_request();
        if self.current == 0 {
            self.current = self.instance.main_dc_id();
        }
        let ids =
            crate::session::SessionEnvironment::dc_options(&self.instance).config_enum_dc_ids();
        assert!(!ids.is_empty());
        let index = ids
            .iter()
            .position(|id| *id == self.current)
            .map_or(0, |i| (i + 1) % ids.len());
        self.current = ids[index];
        self.request = self.send_request(crate::facade::config_dc_id(self.current), false);
        self.enumerate_timer.call_once(8000);
    }
    pub fn set_phone(&mut self, phone: String) {
        if self.phone != phone {
            self.phone = phone;
            self.tried.clear();
        }
    }
    pub fn set_proxy_enabled(&mut self, proxy: bool) {
        self.proxy = proxy;
    }
    pub fn special_to_real_dc_id(dc: i32) -> i32 {
        crate::facade::get_temporary_id_from_real_dc_id(dc)
    }
    pub fn add_special_endpoint(&mut self, endpoint: Endpoint) {
        let same = |a: &Endpoint| {
            a.dc_id == endpoint.dc_id && a.ip == endpoint.ip && a.port == endpoint.port
        };
        if self.endpoints.iter().any(same) || self.tried.iter().any(same) {
            return;
        }
        self.endpoints.push_back(endpoint);
        if !self.special_timer.is_active() {
            self.special_timer.call_once(1);
        }
    }
    pub fn send_special_request(&mut self, random: &mut impl crate::core_types::Random) {
        self.terminate_special_request();
        if self.proxy || self.endpoints.is_empty() {
            return;
        }
        let index = random.u32() as usize % self.endpoints.len();
        let endpoint = self.endpoints.remove(index).unwrap();
        self.special_current = Self::special_to_real_dc_id(endpoint.dc_id);
        self.instance
            .add_temporary_endpoint(self.special_current, &endpoint);
        self.special_request = self.send_request(self.special_current, true);
        self.tried.push(endpoint);
        self.special_timer.call_once(6000);
    }
    pub fn process(
        &mut self,
        random: &mut impl crate::core_types::Random,
    ) -> Vec<Result<(bool, tg_scheme::Config), Error>> {
        if self.enumerate_due.replace(false) {
            self.enumerate();
        }
        if self.special_due.replace(false) {
            self.send_special_request(random);
        }
        self.results.lock().unwrap().drain(..).collect()
    }
}
impl Drop for ConfigLoader {
    fn drop(&mut self) {
        self.terminate_request();
        self.terminate_special_request();
    }
}
