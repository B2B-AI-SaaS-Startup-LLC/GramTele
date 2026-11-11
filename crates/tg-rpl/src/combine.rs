//! Port of `rpl/combine.h`.
//!
//! `rpl::combine(a, b, c)` takes a tuple here: `combine((a, b, c))`, and
//! produces tuples `(A, B, C)`. Nothing is emitted until every producer has
//! produced at least one value; after that every new value of any producer
//! emits the updated tuple. Done when all producers are done; the first
//! error of any producer is forwarded.
//!
//! `rpl::combine(a, b, mapper)` is `combine((a, b)).map(|(a, b)| ...)`,
//! which is exactly how the C++ version is implemented.
//!
//! The C++ error type of the result is a variant of the distinct error
//! types; here all producers must share one error type (every combine in
//! the client uses `no_error`).
//!
//! `rpl::combine(std::vector<producer>)` is [`combine_vec`].

use std::cell::RefCell;

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

struct CombineState<A, L> {
    accumulated: Option<A>,
    latest: Option<L>,
    invalid: usize,
    working: usize,
}

/// Tuples of producers accepted by [`combine`].
pub trait CombineProducers {
    /// `Producer<(V1, V2, ...), E>`.
    type Output;
    /// Builds the combined producer.
    fn combine_producers(self) -> Self::Output;
}

trait UnwrapAll {
    type Output;
    fn unwrap_all(self) -> Self::Output;
}

macro_rules! impl_combine {
    ($count:expr; $($idx:tt $V:ident $p:ident),+) => {
        impl<$($V),+> UnwrapAll for ($(Option<$V>,)+) {
            type Output = ($($V,)+);
            fn unwrap_all(self) -> Self::Output {
                let ($($p,)+) = self;
                ($($p.expect("rpl::combine: missing value"),)+)
            }
        }

        impl<E: 'static, $($V: Clone + 'static),+> CombineProducers for ($(Producer<$V, E>,)+) {
            type Output = Producer<($($V,)+), E>;

            fn combine_producers(self) -> Self::Output {
                Producer::new(move |consumer: Consumer<($($V,)+), E>| {
                    let state = consumer.make_state(RefCell::new(CombineState {
                        accumulated: Some(($(None::<$V>,)+)),
                        latest: None::<($($V,)+)>,
                        invalid: $count,
                        working: $count,
                    }));
                    let ($($p,)+) = self;
                    $(
                        {
                            let next = consumer.clone();
                            let error = consumer.clone();
                            let done = consumer.clone();
                            let next_state = state.clone();
                            let done_state = state.clone();
                            consumer.add_lifetime($p.start_returning(
                                move |value: $V| {
                                    let Some(state) = next_state.try_get() else {
                                        return;
                                    };
                                    let mut guard = state.borrow_mut();
                                    let s = &mut *guard;
                                    let emit = match s.accumulated.as_mut() {
                                        None => {
                                            s.latest
                                                .as_mut()
                                                .expect("rpl::combine: no latest")
                                                .$idx = value;
                                            true
                                        }
                                        Some(accumulated) => {
                                            let was_set = accumulated.$idx.is_some();
                                            accumulated.$idx = Some(value);
                                            if was_set {
                                                false
                                            } else {
                                                s.invalid -= 1;
                                                if s.invalid == 0 {
                                                    let accumulated = s
                                                        .accumulated
                                                        .take()
                                                        .expect("rpl::combine: no accumulated");
                                                    s.latest = Some(accumulated.unwrap_all());
                                                    true
                                                } else {
                                                    false
                                                }
                                            }
                                        }
                                    };
                                    if emit {
                                        let latest = s.latest.clone();
                                        drop(guard);
                                        if let Some(latest) = latest {
                                            next.put_next(latest);
                                        }
                                    }
                                },
                                move |e| error.put_error(e),
                                move || {
                                    let Some(state) = done_state.try_get() else {
                                        return;
                                    };
                                    let finished = {
                                        let mut s = state.borrow_mut();
                                        s.working -= 1;
                                        s.working == 0
                                    };
                                    if finished {
                                        done.put_done();
                                    }
                                },
                            ));
                        }
                    )+
                    Lifetime::new()
                })
            }
        }
    };
}

