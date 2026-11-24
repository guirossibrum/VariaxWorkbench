use line6_devices::family::podxt::{
    Control, EDIT_BUFFER_LEN, EditBuffer, Kind, LiveControl, PodxtRequest, Setting, Value,
    controls, models,
};
use line6_devices::session::Request;
use std::collections::{BTreeMap, BTreeSet};

const TABLE: &str = include_str!("../../../research/podxt-live-parameters.json");

// The table uses one object per line in these arrays. Read the fields used by
// the assertions so the device crate needs no new dependency. The Python
// oracle loads and validates the full JSON with a general-purpose parser.
fn rows(section: &str) -> impl Iterator<Item = &'static str> {
    let heading = format!("  \"{section}\": [\n");
    TABLE
        .split_once(&heading)
        .expect("missing table section")
        .1
        .lines()
        .take_while(|line| line.trim_start().starts_with('{'))
}

fn field<'a>(row: &'a str, key: &str) -> Option<&'a str> {
    row.split_once(&format!("\"{key}\": "))
        .map(|(_, value)| value)
}

fn string<'a>(row: &'a str, key: &str) -> &'a str {
    field(row, key)
        .unwrap_or_else(|| panic!("missing {key} in {row}"))
        .strip_prefix('"')
        .expect("string field")
        .split_once('"')
        .expect("end of string field")
        .0
}

fn number(row: &str, key: &str) -> u32 {
    field(row, key)
        .unwrap_or_else(|| panic!("missing {key} in {row}"))
        .split_once(',')
        .expect("next field")
        .0
        .parse()
        .expect("numeric field")
}

fn numbers(row: &str, key: &str) -> Vec<u8> {
    let Some(value) = field(row, key) else {
        return Vec::new();
    };
    let list = value
        .strip_prefix('[')
        .expect("array field")
        .split_once(']')
        .expect("end of array field")
        .0;
    list.split(", ")
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().expect("numeric array element"))
        .collect()
}

fn parameters() -> BTreeMap<&'static str, &'static str> {
    rows("parameters")
        .map(|row| (string(row, "key"), row))
        .collect()
}

