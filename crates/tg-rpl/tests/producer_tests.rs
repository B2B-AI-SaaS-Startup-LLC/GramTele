//! Port of `rpl/producer_tests.cpp`.
//!
//! Each C++ `SECTION` is one `#[test]`, named after the section. C++
//! captures of locals by reference (`[&stream]`, `[&composite]`) become
//! `Weak` captures of `Rc`-owned locals: a strong capture would create a
//! cycle through the consumer stored in that very stream / lifetime.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use tg_rpl::{make_producer, Consumer, EventStream, Lifetime, NoError, NoValue, Producer};

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

// TEST_CASE("basic producer tests", "[rpl::producer]")

#[test]
fn producer_next_done_and_lifetime_end_test() {
    let lifetime_ended = Rc::new(Cell::new(false));
    let sum = Rc::new(Cell::new(0));
    let done_generated = Rc::new(Cell::new(false));
    let destroyed = Rc::new(Cell::new(false));
    {
        let destroy_caller = {
            let destroyed = destroyed.clone();
            Rc::new(OnDestructor::new(move || destroyed.set(true)))
        };
        {
            let generator_caller = destroy_caller.clone();
            let lifetime_caller = destroy_caller.clone();
            let lifetime_ended = lifetime_ended.clone();
            let next_caller = destroy_caller.clone();
            let error_caller = destroy_caller.clone();
            let done_caller = destroy_caller.clone();
            let sum = sum.clone();
            let done_generated = done_generated.clone();
            let _alive = make_producer::<i32, NoError, _>(move |consumer| {
                let _ = &generator_caller;
                consumer.put_next(1);
                consumer.put_next(2);
                consumer.put_next(3);
                consumer.put_done();
                Lifetime::from_fn(move || {
                    let _ = &lifetime_caller;
                    lifetime_ended.set(true);
                })
            })
            .start_returning(
                move |value| {
                    let _ = &next_caller;
                    sum.set(sum.get() + value);
                },
                move |_: NoError| {
                    let _ = &error_caller;
                },
                move || {
                    let _ = &done_caller;
                    done_generated.set(true);
                },
            );
        }
    }
    assert_eq!(sum.get(), 1 + 2 + 3);
    assert!(done_generated.get());
    assert!(lifetime_ended.get());
    assert!(destroyed.get());
}

#[test]
fn producer_error_test() {
    let error_generated = Rc::new(Cell::new(false));
    {
        let error_generated = error_generated.clone();
        let _alive = make_producer::<NoValue, bool, _>(|consumer| {
            consumer.put_error(true);
            Lifetime::new()
        })
        .start_returning(
            |_: NoValue| {},
            move |error: bool| error_generated.set(error),
            || {},
        );
    }
    assert!(error_generated.get());
}

#[test]
fn nested_lifetimes_test() {
    let lifetime_end_count = Rc::new(Cell::new(0));
    {
        let lifetimes = Lifetime::new();
        {
            let count = lifetime_end_count.clone();
            let test_producer = make_producer::<NoValue, NoError, _>(move |_consumer| {
                let count = count.clone();
                Lifetime::from_fn(move || count.set(count.get() + 1))
            });
            test_producer.start_copy(|_| {}, |_| {}, || {}, &lifetimes);
            test_producer.start(|_| {}, |_| {}, || {}, &lifetimes);
        }
        assert_eq!(lifetime_end_count.get(), 0);
    }
    assert_eq!(lifetime_end_count.get(), 2);
}

#[test]
fn nested_producers_test() {
    let sum = Rc::new(Cell::new(0));
    let lifetime_end_count = Rc::new(Cell::new(0));
    let saved = Lifetime::new();
    {
        let count = lifetime_end_count.clone();
        let sum = sum.clone();
        make_producer::<i32, NoError, _>(move |consumer| {
            let inner = {
                let count = count.clone();
                make_producer::<i32, NoError, _>(move |consumer| {
                    consumer.put_next(1);
                    consumer.put_next(2);
                    consumer.put_next(3);
                    Lifetime::from_fn(move || count.set(count.get() + 1))
                })
            };
            let result = {
                let count = count.clone();
                Lifetime::from_fn(move || count.set(count.get() + 1))
            };
            let first = consumer.clone();
            inner.start_copy(
                move |value| {
                    first.put_next_copy(&value);
                },
                |_| {},
                || {},
                &result,
            );
            let second = consumer.clone();
            inner.start(
                move |value| {
                    second.put_next_copy(&value);
                },
                |_| {},
                || {},
                &result,
            );
            result
        })
        .start(
            move |value| sum.set(sum.get() + value),
            |_| {},
            || {},
            &saved,
        );
    }
    assert_eq!(sum.get(), 1 + 2 + 3 + 1 + 2 + 3);
    assert_eq!(lifetime_end_count.get(), 0);
    saved.destroy();
    assert_eq!(lifetime_end_count.get(), 3);
}

