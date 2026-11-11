//! `crl::on_main_queue(guard, steps)` (`crl_common_on_main_queue.h`): steps
//! run one per main-queue turn; the guard is checked before each step and a
//! dead guard abandons the rest.

use crate::crl_guards::{AsGuard, GuardToken};
use crate::crl_on_main::{on_main, on_main_local};
use std::collections::VecDeque;

macro_rules! steps_impl {
    ($name:ident, $func:ident, $post:ident, $step:ty, $bounds:tt) => {
        #[must_use = "call dispatch()"]
        pub struct $name<G: GuardToken + $bounds + 'static> {
            guard: G,
            steps: VecDeque<$step>,
        }

        impl<G: GuardToken + $bounds + 'static> $name<G> {
            pub fn next(mut self, step: $step) -> Self {
                self.steps.push_back(step);
                self
            }

            pub fn dispatch(self) {
                Self::perform_next(self.guard, self.steps);
            }

            fn perform_next(guard: G, mut steps: VecDeque<$step>) {
                if steps.is_empty() {
                    return;
                }
                $post(move || {
                    if !guard.alive() {
                        return;
                    }
                    let step = steps.pop_front().expect("checked above");
                    step();
                    Self::perform_next(guard, steps);
                });
            }
        }

        pub fn $func<G: AsGuard + ?Sized>(g: &G, steps: Vec<$step>) -> $name<G::Token>
        where
            G::Token: $bounds + 'static,
        {
            $name {
                guard: g.as_guard(),
                steps: steps.into(),
            }
        }
    };
}

steps_impl!(
    GuardedOnMainQueue,
    on_main_queue,
    on_main,
    Box<dyn FnOnce() + Send>,
    Send
);
steps_impl!(
    GuardedOnMainQueueLocal,
    on_main_queue_local,
    on_main_local,
    Box<dyn FnOnce()>,
    Sized
);