fn check_address(row: &str, control: LiveControl) {
    let actual_live = [Some(control.cc()), control.low_cc()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(actual_live, numbers(row, "live_cc"), "{}", control.name());

    let offsets = numbers(row, "tone_offsets");
    assert_eq!(
        offsets,
        actual_live.iter().map(|cc| cc + 32).collect::<Vec<_>>(),
        "{}",
        control.name()
    );
    let mut frame = vec![0; EDIT_BUFFER_LEN];
    frame[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    frame[167] = 0xf7;
    frame[7 + usize::from(offsets[0])] = 77;
    let expected = if let [_, low] = offsets.as_slice() {
        frame[7 + usize::from(*low)] = 19;
        (77 << 7) | 19
    } else {
        77
    };
    let tone = EditBuffer::parse(frame).unwrap();
    assert_eq!(control.read(&tone), expected, "{}", control.name());
}

#[test]
fn all_basic_controls_match_the_table_addresses_and_encodings() {
    let table = parameters();
    let by_cc = table
        .values()
        .filter(|row| {
            matches!(
                field(row, "availability"),
                Some(value) if value.starts_with("\"Live\"") ||
                    value.starts_with("\"no Live model definition\"")
            )
        })
        .filter(|row| {
            matches!(
                string(row, "live_encoding"),
                "float7-nearest" | "switch" | "inverted-switch"
            )
        })
        .filter_map(|row| match numbers(row, "live_cc").as_slice() {
            [cc] => Some((*cc, *row)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut absent_model_definitions = BTreeSet::new();
    let mut seen = BTreeSet::new();

    for control in Control::ALL {
        let row = by_cc[&control.cc()];
        let cc = numbers(row, "live_cc")[0];
        assert!(seen.insert(cc));
        assert_eq!(
            PodxtRequest::Set(control, Value::new(77).unwrap()).bytes(),
            [0xb0, cc, 77]
        );
        assert!(controls::name(control.cc()).is_some());
        check_address(row, LiveControl::Basic(control));

        let expected = match control.kind() {
            Kind::Knob => "float7-nearest",
            Kind::Switch => "switch",
            Kind::InvertedSwitch => "inverted-switch",
        };
        assert_eq!(string(row, "live_encoding"), expected, "{}", control.name());

        if control.is_switch() {
            let choices = numbers(row, "wire_choices");
            let values = [control.switch_value(false), control.switch_value(true)];
            let expected = if control.kind() == Kind::InvertedSwitch {
                [choices[1], choices[0]]
            } else {
                [choices[0], choices[1]]
            };
            assert_eq!(values, expected, "{}", control.name());
        }
        if string(row, "availability") == "no Live model definition" {
            assert!(numbers(row, "applicable_model_ids").is_empty());
            absent_model_definitions.insert(control.name());
        }
    }
    assert_eq!(Control::ALL.len(), 55);
    assert_eq!(seen, by_cc.keys().copied().collect());
    assert_eq!(
        absent_model_definitions,
        BTreeSet::from(["delay-p5", "mod-p5"])
    );
}

#[test]
fn additional_settings_use_the_expected_tone_bytes_and_wire_values() {
    let table = parameters();
    use Setting::*;
    let rows = [
        (AmpModel, "AMP_MODEL"),
        (CabModel, "CAB_MODEL"),
        (StompModel, "STOMP_MODEL"),
        (ModModel, "MOD_MODEL"),
        (DelayModel, "DELAY_MODEL"),
        (ReverbModel, "REVERB_MODEL"),
        (WahModel, "WAH_MODEL"),
        (GateThreshold, "GATE_THRESH"),
        (Mic, "MIC_MODEL"),
        (ModNote, "MOD_NOTE"),
        (DelayNote, "DELAY_NOTE"),
        (TweakParam, "TWEAK_PARAM"),
        (PedalAssign, "PEDAL_ASSIGN"),
        (ModSpeed, "MOD_SPEED"),
        (DelayTime, "DELAY_TIME"),
        (Tempo, "TEMPO"),
    ];
    assert_eq!(rows.map(|(setting, _)| setting), Setting::ALL);

    for (setting, key) in rows {
        check_address(table[key], LiveControl::Additional(setting));
        if setting.model_list().is_some() {
            assert_eq!(string(table[key], "live_encoding"), "model-index");
        }
    }
    for setting in [Mic, ModNote, DelayNote] {
        let key = match setting {
            Mic => "MIC_MODEL",
            ModNote => "MOD_NOTE",
            DelayNote => "DELAY_NOTE",
            _ => unreachable!(),
        };
        let [minimum, maximum] = numbers(table[key], "wire_range").try_into().unwrap();
        for raw in 0..=127 {
            assert_eq!(
                setting.parse(&raw.to_string()).ok(),
                (minimum..=maximum).contains(&raw).then_some(u16::from(raw))
            );
        }
    }
    for (ordinal, raw) in numbers(table["PEDAL_ASSIGN"], "wire_choices")
        .into_iter()
        .enumerate()
    {
        assert_eq!(PedalAssign.parse(&ordinal.to_string()), Ok(u16::from(raw)));
    }
    for db in -96_i16..=0 {
        assert_eq!(GateThreshold.parse(&db.to_string()), Ok((-db) as u16));
    }
    for (setting, key) in [(ModSpeed, "MOD_SPEED"), (DelayTime, "DELAY_TIME")] {
        assert_eq!(string(table[key], "live_encoding"), "float14-nearest");
        assert_eq!(string(table[key], "tone_encoding"), "float14-truncate");
        for raw in 0..=16383 {
            assert_eq!(setting.parse(&raw.to_string()), Ok(raw));
        }
    }
    for tenths in 300..=2400 {
        let bpm = format!("{}.{}", tenths / 10, tenths % 10);
        assert_eq!(Tempo.parse(&bpm), Ok(tenths));
    }

    // The editor's choice space depends on its current FX layouts. The
    // probing API keeps the narrower handbook limit deliberately.
    let tweak = numbers(table["TWEAK_PARAM"], "editor_choices");
    let rejected = tweak
        .into_iter()
        .filter(|raw| TweakParam.parse(&raw.to_string()).is_err())
        .collect::<Vec<_>>();
    assert_eq!(rejected, (14..=20).collect::<Vec<_>>());
}

#[test]
fn every_model_choice_has_matching_rust_id_position_name_and_pack() {
    let names = rows("model_names")
        .map(|row| (number(row, "id"), string(row, "name")))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(names.len(), 246);
    assert_eq!(names, models::NAMES.into_iter().collect());
    let mut seen = BTreeSet::new();
    let mut families = BTreeMap::<&str, Vec<u32>>::new();
    let mut packs = BTreeMap::new();
    for row in rows("catalog") {
        let id = number(row, "id");
        let block = string(row, "block");
        let index = number(row, "index") as usize;
        let pack = number(row, "pack") as u8;
        let ids = families.entry(block).or_default();
        assert_eq!(index, ids.len());
        ids.push(id);
        assert!(seen.insert(id), "duplicate model {id:#x}");
        assert_eq!(models::name(id), names.get(&id).copied());
        assert_eq!(models::required_pack(id).unwrap_or(0), pack, "{id:#x}");
        if pack != 0 {
            packs.insert(id, pack);
        }
    }
    assert_eq!(seen, names.keys().copied().collect());
    assert_eq!(packs, models::PACKS.into_iter().collect());
    for (block, rust) in [
        ("amp", models::AMP_MODELS.as_slice()),
        ("cab", models::CAB_MODELS.as_slice()),
        ("stomp", models::STOMP_MODELS.as_slice()),
        ("mod", models::MOD_MODELS.as_slice()),
        ("delay", models::DELAY_MODELS.as_slice()),
        ("reverb", models::REVERB_MODELS.as_slice()),
        ("wah", models::WAH_MODELS.as_slice()),
    ] {
        assert_eq!(families.remove(block).unwrap(), rust, "{block}");
        let setting = Setting::from_name(&format!("{block}-model")).unwrap();
        for (index, id) in rust.iter().copied().enumerate() {
            assert_eq!(setting.model_index(id), Some(index as u16));
        }
    }
    assert!(families.is_empty());
}

#[test]
fn controller_registry_includes_shared_handlers_and_bass_aliases() {
    let mut table = BTreeSet::new();
    for row in rows("parameters") {
        table.extend(numbers(row, "live_cc"));
        table.extend(numbers(row, "live_cc_defaults_on"));
        for key in ["named_cc", "incoming_cc"] {
            if field(row, key).is_some() {
                table.insert(number(row, key) as u8);
            }
        }
    }
    let rust = controls::CONTROLS
        .iter()
        .map(|(cc, _)| *cc)
        .collect::<BTreeSet<_>>();
    assert!(table.is_subset(&rust));
    // Remaining constants are Bass PODxt-only CCs. Other bass controls
    // reuse numbers already assigned in the guitar product branch.
    assert_eq!(
        rust.difference(&table).copied().collect::<Vec<_>>(),
        [32, 68, 115, 118]
    );
}
