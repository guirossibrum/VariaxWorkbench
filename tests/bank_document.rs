use variax_workbench::{bundle::BundleFile, document::EditorDocument, preset::PresetFile};

fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        out.push(0xa7);
    }
    out
}

fn bundle(fixed: bool, replacement: Option<&[u8]>) -> Vec<u8> {
    let mut data = if fixed {
        b"L6BA".to_vec()
    } else {
        b"L6CO".to_vec()
    };
    data.extend(chunk(b"HEAD", &1u32.to_be_bytes()));
    data.extend(chunk(b"JUNK", b"root unknown"));
    for (i, count) in [0, 2, 3].into_iter().enumerate() {
        let mut bank = if fixed {
            b"BANK".to_vec()
        } else {
            b"FLDR".to_vec()
        };
        let mut info = vec![0; 68];
        for (n, c) in format!("Bank {i}").encode_utf16().enumerate() {
            info[4 + n * 2..6 + n * 2].copy_from_slice(&c.to_be_bytes());
        }
        bank.extend(chunk(b"BINF", &info));
        bank.extend(chunk(b"JUNK", b"bank unknown!"));
        for j in 0..count {
            let mut preset =
                PresetFile::parse(include_bytes!("fixtures/synthetic-host-preset.vwm").to_vec())
                    .unwrap();
            preset.rename(&format!("Preset {i}:{j}")).unwrap();
            bank.extend_from_slice(if i == 2 && j == 1 {
                replacement.unwrap_or(preset.as_bytes())
            } else {
                preset.as_bytes()
            });
        }
        data.extend(chunk(b"LIST", &bank));
    }
    chunk(b"FORM", &data)
}

#[test]
fn bank_browsing_edits_history_and_save_copy_preserve_the_complete_container() {
    for fixed in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("source");
        let copy = directory.path().join("copy");
        let bytes = bundle(fixed, None);
        std::fs::write(&original, &bytes).unwrap();
        let mut doc = EditorDocument::open(&original).unwrap();
        assert_eq!(doc.selection(), Some((1, 0)));
        assert_eq!(doc.extension(), if fixed { "vwb" } else { "l6c" });
        assert_eq!(doc.banks().unwrap().len(), 3);
        doc.select(2, 1).unwrap();
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
        assert!(doc.select(0, 0).is_err());
        assert_eq!(doc.selection(), Some((2, 1)));
        doc.apply(|p| p.rename("Replacement")).unwrap();
        let renamed = doc.preset().clone();
        assert_eq!(doc.as_bytes(), bundle(fixed, Some(renamed.as_bytes())));
        doc.select(1, 1).unwrap();
        doc.apply(|p| p.rename("Other edit")).unwrap();
        doc.undo();
        assert_eq!(doc.as_bytes(), bundle(fixed, Some(renamed.as_bytes())));
        doc.redo();
        assert_eq!(doc.preset().name(), "Other edit");
        doc.select(2, 1).unwrap();
        assert_eq!(doc.preset().name(), "Replacement");
        doc.begin_gesture();
        for value in [3f32, 4., 5.] {
            doc.apply(|p| p.set_parameter(0x02010001, 0x200000, 1, value.to_bits()))
                .unwrap();
        }
        doc.end_gesture();
        doc.undo();
        assert_eq!(doc.preset().as_bytes(), renamed.as_bytes());
        let before = doc.as_bytes().to_vec();
        assert!(
            doc.apply(|p| {
                p.rename("Partial")?;
                p.set_parameter(0x02010001, 0x200000, 1, 100f32.to_bits())
            })
            .is_err()
        );
        assert_eq!(doc.as_bytes(), before);
        doc.save_copy(&copy).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(std::fs::read(&original).unwrap(), bytes);
        assert_eq!(std::fs::read(&copy).unwrap(), before);
        doc.apply(|p| p.rename("Unsaved")).unwrap();
        assert!(doc.save_copy(&copy).is_err());
        assert!(doc.is_dirty());
        assert_eq!(std::fs::read(&copy).unwrap(), before);
    }
}

#[test]
fn resized_replacement_preserves_unknown_chunks_padding_and_following_presets() {
    for fixed in [true, false] {
        let bytes = bundle(fixed, None);
        let mut file = BundleFile::parse(bytes).unwrap();
        let mut replacement = file.banks()[2].presets()[1].as_bytes().to_vec();
        replacement.extend(chunk(b"XTRA", b"odd private extension"));
        let length = (replacement.len() - 8) as u32;
        replacement[4..8].copy_from_slice(&length.to_be_bytes());
        let preset = PresetFile::parse(replacement.clone()).unwrap();
        file.replace(2, 1, &preset).unwrap();
        assert_eq!(file.as_bytes(), bundle(fixed, Some(&replacement)));
        assert_eq!(file.banks()[2].presets()[2].name(), "Preset 2:2");
        let before = file.as_bytes().to_vec();
        assert!(file.replace(9, 0, &preset).is_err());
        assert_eq!(file.as_bytes(), before);
    }
}

#[test]
fn unsupported_containers_and_empty_edit_documents_fail_without_modifying_sources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source");
    let mut little = bundle(true, None);
    little[..4].copy_from_slice(b"RIFF");
    let mut other = bundle(true, None);
    other[8..12].copy_from_slice(b"L6XX");
    for bytes in [little, other, chunk(b"FORM", b"L6BA")] {
        std::fs::write(&path, &bytes).unwrap();
        assert!(EditorDocument::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
