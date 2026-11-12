//! Main-queue submission with weak Instance ownership and an explicit callback queue.
use crate::{
    mtp_instance::Instance,
    sender::{RequestBuilder, Runner, Sender, SentRequest},
};
use tg_tl::Function;
pub struct ConcurrentSender {
    sender: Sender,
}
impl ConcurrentSender {
    pub fn new(instance: Instance, runner: Runner) -> Self {
        Self {
            sender: Sender::with_runner(instance, Some(runner), true),
        }
    }
    pub fn request<R: Function>(&self, request: R) -> RequestBuilder<'_, R> {
        self.sender.request(request)
    }
    pub fn pending(&self, id: i32) -> bool {
        self.sender.pending(id)
    }
    pub fn cancel(&self, id: i32) {
        self.sender.cancel(id);
    }
    pub fn detach(&self, id: i32) {
        self.sender.detach(id);
    }
    pub fn cancel_all(&self) {
        self.sender.cancel_all();
    }
    pub fn sent_request(&self, id: i32) -> SentRequest<'_> {
        self.sender.sent_request(id)
    }
    pub fn request_canceller(&self) -> impl Fn(i32) + '_ {
        self.sender.request_canceller()
    }
}
