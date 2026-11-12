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
        let mut fields = Fields::new(Environment::Test);
        fields.main_dc_id = phone_dc();
        self.add_fields(fields)
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
        if std::env::var_os("AYUGRAM_LOGIN_BUDGET_DIR").is_some() {
            let words = tg_tl::serialize(&request);
            println!(
                "request to main DC {}:\n{}",
                self.senders[index].instance().main_dc_id(),
                tg_mtproto::details::mtproto_dump_to_text::dump_to_text_type_from(&words, 0).0
            );
            println!(
                "request hex: {}",
                words
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
        }
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
                if std::env::var_os("AYUGRAM_LOGIN_BUDGET_DIR").is_some() {
                    let instance = self.senders[index].instance();
                    println!(
                        "response route: DC={}, identity={:?}",
                        instance.main_dc_id(),
                        instance.session(instance.main_dc_id()).map(|s| (
                            s.identity(),
                            s.endpoint(),
                            s.init_prefix()
                        ))
                    );
                }
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
    let digit = phone_dc() as u8 + b'0';
    let phone = format!(
        "99966{}{:04}",
        phone_dc(),
        u32::from_le_bytes(bytes) % 10000
    );
    if let Some(root) = std::env::var_os("AYUGRAM_LOGIN_BUDGET_DIR") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut recorded = false;
        for i in 1..=8 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(format!("attempt-{i}")))
            {
                Ok(mut file) => {
                    use std::io::Write;
                    writeln!(file, "{phone}").map_err(|e| e.to_string())?;
                    println!("login attempt {i}/8");
                    recorded = true;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        if !recorded {
            return Err("eight-attempt login budget exhausted".into());
        }
    }
    println!("test phone: {phone}; initial DC={}", phone_dc());
    if std::env::var_os("AYUGRAM_LOGIN_ORIGINAL_PREFLIGHT").is_some() {
        let nearest = h.rpc(first, HelpGetNearestDc::new()).await?;
        println!(
            "intro nearest: {}",
            tg_mtproto::details::mtproto_dump_to_text::dump_to_text_type_from(
                &tg_tl::serialize(&nearest),
                0
            )
            .0
        );
        let NearestDc::NearestDc(nearest) = nearest;
        h.senders[first]
            .instance()
            .suggest_main_dc_id(nearest.nearest_dc);
        let countries = h.rpc(first, HelpGetCountriesList::new(vec![], 0)).await?;
        println!(
            "intro countries: received {} bytes",
            tg_tl::serialize(&countries).len() * 4
        );
    }
    let send_phone = if std::env::var_os("AYUGRAM_LOGIN_ORIGINAL_PREFLIGHT").is_some() {
        format!("+{phone}")
    } else {
        phone.clone()
    };
    h.senders[first]
        .instance()
        .set_user_phone(send_phone.clone());
    let sent = h
        .rpc(
            first,
            AuthSendCode::new(
                send_phone.as_bytes().to_vec(),
                API_ID,
                API_HASH.as_bytes().to_vec(),
                mtp_code_settings(Default::default(), None, None, None),
            ),
        )
        .await?;
    println!(
        "auth.sendCode reply:\n{}",
        tg_mtproto::details::mtproto_dump_to_text::dump_to_text_type_from(
            &tg_tl::serialize(&sent),
            0
        )
        .0
    );
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
                        Some(vec![digit; length as usize]),
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

fn phone_dc() -> i32 {
    let dc = std::env::var("AYUGRAM_TEST_PHONE_DC")
        .ok()
        .map(|v| v.parse::<i32>().expect("test DC must be 1..3"))
        .unwrap_or(2);
    assert!((1..=3).contains(&dc));
    dc
}
