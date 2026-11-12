//! Main-queue submission and caller-provided callback queue.
use crate::{
    mtp_instance::Instance,
    sender::{Runner, Sender},
};
pub struct ConcurrentSender {
    sender: Sender,
}
impl ConcurrentSender {
    pub fn new(instance: Instance, runner: Runner) -> Self {
        Self {
            sender: Sender::with_runner(instance, Some(runner), true),
        }
    }
}
impl std::ops::Deref for ConcurrentSender {
    type Target = Sender;
    fn deref(&self) -> &Sender {
        &self.sender
    }
}
