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
    instance: Option<Instance>,
    target: crate::mtp_instance::WeakInstance,
    requests: Mutex<BTreeSet<i32>>,
    handlers: Mutex<std::collections::BTreeMap<i32, ResponseHandler>>,
    runner: Option<Runner>,
    concurrent: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let ids = std::mem::take(self.requests.get_mut().unwrap());
        self.handlers.get_mut().unwrap().clear();
        let weak = self.target.clone();
        if self.concurrent {
            tg_runtime::on_main(move || {
                if let Some(instance) = weak.upgrade() {
                    for id in ids {
                        instance.cancel(id);
                    }
                }
            });
        } else if let Some(instance) = weak.upgrade() {
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
                target: instance.downgrade(),
                instance: if concurrent { None } else { Some(instance) },
                requests: Mutex::new(BTreeSet::new()),
                handlers: Mutex::new(std::collections::BTreeMap::new()),
                runner,
                concurrent,
            }),
        }
    }
    pub fn instance(&self) -> &Instance {
        self.owner
            .instance
            .as_ref()
            .expect("ConcurrentSender only holds a weak Instance")
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
            let handlers = self.owner.handlers.lock().unwrap().remove(&id);
            drop(handlers);
            let weak = self.owner.target.clone();
            if self.owner.concurrent {
                tg_runtime::on_main(move || {
                    if let Some(instance) = weak.upgrade() {
                        instance.cancel(id);
                    }
                });
            } else if let Some(instance) = weak.upgrade() {
                instance.cancel(id);
            }
        }
    }
    pub fn detach(&self, id: i32) {
        self.owner.requests.lock().unwrap().remove(&id);
        let handlers = self.owner.handlers.lock().unwrap().remove(&id);
        drop(handlers);
    }
    pub fn request_cancelling_discard(&self) {
        self.owner.requests.lock().unwrap().clear();
    }
    pub fn request_send_delayed(&self) {
        let weak = self.owner.target.clone();
        if self.owner.concurrent {
            tg_runtime::on_main(move || {
                if let Some(instance) = weak.upgrade() {
                    instance.send_anything(0, 0);
                }
            });
        } else if let Some(instance) = weak.upgrade() {
            instance.send_anything(0, 0);
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
    /// `done` for a handler that is not `Send` because it captures state of
    /// the main thread (`Rc`, `tg-rpl` values). Only for a main-thread
    /// [`Sender`]: the handler is kept in a `MainLocal` of the calling thread
    /// and may only run, and be dropped, there.
    pub fn done_local(self, f: impl FnOnce(R::Response) + 'static) -> Self {
        let f = tg_runtime::MainLocal::new(f);
        self.done(move |value| {
            if let Some(f) = f.take() {
                f(value);
            }
        })
    }
    /// `done_local` for `done([=](const Result &result, mtpRequestId id)`.
    pub fn done_local_with_id(self, f: impl FnOnce(R::Response, i32) + 'static) -> Self {
        let f = tg_runtime::MainLocal::new(f);
        self.done_with_id(move |value, id| {
            if let Some(f) = f.take() {
                f(value, id);
            }
        })
    }
    /// `fail` for a handler that is not `Send`; see [`Self::done_local`].
    pub fn fail_local(self, f: impl Fn(&Error) + 'static) -> Self {
        let f = tg_runtime::MainLocal::new(f);
        self.fail(move |error| {
            f.with(|f| f(error));
        })
    }
    /// `fail_local` for `fail([=](const MTP::Error &error, mtpRequestId id)`.
    pub fn fail_local_with_id(self, f: impl Fn(&Error, i32) + 'static) -> Self {
        let f = tg_runtime::MainLocal::new(f);
        self.fail_with_id(move |error, id| {
            f.with(|f| f(error, id));
        })
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
        let mut callback = self.done;
        let has_done = callback.is_some();
        let done = Box::new(move |response: &Response| {
            let onstack = callback.take();
            let mut reader = Reader::new(&response.reply);
            let Ok(value) = R::Response::read(&mut reader) else {
                return false;
            };
            if let Some(callback) = onstack {
                callback(value, response);
            }
            true
        });
        let fail = self.fail;
        let fail = Box::new(move |error: &Error, response: &Response| {
            if let Some(fail) = &fail {
                fail(error, response);
            }
            true
        });
        self.sender
            .owner
            .handlers
            .lock()
            .unwrap()
            .entry(id)
            .or_insert(ResponseHandler {
                done: if has_done { Some(done) } else { None },
                fail: Some(fail),
            });
        let registered = self.sender.owner.requests.lock().unwrap().insert(id);
        let lifetime = if self.sender.owner.concurrent || !registered {
            None
        } else {
            Some(Arc::new(RawLifetime {
                owner: Arc::downgrade(&self.sender.owner),
                id,
            }))
        };
        let done_lifetime = lifetime.clone();
        let weak = Arc::downgrade(&self.sender.owner);
        let done_handler = Box::new(move |response: &Response| {
            let _lifetime = &done_lifetime;
            let Some(owner) = weak.upgrade() else {
                return true;
            };
            if let Some(runner) = &owner.runner {
                let weak = Arc::downgrade(&owner);
                let response = response.clone();
                runner(Box::new(move || {
                    let handlers = weak.upgrade().and_then(|owner| {
                        owner.requests.lock().unwrap().remove(&response.request_id);
                        owner.handlers.lock().unwrap().remove(&response.request_id)
                    });
                    if let Some(mut handlers) = handlers {
                        if handlers.done.as_mut().is_some_and(|done| !done(&response)) {
                            if let Some(fail) = handlers.fail {
                                fail(
                                    &Error::local(
                                        "RESPONSE_PARSE_FAILED",
                                        "ConcurrentSender::senderRequestDone",
                                    ),
                                    &response,
                                );
                            }
                        }
                    }
                }));
                return true;
            }
            owner.requests.lock().unwrap().remove(&response.request_id);
            let mut handlers = owner
                .handlers
                .lock()
                .unwrap()
                .remove(&response.request_id)
                .unwrap_or_default();
            drop(owner);
            let result = handlers.done.as_mut().is_none_or(|done| done(response));
            if !result {
                if let Some(owner) = weak.upgrade() {
                    handlers.done = None;
                    owner
                        .handlers
                        .lock()
                        .unwrap()
                        .insert(response.request_id, handlers);
                }
            }
            result
        });
        let policy = self.policy;
        let weak = Arc::downgrade(&self.sender.owner);
        let fail_handler = Box::new(move |error: &Error, response: &Response| {
            let _lifetime = &lifetime;
            if error.is_default_handled_error()
                && (matches!(policy, FailSkipPolicy::Simple)
                    || (matches!(policy, FailSkipPolicy::HandleFlood) && !error.is_flood_error()))
            {
                return false;
            }
            let Some(owner) = weak.upgrade() else {
                return true;
            };
            if let Some(runner) = &owner.runner {
                let weak = Arc::downgrade(&owner);
                let error = error.clone();
                let response = response.clone();
                runner(Box::new(move || {
                    let handlers = weak.upgrade().and_then(|owner| {
                        owner.requests.lock().unwrap().remove(&response.request_id);
                        owner.handlers.lock().unwrap().remove(&response.request_id)
                    });
                    if let Some(handlers) = handlers {
                        if let Some(fail) = handlers.fail {
                            fail(&error, &response);
                        }
                    }
                }));
            } else {
                owner.requests.lock().unwrap().remove(&response.request_id);
                let handlers = owner.handlers.lock().unwrap().remove(&response.request_id);
                drop(owner);
                if let Some(handlers) = handlers {
                    if let Some(fail) = handlers.fail {
                        fail(error, response);
                    }
                }
            }
            true
        });
        let callbacks = ResponseHandler {
            done: if has_done { Some(done_handler) } else { None },
            fail: Some(fail_handler),
        };
        let weak = self.sender.owner.target.clone();
        let request = SerializedRequest::serialize(&self.request, 0);
        let (dc, wait, after) = (self.dc, self.wait, self.after);
        if self.sender.owner.concurrent {
            let owner = Arc::downgrade(&self.sender.owner);
            tg_runtime::on_main(move || {
                if owner.upgrade().is_some() {
                    if let Some(instance) = weak.upgrade() {
                        instance.send_serialized(id, request, callbacks, dc, wait, after, true);
                    }
                }
            });
        } else if let Some(instance) = weak.upgrade() {
            instance.send_serialized(id, request, callbacks, dc, wait, after, true);
            if !registered {
                instance.cancel(id);
                let handlers = self.sender.owner.handlers.lock().unwrap().remove(&id);
                drop(handlers);
            }
        }
        id
    }
}

pub struct SentRequest<'a> {
    sender: &'a Sender,
    id: i32,
}
impl SentRequest<'_> {
    pub fn cancel(&self) {
        self.sender.cancel(self.id);
    }
    pub fn detach(&self) {
        self.sender.detach(self.id);
    }
}
impl Sender {
    pub fn sent_request(&self, id: i32) -> SentRequest<'_> {
        SentRequest { sender: self, id }
    }
    pub fn request_canceller(&self) -> impl Fn(i32) + '_ {
        move |id| self.cancel(id)
    }
    pub fn cancel_all(&self) {
        let ids = self
            .owner
            .requests
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            self.cancel(id);
        }
    }
}

struct RawLifetime {
    owner: std::sync::Weak<Owner>,
    id: i32,
}
impl Drop for RawLifetime {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            let handlers = owner.handlers.lock().unwrap().remove(&self.id);
            drop(handlers);
        }
    }
}

impl Sender {
    pub(crate) fn new_weak(instance: Instance) -> Self {
        let target = instance.downgrade();
        Self {
            owner: Arc::new(Owner {
                instance: None,
                target,
                requests: Mutex::new(BTreeSet::new()),
                handlers: Mutex::new(std::collections::BTreeMap::new()),
                runner: None,
                concurrent: false,
            }),
        }
    }
    pub(crate) fn upgrade_instance(&self) -> Option<Instance> {
        self.owner.target.upgrade()
    }
}
