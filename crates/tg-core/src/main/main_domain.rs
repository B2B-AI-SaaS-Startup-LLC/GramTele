//! Account index/order and activation used by storage; UI ownership is deferred.
use super::main_account::Authorization;
use crate::storage::details::storage_file_utilities::{Error, Result};
pub const PREMIUM_MAX_ACCOUNTS: i32 = 200;
#[derive(Default)]
pub struct Domain {
    pub accounts: Vec<(i32, Authorization)>,
    account_to_activate: i32,
}
impl Domain {
    pub fn account_added_in_storage(&mut self, index: i32, account: Authorization) -> Result<()> {
        if self.accounts.iter().any(|(i, _)| *i == index) {
            return Err(Error::Invalid("Repeated account index."));
        }
        self.accounts.push((index, account));
        Ok(())
    }
    pub fn activate_from_storage(&mut self, index: i32) {
        self.account_to_activate = index;
    }
    pub fn active_for_storage(&self) -> i32 {
        self.account_to_activate
    }
}