impl_combine!(1; 0 V0 p0);
impl_combine!(2; 0 V0 p0, 1 V1 p1);
impl_combine!(3; 0 V0 p0, 1 V1 p1, 2 V2 p2);
impl_combine!(4; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3);
impl_combine!(5; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4);
impl_combine!(6; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5);
impl_combine!(7; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6);
impl_combine!(8; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6, 7 V7 p7);
impl_combine!(9; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6, 7 V7 p7,
    8 V8 p8);
impl_combine!(10; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6, 7 V7 p7,
    8 V8 p8, 9 V9 p9);
impl_combine!(11; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6, 7 V7 p7,
    8 V8 p8, 9 V9 p9, 10 V10 p10);
impl_combine!(12; 0 V0 p0, 1 V1 p1, 2 V2 p2, 3 V3 p3, 4 V4 p4, 5 V5 p5, 6 V6 p6, 7 V7 p7,
    8 V8 p8, 9 V9 p9, 10 V10 p10, 11 V11 p11);

/// `rpl::combine(a, b, ...)` for a tuple of 1 to 12 producers.
pub fn combine<T: CombineProducers>(producers: T) -> T::Output {
    producers.combine_producers()
}

struct CombineVectorState<V> {
    accumulated: Vec<Option<V>>,
    latest: Vec<V>,
    invalid: usize,
    working: usize,
}

/// `rpl::combine(std::vector<producer<Value, Error>>)`: produces vectors
/// of the latest values. An empty input completes immediately.
///
/// `rpl::combine(std::move(vector), mapper)` is
/// `combine_vec(vector).map(mapper)`.
pub fn combine_vec<V, E>(producers: Vec<Producer<V, E>>) -> Producer<Vec<V>, E>
where
    V: Clone + 'static,
    E: 'static,
{
    Producer::new(move |consumer: Consumer<Vec<V>, E>| {
        let count = producers.len();
        let state = consumer.make_state(RefCell::new(CombineVectorState {
            accumulated: (0..count).map(|_| None).collect(),
            latest: Vec::new(),
            invalid: count,
            working: count,
        }));
        for (index, producer) in producers.into_iter().enumerate() {
            let next = consumer.clone();
            let error = consumer.clone();
            let done = consumer.clone();
            let next_state = state.clone();
            let done_state = state.clone();
            consumer.add_lifetime(producer.start_returning(
                move |value: V| {
                    let Some(state) = next_state.try_get() else {
                        return;
                    };
                    let mut guard = state.borrow_mut();
                    let s = &mut *guard;
                    let emit = if s.accumulated.is_empty() {
                        s.latest[index] = value;
                        true
                    } else if s.accumulated[index].is_some() {
                        s.accumulated[index] = Some(value);
                        false
                    } else {
                        s.accumulated[index] = Some(value);
                        s.invalid -= 1;
                        if s.invalid == 0 {
                            let accumulated = std::mem::take(&mut s.accumulated);
                            s.latest.reserve(accumulated.len());
                            s.latest.extend(
                                accumulated
                                    .into_iter()
                                    .map(|value| value.expect("rpl::combine: missing value")),
                            );
                            true
                        } else {
                            false
                        }
                    };
                    if emit {
                        let latest = s.latest.clone();
                        drop(guard);
                        next.put_next(latest);
                    }
                },
                move |e| error.put_error(e),
                move || {
                    let Some(state) = done_state.try_get() else {
                        return;
                    };
                    let finished = {
                        let mut s = state.borrow_mut();
                        s.working -= 1;
                        s.working == 0
                    };
                    if finished {
                        done.put_done();
                    }
                },
            ));
        }
        if count == 0 {
            consumer.put_done();
        }
        Lifetime::new()
    })
}
