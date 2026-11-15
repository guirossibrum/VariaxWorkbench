//! Deterministic malformed-input coverage; no transport or external resources.
use variax_workbench::{bundle::BundleFile, midi::SysexDecoder, preset::PresetFile};

#[test]
fn mutated_and_arbitrary_files_do_not_panic() {
    let fixture = include_bytes!("fixtures/editor-preset.vwm");
    let mut state = 0x600_u64;
    for index in 0..4096 {
        let mut bytes = if index % 2 == 0 {
            fixture.to_vec()
        } else {
            vec![0; index % 1024]
        };
        for _ in 0..8 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if !bytes.is_empty() {
                let offset = state as usize % bytes.len();
                bytes[offset] = (state >> 32) as u8;
            }
        }
        let result = std::panic::catch_unwind(|| {
            let _ = PresetFile::parse(bytes.clone());
            let _ = BundleFile::parse(bytes.clone());
        });
        assert!(result.is_ok(), "parser panicked on mutation {index}");
    }
}

#[test]
fn oversized_sysex_is_rejected_and_framing_recovers() {
    let mut decoder = SysexDecoder::default();
    let mut oversized = vec![0; 8193];
    oversized[0] = 0xf0;
    assert!(decoder.feed(&oversized).is_err());
    assert_eq!(
        decoder.feed(&[0xf0, 1, 0xf7]).unwrap(),
        [vec![0xf0, 1, 0xf7]]
    );
}
