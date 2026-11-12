use tg_mtproto::details::mtproto_dump_to_text::{dump_to_text_type_from, Core};
use tg_scheme::dump_to_text::DumpCore;
#[test]
fn primitives_vectors_and_layers() {
    for (cons, words, expected) in [
        (tg_tl::ID_INT, vec![-7], "-7 [INT]"),
        (
            tg_tl::ID_LONG,
            tg_tl::serialize(&u64::MAX),
            "18446744073709551615 [LONG]",
        ),
        (
            tg_tl::ID_STRING,
            tg_tl::serialize(&b"a\n\"\\".to_vec()),
            "\"a\\n\\\"\\\\\" [STRING]",
        ),
        (
            tg_tl::ID_DOUBLE,
            tg_tl::serialize(&1.25_f64),
            "1.25 [DOUBLE]",
        ),
    ] {
        let (text, ok) = dump_to_text_type_from(&words, cons);
        assert!(ok);
        assert_eq!(text, expected);
    }
    let mut to = String::new();
    assert!(Core.dump_core(
        &mut to,
        &mut tg_tl::Reader::new(&[2, 1, -2]),
        tg_tl::ID_VECTOR,
        0,
        tg_tl::ID_INT
    ));
    assert_eq!(to, "[ vector<0xa8509bda> (2)\n  1 [INT],\n  -2 [INT],\n]");
    let layer = vec![
        tg_mtproto::core_types::LAYERS[1] as i32,
        tg_tl::ID_BOOL_TRUE as i32,
    ];
    let (text, ok) = dump_to_text_type_from(&layer, 0);
    assert!(ok);
    assert!(text.starts_with("[LAYER2] "));
    let layer = vec![0xda9b0d0d_u32 as i32, 229, tg_tl::ID_BOOL_TRUE as i32];
    assert!(dump_to_text_type_from(&layer, 0)
        .0
        .contains("invokeWithLayer"));
    let mut text = String::new();
    assert!(Core.dump_core(
        &mut text,
        &mut tg_tl::Reader::new(&layer[1..]),
        0xda9b0d0d,
        0,
        0
    ));
    assert!(text.starts_with("[LAYER229] "));
}
#[test]
fn gzip_reference_stream_crc_and_limits() {
    let packed = include_bytes!("fixtures/true.gz");
    let expected = tg_tl::primes_to_bytes(&tg_tl::serialize(&true));
    assert_eq!(
        tg_base::zlib_help::inflate_gzip(packed, 4).unwrap(),
        expected
    );
    assert!(tg_base::zlib_help::inflate_gzip(packed, 3).is_err());
    assert!(tg_base::zlib_help::inflate_gzip(&packed[..packed.len() - 2], 100).is_err());
    let mut damaged = packed.to_vec();
    let n = damaged.len();
    damaged[n - 8] ^= 1;
    assert!(tg_base::zlib_help::inflate_gzip(&damaged, 100).is_err());
    let words = tg_tl::serialize(&packed.to_vec());
    let (text, ok) = dump_to_text_type_from(&words, tg_mtproto::core_types::GZIP_PACKED);
    assert!(ok);
    assert!(text.starts_with("[GZIPPED] "));
}
