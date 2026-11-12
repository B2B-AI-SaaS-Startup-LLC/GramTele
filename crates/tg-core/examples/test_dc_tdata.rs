//! Explicit test-only login -> temporary tdata -> new-process authorization check.
#[path = "../../tg-mtproto/examples/support/test_login.rs"]
mod test_login;
use std::{fs, path::Path};
use test_login::{login_code, login_qr, Harness};
use tg_core::{
    main::main_account::Authorization,
    storage::{
        details::storage_file_utilities::WriteManager, storage_account::Account,
        storage_domain::Domain,
    },
};
use tg_mtproto::{core_types::SystemRandom, mtproto_dc_options::Environment};
async fn child(root: &Path) -> Result<(), String> {
    let mut domain = Domain::new(root, "data");
    domain
        .start_modern(&[])
        .map_err(|e| format!("read synthetic test tdata: {e:?}"))?;
    let (_, auth) = domain.owner.accounts.first().ok_or("no account")?;
    let mut h = Harness::new();
    let index = h.add_fields(auth.fields(Environment::Test));
    let id = h.self_id(index).await?;
    if id != auth.user_id {
        return Err("stored and server user IDs differ".into());
    }
    println!("new process: users.getUsers id={id}, no auth.sendCode/signIn");
    h.rpc(index, tg_scheme::AuthLogOut::new()).await?;
    println!("new process: auth.logOut ok");
    Ok(())
}
async fn parent() -> Result<(), String> {
    let mut h = Harness::new();
    let index = login_code(&mut h).await?;
    if let Err(error) = login_qr(&mut h, index).await {
        let _ = h.rpc(index, tg_scheme::AuthLogOut::new()).await;
        return Err(error);
    }
    let id = h.self_id(index).await?;
    let root = std::env::temp_dir().join(format!("ayugram-live-test-tdata-{}", std::process::id()));
    if root.exists() {
        return Err("temporary output already exists".into());
    }
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let result = async {
        let mut random = SystemRandom;
        let mut manager = WriteManager::default();
        let mut domain = Domain::new(&root, "data");
        domain
            .generate_local_key(&mut random)
            .map_err(|e| format!("{e:?}"))?;
        let mut account = Account::new(&root, "data");
        domain.start_added(&mut account);
        account.authorization = Authorization::from_instance(id, h.senders[index].instance(), None);
        account
            .write_map(&mut manager, &mut random)
            .map_err(|e| format!("{e:?}"))?;
        account
            .write_mtp_data(&mut manager, &mut random)
            .map_err(|e| format!("{e:?}"))?;
        account
            .write_mtp_config(
                &tg_mtproto::mtproto_config::Config::new(Environment::Test),
                &mut manager,
                &mut random,
            )
            .map_err(|e| format!("{e:?}"))?;
        domain
            .owner
            .account_added_in_storage(0, account.authorization)
            .map_err(|e| format!("{e:?}"))?;
        domain
            .write_accounts(&mut manager, &mut random)
            .map_err(|e| format!("{e:?}"))?;
        manager.stop().map_err(|e| format!("{e:?}"))?;
        println!("parent: wrote temporary test tdata for id={id}");
        let output =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--read-test-snapshot")
                .arg(&root)
                .output()
                .map_err(|e| e.to_string())?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            return Err(format!("new process exited {}", output.status));
        }
        Ok(())
    }
    .await;
    let _ = fs::remove_dir_all(&root);
    if result.is_err() {
        let _ = h.rpc(index, tg_scheme::AuthLogOut::new()).await;
    }
    result
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tg_runtime::main_queue().bind_current_thread();
    let args: Vec<_> = std::env::args_os().collect();
    let result = tokio::task::LocalSet::new()
        .run_until(async {
            match args.as_slice() {
                [_] => parent().await,
                [_, mode, path] if mode == "--read-test-snapshot" => {
                    // Only the exact temporary test path created by this executable is accepted.
                    let p = Path::new(path);
                    let valid = p.parent() == Some(std::env::temp_dir().as_path())
                        && p.file_name().is_some_and(|n| {
                            n.to_string_lossy().starts_with("ayugram-live-test-tdata-")
                        });
                    if !valid {
                        return Err("only this example's temporary test tdata is allowed".into());
                    }
                    child(p).await
                }
                _ => Err("usage: test_dc_tdata (no phone/path inputs)".into()),
            }
        })
        .await;
    if let Err(error) = result {
        eprintln!("test tdata stopped: {error}");
        std::process::exit(1);
    }
}
