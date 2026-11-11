//! Port of `rpl/operators_tests.cpp`.
//!
//! Each C++ `SECTION` is one `#[test]`, named after the section.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tg_rpl::{
    combine, combine_vec, complete, deferred, ints_count, single, EventStream, Lifetime, NoError,
    Producer,
};

struct OnDestructor {
    callback: Option<Box<dyn FnOnce()>>,
}

impl OnDestructor {
    fn new(callback: impl FnOnce() + 'static) -> Self {
        Self {
            callback: Some(Box::new(callback)),
        }
    }
}

impl Drop for OnDestructor {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            callback();
        }
    }
}

/// Counts copies. Rust moves are bitwise and cannot be observed, so only
/// the copy counter of the C++ class is meaningful here.
struct InvokeCounter {
    copy_counter: Rc<Cell<i32>>,
}

impl InvokeCounter {
    fn new(copy_counter: &Rc<Cell<i32>>) -> Self {
        Self {
            copy_counter: copy_counter.clone(),
        }
    }
}

impl Clone for InvokeCounter {
    fn clone(&self) -> Self {
        self.copy_counter.set(self.copy_counter.get() + 1);
        Self {
            copy_counter: self.copy_counter.clone(),
        }
    }
}

type Log = Rc<RefCell<String>>;

fn log() -> Log {
    Rc::new(RefCell::new(String::new()))
}

fn append(log: &Log) -> impl Fn(String) + Clone + 'static {
    let log = log.clone();
    move |value: String| {
        let mut log = log.borrow_mut();
        log.push_str(&value);
        log.push(' ');
    }
}

fn to_string(value: i32) -> String {
    value.to_string()
}

#[test]
fn single_test() {
    let sum = Rc::new(Cell::new(0));
    let done_generated = Rc::new(Cell::new(false));
    let destroyed = Rc::new(Cell::new(false));
    let copy_count = Rc::new(Cell::new(0));
    {
        let counter = InvokeCounter::new(&copy_count);
        let destroy_called = {
            let destroyed = destroyed.clone();
            Rc::new(OnDestructor::new(move || destroyed.set(true)))
        };
        let lifetime = Lifetime::new();
        let (d1, d2, d3) = (
            destroy_called.clone(),
            destroy_called.clone(),
            destroy_called.clone(),
        );
        let sum = sum.clone();
        let done_generated = done_generated.clone();
        single(counter).on_next_error_done(
            move |_: InvokeCounter| {
                let _ = &d1;
                sum.set(sum.get() + 1);
            },
            move |_: NoError| {
                let _ = &d2;
            },
            move || {
                let _ = &d3;
                done_generated.set(true);
            },
            &lifetime,
        );
    }
    assert_eq!(sum.get(), 1);
    assert!(done_generated.get());
    assert!(destroyed.get());
    assert_eq!(copy_count.get(), 0);
}

#[test]
fn then_test() {
    let sum = Rc::new(Cell::new(0));
    let done_generated = Rc::new(Cell::new(false));
    let destroyed = Rc::new(Cell::new(false));
    let copy_count = Rc::new(Cell::new(0));
    {
        let mut testing = complete::<InvokeCounter>().type_erased();
        for _ in 0..5 {
            let counter = InvokeCounter::new(&copy_count);
            testing = testing.then(single(counter));
        }
        let destroy_called = {
            let destroyed = destroyed.clone();
            Rc::new(OnDestructor::new(move || destroyed.set(true)))
        };

        let lifetime = Lifetime::new();
        let (d1, d2, d3) = (
            destroy_called.clone(),
            destroy_called.clone(),
            destroy_called.clone(),
        );
        let sum = sum.clone();
        let done_generated = done_generated.clone();
        testing
            .then(complete::<InvokeCounter>())
            .on_next_error_done(
                move |_: InvokeCounter| {
                    let _ = &d1;
                    sum.set(sum.get() + 1);
                },
                move |_: NoError| {
                    let _ = &d2;
                },
                move || {
                    let _ = &d3;
                    done_generated.set(true);
                },
                &lifetime,
            );
    }
    assert_eq!(sum.get(), 5);
    assert!(done_generated.get());
    assert!(destroyed.get());
    assert_eq!(copy_count.get(), 0);
}

