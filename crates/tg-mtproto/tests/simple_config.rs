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
