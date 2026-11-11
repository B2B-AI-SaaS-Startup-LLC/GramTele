//! Port of `rpl/merge.h`.
//!
//! `rpl::merge(a, b, c)` takes a tuple here: `merge((a, b, c))`. All
//! producers must have the same value and error types. Values are passed
//! on as they come; done when all producers are done.

use std::cell::Cell;

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

/// Tuples of producers accepted by [`merge`].
pub trait MergeProducers {
    /// `Producer<V, E>`.
    type Output;
    /// Builds the merged producer.
    fn merge_producers(self) -> Self::Output;
}

macro_rules! impl_merge {
    ($count:expr; $($p:ident),+) => {
        impl<V: 'static, E: 'static> MergeProducers for ($(impl_merge!(@ty $p V E),)+) {
            type Output = Producer<V, E>;

            fn merge_producers(self) -> Self::Output {
                Producer::new(move |consumer: Consumer<V, E>| {
                    let state = consumer.make_state(Cell::<usize>::new($count));
                    let ($($p,)+) = self;
                    $(
                        {
                            let next = consumer.clone();
                            let error = consumer.clone();
                            let done = consumer.clone();
                            let state = state.clone();
                            consumer.add_lifetime($p.start_returning(
                                move |value| {
                                    next.put_next(value);
                                },
                                move |e| error.put_error(e),
                                move || {
                                    let Some(working) = state.try_get() else {
                                        return;
                                    };
                                    working.set(working.get() - 1);
                                    if working.get() == 0 {
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
    (@ty $p:ident $V:ident $E:ident) => { Producer<$V, $E> };
}

impl_merge!(1; p0);
impl_merge!(2; p0, p1);
impl_merge!(3; p0, p1, p2);
impl_merge!(4; p0, p1, p2, p3);
impl_merge!(5; p0, p1, p2, p3, p4);
impl_merge!(6; p0, p1, p2, p3, p4, p5);
impl_merge!(7; p0, p1, p2, p3, p4, p5, p6);
impl_merge!(8; p0, p1, p2, p3, p4, p5, p6, p7);
impl_merge!(9; p0, p1, p2, p3, p4, p5, p6, p7, p8);
impl_merge!(10; p0, p1, p2, p3, p4, p5, p6, p7, p8, p9);
impl_merge!(11; p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10);
impl_merge!(12; p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11);

/// `rpl::merge(a, b, ...)` for a tuple of 1 to 12 producers.
pub fn merge<T: MergeProducers>(producers: T) -> T::Output {
    producers.merge_producers()
}
