//! Reentrancy tests (not in the original). Expected sequences were derived
//! by tracing the C++ `event_stream::fire_forward` / `consumer` code.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use tg_rpl::{EventStream, Lifetime, Variable};

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, entry: impl Into<String>) {
    log.borrow_mut().push(entry.into());
}

fn entries(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

/// Subscribes a consumer that only records `name<value>` and `name done`.
fn subscribe_recorder(stream: &EventStream<i32>, log: &Log, name: &str, lifetime: &Lifetime) {
    let next_log = log.clone();
    let done_log = log.clone();
    let next_name = name.to_owned();
    let done_name = name.to_owned();
    stream.events().on_next_done(
        move |value| push(&next_log, format!("{next_name}{value}")),
        move || push(&done_log, format!("{done_name} done")),
        lifetime,
    );
}

#[test]
fn fire_from_handler_is_delivered_depth_first() {
    let log = new_log();
    let stream = Rc::new(EventStream::<i32>::new());
    let lifetime = Lifetime::new();
    {
        let log = log.clone();
        let weak: Weak<EventStream<i32>> = Rc::downgrade(&stream);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                if value == 1 {
                    if let Some(stream) = weak.upgrade() {
                        stream.fire(2);
                    }
                }
            },
            &lifetime,
        );
    }
    subscribe_recorder(&stream, &log, "B", &lifetime);
    stream.fire(1);
    assert_eq!(entries(&log), ["A1", "A2", "B2", "B1"]);
    stream.fire(3);
    assert_eq!(entries(&log), ["A1", "A2", "B2", "B1", "A3", "B3"]);
}

#[test]
fn unsubscribe_other_during_fire_stops_delivery_at_once() {
    let log = new_log();
    let stream = EventStream::<i32>::new();
    let first = Lifetime::new();
    let second = Rc::new(Lifetime::new());
    {
        let log = log.clone();
        let weak_second = Rc::downgrade(&second);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                if let Some(second) = weak_second.upgrade() {
                    second.destroy();
                }
            },
            &first,
        );
    }
    subscribe_recorder(&stream, &log, "B", &second);
    assert!(stream.has_consumers());
    stream.fire(1);
    stream.fire(2);
    assert_eq!(entries(&log), ["A1", "A2"]);
}

#[test]
fn unsubscribe_self_during_fire() {
    let log = new_log();
    let stream = EventStream::<i32>::new();
    let own = Rc::new(Lifetime::new());
    let other = Lifetime::new();
    {
        let log = log.clone();
        let weak_own = Rc::downgrade(&own);
        stream.events().on_next(
            move |value| {
                if let Some(own) = weak_own.upgrade() {
                    own.destroy();
                }
                // The handler keeps running after its consumer is gone.
                push(&log, format!("A{value}"));
            },
            &own,
        );
    }
    subscribe_recorder(&stream, &log, "B", &other);
    stream.fire(1);
    stream.fire(2);
    assert_eq!(entries(&log), ["A1", "B1", "B2"]);
    // A, stale after the first fire, was erased by that same fire.
    other.destroy();
    assert!(stream.has_consumers(), "B stays listed until the next fire");
    stream.fire(3);
    assert!(!stream.has_consumers());
    assert_eq!(entries(&log), ["A1", "B1", "B2"]);
}

#[test]
fn subscribe_during_fire_gets_only_later_values() {
    let log = new_log();
    let stream = Rc::new(EventStream::<i32>::new());
    let lifetime = Rc::new(Lifetime::new());
    {
        let log = log.clone();
        let weak_stream = Rc::downgrade(&stream);
        let weak_lifetime = Rc::downgrade(&lifetime);
        let subscribed = Cell::new(false);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                if !subscribed.replace(true) {
                    if let (Some(stream), Some(lifetime)) =
                        (weak_stream.upgrade(), weak_lifetime.upgrade())
                    {
                        subscribe_recorder(&stream, &log, "N", &lifetime);
                    }
                }
            },
            &lifetime,
        );
    }
    stream.fire(1);
    stream.fire(2);
    assert_eq!(entries(&log), ["A1", "A2", "N2"]);
}

