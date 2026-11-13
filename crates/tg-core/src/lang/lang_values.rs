//! Port of `lang/lang_values.h`: getting the text of a phrase.
//!
//! The original builds every phrase call from variadic templates over the
//! tag types (`tr::lng_x(tr::now, lt_user, name, lt_count, 5.)`). Rust has no
//! variadic generics, so a call is a short builder instead:
//!
//! | C++ | Rust |
//! |---|---|
//! | `tr::lng_x(tr::now)` | `tr::lng_x.now()` |
//! | `tr::lng_x(tr::now, lt_user, name)` | `tr::lng_x.tag(lt_user, name).now()` |
//! | `tr::lng_x(tr::now, lt_count, n)` | `tr::lng_x.count(n).now()` |
//! | `tr::lng_x(tr::now, lt_count_short, n)` | `tr::lng_x.count_short(n).now()` |
//! | `tr::lng_x(tr::now, lt_count_decimal, n)` | `tr::lng_x.count_decimal(n).now()` |
//! | `tr::lng_x(tr::now, lt_a, tr::bold(a), tr::marked)` | `tr::lng_x.project(tr::marked).tag(lt_a, tr::bold(a)).now()` |
//! | `tr::lng_x()` | `tr::lng_x.producer()` |
//! | `tr::lng_x(lt_user, producer)` | `tr::lng_x.rx().tag(lt_user, producer).producer()` |
//! | `tr::lng_x(lt_count, count \| tr::to_count())` | `tr::lng_x.rx().count(count).producer()` |
//! | `tr::lng_x(lt_a, a, tr::marked)` | `tr::lng_x.rx_project(tr::marked).tag(lt_a, a).producer()` |
//!
//! What the templates checked at compile time is checked at run time here:
//! a tag the phrase does not have, a count for a phrase that is not plural
//! or a missing count are precondition failures.
use super::lang_instance::{current, value};
use super::lang_tag::{self, CountType};
use super::{lt_count, Phrase, PLURAL_COUNT};
use std::rc::Rc;
use tg_rpl::Producer;

/// `StartReplacements<ResultString>` and `ReplaceTag<ResultString>`: a type a
/// phrase can be projected to.
pub trait LangString: Clone + 'static {
    /// `StartReplacements<Self>::Call`.
    fn start_replacements(lang_string: String) -> Self;
    /// `ReplaceTag<Self>::Call`.
    fn replace_tag(self, tag: u16, replacement: &Self) -> Self;
}

impl LangString for String {
    fn start_replacements(lang_string: String) -> Self {
        lang_string
    }

    fn replace_tag(self, tag: u16, replacement: &Self) -> Self {
        lang_tag::replace_tag(self, tag, replacement)
    }
}

type Projector<S> = Rc<dyn Fn(String) -> S>;

fn check_tag(phrase: Phrase, tag: u16) {
    tg_base::expects!(phrase.tags.contains(&tag));
}

/// `ReplaceUnwrap<Tags...>::Call` after the count replacement.
fn finish<S: LangString>(mut accumulated: S, tags: &[(u16, S)]) -> S {
    for (tag, value) in tags {
        accumulated = accumulated.replace_tag(*tag, value);
    }
    accumulated
}

/// The arguments of an immediate phrase call, see the module docs.
pub struct Args<S: LangString> {
    phrase: Phrase,
    projector: Projector<S>,
    count: Option<(CountType, f64)>,
    tags: Vec<(u16, S)>,
}

impl<S: LangString> Args<S> {
    fn new(phrase: Phrase, projector: Projector<S>) -> Self {
        Self {
            phrase,
            projector,
            count: None,
            tags: Vec::new(),
        }
    }

    /// `lt_<tag>, value`.
    pub fn tag(mut self, tag: u16, value: impl Into<S>) -> Self {
        check_tag(self.phrase, tag);
        self.tags.push((tag, value.into()));
        self
    }

