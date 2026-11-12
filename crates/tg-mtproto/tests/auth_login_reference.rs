//! Original lib_tl + generated C++ constructors, fixed values from diagnostic attempt 2.
use tg_scheme::*;
#[test]
fn native_send_code_sign_in_and_init_wrapper() {
    let words = |value: &dyn tg_tl::Serialize| {
        let mut words = Vec::new();
        value.write(&mut words);
        words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let send = AuthSendCode::new(
        b"9996629716".to_vec(),
        17349,
        b"344583e45741c457fe1862106095a5eb".to_vec(),
        mtp_code_settings(Default::default(), None, None, None),
    );
    let signin = AuthSignIn::new(
        AuthSignIn::F_PHONE_CODE,
        b"9996629716".to_vec(),
        b"a6742dda4e70e95b74".to_vec(),
        Some(b"22222".to_vec()),
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
    let mut input = include_bytes!("fixtures/auth-login/original.bin").as_slice();
    for actual in [words(&send), words(&signin), words(&init)] {
        let size = u32::from_le_bytes(input[..4].try_into().unwrap()) as usize;
        assert_eq!(actual, &input[4..4 + size]);
        input = &input[4 + size..];
    }
    assert!(input.is_empty());
}
