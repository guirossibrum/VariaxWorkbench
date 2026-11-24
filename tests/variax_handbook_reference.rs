#![cfg(feature = "usb")]

use line6_devices::family::variax::{BANKS, Preset};

fn preset(bank: &str, position: &str) -> Preset {
    let bank = BANKS.iter().position(|&name| name == bank).unwrap() as u8;
    let slot = position.parse::<u8>().unwrap().checked_sub(1).unwrap();
    Preset::new(bank, slot).unwrap()
}

#[test]
fn handbook_custom_assignments_map_to_workbench_programs() {
    let mut lines = include_str!("../research/variax-300-handbook-custom.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'));
    assert_eq!(
        lines.next(),
        Some("custom_bank\tcustom_position\tsource_bank\tsource_position")
    );

    let programs: Vec<_> = lines
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let [bank, position, source_bank, source_position] = fields.as_slice() else {
                panic!("expected a destination and source in each row: {line}");
            };
            (
                preset(bank, position).program_number(),
                preset(source_bank, source_position).program_number(),
            )
        })
        .collect();

    // Independently state the handbook assignments in Workbench MIDI numbering.
    assert_eq!(
        programs,
        [
            (4, 100),
            (3, 106),
            (2, 101),
            (1, 108),
            (0, 109),
            (59, 74),
            (58, 83),
            (57, 87),
            (56, 66),
            (55, 91),
        ]
    );
}