    fn with_count(mut self, type_: CountType, count: f64) -> Self {
        tg_base::expects!(self.phrase.plural);
        self.count = Some((type_, count));
        self
    }

    /// `lt_count, count`.
    pub fn count(self, count: impl Into<f64>) -> Self {
        self.with_count(CountType::Count, count.into())
    }

    /// `lt_count_short, count`.
    pub fn count_short(self, count: impl Into<f64>) -> Self {
        self.with_count(CountType::Short, count.into())
    }

    /// `lt_count_decimal, count`.
    pub fn count_decimal(self, count: impl Into<f64>) -> Self {
        self.with_count(CountType::Decimal, count.into())
    }

    /// `Producer<Tags...>::Current`.
    pub fn now(self) -> S {
        let base = self.phrase.base;
        tg_base::expects!(self.phrase.plural == self.count.is_some());
        let Some((type_, count)) = self.count else {
            return finish((self.projector)(current(base)), &self.tags);
        };
        let plural = lang_tag::plural(base, count, type_);
        let accumulated = (self.projector)(current(base + plural.key_shift))
            .replace_tag(lt_count, &S::start_replacements(plural.replacement));
        finish(accumulated, &self.tags)
    }
}

/// The arguments of a reactive phrase call, see the module docs.
pub struct RxArgs<S: LangString> {
    phrase: Phrase,
    projector: Projector<S>,
    count: Option<(CountType, Producer<f64>)>,
    tags: Vec<(u16, Producer<S>)>,
}

impl<S: LangString> RxArgs<S> {
    fn new(phrase: Phrase, projector: Projector<S>) -> Self {
        Self {
            phrase,
            projector,
            count: None,
            tags: Vec::new(),
        }
    }

    /// `lt_<tag>, std::move(producer)`.
    pub fn tag(mut self, tag: u16, value: Producer<S>) -> Self {
        check_tag(self.phrase, tag);
        self.tags.push((tag, value));
        self
    }

    fn with_count(mut self, type_: CountType, count: Producer<f64>) -> Self {
        tg_base::expects!(self.phrase.plural);
        self.count = Some((type_, count));
        self
    }

    /// `lt_count, std::move(count)`.
    pub fn count(self, count: Producer<f64>) -> Self {
        self.with_count(CountType::Count, count)
    }

    /// `lt_count_short, std::move(count)`.
    pub fn count_short(self, count: Producer<f64>) -> Self {
        self.with_count(CountType::Short, count)
    }

    /// `lt_count_decimal, std::move(count)`.
    pub fn count_decimal(self, count: Producer<f64>) -> Self {
        self.with_count(CountType::Decimal, count)
    }

    /// `Producer<Tags...>::Combine`: a new text whenever the language or one
    /// of the arguments changes.
    pub fn producer(self) -> Producer<S> {
        let base = self.phrase.base;
        let projector = self.projector;
        tg_base::expects!(self.phrase.plural == self.count.is_some());
        let (tag_ids, tag_values): (Vec<u16>, Vec<Producer<S>>) = self.tags.into_iter().unzip();
        // `rpl::combine` of nothing never fires: a phrase without tags is
        // combined with a single empty set instead.
        let tags = if tag_values.is_empty() {
            tg_rpl::single(Vec::new())
        } else {
            tg_rpl::combine_vec(tag_values)
        };
        let Some((type_, count)) = self.count else {
            return tg_rpl::combine((value(base), tags)).map(move |(text, values)| {
                let mut accumulated = projector(text);
                for (tag, value) in tag_ids.iter().zip(&values) {
                    accumulated = accumulated.replace_tag(*tag, value);
                }
                accumulated
            });
        };
        let forms =
            tg_rpl::combine_vec((0..PLURAL_COUNT).map(|shift| value(base + shift)).collect());
        tg_rpl::combine((forms, count, tags)).map(move |(forms, count, values)| {
            let plural = lang_tag::plural(base, count, type_);
            let text = forms[usize::from(plural.key_shift)].clone();
            let mut accumulated =
                projector(text).replace_tag(lt_count, &S::start_replacements(plural.replacement));
            for (tag, value) in tag_ids.iter().zip(&values) {
                accumulated = accumulated.replace_tag(*tag, value);
            }
            accumulated
        })
    }
}

