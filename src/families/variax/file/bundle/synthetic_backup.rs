//! Synthetic Device check backup fixtures for library and headless editor tests.
use line6_devices::{
    backup::BackupFolder,
    family::variax::{Preset, backup::variax_preset_stem},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn make_backup(root: &Path, name: &str, count: usize, member: u8) -> PathBuf {
    make_backup_with_names(
        root,
        name,
        count,
        member,
        &[(0, "Renamed Custom"), (11, "")],
    )
}

pub(crate) fn make_backup_with_names(
    root: &Path,
    name: &str,
    count: usize,
    member: u8,
    names: &[(usize, &str)],
) -> PathBuf {
    make_backup_with_files(root, name, count, member, names, |_| {})
}

pub(crate) fn make_backup_with_files(
    root: &Path,
    name: &str,
    count: usize,
    member: u8,
    names: &[(usize, &str)],
    add_files: impl FnOnce(&mut BackupFolder),
) -> PathBuf {
    let mut folder = BackupFolder::create(root, name).unwrap();
    let mut identity = vec![0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 7, 0, member, 0];
    identity.extend_from_slice(b"0310");
    identity.push(0xf7);
    folder.file("identity.syx", &identity).unwrap();

    for slot in Preset::all().take(count) {
        let twin = slot.bank() == 0 && slot.slot() < 3;
        let name = if twin {
            "Twin".into()
        } else {
            format!("Preset {}", slot.program_number())
        };
        let mut raw = [0; 96];
        raw[..name.len()].copy_from_slice(name.as_bytes());
        raw[18..22].copy_from_slice(&0x0310_0003_u32.to_be_bytes());
        raw[22] = 1;
        raw[23..27].copy_from_slice(&0x0311_0004_u32.to_be_bytes());
        let tone = if twin && slot.slot() != 2 {
            0.25_f32
        } else {
            0.75_f32
        };
        raw[61..64].copy_from_slice(&tone.to_be_bytes()[..3]);
        raw[64] = u8::from(twin && slot.slot() != 0);
        let stem = variax_preset_stem(slot);
        folder.file(&format!("{stem}-1.syx"), &reply(raw)).unwrap();
        folder
            .file(&format!("{stem}-2.syx"), &reply([0; 96]))
            .unwrap();
    }

    let mut original = [0; 96];
    original[..13].copy_from_slice(b"Active buffer");
    folder.file("original-1.syx", &reply(original)).unwrap();
    folder.file("original-2.syx", &reply([0; 96])).unwrap();
    for &(bank, name) in names {
        let mut bank_reply = vec![0xf0, 0, 1, 12, 7, 4, 0x6d];
        bank_reply.extend_from_slice(name.as_bytes());
        bank_reply.resize(25, 0);
        bank_reply.push(0xf7);
        folder
            .file(&format!("bank-names/{bank:02}.syx"), &bank_reply)
            .unwrap();
    }
    add_files(&mut folder);
    folder.finish(count == Preset::all().count()).unwrap()
}

fn reply(raw: [u8; 96]) -> Vec<u8> {
    let mut bytes = vec![0xf0, 0, 1, 12, 7, 4, 0x3c];
    for word in raw.as_chunks::<3>().0 {
        let value = u32::from_be_bytes([0, word[0], word[1], word[2]]);
        bytes.extend((0..6).map(|n| ((value >> (4 * n)) & 0xf) as u8));
    }
    bytes.push(0xf7);
    bytes
}

pub(crate) fn snapshot(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(directory: &Path, tree: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                collect(&entry.path(), tree);
            } else {
                tree.insert(entry.path(), fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut tree = BTreeMap::new();
    collect(directory, &mut tree);
    tree
}
