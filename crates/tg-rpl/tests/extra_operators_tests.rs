//! Tests of operators and helpers that the original test suite does not
//! cover.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tg_rpl::{
    combine, conditional, duplicate, fail, ints, merge, never, range, single, vector, EventStream,
    Lifetime, NoError, Producer,
};

fn recorder<T: 'static>() -> (Rc<RefCell<Vec<T>>>, impl Fn(T) + 'static) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let writer = log.clone();
    (log, move |value| writer.borrow_mut().push(value))
}

#[test]
fn merge_passes_values_and_completes_after_all() {
    let a = EventStream::<i32>::new();
    let (log, record) = recorder();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        merge((a.events(), single(10), vector(vec![20, 21]))).on_next_done(
            record,
            move || done.set(true),
            &lifetime,
        );
    }
    a.fire(1);
    assert_eq!(*log.borrow(), vec![10, 20, 21, 1]);
    assert!(!done.get());
    a.fire_done();
    assert!(done.get());
}

#[test]
fn conditional_switches_sources() {
    let test = EventStream::<bool>::new();
    let a = EventStream::<i32>::new();
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    conditional(test.events(), a.events(), single(0)).on_next(record, &lifetime);
    test.fire(true);
    assert!(log.borrow().is_empty());
    a.fire(5);
    test.fire(false);
    a.fire(6);
    test.fire(true);
    assert_eq!(*log.borrow(), vec![5, 0, 0, 6]);
}

#[test]
fn filter_producer_gates_values() {
    let values = EventStream::<i32>::new();
    let gate = EventStream::<bool>::new();
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    values
        .events()
        .filter_producer(gate.events())
        .on_next(record, &lifetime);
    values.fire(1);
    gate.fire(true);
    values.fire(2);
    gate.fire(false);
    values.fire(3);
    gate.fire(true);
    assert_eq!(*log.borrow(), vec![1, 2, 3]);
}

#[test]
fn take_while_completes_on_first_failure() {
    let (log, record) = recorder();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        ints(0, 10).take_while(|value| *value < 3).on_next_done(
            record,
            move || done.set(true),
            &lifetime,
        );
    }
    assert_eq!(*log.borrow(), vec![0, 1, 2]);
    assert!(done.get());
}

#[test]
fn take_zero_completes_on_first_value() {
    let a = EventStream::<i32>::new();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        a.events()
            .take(0)
            .on_done(move || done.set(true), &lifetime);
    }
    assert!(!done.get());
    a.fire(1);
    assert!(done.get());
    assert!(a.has_consumers(), "stale until the next fire");
    a.fire(2);
    assert!(!a.has_consumers());
}

#[test]
fn map_error_and_map_error_to_done() {
    let error = Rc::new(RefCell::new(None));
    let lifetime = Lifetime::new();
    {
        let error = error.clone();
        fail::<i32, i32>(5)
            .map_error(|e| format!("error {e}"))
            .on_error(move |e| *error.borrow_mut() = Some(e), &lifetime);
    }
    assert_eq!(error.borrow().as_deref(), Some("error 5"));

    let done = Rc::new(Cell::new(false));
    {
        let done = done.clone();
        let failing: Producer<i32, NoError> = fail::<i32, i32>(5).map_error_to_done();
        failing.on_done(move || done.set(true), &lifetime);
    }
    assert!(done.get());
}

#[test]
fn filter_optional_and_nullptr() {
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    vector(vec![Some(1), None, Some(3)])
        .filter_optional()
        .on_next(record, &lifetime);
    let (log2, record2) = recorder();
    vector(vec![None, Some("x")])
        .filter_nullptr()
        .on_next(record2, &lifetime);
    assert_eq!(*log.borrow(), vec![1, 3]);
    assert_eq!(*log2.borrow(), vec!["x"]);
}

#[test]
fn before_next_runs_before_the_handler() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let lifetime = Lifetime::new();
    let before = log.clone();
    let next = log.clone();
    range([1, 2])
        .before_next(move |value| before.borrow_mut().push(format!("before {value}")))
        .on_next(
            move |value| next.borrow_mut().push(format!("next {value}")),
            &lifetime,
        );
    assert_eq!(*log.borrow(), ["before 1", "next 1", "before 2", "next 2"]);
}

