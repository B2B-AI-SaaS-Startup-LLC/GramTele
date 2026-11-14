//! LocationPoint data slice required by ChannelLocation. Formatting/hash/image binding stays package 09/5.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocationPoint {
    pub lat: f64,
    pub lon: f64,
    pub access_hash: u64,
}
impl PartialEq for LocationPoint {
    fn eq(&self, o: &Self) -> bool {
        self.lat == o.lat && self.lon == o.lon
    }
}
impl LocationPoint {
    pub fn from_mtp(d: &tg_scheme::GeoPointData) -> Self {
        Self {
            lat: d.lat,
            lon: d.long,
            access_hash: d.access_hash,
        }
    }
}
