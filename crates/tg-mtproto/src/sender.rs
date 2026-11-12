//! Typed request builders whose callbacks belong to a Sender lifetime.
use crate::{
    details::mtproto_serialized_request::SerializedRequest,
    mtp_instance::{get_next_request_id, Instance},
    mtproto_response::{Error, Response, ResponseHandler},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tg_tl::{Deserialize, Function, Reader};
#[derive(Clone, Copy)]
pub enum FailSkipPolicy {
    Simple,
    HandleFlood,
    HandleAll,
}
pub type Runner = Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;
pub(crate) struct Owner {
    instance: Instance,
    requests: Mutex<BTreeSet<i32>>,
    runner: Option<Runner>,
    concurrent: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let ids = std::mem::take(self.requests.get_mut().unwrap());
        let instance = self.instance.clone();
        if self.concurrent {
            tg_runtime::on_main(move || {
                for id in ids {
                    instance.cancel(id);
                }
            });
        } else {
            for id in ids {
                instance.cancel(id);
            }
        }
    }
}
pub struct Sender {
    pub(crate) owner: Arc<Owner>,
}
impl Sender {
    pub fn new(instance: Instance) -> Self {
        Self::with_runner(instance, None, false)
    }
    pub(crate) fn with_runner(
        instance: Instance,
        runner: Option<Runner>,
        concurrent: bool,
    ) -> Self {
        Self {
            owner: Arc::new(Owner {
                instance,
                requests: Mutex::new(BTreeSet::new()),
                runner,
                concurrent,
            }),
        }
    }
    pub fn instance(&self) -> &Instance {
        &self.owner.instance
    }
    pub fn request<R: Function>(&self, request: R) -> RequestBuilder<'_, R> {
        RequestBuilder {
            sender: self,
            request,
            dc: 0,
            wait: 0,
            after: 0,
            override_id: 0,
            done: None,
            fail: None,
            policy: FailSkipPolicy::Simple,
        }
    }
    pub fn allocate_request_id(&self) -> i32 {
        get_next_request_id()
    }
    pub fn pending(&self, id: i32) -> bool {
        self.owner.requests.lock().unwrap().contains(&id)
    }
    pub fn cancel(&self, id: i32) {
        if self.owner.requests.lock().unwrap().remove(&id) {
            let instance = self.instance().clone();
            if self.owner.concurrent {
                tg_runtime::on_main(move || instance.cancel(id));
            } else {
                instance.cancel(id);
            }
        }
    }
    pub fn detach(&self, id: i32) {
        self.owner.requests.lock().unwrap().remove(&id);
    }
    pub fn request_cancelling_discard(&self) {
        self.owner.requests.lock().unwrap().clear();
    }
    pub fn request_send_delayed(&self) {
        if let Some(session) = self.instance().session(0) {
            session.send(crate::session::SessionCommand::Wake);
        }
    }
}
type TypedDone<T> = Box<dyn FnOnce(T, &Response) + Send>;
type TypedFail = Box<dyn Fn(&Error, &Response) + Send>;
pub struct RequestBuilder<'a, R: Function> {
    sender: &'a Sender,
    request: R,
    dc: i32,
    wait: i64,
    after: i32,
    override_id: i32,
    done: Option<TypedDone<R::Response>>,
    fail: Option<TypedFail>,
    policy: FailSkipPolicy,
}
impl<R: Function> RequestBuilder<'_, R>
where
    R::Response: Send + 'static,
{
    pub fn to_dc(mut self, id: i32) -> Self {
        self.dc = id;
        self
    }
    pub fn after_delay(mut self, ms: i64) -> Self {
        self.wait = ms;
        self
    }
    pub fn after_request(mut self, id: i32) -> Self {
        self.after = id;
        self
    }
    pub fn override_id(mut self, id: i32) -> Self {
        self.override_id = id;
        self
    }
    pub fn done(mut self, f: impl FnOnce(R::Response) + Send + 'static) -> Self {
        self.done = Some(Box::new(move |value, _| f(value)));
        self
    }
    pub fn done_with_response(
        mut self,
        f: impl FnOnce(R::Response, &Response) + Send + 'static,
    ) -> Self {
        self.done = Some(Box::new(f));
        self
    }
    pub fn done_with_id(self, f: impl FnOnce(R::Response, i32) + Send + 'static) -> Self {
        self.done_with_response(move |value, r| f(value, r.request_id))
    }
    pub fn done_plain(self, f: impl FnOnce() + Send + 'static) -> Self {
        self.done(move |_| f())
    }
    pub fn fail(mut self, f: impl Fn(&Error) + Send + 'static) -> Self {
        self.fail = Some(Box::new(move |error, _| f(error)));
        self
    }
    pub fn fail_with_response(mut self, f: impl Fn(&Error, &Response) + Send + 'static) -> Self {
        self.fail = Some(Box::new(f));
        self
    }
    pub fn fail_with_id(self, f: impl Fn(&Error, i32) + Send + 'static) -> Self {
        self.fail_with_response(move |error, r| f(error, r.request_id))
    }
    pub fn fail_plain(self, f: impl Fn() + Send + 'static) -> Self {
        self.fail(move |_| f())
    }
    pub fn handle_flood_errors(mut self) -> Self {
        self.policy = FailSkipPolicy::HandleFlood;
        self
    }
    pub fn handle_all_errors(mut self) -> Self {
        self.policy = FailSkipPolicy::HandleAll;
        self
    }
    pub fn send(self) -> i32 {
        let id = if self.override_id == 0 {
            get_next_request_id()
        } else {
            self.override_id
        };
        self.sender.owner.requests.lock().unwrap().insert(id);
        let weak = Arc::downgrade(&self.sender.owner);
        let mut done = self.done;
        let done_handler = Box::new(move |response: &Response| {
            let Some(owner) = weak.upgrade() else {
                return true;
            };
            if !owner.requests.lock().unwrap().remove(&response.request_id) {
                return true;
            }
            let handler = done.take();
            let mut reader = Reader::new(&response.reply);
            let Ok(value) = R::Response::read(&mut reader) else {
                return false;
            };
            if let Some(handler) = handler {
                if let Some(runner) = &owner.runner {
                    let weak = Arc::downgrade(&owner);
                    let response = response.clone();
                    runner(Box::new(move || {
                        if weak.upgrade().is_some() {
                            handler(value, &response);
                        }
                    }));
                } else {
                    handler(value, response);
                }
            }
            true
        });
        let weak = Arc::downgrade(&self.sender.owner);
        let policy = self.policy;
        let fail = self.fail;
        let fail = Arc::new(Mutex::new(fail));
        let fail_handler = Box::new(move |error: &Error, response: &Response| {
            if error.is_default_handled_error() && matches!(policy, FailSkipPolicy::Simple) {
                return false;
            }
            if error.is_default_handled_error()
                && !error.is_flood_error()
                && matches!(policy, FailSkipPolicy::HandleFlood)
            {
                return false;
            }
            let Some(owner) = weak.upgrade() else {
                return true;
            };
            owner.requests.lock().unwrap().remove(&response.request_id);
            let fail = fail.clone();
            if let Some(runner) = &owner.runner {
                let weak = Arc::downgrade(&owner);
                let error = error.clone();
                let response = response.clone();
                runner(Box::new(move || {
                    if weak.upgrade().is_some() {
                        if let Some(f) = &*fail.lock().unwrap() {
                            f(&error, &response);
                        }
                    }
                }));
            } else if let Some(f) = &*fail.lock().unwrap() {
                f(error, response);
            }
            true
        });
        let callbacks = ResponseHandler {
            done: Some(done_handler),
            fail: Some(fail_handler),
        };
        let instance = self.sender.instance().clone();
        let request = SerializedRequest::serialize(&self.request, 0);
        let (dc, wait, after) = (self.dc, self.wait, self.after);
        if self.sender.owner.concurrent {
            let weak = Arc::downgrade(&self.sender.owner);
            tg_runtime::on_main(move || {
                if weak.upgrade().is_some() {
                    instance.send_serialized(id, request, callbacks, dc, wait, after, true);
                }
            });
        } else {
            instance.send_serialized(id, request, callbacks, dc, wait, after, true);
        }
        id
    }
}
