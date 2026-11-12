//! Test DCs only. No real phone or production endpoint input.
#[path = "support/test_login.rs"]
mod test_login;
use test_login::{login_code, login_qr, Harness};
async fn run() -> Result<(), String> {
    let mut h = Harness::new();
    let first = login_code(&mut h).await?;
    let qr = login_qr(&mut h, first).await;
    let logout = h.rpc(first, tg_scheme::AuthLogOut::new()).await;
    if logout.is_ok() {
        println!("code auth.logOut: ok");
    }
    qr.and(logout.map(|_| ()))
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tg_runtime::main_queue().bind_current_thread();
    if let Err(error) = tokio::task::LocalSet::new().run_until(run()).await {
        eprintln!("test login stopped: {error}");
        std::process::exit(1);
    }
}
