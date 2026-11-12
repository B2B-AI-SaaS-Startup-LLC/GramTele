//! Only test DCs; the only application RPCs are help.getConfig/help.getNearestDc.
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tg_base::timer::{SystemClock, ThreadAlarm, TimerHost};
use tg_mtproto::{
    connection_abstract::RuntimeClock,
    core_types::SystemRandom,
    mtp_instance::{network::NetworkTransport, Fields, Instance, Mode},
    mtproto_dc_options::Environment,
    sender::Sender,
    session::{Session, SessionParameters},
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
    let instance = Instance::new(Mode::Normal, fields);
    instance.start();
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
    let mut tasks = Vec::new();
    loop {
        for spawn in instance.take_session_spawns() {
            let clock = Arc::new(SystemClock);
            let host = TimerHost::new(clock.clone());
            let (mut session, handle) = Session::new(
                spawn.id,
                spawn.dc,
                Arc::new(instance.clone()),
                instance.store.clone(),
                SessionParameters {
                    clock: RuntimeClock,
                    random: SystemRandom,
                    host: host.clone(),
                    info: ClientInfo::default(),
                },
            );
            let mut options = session.data.options();
            options.use_ipv6 = false;
            options.use_http = http;
            options.use_tcp = !http;
            session.data.set_options(options);
            session.re_init_connection();
            let wake = handle.clone();
            host.set_alarm(Some(Rc::new(ThreadAlarm::new(clock, move || wake.wake()))));
            instance.attach_session(spawn.id, handle);
            tasks.push(tokio::task::spawn_local(
                session.run(NetworkTransport::default()),
            ));
        }
        tg_runtime::process_main_queue();
        instance.process(tg_runtime::now());
        let len = results.lock().unwrap().len();
        if len > printed {
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
        if started.elapsed().as_secs() > 60 {
            eprintln!(
                "{} TIMEOUT state={} transport={}",
                if http { "HTTP" } else { "TCP" },
                instance.dcstate(0),
                instance.dctransport(0)
            );
            break;
        }
        tokio::task::yield_now().await;
    }
    for task in tasks {
        task.abort();
    }
}
