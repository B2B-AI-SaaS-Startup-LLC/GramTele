use tg_mtproto::{details::mtproto_rsa_public_key::RsaPublicKey, special_config_request::*};
use tg_tl::{Deserialize, Reader};
#[test]
fn original_simple_config_signed_with_test_key() {
    let mut bytes = include_bytes!("fixtures/simple-config.bin").as_slice();
    fn take<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let part = &bytes[4..4 + size];
        *bytes = &bytes[4 + size..];
        part
    }
    let pem = std::str::from_utf8(take(&mut bytes)).unwrap();
    let encoded = take(&mut bytes);
    let expected = take(&mut bytes);
    assert!(bytes.is_empty());
    let key = RsaPublicKey::from_pem(pem).unwrap();
    let config = decrypt_simple_config(encoded, &key).unwrap();
    assert_eq!(tg_tl::primes_to_bytes(&tg_tl::serialize(&config)), expected);
    let endpoints = handle_response(&config, "", 1700000000).unwrap();
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].ip, "149.154.167.167");
    assert!(handle_response(&config, "", 1700003601).is_err());
    let mut changed = encoded.to_vec();
    changed[50] = if changed[50] == b'A' { b'B' } else { b'A' };
    assert!(decrypt_simple_config(&changed, &key).is_err());
    let _ = tg_scheme::HelpConfigSimple::read(&mut Reader::new(&tg_tl::bytes_to_primes(expected)))
        .unwrap();
}
#[test]
fn phone_rules_json_and_txt() {
    assert!(check_phone_by_prefixes_rules("+7 (123)", "+7,-44"));
    assert!(!check_phone_by_prefixes_rules("7123", ",-7,+71"));
    assert!(!check_phone_by_prefixes_rules("44", "+7"));
    assert_eq!(
        concatenate_dns_txt_fields(&["AA".into(), "BB".into(), "C".into()]),
        b"BBAAC"
    );
    assert_eq!(
        parse_remote_config_response(br#"{"entries":{"ipconfigv3":"value"}}"#),
        b"value"
    );
    assert_eq!(
        parse_fire_store_response(br#"{"fields":{"data":{"stringValue":"value"}}}"#),
        b"value"
    );
    assert_eq!(parse_realtime_response(br#""abc""#), b"abc");
}
#[test]
fn original_date_shape_and_attempt_http_fields() {
    assert_eq!(
        parse_http_date("Wed, 10 Jul 2019 14:33:38 GMT"),
        Some(1562769218)
    );
    assert!(parse_http_date("Sun, 30 Feb 2020 12:00:00 GMT").is_none());
    assert!(parse_http_date("Sun, 29 Feb 2020 24:00:00 GMT").is_none());
    struct Random;
    impl tg_mtproto::core_types::Random for Random {
        fn fill(&mut self, data: &mut [u8]) {
            data.fill(27);
        }
    }
    for (service, path) in [
        (Service::Mozilla, "/dns-query?name=test&type=16"),
        (Service::Google, "/resolve?name=test&type=ANY"),
        (Service::RemoteConfig, "/v1/projects/peak-vista-421/"),
        (Service::Realtime, "/ipconfigv3.json"),
        (Service::FireStore, "/v1/projects/reserve-5a846/"),
    ] {
        let req = request_for_attempt(
            &Attempt {
                service,
                data: "firestore".into(),
                host: String::new(),
            },
            "test",
            "instance",
            &mut Random,
        );
        assert!(req.url.contains(path));
        assert!(req.headers.iter().any(|(key, _)| key == "User-Agent"));
    }
    assert_eq!(generate_instance_id(&mut Random).len(), 22);
    assert!(RsaPublicKey::from_pem(PUBLIC_KEY).is_some());
}
#[tokio::test(flavor = "current_thread")]
async fn fallback_https_runner_uses_native_signed_sample_and_virtual_stagger() {
    use std::{rc::Rc, sync::Arc};
    use tg_base::timer::{ManualClock, TimerHost};
    use tg_mtproto::connection_abstract::TransportError;
    struct Random;
    impl tg_mtproto::core_types::Random for Random {
        fn fill(&mut self, b: &mut [u8]) {
            b.fill(27);
        }
    }
    struct Fake {
        encoded: String,
        requests: std::cell::RefCell<Vec<WebRequest>>,
    }
    impl WebExecutor for Fake {
        async fn perform_request(&self, request: WebRequest) -> Result<WebReply, TransportError> {
            self.requests.borrow_mut().push(request);
            Ok(WebReply {
                headers: vec![("Date".into(), "Wed, 10 Jul 2019 14:33:38 GMT".into())],
                body: format!(
                    "{{\"Answer\":[{{\"type\":16,\"TTL\":60,\"data\":\"{}\"}}]}}",
                    self.encoded
                )
                .into_bytes(),
                error: false,
            })
        }
    }
    tokio::task::LocalSet::new()
        .run_until(async {
            let mut bytes = include_bytes!("fixtures/simple-config.bin").as_slice();
            let mut fields = vec![];
            while !bytes.is_empty() {
                let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
                fields.push(&bytes[4..n + 4]);
                bytes = &bytes[n + 4..];
            }
            let key = RsaPublicKey::from_pem(std::str::from_utf8(fields[0]).unwrap()).unwrap();
            let fake = Rc::new(Fake {
                encoded: String::from_utf8(fields[1].to_vec()).unwrap(),
                requests: Default::default(),
            });
            let clock = Arc::new(ManualClock::new(100));
            let host = TimerHost::new(clock.clone());
            let mut runner = SpecialConfigRunner::new(
                true,
                false,
                "tapv3.stel.com".into(),
                String::new(),
                &host,
                fake.clone(),
                &mut Random,
            )
            .with_key(key);
            assert!(runner.process(1700000000, &mut Random).is_empty());
            tokio::task::yield_now().await;
            let endpoints = runner.process(1700000000, &mut Random);
            assert_eq!(endpoints.len(), 2);
            assert_eq!(endpoints[0].as_ref().unwrap().dc_id, 2);
            assert!(endpoints[1].is_none());
            assert!(runner.time_done());
            assert_eq!(fake.requests.borrow().len(), 1);
            clock.advance(800);
            host.process();
            runner.process(1700000000, &mut Random);
            tokio::task::yield_now().await;
            assert_eq!(runner.process(1700000000, &mut Random).len(), 2);
            assert_eq!(fake.requests.borrow().len(), 2);
        })
        .await;
}