#[test]
fn start_spawning_replays_initial_values_then_follows() {
    let source = EventStream::<i32>::new();
    let alive = Lifetime::new();
    let spawned = source.events_starting_with(1).start_spawning(&alive);
    let (log1, record1) = recorder();
    let (log2, record2) = recorder();
    let lifetime = Lifetime::new();
    duplicate(&spawned).on_next(record1, &lifetime);
    source.fire(2);
    spawned.on_next(record2, &lifetime);
    source.fire(3);
    // Every copy replays the values produced while spawning.
    assert_eq!(*log1.borrow(), vec![1, 2, 3]);
    assert_eq!(*log2.borrow(), vec![1, 3]);
    alive.destroy();
    source.fire(4);
    assert_eq!(*log1.borrow(), vec![1, 2, 3]);
}

#[test]
fn start_to_stream_forwards_values_but_not_done() {
    let target = EventStream::<i32>::new();
    let (log, record) = recorder();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        target
            .events()
            .on_next_done(record, move || done.set(true), &lifetime);
    }
    vector(vec![1, 2]).start_to_stream(&target, &lifetime);
    assert_eq!(*log.borrow(), vec![1, 2]);
    assert!(!done.get());
}

#[test]
fn duplicate_restarts_and_copies_the_generator() {
    let starts = Rc::new(Cell::new(0));
    let producer = {
        let starts = starts.clone();
        Producer::<i32>::new(move |consumer| {
            starts.set(starts.get() + 1);
            consumer.put_next(starts.get());
            Lifetime::new()
        })
    };
    let (log, record) = recorder();
    let record = Rc::new(record);
    let lifetime = Lifetime::new();
    {
        let record = record.clone();
        duplicate(&producer).on_next(move |v| record(v), &lifetime);
    }
    {
        let record = record.clone();
        producer.clone().on_next(move |v| record(v), &lifetime);
    }
    producer.on_next(move |v| record(v), &lifetime);
    assert_eq!(*log.borrow(), vec![1, 2, 3]);
}

#[test]
fn default_producer_is_empty_and_never_emits() {
    let producer = Producer::<i32>::default();
    assert!(producer.is_empty());
    assert!(!single(1).is_empty());
    let called = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let next = called.clone();
        let done = called.clone();
        producer.on_next_done(move |_| next.set(true), move || done.set(true), &lifetime);
    }
    never::<i32>().on_next(|_| panic!("never emits"), &lifetime);
    assert!(!called.get());
}

#[test]
fn pipe_applies_application_operator() {
    fn doubled() -> impl FnOnce(Producer<i32>) -> Producer<i32> {
        |producer| producer.map(|value| value * 2)
    }
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    ints(1, 3).pipe(doubled()).on_next(record, &lifetime);
    assert_eq!(*log.borrow(), vec![2, 4]);
}

#[test]
fn flatten_latest_unsubscribes_previous_inner() {
    let outer = EventStream::<Producer<i32>>::new();
    let first = EventStream::<i32>::new();
    let second = EventStream::<i32>::new();
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    outer.events().flatten_latest().on_next(record, &lifetime);
    outer.fire(first.events());
    first.fire(1);
    outer.fire(second.events());
    first.fire(2);
    second.fire(3);
    assert_eq!(*log.borrow(), vec![1, 3]);
}

#[test]
fn combine_waits_for_all_and_completes_after_all() {
    let a = EventStream::<i32>::new();
    let (log, record) = recorder();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        combine((a.events(), single("x"))).on_next_done(record, move || done.set(true), &lifetime);
    }
    assert!(log.borrow().is_empty());
    a.fire(1);
    a.fire(2);
    assert!(!done.get());
    a.fire_done();
    assert!(done.get());
    assert_eq!(*log.borrow(), vec![(1, "x"), (2, "x")]);
}

#[test]
fn error_is_forwarded_through_operators() {
    let stream = EventStream::<i32, String>::new();
    let error = Rc::new(RefCell::new(None));
    let lifetime = Lifetime::new();
    {
        let error = error.clone();
        stream
            .events()
            .map(|value| value + 1)
            .filter(|_| true)
            .on_error(move |e| *error.borrow_mut() = Some(e), &lifetime);
    }
    stream.fire_error("bad".to_owned());
    assert_eq!(error.borrow().as_deref(), Some("bad"));
    assert!(!stream.has_consumers());
}
