//! Account index/order and activation used by storage; UI ownership is deferred.
use super::main_account::Authorization;
use crate::storage::details::storage_file_utilities::Result;
pub const PREMIUM_MAX_ACCOUNTS: i32 = 200;
#[derive(Default)]
pub struct Domain {
    pub accounts: Vec<(i32, Authorization)>,
    account_to_activate: i32,
}
impl Domain {
    pub fn account_added_in_storage(&mut self, index: i32, account: Authorization) -> Result<()> {
        assert!(
            !self.accounts.iter().any(|(i, _)| *i == index),
            "Repeated account index."
        );
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

/// Coalesce list writes in the main queue, retaining a weak owner like crl::guard.
pub fn schedule_write_accounts(
    domain: &std::rc::Rc<std::cell::RefCell<crate::storage::storage_domain::Domain>>,
    manager: std::sync::Arc<
        std::sync::Mutex<crate::storage::details::storage_file_utilities::WriteManager>,
    >,
) {
    let mut state = domain.borrow_mut();
    if state.write_accounts_scheduled {
        return;
    }
    state.write_accounts_scheduled = true;
    drop(state);
    let weak = std::rc::Rc::downgrade(domain);
    tg_runtime::on_main_local(move || {
        if let Some(domain) = weak.upgrade() {
            let mut d = domain.borrow_mut();
            d.write_accounts_scheduled = false;
            if let Err(error) = d.write_accounts(
                &mut manager.lock().unwrap(),
                &mut tg_mtproto::core_types::SystemRandom,
            ) {
                eprintln!("tdata account list write: {error:?}");
            }
        }
    });
}
