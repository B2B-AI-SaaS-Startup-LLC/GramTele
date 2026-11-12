//! No login: only help.getConfig and help.getNearestDc, on test DCs.
use std::sync::{Arc, Mutex};
use tg_mtproto::{
    mtp_instance::{
        runtime::{RuntimeInstance, TransportPolicy},
        Fields, Mode,
    },
    mtproto_dc_options::Environment,
    sender::Sender,
    session_private::ClientInfo,
};
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tg_runtime::main_queue().bind_current_thread();
    tokio::task::LocalSet::new()
        .run_until(async {
            for http in [false, true] {
                run(http).await;
            }
        })
        .await;
}
async fn run(http: bool) {
    let mut fields = Fields::new(Environment::Test);
    fields.device_model = "Desktop".into();
    fields.system_version = "Linux".into();
    let mut runtime = RuntimeInstance::new(
        Mode::Normal,
        fields,
        ClientInfo {
            api_id: 17349,
            ..Default::default()
        },
        if http {
            TransportPolicy::HttpOnly
        } else {
            TransportPolicy::TcpOnly
        },
    );
    let instance = runtime.instance().clone();
    let sender = Sender::new(instance.clone());
    let results = Arc::new(Mutex::new(Vec::new()));
    let output = results.clone();
    sender
        .request(tg_scheme::HelpGetConfig::new())
        .done(move |config| {
            output.lock().unwrap().push(format!(
                "help.getConfig: {} DC options",
                config.c_config().dc_options.len()
            ))
        })
        .fail(|error| eprintln!("getConfig: {error:?}"))
        .send();
    let started = std::time::Instant::now();
    let mut nearest = false;
    let mut printed = 0;
    loop {
        tg_runtime::process_main_queue();
        runtime.process();
        let len = results.lock().unwrap().len();
        if len > printed {
            assert_eq!(
                instance.dctransport(0),
                if http { "HTTP" } else { "TCP" },
                "incorrect live transport label"
            );
            for value in &results.lock().unwrap()[printed..] {
                println!("{} {value}", if http { "HTTP" } else { "TCP" });
            }
            printed = len;
        }
        if len == 1 && !nearest {
            nearest = true;
            let output = results.clone();
            sender
                .request(tg_scheme::HelpGetNearestDc::new())
                .done(move |dc| {
                    let dc = dc.c_nearest_dc();
                    output.lock().unwrap().push(format!(
                        "help.getNearestDc: country={}, this_dc={}, nearest_dc={}",
                        String::from_utf8_lossy(&dc.country),
                        dc.this_dc,
                        dc.nearest_dc
                    ));
                })
                .fail(|error| eprintln!("getNearestDc: {error:?}"))
                .send();
        }
        if len == 2 {
            break;
        }
        assert!(
            started.elapsed().as_secs() <= 60,
            "{} TIMEOUT state={} transport={}",
            if http { "HTTP" } else { "TCP" },
            instance.dcstate(0),
            instance.dctransport(0)
        );
        tokio::task::yield_now().await;
    }
}
