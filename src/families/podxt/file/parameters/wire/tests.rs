use super::*;

fn fixture() -> [u8; PODXT_PAYLOAD_BYTES] {
    let mut payload = [0; PODXT_PAYLOAD_BYTES];
    payload[..32].fill(b' ');
    payload[..14].copy_from_slice(b"Locker fixture");
    payload[32 + 13] = 64;
    payload[32 + 57] = 127;
    payload[32 + 65] = 64;
    payload[32 + 89] = 9;
    payload[32 + 90] = 48;
    payload[32 + 120] = 10;
    payload
}

#[test]
fn setup_and_tone_payloads_preserve_their_kind_and_linked_fields() {
    for (_, kind, _) in PODXT_BANKS {
        let payload = fixture();
        let tone = PodTone::from_podxt_payload(&payload, kind).unwrap();
        assert_eq!(tone.container(), kind);
        assert_eq!(tone.name(), "Locker fixture");
        let output = tone.to_podxt_payload().unwrap();
        let mut expected = payload;
        expected[32 + 7] = 127; // Line 6 Edit always writes pedal level at full.
        assert_eq!(output, expected);
        assert_eq!(pod_variax_link(&output).label, "F-T-Model - 1");
    }
}

#[test]
fn backup_bundle_has_all_device_slots_in_line6_order() {
    let banks: Vec<_> = PODXT_BANKS
        .into_iter()
        .map(|(name, kind, count)| {
            let tone = PodTone::from_podxt_payload(&fixture(), kind).unwrap();
            (name, vec![tone; count])
        })
        .collect();
    let bundle = pod_bundle_from_tones(true, &banks).unwrap();
    assert_eq!(bundle.entry_count(), 299);
    assert_eq!(&bundle.as_bytes()[..4], b"FORM");
    assert_eq!(
        &bundle.as_bytes()[8..24],
        b"L6BAHEAD\x00\x00\x00\x04\x00\x00\x00\x01"
    );
    for (index, (name, kind, count)) in PODXT_BANKS.into_iter().enumerate() {
        assert_eq!(bundle.bank_name(index), Some(name));
        assert_eq!(bundle.preset_count(index), Some(count));
        assert_eq!(bundle.tone(index, count - 1).unwrap().container(), kind);
    }
    assert!(pod_bundle_from_tones(true, &banks[..2]).is_err());
    if let Some(path) = std::env::var_os("PODXT_LOCKER_ORACLE_INPUT") {
        let vendor = PodBundleLibrary::parse(std::fs::read(path).unwrap()).unwrap();
        assert_eq!(vendor.bank_count(), 3);
        for (index, (name, kind, _)) in PODXT_BANKS.into_iter().enumerate() {
            assert_eq!(vendor.bank_name(index), Some(name));
            let tone = vendor.tone(index, 0).unwrap();
            assert_eq!(tone.container(), kind);
            assert_eq!(tone.name(), "Locker fixture");
            assert_eq!(
                tone.to_podxt_payload().unwrap(),
                banks[index].1[0].to_podxt_payload().unwrap()
            );
        }
    }
    if let Some(path) = std::env::var_os("PODXT_LOCKER_ORACLE_OUTPUT") {
        bundle.save_copy(Path::new(&path)).unwrap();
    }
}

#[test]
fn new_documents_have_local_defaults_and_html_is_escaped() {
    let collection = new_podxt_bundle(false).unwrap();
    assert_eq!(collection.entry_count(), 1);
    assert!(collection.membership_editable(0));
    let tone = collection.tone(0, 0).unwrap().to_podxt_payload().unwrap();
    assert_eq!(tone[32 + 101], 127);
    assert_eq!(
        (u16::from(tone[32 + 89]) << 7) | u16::from(tone[32 + 90]),
        1200
    );
    assert_eq!(new_podxt_bundle(true).unwrap().entry_count(), 299);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tones.html");
    save_podxt_html(&path, [("1A", "<Tone & Co.>")]).unwrap();
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains("&lt;Tone &amp; Co.&gt;"));
    assert!(save_podxt_html(&path, [("1A", "Other")]).is_err());
}