#[test]
fn tuple_producer_test() {
    let result = Rc::new(Cell::new(0));
    {
        let result = result.clone();
        let _alive = make_producer::<(i32, f64), NoError, _>(|consumer| {
            consumer.put_next((1, 2.));
            Lifetime::new()
        })
        .start_returning(
            move |(a, b): (i32, f64)| result.set(a + b as i32),
            |_| {},
            || {},
        );
    }
    assert_eq!(result.get(), 3);
}

// TEST_CASE("basic event_streams tests", "[rpl::event_stream]")

#[test]
fn event_stream_basic_test() {
    let sum = Rc::new(Cell::new(0));
    let stream = EventStream::<i32>::new();
    stream.fire(1);
    stream.fire(2);
    stream.fire(3);
    {
        let saved = Lifetime::new();
        let sum = sum.clone();
        stream.events().start(
            move |value| sum.set(sum.get() + value),
            |_| {},
            || {},
            &saved,
        );
        stream.fire(11);
        stream.fire(12);
        stream.fire(13);
    }
    stream.fire(21);
    stream.fire(22);
    stream.fire(23);

    // The C++ test has `REQUIRE(11 + 12 + 13)`, which is always true; the
    // intended check is written out here.
    assert_eq!(sum.get(), 11 + 12 + 13);
}

fn subscribe_adder(stream: &EventStream<i32>, sum: &Rc<Cell<i32>>, lifetime: &Lifetime) {
    let sum = sum.clone();
    stream.events().start(
        move |value| sum.set(sum.get() + value),
        |_| {},
        || {},
        lifetime,
    );
}

#[test]
fn event_stream_add_in_handler_test() {
    let sum = Rc::new(Cell::new(0));
    let stream = Rc::new(EventStream::<i32>::new());

    {
        let composite = Rc::new(Lifetime::new());
        {
            let sum = sum.clone();
            let weak_stream: Weak<EventStream<i32>> = Rc::downgrade(&stream);
            let weak_composite: Weak<Lifetime> = Rc::downgrade(&composite);
            stream.events().start(
                move |value| {
                    sum.set(sum.get() + value);
                    let (Some(stream), Some(composite)) =
                        (weak_stream.upgrade(), weak_composite.upgrade())
                    else {
                        return;
                    };
                    subscribe_adder(&stream, &sum, &composite);
                },
                |_| {},
                || {},
                &composite,
            );
        }

        {
            let inner = Rc::new(Lifetime::new());
            {
                let sum = sum.clone();
                let weak_stream = Rc::downgrade(&stream);
                let weak_inner = Rc::downgrade(&inner);
                stream.events().start(
                    move |value| {
                        sum.set(sum.get() + value);
                        let (Some(stream), Some(inner)) =
                            (weak_stream.upgrade(), weak_inner.upgrade())
                        else {
                            return;
                        };
                        subscribe_adder(&stream, &sum, &inner);
                    },
                    |_| {},
                    || {},
                    &inner,
                );
            }

            stream.fire(1);
            stream.fire(2);
            stream.fire(3);
        }
        stream.fire(11);
        stream.fire(12);
        stream.fire(13);
    }
    stream.fire(21);
    stream.fire(22);
    stream.fire(23);

    assert_eq!(
        sum.get(),
        (1 + 1)
            + ((2 + 2) + (2 + 2))
            + ((3 + 3 + 3) + (3 + 3 + 3))
            + (11 + 11 + 11 + 11)
            + (12 + 12 + 12 + 12 + 12)
            + (13 + 13 + 13 + 13 + 13 + 13)
    );
}

#[test]
fn event_stream_add_and_remove_in_handler_test() {
    let sum = Rc::new(Cell::new(0));
    let stream = Rc::new(EventStream::<i32>::new());

    {
        let composite = Rc::new(Lifetime::new());
        {
            let sum = sum.clone();
            let weak_stream = Rc::downgrade(&stream);
            let weak_composite = Rc::downgrade(&composite);
            stream.events().start(
                move |value| {
                    sum.set(sum.get() + value);
                    let (Some(stream), Some(composite)) =
                        (weak_stream.upgrade(), weak_composite.upgrade())
                    else {
                        return;
                    };
                    composite.destroy();
                    subscribe_adder(&stream, &sum, &composite);
                },
                |_| {},
                || {},
                &composite,
            );
        }

        {
            let inner = Rc::new(Lifetime::new());
            {
                let sum = sum.clone();
                let weak_stream = Rc::downgrade(&stream);
                let weak_inner = Rc::downgrade(&inner);
                stream.events().start(
                    move |value| {
                        sum.set(sum.get() + value);
                        let (Some(stream), Some(inner)) =
                            (weak_stream.upgrade(), weak_inner.upgrade())
                        else {
                            return;
                        };
                        inner.destroy();
                        subscribe_adder(&stream, &sum, &inner);
                    },
                    |_| {},
                    || {},
                    &inner,
                );
            }

            stream.fire(1);
            stream.fire(2);
            stream.fire(3);
        }
        stream.fire(11);
        stream.fire(12);
        stream.fire(13);
    }
    stream.fire(21);
    stream.fire(22);
    stream.fire(23);

    assert_eq!(sum.get(), (1 + 1) + (2 + 2) + (3 + 3) + 11 + 12 + 13);
}

