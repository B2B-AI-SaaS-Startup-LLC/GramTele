//! Test DCs only. Never accepts a real phone number or production configuration.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tg_mtproto::{
    core_types::{Random, SystemRandom},
    mtp_instance::{
        runtime::{RuntimeInstance, TransportPolicy},
        Fields, Mode,
    },
    mtproto_dc_options::Environment,
    sender::Sender,
    session_private::ClientInfo,
};
use tg_scheme::*;
const API_ID: i32 = 17349;
const API_HASH: &str = "344583e45741c457fe1862106095a5eb";
struct Harness {
    runtimes: Vec<RuntimeInstance>,
    senders: Vec<Sender>,
}
impl Harness {
    fn new() -> Self {
        Self {
            runtimes: vec![],
            senders: vec![],
        }
    }
    fn add(&mut self) -> usize {
        let mut fields = Fields::new(Environment::Test);
        fields.device_model = "Desktop".into();
        fields.system_version = "Linux".into();
        let runtime = RuntimeInstance::new(
            Mode::Normal,
            fields,
            ClientInfo {
                api_id: API_ID,
                ..Default::default()
            },
            TransportPolicy::TcpOnly,
        );
        self.senders.push(Sender::new(runtime.instance().clone()));
        self.runtimes.push(runtime);
        self.runtimes.len() - 1
    }
    async fn tick(&mut self) {
        tg_runtime::process_main_queue();
        for r in &mut self.runtimes {
            r.process();
        }
        tokio::task::yield_now().await;
    }
    async fn rpc<R: tg_tl::Function>(
        &mut self,
        index: usize,
        request: R,
    ) -> Result<R::Response, String>
    where
        R::Response: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let fail = tx.clone();
        let id = self.senders[index]
            .request(request)
            .done(move |value| {
                let _ = tx.send(Ok(value));
            })
            .fail(move |error| {
                let _ = fail.send(Err(format!("{error:?}")));
            })
            .handle_flood_errors()
            .send();
        let start = std::time::Instant::now();
        loop {
            self.tick().await;
            if let Ok(result) = rx.try_recv() {
                return result;
            }
            if start.elapsed().as_secs() >= 90 {
                self.senders[index].sent_request(id).cancel();
                return Err("RPC timeout (90 seconds)".into());
            }
        }
    }
    async fn self_id(&mut self, index: usize) -> Result<u64, String> {
        let users = self
            .rpc(index, UsersGetUsers::new(vec![mtp_input_user_self()]))
            .await?;
        match users.first() {
            Some(User::User(user)) => Ok(user.id),
            _ => Err("missing self user".into()),
        }
    }
}
async fn run() -> Result<(), String> {
    let mut h = Harness::new();
    let first = h.add();
    let mut bytes = [0; 4];
    SystemRandom.fill(&mut bytes);
    let phone = format!("999662{:04}", u32::from_le_bytes(bytes) % 10000);
    println!("test phone: {phone}; initial DC=2");
    h.senders[first].instance().set_user_phone(phone.clone());
    let sent = h
        .rpc(
            first,
            AuthSendCode::new(
                phone.as_bytes().to_vec(),
                API_ID,
                API_HASH.as_bytes().to_vec(),
                mtp_code_settings(Default::default(), None, None, None),
            ),
        )
        .await?;
    let authorization = match sent {
        AuthSentCode::AuthSentCodeSuccess(data) => data.authorization.clone(),
        AuthSentCode::AuthSentCode(data) => {
            let length = match &data.type_ {
                AuthSentCodeType::AuthSentCodeTypeSms(t) => t.length,
                AuthSentCodeType::AuthSentCodeTypeApp(t) => t.length,
                AuthSentCodeType::AuthSentCodeTypeCall(t) => t.length,
                _ => return Err("test DC returned unsupported confirmation type".into()),
            };
            if !(1..=32).contains(&length) {
                return Err("invalid confirmation length".into());
            }
            println!(
                "auth.sendCode: length={length}; main DC={}",
                h.senders[first].instance().main_dc_id()
            );
            let result = h
                .rpc(
                    first,
                    AuthSignIn::new(
                        AuthSignIn::F_PHONE_CODE,
                        phone.as_bytes().to_vec(),
                        data.phone_code_hash.clone(),
                        Some(vec![b'2'; length as usize]),
                        None,
                    ),
                )
                .await;
            match result {
                Ok(AuthAuthorization::AuthAuthorizationSignUpRequired(_)) => {
                    h.rpc(
                        first,
                        AuthSignUp::new(
                            Default::default(),
                            phone.as_bytes().to_vec(),
                            data.phone_code_hash.clone(),
                            b"Port".to_vec(),
                            b"Test".to_vec(),
                        ),
                    )
                    .await?
                }
                Err(error) if error.contains("PHONE_NUMBER_UNOCCUPIED") => {
                    h.rpc(
                        first,
                        AuthSignUp::new(
                            Default::default(),
                            phone.as_bytes().to_vec(),
                            data.phone_code_hash.clone(),
                            b"Port".to_vec(),
                            b"Test".to_vec(),
                        ),
                    )
                    .await?
                }
                other => other?,
            }
        }
        _ => return Err("test DC requires payment for sendCode".into()),
    };
    if !matches!(authorization, AuthAuthorization::AuthAuthorization(_)) {
        return Err("code authorization incomplete".into());
    }
    println!("code users.getUsers: id={}", h.self_id(first).await?);
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
        if let AuthLoginToken::AuthLoginTokenMigrateTo(data) = result {
            println!("QR migrate: DC={}", data.dc_id);
            h.senders[second].instance().set_main_dc_id(data.dc_id);
            result = h
                .rpc(second, AuthImportLoginToken::new(data.token.clone()))
                .await?;
        }
        if !matches!(result, AuthLoginToken::AuthLoginTokenSuccess(_)) {
            return Err("QR authorization incomplete".into());
        }
        println!("QR users.getUsers: id={}", h.self_id(second).await?);
        Ok(())
    }
    .await;
    if qr_result.is_ok() {
        h.rpc(second, AuthLogOut::new()).await?;
        println!("QR auth.logOut: ok");
    }
    h.rpc(first, AuthLogOut::new()).await?;
    println!("code auth.logOut: ok");
    qr_result
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tg_runtime::main_queue().bind_current_thread();
    if let Err(error) = tokio::task::LocalSet::new().run_until(run()).await {
        eprintln!("test login stopped: {error}");
        std::process::exit(1);
    }
}