#[test]
fn map_test() {
    let sum = log();
    {
        let lifetime = Lifetime::new();
        single(1)
            .then(single(2))
            .then(single(3))
            .then(single(4))
            .then(single(5))
            .map(to_string)
            .on_next(append(&sum), &lifetime);
    }
    assert_eq!(*sum.borrow(), "1 2 3 4 5 ");
}

#[test]
fn deferred_test() {
    let launched = Rc::new(Cell::new(0));
    let checked = Rc::new(Cell::new(0));
    {
        let lifetime = Lifetime::new();
        let make_next = {
            let launched = launched.clone();
            move || {
                let launched = launched.clone();
                deferred(move || {
                    launched.set(launched.get() + 1);
                    single(launched.get())
                })
            }
        };
        {
            let launched = launched.clone();
            let checked = checked.clone();
            make_next()
                .then(make_next())
                .then(make_next())
                .then(make_next())
                .then(make_next())
                .on_next(
                    move |value| {
                        checked.set(checked.get() + 1);
                        assert_eq!(checked.get(), launched.get());
                        assert_eq!(checked.get(), value);
                    },
                    &lifetime,
                );
        }
        assert_eq!(launched.get(), 5);
    }
}

#[test]
fn filter_test() {
    let sum = log();
    {
        let lifetime = Lifetime::new();
        single(1)
            .then(single(1))
            .then(single(2))
            .then(single(2))
            .then(single(3))
            .filter(|value| *value != 2)
            .map(to_string)
            .on_next(append(&sum), &lifetime);
    }
    assert_eq!(*sum.borrow(), "1 1 3 ");
}

#[test]
fn filter_tuple_test() {
    let sum = log();
    {
        let _lifetime = single((1, 2))
            .then(single((1, 2)))
            .then(single((2, 3)))
            .then(single((2, 3)))
            .then(single((3, 4)))
            .filter(|(first, _second)| *first != 2)
            .map(|(_first, second)| second.to_string())
            .on_next_returning(append(&sum));
    }
    assert_eq!(*sum.borrow(), "2 2 4 ");
}

#[test]
fn distinct_until_changed_test() {
    let sum = log();
    {
        let lifetime = Lifetime::new();
        single(1)
            .then(single(1))
            .then(single(2))
            .then(single(2))
            .then(single(3))
            .distinct_until_changed()
            .map(to_string)
            .on_next(append(&sum), &lifetime);
    }
    assert_eq!(*sum.borrow(), "1 2 3 ");
}

#[test]
fn flatten_latest_test() {
    let sum = log();
    {
        let lifetime = Lifetime::new();
        {
            let stream = EventStream::<i32>::new();
            let done_log = sum.clone();
            single(single(1).then(single(2)))
                .then(single(single(3).then(single(4))))
                .then(single(single(5).then(stream.events())))
                .flatten_latest()
                .map(to_string)
                .on_next_done(
                    append(&sum),
                    move || done_log.borrow_mut().push_str("done "),
                    &lifetime,
                );
            stream.fire(6);
        }
        single(single(1))
            .then(single(single(2).then(single(3))))
            .then(single(single(4).then(single(5)).then(single(6))))
            .flatten_latest()
            .map(to_string)
            .on_next(append(&sum), &lifetime);
    }
    assert_eq!(*sum.borrow(), "1 2 3 4 5 6 done 1 2 3 4 5 6 ");
}

#[test]
fn combine_vector_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let a = EventStream::<bool>::new();
        let b = EventStream::<bool>::new();
        let c = EventStream::<bool>::new();

        let v: Vec<Producer<bool>> = vec![a.events(), b.events(), c.events()];

        let sum = sum.clone();
        combine_vec(v)
            .map(|values| values[0] && values[1] && !values[2])
            .on_next(
                move |value| sum.borrow_mut().push_str(if value { "1" } else { "0" }),
                &lifetime,
            );

        a.fire(true);
        b.fire(true);
        c.fire(false);
        a.fire(false);
        b.fire(true);
        a.fire(true);
        c.fire(true);
    }
    assert_eq!(*sum.borrow(), "10010");
}

