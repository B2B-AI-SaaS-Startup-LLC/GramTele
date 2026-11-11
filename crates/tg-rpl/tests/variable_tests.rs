//! Port of `rpl/variable_tests.cpp`, plus tests of the rest of the
//! `rpl::variable` interface (not covered by the original).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tg_rpl::{EventStream, Lifetime, Variable};

#[test]
fn simple_test() {
    let sum = Rc::new(Cell::new(0));
    {
        let var = Variable::<i32>::new(1);
        let sum = sum.clone();
        let _lifeftime = var
            .value()
            .on_next_returning(move |value| sum.set(sum.get() + value));
        var.set(1);
        var.set(11);
        var.set(111);
        var.set(111);
    }
    assert_eq!(sum.get(), 1 + 11 + 111);
}

// Tests below are not in the original.

fn recorder<T: Clone + 'static>() -> (Rc<RefCell<Vec<T>>>, impl Fn(T) + 'static) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let writer = log.clone();
    (log, move |value| writer.borrow_mut().push(value))
}

#[test]
fn value_copies_current_at_call_time() {
    let var = Variable::<i32>::new(1);
    let value = var.value();
    var.set(2);
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    value.on_next(record, &lifetime);
    var.set(3);
    // The first value is the one current when `value()` was called.
    assert_eq!(*log.borrow(), vec![1, 3]);
}

#[test]
fn changes_skip_current() {
    let var = Variable::<i32>::new(1);
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    var.changes().on_next(record, &lifetime);
    var.set(1);
    var.set(2);
    var.force_assign(2);
    assert_eq!(*log.borrow(), vec![2, 2]);
    assert_eq!(var.current(), 2);
}

#[test]
fn non_comparable_fires_every_time() {
    #[derive(Clone)]
    struct NoEq(i32);
    let var = Variable::<NoEq>::new_non_comparable(NoEq(1));
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    var.changes()
        .on_next(move |value: NoEq| record(value.0), &lifetime);
    var.set(NoEq(1));
    var.set(NoEq(1));
    assert_eq!(*log.borrow(), vec![1, 1]);
}

#[test]
fn producer_assignment_follows_and_set_stops_it() {
    let source = EventStream::<i32>::new();
    let var = Variable::<i32>::from_producer(source.events());
    assert_eq!(var.current(), 0);
    source.fire(5);
    assert_eq!(var.current(), 5);
    var.set(7);
    source.fire(9);
    assert_eq!(var.current(), 7);
    var.set_producer(source.events());
    source.fire(10);
    assert_eq!(var.current(), 10);
    var.force_assign(1);
    source.fire(11);
    assert_eq!(var.current(), 1);
}

#[test]
fn reset_sends_done_without_value() {
    let var = Variable::<i32>::new(1);
    let (log, record) = recorder();
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let done = done.clone();
        var.value()
            .on_next_done(record, move || done.set(true), &lifetime);
    }
    var.reset(5);
    assert!(done.get());
    assert_eq!(var.current(), 5);
    var.set(6);
    assert_eq!(*log.borrow(), vec![1]);
}

#[test]
fn reset_with_error_sends_error() {
    let var = Variable::<i32, String>::new(1);
    let error = Rc::new(RefCell::new(None));
    let lifetime = Lifetime::new();
    {
        let error = error.clone();
        var.changes()
            .on_error(move |e| *error.borrow_mut() = Some(e), &lifetime);
    }
    var.reset_with_error("failed".to_owned());
    assert_eq!(error.borrow().as_deref(), Some("failed"));
}

#[test]
fn clone_copies_value_only() {
    let var = Variable::<i32>::new(1);
    let (log, record) = recorder();
    let lifetime = Lifetime::new();
    var.changes().on_next(record, &lifetime);
    let copy = var.clone();
    copy.set(2);
    assert_eq!(copy.current(), 2);
    assert_eq!(var.current(), 1);
    assert!(log.borrow().is_empty());
}

#[test]
fn drop_completes_subscribers() {
    let done = Rc::new(Cell::new(false));
    let lifetime = Lifetime::new();
    {
        let var = Variable::<i32>::new(1);
        let done = done.clone();
        var.value().on_done(move || done.set(true), &lifetime);
    }
    assert!(done.get());
}
