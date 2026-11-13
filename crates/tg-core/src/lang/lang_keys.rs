//! Port of `lang/lang_keys.{h,cpp}`: date and name phrases.
//!
//! Qt types in the signatures are replaced by plain values: a date is
//! `(year, month, day)`, the current date and the locale short formats come
//! from [`DateEnvironment`] (`QDate::currentDate()`, `QLocale().toString(...,
//! QLocale::ShortFormat)`), which the platform layer provides.
use super::lang_values::PhraseExt;
use super::{lt_date, lt_day, lt_first_name, lt_last_name, lt_month, lt_time, lt_year, tr, Phrase};
use std::cell::RefCell;
use std::rc::Rc;

const DEFAULT_LANGUAGE: &str = "en";

/// A calendar date, `QDate` in the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    /// 1..=12.
    pub month: i32,
    /// 1..=31.
    pub day: i32,
}

/// A time of day, `QTime` in the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time {
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
}

/// What the date helpers take from Qt and the system. See the module docs.
pub struct DateEnvironment {
    /// `QDate::currentDate()`.
    pub current_date: Box<dyn Fn() -> Date>,
    /// `QLocale().toString(date, QLocale::ShortFormat)`.
    pub short_date: Box<dyn Fn(Date) -> String>,
    /// `QLocale().toString(time, QLocale::ShortFormat)`.
    pub short_time: Box<dyn Fn(Time) -> String>,
}

thread_local! {
    static ENVIRONMENT: RefCell<Option<Rc<DateEnvironment>>> = const { RefCell::new(None) };
}

/// Installs the date environment of this thread.
pub fn set_date_environment(environment: Option<DateEnvironment>) {
    ENVIRONMENT.with(|current| *current.borrow_mut() = environment.map(Rc::new));
}

fn environment() -> Rc<DateEnvironment> {
    ENVIRONMENT
        .with(|current| current.borrow().clone())
        .expect("lang_keys: the date environment is not set")
}

/// `langDateMaybeWithYear`.
fn lang_date_maybe_with_year(
    date: Date,
    with_year: impl FnOnce(i32, i32) -> String,
    without_year: impl FnOnce(i32, i32) -> String,
) -> String {
    let month = date.month;
    if month <= 0 || month > 12 {
        return "MONTH_ERR".into();
    }
    let year = date.year;
    let current = (environment().current_date)();
    let current_year = current.year;
    let current_month = current.month;
    if year != current_year {
        let year_is_much_greater = |year: i32, other_year: i32| year > other_year + 1;
        let month_is_much_greater = |year: i32, month: i32, other_year: i32, other_month: i32| {
            (year == other_year + 1) && (month + 12 > other_month + 3)
        };
        if year_is_much_greater(year, current_year)
            || year_is_much_greater(current_year, year)
            || month_is_much_greater(year, month, current_year, current_month)
            || month_is_much_greater(current_year, current_month, year, month)
        {
            return with_year(month, year);
        }
    }
    without_year(month, year)
}

/// `langFirstNameGoesSecond`.
pub fn lang_first_name_goes_second() -> bool {
    const FIRST_NAME: &str = "\u{1}";
    const LAST_NAME: &str = "\u{2}";
    let fullname = tr::lng_full_name
        .tag(lt_first_name, FIRST_NAME)
        .tag(lt_last_name, LAST_NAME)
        .now();
    // `indexOf` gives -1 for a missing part, which also compares as "less".
    let index = |part: &str| fullname.find(part).map_or(-1, |index| index as i64);
    index(LAST_NAME) < index(FIRST_NAME)
}

/// `langFullName`.
pub fn lang_full_name(first_name: &str, last_name: &str) -> String {
    if first_name.is_empty() {
        return last_name.to_owned();
    } else if last_name.is_empty() {
        return first_name.to_owned();
    }
    if lang_first_name_goes_second() {
        format!("{last_name} {first_name}")
    } else {
        format!("{first_name} {last_name}")
    }
}