#[test]
fn combine_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let a = EventStream::<i32>::new();
        let b = EventStream::<i16>::new();
        let c = EventStream::<i8>::new();

        {
            let sum = sum.clone();
            combine((a.events(), b.events(), c.events()))
                .map(|(a, b, c)| {
                    let (a, _b, _c) = (i64::from(a), i64::from(b), i64::from(c));
                    a
                })
                .on_next(
                    move |value| sum.borrow_mut().push_str(&value.to_string()),
                    &lifetime,
                );
        }

        {
            let sum = sum.clone();
            combine((a.events(), b.events(), c.events()))
                .map(|value| value.1)
                .on_next(
                    move |value| sum.borrow_mut().push_str(&value.to_string()),
                    &lifetime,
                );
        }

        {
            let sum = sum.clone();
            combine((a.events(), b.events(), c.events()))
                .map(|value| {
                    (
                        value.0.to_string(),
                        value.1.to_string(),
                        value.2.to_string(),
                    )
                })
                .on_next(
                    move |value| {
                        let mut sum = sum.borrow_mut();
                        sum.push_str(&format!("{} {} {} ", value.0, value.1, value.2));
                    },
                    &lifetime,
                );
        }
        a.fire(1);
        b.fire(2);
        c.fire(3);
        a.fire(4);
        b.fire(5);
        c.fire(6);
    }
    assert_eq!(*sum.borrow(), "121 2 3 424 2 3 454 5 3 454 5 6 ");
}

#[test]
fn mappers_test() {
    // `rpl::mappers` placeholders (`_1 + _2 + _3 + 10`) are closures.
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let a = EventStream::<i32>::new();
        let b = EventStream::<i16>::new();
        let c = EventStream::<i8>::new();

        {
            let sum = sum.clone();
            combine((a.events(), b.events(), c.events()))
                .map(|(a, b, c)| a + i32::from(b) + i32::from(c) + 10)
                .on_next(
                    move |value| sum.borrow_mut().push_str(&value.to_string()),
                    &lifetime,
                );
        }

        a.fire(1);
        b.fire(2);
        c.fire(3);
        a.fire(4);
        b.fire(5);
        c.fire(6);
    }
    assert_eq!(*sum.borrow(), "16192225");
}

#[test]
fn after_next_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let after = sum.clone();
        let next = sum.clone();
        ints_count(3)
            .after_next(move |value| after.borrow_mut().push_str(&(-value - 1).to_string()))
            .on_next(
                move |value| next.borrow_mut().push_str(&value.to_string()),
                &lifetime,
            );
    }
    assert_eq!(*sum.borrow(), "0-11-22-3");
}

#[test]
fn combine_previous_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let a = EventStream::<i32>::new();

        {
            let sum = sum.clone();
            a.events().combine_previous().on_next(
                move |(previous, next)| {
                    sum.borrow_mut().push_str(&format!("{previous} {next} "));
                },
                &lifetime,
            );
        }

        {
            let sum = sum.clone();
            a.events().combine_previous_with_default(5).on_next(
                move |(previous, next)| {
                    sum.borrow_mut()
                        .push_str(&format!("{} {next} ", 10 + previous));
                },
                &lifetime,
            );
        }

        a.fire(1);
        a.fire(2);
        a.fire(3);
        a.fire(4);
    }
    assert_eq!(*sum.borrow(), "15 1 1 2 11 2 2 3 12 3 3 4 13 4 ");
}

fn next_done_log(sum: &Rc<RefCell<String>>) -> (impl Fn(i32) + 'static, impl FnOnce() + 'static) {
    let next = sum.clone();
    let done = sum.clone();
    (
        move |value: i32| next.borrow_mut().push_str(&value.to_string()),
        move || done.borrow_mut().push_str("done"),
    )
}

#[test]
fn take_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(10).take(3).on_next_done(next, done, &lifetime);
    }
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(3).take(3).on_next_done(next, done, &lifetime);
    }
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(3).take(10).on_next_done(next, done, &lifetime);
    }
    assert_eq!(*sum.borrow(), "012done012done012done");
}

#[test]
fn skip_test() {
    let sum = Rc::new(RefCell::new(String::new()));
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(10).skip(5).on_next_done(next, done, &lifetime);
    }
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(3).skip(3).on_next_done(next, done, &lifetime);
    }
    {
        let lifetime = Lifetime::new();
        let (next, done) = next_done_log(&sum);
        ints_count(3).skip(10).on_next_done(next, done, &lifetime);
    }
    assert_eq!(*sum.borrow(), "56789donedonedone");
}