/// The calls of a phrase; implemented for [`Phrase`].
pub trait PhraseExt: Sized {
    /// `tr::lng_x(tr::now)`.
    fn now(self) -> String;
    /// Starts `tr::lng_x(tr::now, lt_<tag>, value, ...)`.
    fn tag(self, tag: u16, value: impl Into<String>) -> Args<String>;
    /// Starts `tr::lng_x(tr::now, lt_count, count, ...)`.
    fn count(self, count: impl Into<f64>) -> Args<String>;
    /// Starts `tr::lng_x(tr::now, lt_count_short, count, ...)`.
    fn count_short(self, count: impl Into<f64>) -> Args<String>;
    /// Starts `tr::lng_x(tr::now, lt_count_decimal, count, ...)`.
    fn count_decimal(self, count: impl Into<f64>) -> Args<String>;
    /// Starts `tr::lng_x(tr::now, ..., projector)`.
    fn project<S: LangString>(self, projector: impl Fn(String) -> S + 'static) -> Args<S>;
    /// `tr::lng_x()`.
    fn producer(self) -> Producer<String>;
    /// Starts `tr::lng_x(lt_<tag>, producer, ...)`.
    fn rx(self) -> RxArgs<String>;
    /// Starts `tr::lng_x(..., projector)`.
    fn rx_project<S: LangString>(self, projector: impl Fn(String) -> S + 'static) -> RxArgs<S>;
}

impl PhraseExt for Phrase {
    fn now(self) -> String {
        Args::new(self, Rc::new(|text| text)).now()
    }

    fn tag(self, tag: u16, value: impl Into<String>) -> Args<String> {
        Args::new(self, Rc::new(|text| text)).tag(tag, value)
    }

    fn count(self, count: impl Into<f64>) -> Args<String> {
        Args::new(self, Rc::new(|text| text)).count(count)
    }

    fn count_short(self, count: impl Into<f64>) -> Args<String> {
        Args::new(self, Rc::new(|text| text)).count_short(count)
    }

    fn count_decimal(self, count: impl Into<f64>) -> Args<String> {
        Args::new(self, Rc::new(|text| text)).count_decimal(count)
    }

    fn project<S: LangString>(self, projector: impl Fn(String) -> S + 'static) -> Args<S> {
        Args::new(self, Rc::new(projector))
    }

    fn producer(self) -> Producer<String> {
        tg_base::expects!(!self.plural);
        value(self.base)
    }

    fn rx(self) -> RxArgs<String> {
        RxArgs::new(self, Rc::new(|text| text))
    }

    fn rx_project<S: LangString>(self, projector: impl Fn(String) -> S + 'static) -> RxArgs<S> {
        RxArgs::new(self, Rc::new(projector))
    }
}

/// `tr::to_count()`: counts are `float64` for the plural rules.
pub fn to_count(value: i32) -> f64 {
    f64::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lang_instance::{instance, Language, Pack};
    use crate::lang::{lt_first_name, lt_last_name, tr};
    use std::cell::RefCell;
    use std::sync::Arc;
    use tg_rpl::{EventStream, Lifetime};
    use tg_scheme::{LangPackDifferenceData, LangPackString, LangPackStringData};

    fn switch(id: &str) {
        instance().switch_to_id(&Language {
            id: id.into(),
            ..Default::default()
        });
    }

    fn apply(code: &str, from: i32, key: &str, value: &str) {
        instance().apply_difference(
            Pack::Current,
            &LangPackDifferenceData {
                lang_code: code.into(),
                from_version: from,
                version: from + 1,
                strings: vec![LangPackString::LangPackString(Arc::new(
                    LangPackStringData {
                        key: key.into(),
                        value: value.into(),
                    },
                ))],
            },
        );
    }

    #[test]
    fn immediate_values() {
        switch("en");
        assert_eq!(tr::lng_language_name.now(), "English");
        assert_eq!(
            tr::lng_full_name
                .tag(lt_first_name, "A")
                .tag(lt_last_name, String::from("B"))
                .now(),
            "A B"
        );
        // Tags may come in any order and a missing one stays as a marker.
        assert_eq!(
            tr::lng_full_name
                .tag(lt_last_name, "B")
                .tag(lt_first_name, "A")
                .now(),
            "A B"
        );
        let partial = tr::lng_full_name.tag(lt_first_name, "A").now();
        assert!(partial.starts_with("A \u{10}"));
    }

    #[test]
    fn plural_values_select_the_form_and_format_the_count() {
        switch("en");
        let one = tr::lng_filter_unread_chats.count(1).now();
        let many = tr::lng_filter_unread_chats.count(12_345).now();
        assert!(one.contains('1') && !one.contains("12"));
        assert!(many.contains("12345"));
        assert_ne!(
            one.replace('1', ""),
            many.replace("12345", ""),
            "the singular and the plural form differ"
        );
        assert!(tr::lng_filter_unread_chats
            .count_decimal(12_345)
            .now()
            .contains("12,345"));
        assert!(tr::lng_filter_unread_chats
            .count_short(12_345)
            .now()
            .contains("12.3K"));
        assert!(tr::lng_filter_unread_chats.count(1.5).now().contains("1.5"));
    }

    #[test]
    #[should_panic]
    fn a_plural_phrase_needs_a_count() {
        switch("en");
        let _ = tr::lng_filter_unread_chats.now();
    }

    #[test]
    #[should_panic]
    fn a_tag_must_belong_to_the_phrase() {
        switch("en");
        let _ = tr::lng_language_name.tag(lt_first_name, "A").now();
    }

    #[test]
    #[should_panic]
    fn a_plain_phrase_takes_no_count() {
        switch("en");
        let _ = tr::lng_language_name.count(1).now();
    }

    #[test]
    fn reactive_values_follow_the_language_and_the_arguments() {
        switch("en");
        let lifetime = Lifetime::new();
        let seen = Rc::new(RefCell::new(Vec::new()));

        let log = seen.clone();
        tr::lng_language_name
            .producer()
            .on_next(move |text| log.borrow_mut().push(text), &lifetime);
        apply("en", 0, "lng_language_name", "Eng");
        assert_eq!(*seen.borrow(), vec!["English".to_owned(), "Eng".to_owned()]);

        seen.borrow_mut().clear();
        let first = EventStream::<String>::new();
        let last = EventStream::<String>::new();
        let log = seen.clone();
        tr::lng_full_name
            .rx()
            .tag(lt_first_name, first.events_starting_with("A".into()))
            .tag(lt_last_name, last.events_starting_with("B".into()))
            .producer()
            .on_next(move |text| log.borrow_mut().push(text), &lifetime);
        first.fire("C".into());
        apply("en", 1, "lng_full_name", "{last_name}, {first_name}");
        last.fire("D".into());
        assert_eq!(
            *seen.borrow(),
            vec![
                "A B".to_owned(),
                "C B".to_owned(),
                "B, C".to_owned(),
                "D, C".to_owned(),
            ]
        );
    }

    #[test]
    fn reactive_plural_values() {
        switch("en");
        let lifetime = Lifetime::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let count = EventStream::<f64>::new();
        let log = seen.clone();
        tr::lng_filter_unread_chats
            .rx()
            .count(count.events_starting_with(to_count(1)))
            .producer()
            .on_next(move |text| log.borrow_mut().push(text), &lifetime);
        count.fire(to_count(7));
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], tr::lng_filter_unread_chats.count(1).now());
        assert_eq!(seen[1], tr::lng_filter_unread_chats.count(7).now());
    }
}