fn month_day_year(month: String, day: i32, year: i32) -> String {
    tr::lng_month_day_year
        .tag(lt_month, month)
        .tag(lt_day, day.to_string())
        .tag(lt_year, year.to_string())
        .now()
}

fn month_day(month: String, day: i32) -> String {
    tr::lng_month_day
        .tag(lt_month, month)
        .tag(lt_day, day.to_string())
        .now()
}

/// `langDayOfMonth`.
pub fn lang_day_of_month(date: Date) -> String {
    let day = date.day;
    lang_date_maybe_with_year(
        date,
        |month, year| month_day_year(month_small(month).now(), day, year),
        |month, _| month_day(month_small(month).now(), day),
    )
}

/// `langDayOfMonthFull`.
pub fn lang_day_of_month_full(date: Date) -> String {
    let day = date.day;
    lang_date_maybe_with_year(
        date,
        |month, year| month_day_year(self::month_day_phrase(month).now(), day, year),
        |month, _| month_day(self::month_day_phrase(month).now(), day),
    )
}

/// `langDayOfMonthShort`.
pub fn lang_day_of_month_short(date: Date) -> String {
    let day = date.day;
    lang_date_maybe_with_year(
        date,
        |_, _| (environment().short_date)(date),
        |month, _| month_day(month_small(month).now(), day),
    )
}

/// `langMonthOfYear`.
pub fn lang_month_of_year(month: i32, year: i32) -> String {
    if month > 0 && month <= 12 {
        tr::lng_month_year
            .tag(lt_month, month_small(month).now())
            .tag(lt_year, year.to_string())
            .now()
    } else {
        "MONTH_ERR".into()
    }
}

/// `langMonth`.
pub fn lang_month(date: Date) -> String {
    lang_date_maybe_with_year(date, lang_month_of_year, |month, _| {
        month_small(month).now()
    })
}

/// `langMonthOfYearFull`.
pub fn lang_month_of_year_full(month: i32, year: i32) -> String {
    if month > 0 && month <= 12 {
        tr::lng_month_year
            .tag(lt_month, self::month(month).now())
            .tag(lt_year, year.to_string())
            .now()
    } else {
        "MONTH_ERR".into()
    }
}

/// `langMonthFull`.
pub fn lang_month_full(date: Date) -> String {
    lang_date_maybe_with_year(date, lang_month_of_year_full, |month, _| {
        self::month(month).now()
    })
}

/// `langDayOfWeek(int index)`; 1 is Monday.
pub fn lang_day_of_week(index: i32) -> String {
    if index > 0 && index <= 7 {
        weekday(index).now()
    } else {
        "DAY_ERR".into()
    }
}

/// `langDayOfWeekFull(int index)`; 1 is Monday.
pub fn lang_day_of_week_full(index: i32) -> String {
    if index > 0 && index <= 7 {
        weekday_full(index).now()
    } else {
        "DAY_ERR".into()
    }
}

/// `langDateTime`.
pub fn lang_date_time(date: Date, time: Time) -> String {
    tr::lng_mediaview_date_time
        .tag(lt_date, lang_day_of_month(date))
        .tag(lt_time, (environment().short_time)(time))
        .now()
}

/// `langDateTimeFull`.
pub fn lang_date_time_full(date: Date, time: Time) -> String {
    tr::lng_mediaview_date_time
        .tag(lt_date, lang_day_of_month_full(date))
        .tag(lt_time, (environment().short_time)(time))
        .now()
}

/// `Lang::DefaultLanguageId`.
pub fn default_language_id() -> String {
    DEFAULT_LANGUAGE.to_owned()
}

/// `Lang::LanguageIdOrDefault`.
pub fn language_id_or_default(id: &str) -> String {
    if !id.is_empty() {
        id.to_owned()
    } else {
        default_language_id()
    }
}

