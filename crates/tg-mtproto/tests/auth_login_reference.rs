//! Original lib_tl + generated C++ constructors for every diagnostic request pair.
use tg_scheme::*;
#[test]
fn native_send_code_sign_in_and_init_wrapper() {
    let cases = include_str!("fixtures/auth-login/cases.tsv")
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let fixture = include_bytes!("fixtures/auth-login/cases.bin");
    assert_eq!(cases.len(), 7);
    for c in cases {
        let bytes = |key: usize| c[key].as_bytes().to_vec();
        let send = AuthSendCode::new(
            bytes(1),
            17349,
            b"344583e45741c457fe1862106095a5eb".to_vec(),
            mtp_code_settings(Default::default(), None, None, None),
        );
        let signin = AuthSignIn::new(
            AuthSignIn::F_PHONE_CODE,
            bytes(2),
            bytes(3),
            Some(bytes(4)),
            None,
        );
        let init = InvokeWithLayer::new(
            229,
            InitConnection::new(
                tg_tl::Flags::from_raw(2),
                17349,
                b"Desktop".to_vec(),
                b"Linux".to_vec(),
                b"7.2.9".to_vec(),
                b"en".to_vec(),
                b"tdesktop".to_vec(),
                b"en".to_vec(),
                None,
                Some(mtp_json_object(vec![mtp_json_object_value(
                    b"tz_offset".to_vec(),
                    mtp_json_number(0.0),
                )])),
                send.clone(),
            ),
        );
        let mut input =
            &fixture[c[5].parse::<usize>().unwrap()..][..c[6].parse::<usize>().unwrap()];
        for actual in [
            tg_tl::serialize(&send),
            tg_tl::serialize(&signin),
            tg_tl::serialize(&init),
        ] {
            let actual = actual
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>();
            let size = u32::from_le_bytes(input[..4].try_into().unwrap()) as usize;
            assert_eq!(actual, &input[4..4 + size], "attempt {}", c[0]);
            input = &input[4 + size..];
        }
        assert!(input.is_empty());
        if !c[7].is_empty() {
            let prefix: Vec<i32> = c[7].split(',').map(|w| w.parse().unwrap()).collect();
            let mut logged = prefix;
            logged.extend(tg_tl::serialize(&send));
            assert_eq!(
                logged,
                tg_tl::serialize(&init),
                "actual session prefix attempt {}",
                c[0]
            );
        }
    }
}