#[test]
fn event_stream_ends_before_handler_lifetime() {
    let sum = Rc::new(Cell::new(0));
    let extended = Lifetime::new();
    {
        let stream = EventStream::<i32>::new();
        subscribe_adder(&stream, &sum, &extended);
        stream.fire(1);
        stream.fire(2);
        stream.fire(3);
    }
    assert_eq!(sum.get(), 1 + 2 + 3);
}

#[test]
fn event_stream_move_test() {
    let sum = Rc::new(Cell::new(0));
    let extended = Lifetime::new();
    {
        let stream = EventStream::<i32>::new();
        {
            let sum = sum.clone();
            stream
                .events()
                .on_next(move |value| sum.set(sum.get() + value), &extended);
        }

        stream.fire(1);
        stream.fire(2);

        let moved_stream = stream;
        moved_stream.fire(3);
        moved_stream.fire(4);
    }
    assert_eq!(sum.get(), 1 + 2 + 3 + 4);
}

// TEST_CASE("basic piping tests", "[rpl::producer]")

fn int_producer(body: impl FnOnce(&Consumer<i32, i32>) + Clone + 'static) -> Producer<i32, i32> {
    make_producer(move |consumer: Consumer<i32, i32>| {
        body(&consumer);
        Lifetime::new()
    })
}

#[test]
fn on_star() {
    let sum = Rc::new(Cell::new(0));
    let dones = Rc::new(Cell::new(0));
    let add = |sum: &Rc<Cell<i32>>| {
        let sum = sum.clone();
        move |value: i32| sum.set(sum.get() + value)
    };
    {
        let alive = Lifetime::new();
        int_producer(|c| {
            c.put_next(1);
            c.put_done();
        })
        .on_next(add(&sum), &alive);

        int_producer(|c| {
            c.put_next(11);
            c.put_error(111);
        })
        .on_error(add(&sum), &alive);

        {
            let dones = dones.clone();
            int_producer(|c| {
                c.put_next(1111);
                c.put_done();
            })
            .on_done(move || dones.set(dones.get() + 1), &alive);
        }

        int_producer(|c| {
            c.put_next(11111);
            c.put_next(11112);
            c.put_next(11113);
            c.put_error(11114);
        })
        .on_next_error(add(&sum), add(&sum), &alive);
    }

    let alive = Lifetime::new();
    {
        let dones = dones.clone();
        int_producer(|c| {
            c.put_next(111111);
            c.put_next(111112);
            c.put_next(111113);
            c.put_done();
        })
        .on_next_done(add(&sum), move || dones.set(dones.get() + 11), &alive);
    }

    {
        let dones = dones.clone();
        int_producer(|c| {
            c.put_error(1111111);
        })
        .on_error_done(add(&sum), move || dones.set(0), &alive);
    }

    {
        let dones = dones.clone();
        int_producer(|c| {
            c.put_next(11111111);
            c.put_next(11111112);
            c.put_next(11111113);
            c.put_error(11111114);
        })
        .on_next_error_done(add(&sum), add(&sum), move || dones.set(0), &alive);
    }

    assert_eq!(
        sum.get(),
        1 + 111
            + 11111
            + 11112
            + 11113
            + 11114
            + 111111
            + 111112
            + 111113
            + 1111111
            + 11111111
            + 11111112
            + 11111113
            + 11111114
    );
    assert_eq!(dones.get(), 1 + 11);
}

#[test]
fn on_next_should_copy_its_callback() {
    let sum = Rc::new(RefCell::new(Some(0)));
    {
        let next = {
            let sum = sum.clone();
            move |value: i32| {
                let mut sum = sum.borrow_mut();
                let sum = sum.as_mut().expect("sum is alive");
                *sum += value;
            }
        };

        for _ in 0..3 {
            let alive = Lifetime::new();
            int_producer(|c| {
                c.put_next(1);
                c.put_done();
            })
            .on_next(next.clone(), &alive);
        }
    }
    assert_eq!(*sum.borrow(), Some(3));
}