#[test]
fn stream_dropped_from_handler() {
    let log = new_log();
    let holder = Rc::new(RefCell::new(Some(EventStream::<i32>::new())));
    let lifetime = Lifetime::new();
    {
        let a_log = log.clone();
        let done_log = log.clone();
        let weak_holder = Rc::downgrade(&holder);
        let stream = holder.borrow();
        let stream = stream.as_ref().expect("stream");
        stream.events().on_next_done(
            move |value| {
                push(&a_log, format!("A{value}"));
                if let Some(holder) = weak_holder.upgrade() {
                    let taken = holder.borrow_mut().take();
                    drop(taken);
                }
                push(&a_log, "A after drop");
            },
            move || push(&done_log, "A done"),
            &lifetime,
        );
        subscribe_recorder(stream, &log, "B", &lifetime);
    }
    // Fire without holding a `RefCell` borrow during the call: the handler
    // takes the stream out of the holder.
    let fire = {
        let guard = holder.borrow();
        let stream = guard.as_ref().expect("stream");
        // `start_to_stream` fires through a weak handle, so no borrow of
        // the holder is active while the handler drops the stream.
        let trigger = EventStream::<i32>::new();
        trigger.events().start_to_stream(stream, &lifetime);
        trigger
    };
    fire.fire(1);
    assert!(holder.borrow().is_none());
    assert_eq!(entries(&log), ["A1", "A done", "B done", "A after drop"]);
    fire.fire(2);
    assert_eq!(entries(&log).len(), 4);
}

#[test]
fn fire_done_from_handler_then_resubscribe() {
    let log = new_log();
    let stream = Rc::new(EventStream::<i32>::new());
    let lifetime = Rc::new(Lifetime::new());
    {
        let log = log.clone();
        let weak_stream = Rc::downgrade(&stream);
        let weak_lifetime = Rc::downgrade(&lifetime);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                if let (Some(stream), Some(lifetime)) =
                    (weak_stream.upgrade(), weak_lifetime.upgrade())
                {
                    stream.fire_done();
                    // A new `events()` after `fire_done` starts a fresh list.
                    subscribe_recorder(&stream, &log, "N", &lifetime);
                }
            },
            &lifetime,
        );
    }
    subscribe_recorder(&stream, &log, "B", &lifetime);
    stream.fire(1);
    stream.fire(2);
    assert_eq!(entries(&log), ["A1", "B done", "N2"]);
}

#[test]
fn destroy_own_lifetime_keeps_handler_state_alive() {
    let log = new_log();
    let stream = EventStream::<i32>::new();
    let lifetime = Rc::new(Lifetime::new());
    let state = lifetime.make_state(RefCell::new(String::from("state")));
    {
        let log = log.clone();
        let weak_lifetime = Rc::downgrade(&lifetime);
        stream.events().on_next(
            move |value| {
                let state = state.get();
                if let Some(lifetime) = weak_lifetime.upgrade() {
                    lifetime.destroy();
                }
                // The state is still reachable through the `Rc` taken above.
                push(&log, format!("{}{value}", state.borrow()));
            },
            &lifetime,
        );
    }
    stream.fire(1);
    stream.fire(2);
    assert_eq!(entries(&log), ["state1"]);
}

#[test]
fn variable_assigned_from_its_own_value_handler() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let var = Rc::new(Variable::<i32>::new(0));
    let lifetime = Lifetime::new();
    {
        let log = log.clone();
        let weak = Rc::downgrade(&var);
        var.value().on_next(
            move |value| {
                log.borrow_mut().push(value);
                if let Some(var) = weak.upgrade() {
                    // Reading the current value inside the handler is fine.
                    assert_eq!(var.current(), value);
                    if value < 7 {
                        var.set(value + 1);
                    }
                }
            },
            &lifetime,
        );
    }
    // While the initial value is delivered the consumer is not yet
    // subscribed to changes, so the assignment of 1 is not seen.
    assert_eq!(*log.borrow(), vec![0]);
    assert_eq!(var.current(), 1);
    var.set(5);
    assert_eq!(*log.borrow(), vec![0, 5, 6, 7]);
    assert_eq!(var.current(), 7);
}

