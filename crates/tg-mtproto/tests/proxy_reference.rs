use tg_mtproto::{
    details::mtproto_tcp_socket::{
        http_connect_handshake, http_connect_request, socks_connect_request, socks_credentials,
        socks_greeting, socks_handshake,
    },
    mtproto_proxy_data::*,
};
fn records(bytes: &[u8]) -> Vec<&[u8]> {
    let mut records = Vec::new();
    let mut bytes = bytes;
    while !bytes.is_empty() {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        records.push(&bytes[4..4 + size]);
        bytes = &bytes[4 + size..];
    }
    records
}
#[test]
fn native_qt_socks5_and_connect() {
    let fixture = records(include_bytes!("fixtures/socks.bin"));
    assert_eq!(socks_greeting(false), fixture[0]);
    assert_eq!(
        socks_connect_request("example.com", 443).unwrap(),
        fixture[1]
    );
    assert_eq!(socks_greeting(true), fixture[2]);
    assert_eq!(socks_credentials("user", "pass").unwrap(), fixture[3]);
    assert_eq!(
        socks_connect_request("example.com", 443).unwrap(),
        fixture[4]
    );
    let fixture = records(include_bytes!("fixtures/connect.bin"));
    assert_eq!(http_connect_request("example.com", 443, "", ""), fixture[0]);
    assert_eq!(http_connect_request("example.com", 443, "", ""), fixture[1]);
    assert_eq!(
        http_connect_request("example.com", 443, "user", "pass"),
        fixture[2]
    );
}
#[tokio::test(flavor = "current_thread")]
async fn memory_socks5_auth_and_rejection() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut client, mut server) = tokio::io::duplex(64);
    let peer = async {
        let mut greeting = [0; 3];
        server.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 1, 2]);
        server.write_all(&[5, 2]).await.unwrap();
        let mut creds = [0; 11];
        server.read_exact(&mut creds).await.unwrap();
        assert_eq!(
            &creds,
            &[1, 4, b'u', b's', b'e', b'r', 4, b'p', b'a', b's', b's']
        );
        server.write_all(&[1, 0]).await.unwrap();
        let mut request = [0; 18];
        server.read_exact(&mut request).await.unwrap();
        assert_eq!(
            request.as_slice(),
            socks_connect_request("example.com", 443).unwrap()
        );
        server
            .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 1, 2])
            .await
            .unwrap();
    };
    let (call, ()) = tokio::join!(
        socks_handshake(&mut client, "example.com", 443, "user", "pass"),
        peer
    );
    call.unwrap();
    let (mut client, mut server) = tokio::io::duplex(64);
    server.write_all(&[5, 255]).await.unwrap();
    assert!(socks_handshake(&mut client, "example.com", 443, "", "")
        .await
        .is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn memory_connect_basic_challenge() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut client, mut server) = tokio::io::duplex(512);
    let peer = async {
        for auth in [false, true] {
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(server.read_u8().await.unwrap());
            }
            assert_eq!(
                bytes,
                http_connect_request(
                    "example.com",
                    443,
                    if auth { "user" } else { "" },
                    if auth { "pass" } else { "" }
                )
            );
            server.write_all(if auth{b"HTTP/1.1 200 Connected\r\n\r\n"}else{b"HTTP/1.1 407 Auth\r\nProxy-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\n\r\n"}).await.unwrap();
        }
    };
    let (call, ()) = tokio::join!(
        http_connect_handshake(&mut client, "example.com", 443, "user", "pass"),
        peer
    );
    call.unwrap();
}
#[test]
fn original_web_proxy_embedded_assertions() {
    let mut proxy = ProxyData {
        kind: Type::Web,
        host: "proxy.example.com".into(),
        port: 443,
        password: "000102030405060708090a0b0c0d0e0f".into(),
        ..Default::default()
    };
    assert_eq!(
        web_proxy_bridge_capability(&proxy),
        "MHLEY5PmW1GWqJkSrlmJpvJUiLhBH_QKy6yKg8a0JPk"
    );
    proxy.user = "dobry-cola-super-app".into();
    assert_eq!(
        web_proxy_bridge_capability(&proxy),
        "hHz99Xs93EN1j91G9gpNepXwGNNt5YdAFkEVk_LlqdQ"
    );
    proxy.password.insert_str(0, "dd");
    assert_eq!(
        web_proxy_bridge_capability(&proxy),
        "TGUkZaevsavLbHvlNWipnRoYxgzZ51ioWvbxgGT3wHo"
    );
    proxy.user.clear();
    assert_eq!(
        web_proxy_bridge_capability(&proxy),
        "IpJrt3e7sKtzPyoXy6w-Zj6GGEvsvclN66JzQEfPYLA"
    );
    for (host, expected) in [
        (" Proxy.Example.COM ", "proxy.example.com"),
        ("bücher.de", "xn--bcher-kva.de"),
        ("xn--strae-oqa.example", "xn--strae-oqa.example"),
    ] {
        assert_eq!(normalize_web_proxy_host(host), expected);
    }
    for host in [
        "localhost",
        "127.0.0.1",
        "127.1",
        "0x7f.1",
        "0177.0.0.1",
        "1.2.3",
        "site.example:443",
        "site..example",
    ] {
        assert!(normalize_web_proxy_host(host).is_empty(), "{host}");
    }
    assert_eq!(
        normalize_web_proxy_base_path(" /two/segments/ "),
        "two/segments"
    );
    for path in [
        "-lead",
        "with space",
        "empty//segment",
        "trailing//",
        "a/-lead",
        "per%20cent",
        "dot.ted",
        "..",
    ] {
        assert!(normalize_web_proxy_base_path(path).is_empty());
    }
    let plain = "8561944064fc730cbfa4473562d8ec59";
    let marked = "cIVhlEBk_HMMv6RHNWLY7Fk";
    assert_eq!(decode_web_proxy_link_secret(marked, true), plain);
    assert_eq!(decode_web_proxy_link_secret(marked, false), plain);
    assert_eq!(decode_web_proxy_link_secret(plain, false), plain);
    assert!(decode_web_proxy_link_secret(plain, true).is_empty());
}
#[test]
fn proxy_links_secrets_and_serialized_records() {
    for secret in [
        "000102030405060708090a0b0c0d0e0f",
        "dd000102030405060708090a0b0c0d0e0f",
        "ee000102030405060708090a0b0c0d0e0f6578616d706c652e636f6d",
    ] {
        assert_eq!(mtproto_password_status(secret), Status::Valid);
        let proxy = ProxyData::from_link(&format!(
            "tg://proxy?server=localhost&port=443&secret={secret}"
        ))
        .unwrap();
        assert_eq!(proxy.secret_from_mtproto_password().len(), secret.len() / 2);
        assert!(!proxy.supports_calls());
        assert!(proxy.try_custom_resolve());
        let bytes = serialize_list(std::slice::from_ref(&proxy));
        assert_eq!(deserialize_list(&bytes).unwrap(), vec![proxy]);
    }
    let proxy = ProxyData::from_link("tg://socks?server=127.0.0.1&port=1080&user=a%2Bb&pass=x%26y")
        .unwrap();
    assert_eq!(proxy.user, "a+b");
    assert_eq!(proxy.password, "x&y");
    assert!(!proxy.try_custom_resolve());
    assert!(ProxyData::from_link("tg://proxy?server=x&port=1&secret=bad").is_none());
}
#[test]
fn native_proxy_records_preserve_null_qstrings() {
    let fixture = records(include_bytes!("fixtures/proxy.bin"));
    assert_eq!(serialize_data(&ProxyData::default()), fixture[0]);
    let mut proxy = ProxyData {
        kind: Type::Mtproto,
        host: "proxy.example.com".into(),
        port: 443,
        password: "000102030405060708090a0b0c0d0e0f".into(),
        ..Default::default()
    };
    assert_eq!(serialize_data(&proxy), fixture[1]);
    proxy.string_nulls &= !2;
    assert_eq!(serialize_data(&proxy), fixture[2]);
    for bytes in fixture {
        assert_eq!(serialize_data(&deserialize_data(bytes).unwrap()), bytes);
    }
}
