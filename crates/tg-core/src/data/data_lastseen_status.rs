//! Exact value/bitfield contract of data_lastseen_status.h.
pub const LIFE_START_DATE: i32 = 1_375_315_200;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct LastseenStatus {
    value: u32,
    available: bool,
    hidden_by_me: bool,
}
impl LastseenStatus {
    fn new(value: u32, available: bool, hidden_by_me: bool) -> Self {
        Self {
            value: value & 0x3fff_ffff,
            available,
            hidden_by_me,
        }
    }
    pub fn recently(by_me: bool) -> Self {
        Self::new(1, false, by_me)
    }
    pub fn within_week(by_me: bool) -> Self {
        Self::new(2, false, by_me)
    }
    pub fn within_month(by_me: bool) -> Self {
        Self::new(3, false, by_me)
    }
    pub fn long_ago(by_me: bool) -> Self {
        Self::new(0, false, by_me)
    }
    pub fn online_till_value(till: i32, local: bool, hidden_by_me: bool) -> Self {
        // C++ promotes the comparison to uint32 because kSpecialValueSkip is unsigned.
        if (till as u32) >= LIFE_START_DATE as u32 + 4 {
            Self::new(
                till.wrapping_sub(LIFE_START_DATE) as u32,
                !local,
                hidden_by_me,
            )
        } else {
            Self::long_ago(hidden_by_me)
        }
    }
    pub fn is_hidden(self) -> bool {
        !self.available
    }
    pub fn is_recently(self) -> bool {
        !self.available && self.value == 1
    }
    pub fn is_within_week(self) -> bool {
        !self.available && self.value == 2
    }
    pub fn is_within_month(self) -> bool {
        !self.available && self.value == 3
    }
    pub fn is_long_ago(self) -> bool {
        !self.available && self.value == 0
    }
    pub fn is_hidden_by_me(self) -> bool {
        self.hidden_by_me
    }
    pub fn is_online(self, now: i32) -> bool {
        self.value >= 4 && (LIFE_START_DATE as u32).wrapping_add(self.value) as i32 > now
    }
    pub fn is_local_online_value(self) -> bool {
        !self.available && self.value >= 4
    }
    pub fn online_till(self) -> i32 {
        if self.value >= 4 {
            (LIFE_START_DATE as u32).wrapping_add(self.value) as i32
        } else {
            0
        }
    }
    pub fn serialize(self) -> u32 {
        self.value | ((self.available as u32) << 30) | ((self.hidden_by_me as u32) << 31)
    }
    pub fn from_serialized(raw: u32) -> Self {
        let r = Self::new(
            raw & 0x3fff_ffff,
            raw & (1 << 30) != 0,
            raw & (1 << 31) != 0,
        );
        if r.valid() {
            r
        } else {
            Self::default()
        }
    }
    pub fn from_legacy(value: i32) -> Self {
        match value {
            -2 => Self::recently(false),
            -3 => Self::within_week(false),
            -4 => Self::within_month(false),
            i32::MIN..=-31 => Self::online_till_value(value.wrapping_neg(), true, false),
            1.. => Self::online_till_value(value, false, false),
            _ => Self::default(),
        }
    }
    fn valid(self) -> bool {
        i32::MAX as u32 - self.value > LIFE_START_DATE as u32
            && (!self.available || self.value >= 4)
    }
}