/// `Lang::Month`.
pub fn month(index: i32) -> Phrase {
    match index {
        1 => tr::lng_month1,
        2 => tr::lng_month2,
        3 => tr::lng_month3,
        4 => tr::lng_month4,
        5 => tr::lng_month5,
        6 => tr::lng_month6,
        7 => tr::lng_month7,
        8 => tr::lng_month8,
        9 => tr::lng_month9,
        10 => tr::lng_month10,
        11 => tr::lng_month11,
        12 => tr::lng_month12,
        _ => tg_base::unexpected!("Index in MonthSmall."),
    }
}

/// `Lang::MonthSmall`.
pub fn month_small(index: i32) -> Phrase {
    match index {
        1 => tr::lng_month1_small,
        2 => tr::lng_month2_small,
        3 => tr::lng_month3_small,
        4 => tr::lng_month4_small,
        5 => tr::lng_month5_small,
        6 => tr::lng_month6_small,
        7 => tr::lng_month7_small,
        8 => tr::lng_month8_small,
        9 => tr::lng_month9_small,
        10 => tr::lng_month10_small,
        11 => tr::lng_month11_small,
        12 => tr::lng_month12_small,
        _ => tg_base::unexpected!("Index in MonthSmall."),
    }
}

/// `Lang::MonthDay`.
pub fn month_day_phrase(index: i32) -> Phrase {
    match index {
        1 => tr::lng_month_day1,
        2 => tr::lng_month_day2,
        3 => tr::lng_month_day3,
        4 => tr::lng_month_day4,
        5 => tr::lng_month_day5,
        6 => tr::lng_month_day6,
        7 => tr::lng_month_day7,
        8 => tr::lng_month_day8,
        9 => tr::lng_month_day9,
        10 => tr::lng_month_day10,
        11 => tr::lng_month_day11,
        12 => tr::lng_month_day12,
        _ => tg_base::unexpected!("Index in MonthDay."),
    }
}

/// `Lang::Weekday`.
pub fn weekday(index: i32) -> Phrase {
    match index {
        1 => tr::lng_weekday1,
        2 => tr::lng_weekday2,
        3 => tr::lng_weekday3,
        4 => tr::lng_weekday4,
        5 => tr::lng_weekday5,
        6 => tr::lng_weekday6,
        7 => tr::lng_weekday7,
        _ => tg_base::unexpected!("Index in Weekday."),
    }
}

