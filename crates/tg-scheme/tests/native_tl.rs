//! Typed builders from an independent raw-TL recipe; native bytes are captured
//! by the unchanged C++ writer, not synthesized by the Rust schema parser.
#[path = "fixtures/native_tl/builders.rs"]
mod builders;
#[test]
fn every_definition_matches_native_cpp() {
    let fixture = include_bytes!("fixtures/native_tl/reference.bin");
    let mut position = 0;
    for definition in 0..2520 {
        for mode in 0..6 {
            let length =
                u32::from_le_bytes(fixture[position..position + 4].try_into().unwrap()) as usize;
            position += 4;
            let expected = &fixture[position..position + length];
            position += length;
            let words = builders::sample(definition, mode);
            assert_eq!(
                tg_scheme::tl::primes_to_bytes(&words),
                expected,
                "definition {definition} mode {mode}"
            );
            assert_eq!(
                builders::reread(definition, &tg_scheme::tl::bytes_to_primes(expected)).unwrap(),
                words,
                "read definition {definition} mode {mode}"
            );
        }
    }
    assert_eq!(position, fixture.len());
}

#[test]
fn builtin_vector_matches_native_cpp() {
    let mut bytes = include_bytes!("fixtures/native_tl/vectors.bin").as_slice();
    for count in [0, 1, 2] {
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let expected = &bytes[4..4 + size];
        bytes = &bytes[4 + size..];
        let value = vec![-1234567_i32; count];
        assert_eq!(
            tg_scheme::tl::primes_to_bytes(&tg_scheme::tl::serialize(&value)),
            expected
        );
        let words = tg_scheme::tl::bytes_to_primes(expected);
        assert_eq!(
            tg_scheme::tl::deserialize_exact::<Vec<i32>>(&words).unwrap(),
            value
        );
    }
    assert!(bytes.is_empty());
}
