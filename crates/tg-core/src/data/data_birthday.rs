//! Birthday model. Local calendar date is injected at the platform boundary;
//! this keeps both minute ticks and calendar changes deterministic in tests.
use std::rc::Rc;
use tg_base::timer::TimerHost;
use tg_rpl::Producer;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Birthday {
    value: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDate {
    pub year: i32,
    pub month: i32,
    pub day: i32,
}
fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn valid_date(day: i32, month: i32, year: i32) -> bool {
    day >= 1
        && match month {
            2 => day <= if year == 0 || leap(year) { 29 } else { 28 },
            4 | 6 | 9 | 11 => day <= 30,
            1..=12 => day <= 31,
            _ => false,
        }
}
impl Birthday {
    pub const YEAR_MIN: i32 = 1875;
    pub const YEAR_MAX: i32 = 2100;
    pub fn new(day: i32, month: i32, year: i32) -> Self {
        Self {
            value: if (year == 0 || (Self::YEAR_MIN..=Self::YEAR_MAX).contains(&year))
                && valid_date(day, month, year)
            {
                day + month * 100 + year * 10000
            } else {
                0
            },
        }
    }
    pub fn from_serialized(value: i32) -> Self {
        Self::new(value % 100, (value / 100) % 100, value / 10000)
    }
    pub fn serialize(self) -> i32 {
        self.value
    }
    pub fn valid(self) -> bool {
        self.value != 0
    }
    pub fn day(self) -> i32 {
        self.value % 100
    }
    pub fn month(self) -> i32 {
        (self.value / 100) % 100
    }
    pub fn year(self) -> i32 {
        self.value / 10000
    }
}
pub fn birthday_cake() -> String {
    "🎂".into()
}
pub fn birthday_age(date: Birthday, now: CalendarDate) -> i32 {
    if date.year() == 0 {
        return 0;
    }
    let born = CalendarDate {
        year: date.year(),
        month: date.month(),
        day: date.day(),
    };
    if !valid_date(born.day, born.month, born.year) || born >= now {
        return 0;
    }
    let mut age = now.year - date.year();
    let anniversary = CalendarDate {
        year: date.year() + age,
        ..born
    };
    // QDate(year,2,29) is INVALID in non-leap years; every valid date sorts
    // after invalid QDate. Preserve the original age calculation on that edge.
    if valid_date(anniversary.day, anniversary.month, anniversary.year) && now < anniversary {
        age -= 1;
    }
    age
}
pub fn is_birthday_today(date: Birthday, now: CalendarDate) -> bool {
    date.valid() && date.day() == now.day && date.month() == now.month
}
pub fn is_birthday_today_value(
    date: Birthday,
    host: &Rc<TimerHost>,
    today: impl Fn() -> CalendarDate + 'static,
) -> Producer<bool> {
    tg_rpl::single(())
        .then(tg_base::timer_rpl::timer_each_in(host, 60_000))
        .map(move |_| is_birthday_today(date, today()))
        .distinct_until_changed()
}
// BirthdayText depends on unfinished lang; tracked partial, stage4 package01.
