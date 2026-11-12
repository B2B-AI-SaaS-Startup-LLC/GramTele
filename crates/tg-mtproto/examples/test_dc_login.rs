//! Test DCs only. Never accepts a real phone number or production configuration.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[path = "support/test_login.rs"]
mod test_login;
use test_login::{login_code, Harness, API_HASH, API_ID};
use tg_scheme::*;
async fn run() -> Result<(), String> {
    let mut h = Harness::new();
    let first = login_code(&mut h).await?;
    let second = h.add();
    let update = Arc::new(AtomicBool::new(false));
    let notified = update.clone();
    h.senders[second]
        .instance()
        .set_updates_handler(Some(Box::new(move |response| {
            use tg_tl::Deserialize;
            let mut reader = tg_tl::Reader::new(&response.reply);
            let has = |u: &Update| matches!(u, Update::UpdateLoginToken(_));
            let found = match Updates::read(&mut reader) {
                Ok(Updates::UpdateShort(d)) => has(&d.update),
                Ok(Updates::Updates(d)) => d.updates.iter().any(has),
                Ok(Updates::UpdatesCombined(d)) => d.updates.iter().any(has),
                _ => false,
            };
            if found {
                notified.store(true, Ordering::Release);
            }
        })));
    let mut qr_authorized = false;
    let qr_result = async {
        let token = h
            .rpc(
                second,
                AuthExportLoginToken::new(API_ID, API_HASH.as_bytes().to_vec(), vec![]),
            )
            .await?;
        let AuthLoginToken::AuthLoginToken(token) = token else {
            return Err("initial QR token missing".to_string());
        };
        h.rpc(first, AuthAcceptLoginToken::new(token.token.clone()))
            .await?;
        println!("auth.acceptLoginToken: accepted");
        let start = std::time::Instant::now();
        while !update.load(Ordering::Acquire) {
            h.tick().await;
            if start.elapsed().as_secs() >= 60 {
                return Err("updateLoginToken timeout".into());
            }
        }
        println!("second Instance: updateLoginToken received");
        let mut result = h
            .rpc(
                second,
                AuthExportLoginToken::new(API_ID, API_HASH.as_bytes().to_vec(), vec![]),
            )
            .await?;
        while let AuthLoginToken::AuthLoginTokenMigrateTo(data) = result {
            println!("QR migrate: DC={}", data.dc_id);
            h.senders[second].instance().set_main_dc_id(data.dc_id);
            result = h
                .rpc(second, AuthImportLoginToken::new(data.token.clone()))
                .await?;
        }
        if !matches!(result, AuthLoginToken::AuthLoginTokenSuccess(_)) {
            return Err("QR authorization incomplete".into());
        }
        qr_authorized = true;
        println!("QR users.getUsers: id={}", h.self_id(second).await?);
        Ok(())
    }
    .await;
    let qr_logout = if qr_authorized {
        let result = h.rpc(second, AuthLogOut::new()).await;
        if result.is_ok() {
            println!("QR auth.logOut: ok");
        }
        result.map(|_| ())
    } else {
        Ok(())
    };
    let code_logout = h.rpc(first, AuthLogOut::new()).await;
    if code_logout.is_ok() {
        println!("code auth.logOut: ok");
    }
    qr_result.and(qr_logout).and(code_logout.map(|_| ()))
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tg_runtime::main_queue().bind_current_thread();
    if let Err(error) = tokio::task::LocalSet::new().run_until(run()).await {
        eprintln!("test login stopped: {error}");
        std::process::exit(1);
    }
}