#[test]
fn combine_previous_reentrant_order_matches_original() {
    // C++: the pair is passed on before the previous value is replaced, so
    // a reentrant value sees the old previous one and the outer value wins.
    let log = Rc::new(RefCell::new(Vec::new()));
    let stream = Rc::new(EventStream::<i32>::new());
    let lifetime = Lifetime::new();
    {
        let log = log.clone();
        let weak = Rc::downgrade(&stream);
        stream.events().combine_previous().on_next(
            move |(previous, next)| {
                log.borrow_mut().push((previous, next));
                if next == 1 {
                    if let Some(stream) = weak.upgrade() {
                        stream.fire(2);
                    }
                }
            },
            &lifetime,
        );
    }
    stream.fire(0);
    stream.fire(1);
    stream.fire(3);
    assert_eq!(*log.borrow(), vec![(0, 1), (0, 2), (1, 3)]);
}

/// Pins down a defect of the original `fire_forward`: a nested `fire` while
/// a stale (unsubscribed but not yet erased) consumer precedes the firing
/// one compacts the vector under the outer `std::remove_if`, which then
/// overwrites a live consumer. The port reproduces it on purpose.
#[test]
fn nested_fire_with_stale_consumer_loses_live_consumer_like_original() {
    let log = new_log();
    let stream = Rc::new(EventStream::<i32>::new());
    let stale = Lifetime::new();
    let alive = Lifetime::new();

    subscribe_recorder(&stream, &log, "X", &stale);
    {
        let log = log.clone();
        let weak = Rc::downgrade(&stream);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                if value == 1 {
                    if let Some(stream) = weak.upgrade() {
                        stream.fire(2);
                    }
                }
            },
            &alive,
        );
    }
    subscribe_recorder(&stream, &log, "B", &alive);

    // X becomes stale; it stays in the list until the next fire.
    stale.destroy();

    // Outer fire: X is stale, A fires 2 (A2, B2), the nested compaction
    // shifts B to A's slot, the outer remove_if then moves B over A, and
    // the last slot (moved-from) gets nothing: B never receives 1.
    stream.fire(1);
    assert_eq!(entries(&log), ["A1", "A2", "B2"]);

    // A was dropped from the list although its lifetime is alive.
    stream.fire(3);
    assert_eq!(entries(&log), ["A1", "A2", "B2", "B3"]);
}

#[test]
fn nested_fire_with_new_consumer_duplicates_it_and_loses_firing_one_like_original() {
    // Another consequence of the original algorithm: a consumer added
    // during a nested fire, while a stale one exists, is copied into the
    // compacted part and also kept in its slot (the nested fire does not
    // erase); the outer compaction then overwrites the firing consumer and
    // keeps both copies of the new one, which receives every later value
    // twice.
    let log = new_log();
    let stream = Rc::new(EventStream::<i32>::new());
    let stale = Lifetime::new();
    let alive = Rc::new(Lifetime::new());

    subscribe_recorder(&stream, &log, "X", &stale);
    {
        let log = log.clone();
        let weak_stream = Rc::downgrade(&stream);
        let weak_alive = Rc::downgrade(&alive);
        stream.events().on_next(
            move |value| {
                push(&log, format!("A{value}"));
                let (Some(stream), Some(alive)) = (weak_stream.upgrade(), weak_alive.upgrade())
                else {
                    return;
                };
                if value == 1 {
                    stream.fire(2);
                } else if value == 2 {
                    subscribe_recorder(&stream, &log, "N", &alive);
                }
            },
            &alive,
        );
    }
    stale.destroy();

    // Outer fire(1), list [X(stale), A], A is the last ("prev") slot.
    // Nested fire(2): A gets 2 and subscribes N (list [X, A, N]); A is
    // moved to slot 0, N is copied to slot 1 and also stays in slot 2;
    // depth 2, nothing erased: [A, N, N]. Back in the outer fire, A (at
    // outer "prev" = slot 1) returned alive, so slot 1, which now holds N,
    // is moved to slot 0, N from slot 2 is copied again, and the list is
    // truncated to [N, N]: A is lost and N is present twice.
    stream.fire(1);
    stream.fire(3);
    let log_entries = entries(&log);
    assert_eq!(&log_entries[..2], ["A1", "A2"]);
    assert_eq!(&log_entries[2..], ["N3", "N3"]);
}
