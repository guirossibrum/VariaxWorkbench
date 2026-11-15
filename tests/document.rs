use variax_workbench::document::EditorDocument;

#[test]
fn undo_groups_failed_edits_and_atomic_saves_preserve_original() {
    let temporary = tempfile::tempdir().unwrap();
    let original = temporary.path().join("original.vwm");
    let bytes = include_bytes!("fixtures/editor-preset.vwm");
    std::fs::write(&original, bytes).unwrap();
    let mut document = EditorDocument::open(&original).unwrap();
    assert!(!document.is_dirty());
    document.apply(|p| p.rename("First")).unwrap();
    let failed = document.apply(|p| {
        p.rename("Partial")?;
        Err("validation failed".into())
    });
    assert!(failed.is_err());
    assert_eq!(document.preset().name(), "First");
    document.undo();
    assert_eq!(document.preset().name(), "Synthetic");
    assert!(document.can_redo());
    document.begin_gesture();
    for name in ["Drag 1", "Drag 2", "Drag 3"] {
        document.apply(|p| p.rename(name)).unwrap();
    }
    document.end_gesture();
    document.undo();
    assert_eq!(document.preset().name(), "Synthetic");
    document.redo();
    assert_eq!(document.preset().name(), "Drag 3");
    let copy = temporary.path().join("copy.vwm");
    document.save_copy(&copy).unwrap();
    assert!(!document.is_dirty());
    assert_eq!(
        EditorDocument::open(&copy).unwrap().preset().name(),
        "Drag 3"
    );
    document.apply(|p| p.rename("Later")).unwrap();
    let saved = std::fs::read(&copy).unwrap();
    assert!(document.save_copy(&copy).is_err());
    assert_eq!(std::fs::read(&copy).unwrap(), saved);
    assert_eq!(std::fs::read(&original).unwrap(), bytes);
    document.undo();
    assert!(!document.is_dirty());
}
