//! Destruction order tests (not in the original).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tg_rpl::{single, Consumer, EventStream, Lifetime, NoError, Producer, Variable};

type Log = Rc<RefCell<Vec<&'static str>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

struct Tracked {
    log: Log,
    name: &'static str,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.log.borrow_mut().push(self.name);
    }
}

fn adder(log: &Log, name: &'static str) -> impl FnOnce() + 'static {
    let log = log.clone();
    move || log.borrow_mut().push(name)
}

#[test]
fn lifetime_runs_callbacks_in_reverse_order() {
    let log = new_log();
    {
        let lifetime = Lifetime::new();
        lifetime.add(adder(&log, "first"));
        lifetime.add(adder(&log, "second"));
        let other = Lifetime::from_fn(adder(&log, "third"));
        other.add(adder(&log, "fourth"));
        lifetime.add_lifetime(other);
        assert!(!lifetime.is_empty());
    }
    assert_eq!(*log.borrow(), ["fourth", "third", "second", "first"]);
}

#[test]
fn callbacks_added_during_destroy_run_later() {
    let log = new_log();
    let lifetime = Rc::new(Lifetime::new());
    {
        let weak = Rc::downgrade(&lifetime);
        let inner_log = log.clone();
        lifetime.add(move || {
            inner_log.borrow_mut().push("outer");
            if let Some(lifetime) = weak.upgrade() {
                lifetime.add(adder(&inner_log, "added while destroying"));
            }
        });
    }
    lifetime.destroy();
    assert_eq!(*log.borrow(), ["outer"]);
    assert!(!lifetime.is_empty());
    lifetime.destroy();
    assert_eq!(*log.borrow(), ["outer", "added while destroying"]);
    assert!(lifetime.is_empty());
}

#[test]
fn make_state_is_dropped_in_order_with_other_callbacks() {
    let log = new_log();
    let lifetime = Lifetime::new();
    lifetime.add(adder(&log, "before state"));
    let state = lifetime.make_state(Tracked {
        log: log.clone(),
        name: "state",
    });
    lifetime.add(adder(&log, "after state"));
    assert!(state.is_alive());
    lifetime.destroy();
    assert_eq!(*log.borrow(), ["after state", "state", "before state"]);
    assert!(!state.is_alive());
    assert!(state.try_get().is_none());
}

#[test]
fn release_forgets_callbacks() {
    let log = new_log();
    let lifetime = Lifetime::new();
    lifetime.add(adder(&log, "never"));
    lifetime.release();
    drop(lifetime);
    assert!(log.borrow().is_empty());
}

#[test]
fn moved_assignment_destroys_old_lifetime() {
    let log = new_log();
    let mut lifetime = Lifetime::from_fn(adder(&log, "old"));
    assert!(!lifetime.is_empty());
    lifetime = Lifetime::from_fn(adder(&log, "new"));
    assert_eq!(*log.borrow(), ["old"]);
    drop(lifetime);
    assert_eq!(*log.borrow(), ["old", "new"]);
}

#[test]
fn event_stream_drop_sends_done_in_subscription_order() {
    let log = new_log();
    let lifetime = Lifetime::new();
    {
        let stream = EventStream::<i32>::new();
        stream
            .events()
            .on_done(adder(&log, "first done"), &lifetime);
        stream
            .events()
            .on_done(adder(&log, "second done"), &lifetime);
    }
    assert_eq!(*log.borrow(), ["first done", "second done"]);
}

#[test]
fn termination_drops_handlers_and_their_captures() {
    // Documented difference: handlers are dropped at termination, not when
    // the last copy of the consumer goes away.
    let log = new_log();
    let consumer: Consumer<i32, NoError> = {
        let tracked = Tracked {
            log: log.clone(),
            name: "handler captures",
        };
        Consumer::new(
            move |_| {
                let _ = &tracked;
            },
            |_| {},
            || {},
        )
    };
    let copy = consumer.clone();
    assert!(log.borrow().is_empty());
    consumer.terminate();
    assert_eq!(*log.borrow(), ["handler captures"]);
    assert!(copy.is_terminated());
    assert!(!copy.put_next(1));
}

#[test]
fn add_lifetime_to_terminated_consumer_destroys_it_at_once() {
    let log = new_log();
    let consumer: Consumer<i32, NoError> = Consumer::new(|_| {}, |_| {}, || {});
    consumer.put_done();
    assert!(!consumer.add_lifetime(Lifetime::from_fn(adder(&log, "destroyed"))));
    assert_eq!(*log.borrow(), ["destroyed"]);
    assert!(!consumer.make_state(0).is_alive());
}

#[test]
fn done_runs_before_consumer_lifetime_is_destroyed() {
    let log = new_log();
    let lifetime = Lifetime::new();
    let generator_log = log.clone();
    Producer::<i32>::new(move |consumer| {
        consumer.add_lifetime(Lifetime::from_fn(adder(
            &generator_log,
            "consumer lifetime",
        )));
        let returned = Lifetime::from_fn(adder(&generator_log, "returned lifetime"));
        consumer.put_done();
        returned
    })
    .on_done(adder(&log, "done"), &lifetime);
    assert_eq!(
        *log.borrow(),
        ["done", "consumer lifetime", "returned lifetime"]
    );
}

#[test]
fn single_value_is_dropped_with_the_subscription() {
    let log = new_log();
    let lifetime = Lifetime::new();
    let value = Rc::new(Tracked {
        log: log.clone(),
        name: "value",
    });
    let unused = single(value.clone());
    drop(value);
    assert!(log.borrow().is_empty());
    let received = Rc::new(Cell::new(false));
    {
        let received = received.clone();
        unused.on_next(move |_| received.set(true), &lifetime);
    }
    assert!(received.get());
    assert_eq!(*log.borrow(), ["value"]);
}

#[test]
fn variable_drop_order_matches_cpp_members() {
    // C++ destroys `_lifetime` (the producer subscription), then `_changes`
    // (sending done), then `_data`.
    let log = new_log();
    let lifetime = Lifetime::new();
    {
        let source = EventStream::<i32>::new();
        let var = Variable::<i32>::from_producer(source.events());
        let source_log = log.clone();
        source.events().on_done(
            move || source_log.borrow_mut().push("source done"),
            &lifetime,
        );
        var.changes()
            .on_done(adder(&log, "variable done"), &lifetime);
        // Unsubscribing the variable from the source is observable through
        // `has_consumers` only after the next fire.
        drop(var);
        assert_eq!(*log.borrow(), ["variable done"]);
        source.fire(1);
        assert!(source.has_consumers());
    }
    assert_eq!(*log.borrow(), ["variable done", "source done"]);
}
