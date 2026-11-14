//! Value contracts from data_peer_common.h.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StarsRating {
    pub level: i32,
    pub stars: i32,
    pub this_level_stars: i32,
    pub next_level_stars: i32,
}
impl StarsRating {
    pub fn is_present(self) -> bool {
        self.level != 0 || self.this_level_stars != 0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StarsRatingPending {
    pub value: StarsRating,
    pub date: i32,
}
impl StarsRatingPending {
    pub fn is_present(self) -> bool {
        self.value.is_present() && self.date != 0
    }
}
