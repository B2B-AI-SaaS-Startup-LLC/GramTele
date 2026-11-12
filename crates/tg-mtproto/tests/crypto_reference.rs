use tg_mtproto::{details::mtproto_rsa_public_key::RsaPublicKey, mtproto_auth_key::AuthKey};
#[test]
fn original_auth_key_schedules_key_id_and_rsa() {
    let fixture = include_bytes!("fixtures/crypto.bin");
    assert!(fixture.len() > 528);
    let data = std::array::from_fn(|i| i as u8);
    let key = AuthKey::local(data);
    assert_eq!(key.key_id().to_le_bytes(), fixture[..8]);
    let msg = std::array::from_fn(|i| 0xf0 + i as u8);
    let mut position = 8;
    for old in [false, true] {
        for send in [true, false] {
            let (k, iv) = if old {
                key.prepare_aes_oldmtp(&msg, send)
            } else {
                key.prepare_aes(&msg, send)
            };
            assert_eq!(k, fixture[position..position + 32]);
            assert_eq!(iv, fixture[position + 32..position + 64]);
            position += 64;
        }
    }
    let rsa = RsaPublicKey::from_components(&[0xff; 256], &[1, 0, 1]).unwrap();
    assert_eq!(
        rsa.fingerprint().to_le_bytes(),
        fixture[position..position + 8]
    );
    position += 8;
    assert_eq!(rsa.encrypt(&data).unwrap(), fixture[position..528]);
    assert!(rsa.encrypt(&[0xff; 256]).is_none());
    assert!(rsa.encrypt(&[1; 255]).is_none());
}
#[test]
fn original_dc_options_and_config_serialization() {
    use tg_mtproto::{
        mtproto_config::Config,
        mtproto_dc_options::{DcOptions, Environment},
    };
    let mut bytes = &include_bytes!("fixtures/crypto.bin")[528..];
    fn record<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let result = &bytes[4..4 + size];
        *bytes = &bytes[4 + size..];
        result
    }
    for environment in [Environment::Production, Environment::Test] {
        let expected = record(&mut bytes);
        let options = DcOptions::new(environment);
        assert_eq!(options.serialize(), expected);
        let mut parsed = DcOptions::new(environment);
        assert!(parsed.construct_from_serialized(expected));
        assert_eq!(parsed.serialize(), expected);
        let expected = record(&mut bytes);
        let config = Config::new(environment);
        assert_eq!(config.serialize(), expected);
        assert_eq!(
            Config::from_serialized(expected).unwrap().serialize(),
            expected
        );
    }
    assert!(bytes.is_empty());
}

#[test]
fn original_config_every_field_nondefault_and_utf16() {
    use tg_mtproto::{mtproto_config::Config, mtproto_dc_options::Environment};
    let mut bytes = include_bytes!("fixtures/storage-custom.bin").as_slice();
    for environment in [Environment::Production, Environment::Test] {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let expected = &bytes[4..4 + size];
        bytes = &bytes[4 + size..];
        let mut config = Config::new(environment);
        config.fields.chat_size_max = 101;
        config.fields.megagroup_size_max = 102;
        config.fields.forwarded_count_max = 103;
        config.fields.online_update_period = 104;
        config.fields.offline_blur_timeout = 105;
        config.fields.offline_idle_timeout = 106;
        config.fields.online_focus_timeout = 107;
        config.fields.online_cloud_timeout = 108;
        config.fields.notify_cloud_delay = 109;
        config.fields.notify_default_delay = 110;
        config.fields.edit_time_limit = 111;
        config.fields.revoke_time_limit = 112;
        config.fields.revoke_private_time_limit = 113;
        config.fields.revoke_private_inbox = true;
        config.fields.stickers_recent_limit = 115;
        config.fields.internal_links_domain = "internalLinksDomain_😀".into();
        config.fields.channels_read_media_period = 117;
        config.fields.call_receive_timeout_ms = 118;
        config.fields.call_ring_timeout_ms = 119;
        config.fields.call_connect_timeout_ms = 120;
        config.fields.call_packet_timeout_ms = 121;
        config.fields.web_file_dc_id = 122;
        config.fields.txt_domain_string = "txtDomainString_😀".into();
        config.fields.blocked_mode = true;
        config.fields.caption_length_max = 125;
        config.fields.rating_decay = 126;
        config.fields.reaction_default_emoji = "reactionDefaultEmoji_😀".into();
        config.fields.reaction_default_custom = 0xfedcba9876543210;
        config.fields.autologin_token = "autologinToken_😀".into();
        config.fields.gif_search_username = "gifSearchUsername_😀".into();
        config.fields.venue_search_username = "venueSearchUsername_😀".into();
        assert_eq!(config.serialize(), expected);
        assert_eq!(
            Config::from_serialized(expected).unwrap().serialize(),
            expected
        );
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let expected = &bytes[4..4 + size];
        bytes = &bytes[4 + size..];
        let parsed = Config::from_serialized(expected).unwrap();
        assert_eq!(parsed.fields.string_nulls, 31);
        assert_eq!(parsed.serialize(), expected);
    }
    assert!(bytes.is_empty());
}
#[test]
fn original_aes_ige_encrypt_and_decrypt() {
    use tg_mtproto::mtproto_auth_key::{aes_ige_decrypt_raw, aes_ige_encrypt_raw};
    let key = std::array::from_fn(|i| i as u8);
    let iv = std::array::from_fn(|i| i as u8 + 32);
    let plain: Vec<u8> = (0..64).collect();
    let expected = include_bytes!("fixtures/ige.bin");
    let mut encrypted = plain.clone();
    aes_ige_encrypt_raw(&mut encrypted, &key, &iv).unwrap();
    assert_eq!(encrypted, expected.as_slice());
    let mut decrypted = expected.to_vec();
    aes_ige_decrypt_raw(&mut decrypted, &key, &iv).unwrap();
    assert_eq!(decrypted, plain);
    assert!(aes_ige_encrypt_raw(&mut [0; 15], &key, &iv).is_err());
}