/// `Lang::WeekdayFull`.
pub fn weekday_full(index: i32) -> Phrase {
    match index {
        1 => tr::lng_hours_monday,
        2 => tr::lng_hours_tuesday,
        3 => tr::lng_hours_wednesday,
        4 => tr::lng_hours_thursday,
        5 => tr::lng_hours_friday,
        6 => tr::lng_hours_saturday,
        7 => tr::lng_hours_sunday,
        _ => tg_base::unexpected!("Index in WeekdayFull."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lang_instance::{instance, Language};

    fn date(year: i32, month: i32, day: i32) -> Date {
        Date { year, month, day }
    }

    fn setup(today: Date) {
        instance().switch_to_id(&Language {
            id: "en".into(),
            ..Default::default()
        });
        set_date_environment(Some(DateEnvironment {
            current_date: Box::new(move || today),
            short_date: Box::new(|date| format!("{}/{}/{}", date.month, date.day, date.year % 100)),
            short_time: Box::new(|time| format!("{}:{:02}", time.hour, time.minute)),
        }));
    }

    #[test]
    fn year_is_shown_only_for_distant_dates() {
        setup(date(2026, 2, 10));
        assert_eq!(lang_day_of_month(date(2026, 3, 5)), "Mar 5");
        // Last year, within the last months: no year.
        assert_eq!(lang_day_of_month(date(2025, 12, 31)), "Dec 31");
        assert_eq!(lang_day_of_month(date(2025, 11, 1)), "Nov 1");
        // Last year, more than about nine months ago: with the year.
        assert_eq!(lang_day_of_month(date(2025, 4, 1)), "Apr 1, 2025");
        // Two years away in either direction: with the year.
        assert_eq!(lang_day_of_month(date(2024, 12, 31)), "Dec 31, 2024");
        assert_eq!(lang_day_of_month(date(2028, 1, 1)), "Jan 1, 2028");
        // Next year: only when far enough ahead.
        assert_eq!(lang_day_of_month(date(2027, 1, 15)), "Jan 15, 2027");
        assert_eq!(lang_day_of_month(date(0, 13, 1)), "MONTH_ERR");
        assert_eq!(lang_day_of_month(date(2026, 0, 1)), "MONTH_ERR");
    }

    #[test]
    fn next_year_boundary_of_the_original() {
        // `month + 12 > otherMonth + 3` with the years one apart.
        setup(date(2026, 11, 20));
        assert_eq!(lang_day_of_month(date(2027, 1, 5)), "Jan 5");
        assert_eq!(lang_day_of_month(date(2027, 3, 5)), "Mar 5, 2027");
        setup(date(2027, 1, 5));
        assert_eq!(lang_day_of_month(date(2026, 11, 20)), "Nov 20");
        assert_eq!(lang_day_of_month(date(2026, 3, 1)), "Mar 1, 2026");
    }

    #[test]
    fn month_week_and_time_phrases() {
        setup(date(2026, 2, 10));
        assert_eq!(lang_day_of_month_full(date(2026, 3, 5)), "March 5");
        assert_eq!(lang_day_of_month_full(date(2020, 3, 5)), "March 5, 2020");
        assert_eq!(lang_day_of_month_short(date(2026, 3, 5)), "Mar 5");
        assert_eq!(lang_day_of_month_short(date(2020, 3, 5)), "3/5/20");
        assert_eq!(lang_month_of_year(3, 2020), "Mar 2020");
        assert_eq!(lang_month_of_year(13, 2020), "MONTH_ERR");
        assert_eq!(lang_month_of_year_full(3, 2020), "March 2020");
        assert_eq!(lang_month_of_year_full(0, 2020), "MONTH_ERR");
        assert_eq!(lang_month(date(2026, 3, 5)), "Mar");
        assert_eq!(lang_month(date(2020, 3, 5)), "Mar 2020");
        assert_eq!(lang_month_full(date(2026, 3, 5)), "March");
        assert_eq!(lang_month_full(date(2020, 3, 5)), "March 2020");
        assert_eq!(lang_day_of_week(1), "Mon");
        assert_eq!(lang_day_of_week(7), "Sun");
        assert_eq!(lang_day_of_week(0), "DAY_ERR");
        assert_eq!(lang_day_of_week_full(1), "Monday");
        assert_eq!(lang_day_of_week_full(8), "DAY_ERR");
        let time = Time {
            hour: 9,
            minute: 5,
            second: 0,
        };
        assert_eq!(lang_date_time(date(2026, 3, 5), time), "Mar 5 at 9:05");
        assert_eq!(
            lang_date_time_full(date(2026, 3, 5), time),
            "March 5 at 9:05"
        );
    }

    #[test]
    fn full_name_order_follows_the_language() {
        setup(date(2026, 2, 10));
        assert!(!lang_first_name_goes_second());
        assert_eq!(lang_full_name("A", "B"), "A B");
        assert_eq!(lang_full_name("", "B"), "B");
        assert_eq!(lang_full_name("A", ""), "A");
        instance().apply_value(b"lng_full_name", b"{last_name} {first_name}");
        assert!(lang_first_name_goes_second());
        assert_eq!(lang_full_name("A", "B"), "B A");
        assert_eq!(default_language_id(), "en");
        assert_eq!(language_id_or_default(""), "en");
        assert_eq!(language_id_or_default("ru"), "ru");
    }
}
