//! Shared test-only API driver. No production environment or arbitrary phone input.
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
pub const API_ID: i32 = 17349;
pub const API_HASH: &str = "344583e45741c457fe1862106095a5eb";
pub struct Harness {
    runtimes: Vec<RuntimeInstance>,
    pub senders: Vec<Sender>,
}
impl Harness {
    pub fn new() -> Self {
        Self {
            runtimes: vec![],
            senders: vec![],
        }
    }
    pub fn add(&mut self) -> usize {
        self.add_fields(Fields::new(Environment::Test))
    }
    pub fn add_fields(&mut self, mut fields: Fields) -> usize {
        assert!(fields.config.is_test_mode());
        fields.device_model = "Desktop".into();
        fields.system_version = "Linux".into();
        let mut runtime = RuntimeInstance::new(
            Mode::Normal,
            fields,
            ClientInfo {
                api_id: API_ID,
                ..Default::default()
            },
            TransportPolicy::TcpOnly,
        );
        runtime.set_automatic_config(false);
        self.senders.push(Sender::new(runtime.instance().clone()));
        self.runtimes.push(runtime);
        self.runtimes.len() - 1
    }
    pub async fn tick(&mut self) {
        tg_runtime::process_main_queue();
        for r in &mut self.runtimes {
            r.process();
        }
        tokio::task::yield_now().await;
    }
    pub async fn rpc<R: tg_tl::Function>(
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
    pub async fn self_id(&mut self, index: usize) -> Result<u64, String> {
        let users = self
            .rpc(index, UsersGetUsers::new(vec![mtp_input_user_self()]))
            .await?;
        match users.first() {
            Some(User::User(user)) => Ok(user.id),
            _ => Err("missing self user".into()),
        }
    }
}
pub async fn login_code(h: &mut Harness) -> Result<usize, String> {
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
    match h.self_id(first).await {
        Ok(id) => println!("code users.getUsers: id={id}"),
        Err(error) => {
            let _ = h.rpc(first, AuthLogOut::new()).await;
            return Err(error);
        }
    }
    Ok(first)
}
