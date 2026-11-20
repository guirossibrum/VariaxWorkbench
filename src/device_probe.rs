#![forbid(unsafe_code)]
//! Staged read-only device check.
//!
//! Each stage shows its intent and its risk before it runs. The risk rises
//! from USB identification to a second metadata read. The person changes the
//! named setting on the instrument. This module does not write a preset, a
//! setting, a bank, a reset, or firmware.
//!
//! `usb::read_current` is called only while `LIVE_ACCESS_LOCKED` is set, so the
//! call returns before a device opens. The named model and setting come from
//! `read_session::read_current_on` on the existing in-memory engine.

use crate::device_inventory::{Inventory, NameMatch, Role};
use crate::midi::Identity;
use crate::read_session::{self, ControlTransport, read_current_on};
use crate::rig::{CapabilityState, Operation, ProductProfile, resolve};
use crate::simulation::VirtualControl;
use crate::{ParameterValue, PresetMetadata};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const WATCHED_SETTING: &str = "string1_tuning";
const MODEL_FIELD: &str = "body_model";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageId {
    UsbRecognition,
    Identification,
    InventoryCheck,
    SettingsRead,
    SettingChange,
}

impl StageId {
    pub const LADDER: [Self; 5] = [
        Self::UsbRecognition,
        Self::Identification,
        Self::InventoryCheck,
        Self::SettingsRead,
        Self::SettingChange,
    ];

    pub const fn risk(self) -> u8 {
        match self {
            Self::UsbRecognition => 1,
            Self::Identification => 2,
            Self::InventoryCheck => 3,
            Self::SettingsRead => 4,
            Self::SettingChange => 5,
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::UsbRecognition => "USB identification",
            Self::Identification => "Device identification",
            Self::InventoryCheck => "Inventory check",
            Self::SettingsRead => "Settings read",
            Self::SettingChange => "Change check",
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::UsbRecognition => Some(Self::Identification),
            Self::Identification => Some(Self::InventoryCheck),
            Self::InventoryCheck => Some(Self::SettingsRead),
            Self::SettingsRead => Some(Self::SettingChange),
            Self::SettingChange => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailState {
    Locked,
    Current,
    Passed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disclaimer {
    pub stage: StageId,
    pub risk: u8,
    pub title: &'static str,
    pub intent: String,
    pub risk_text: &'static str,
}

pub fn disclaimer_for(stage: StageId, reading: Option<&Reading>) -> Disclaimer {
    let intent = match stage {
        StageId::UsbRecognition => USB_INTENT.to_string(),
        StageId::Identification => IDENTITY_INTENT.to_string(),
        StageId::InventoryCheck => INVENTORY_INTENT.to_string(),
        StageId::SettingsRead => SETTINGS_INTENT.to_string(),
        StageId::SettingChange => match reading {
            Some(reading) => format!(
                "The read names {guitar}, body model {body}, and {setting} = {value}. Change {setting} on the instrument. The app then reads again with the same read-only session and reports whether {setting} changed. {NO_WRITE}",
                guitar = reading.guitar,
                body = reading.body_model,
                setting = reading.setting,
                value = reading.value,
            ),
            None => format!(
                "Change {WATCHED_SETTING} on the instrument after a settings read has named it. The app then reads again. {NO_WRITE}"
            ),
        },
    };
    Disclaimer {
        stage,
        risk: stage.risk(),
        title: stage.title(),
        intent,
        risk_text: risk_text(stage),
    }
}

const NO_WRITE: &str =
    "This step does not send a preset, a setting change, a bank change, a reset, or firmware.";

const USB_INTENT: &str = "Read the USB vendor and product numbers supplied for this device and match them to the inventory. This step does not claim an interface and does not send a message to the instrument. This step does not send a preset, a setting change, a bank change, a reset, or firmware.";

const IDENTITY_INTENT: &str = "Parse the identity reply with the existing identity parser and record the product id. A live identity request is traffic to the instrument. This build calls the shared live function only while it is locked, so the call returns before a device is opened. This step does not send a preset, a setting change, a bank change, a reset, or firmware.";

const INVENTORY_INTENT: &str = "Match that product id to one inventory row. The USB bridge name is kept separate from the guitar name. An alias is not selected. This step does not send a preset, a setting change, a bank change, a reset, or firmware.";

const SETTINGS_INTENT: &str = "Read the current preset metadata with the existing read session. The report names the guitar, the body model, and string1_tuning. The live form of this read sends the identity request, MIDI enable, two metadata reads, and MIDI disable. This build calls the shared live function only while it is locked, so the guitar is not opened. The named model and setting come from the existing in-memory engine. This step does not send a preset, a setting change, a bank change, a reset, or firmware.";

const fn risk_text(stage: StageId) -> &'static str {
    match stage {
        StageId::UsbRecognition => {
            "Low risk. This is the first step. The result can name the wrong device when more than one Line 6 device is connected. It does not read guitar settings."
        }
        StageId::Identification => {
            "Higher risk than USB identification. A live identity request is traffic to the instrument. It can change what the interface is doing. It does not store a preset or a setting."
        }
        StageId::InventoryCheck => {
            "Higher risk than identification. A name match can be mistaken for permission to read settings. Membership is not a working protocol and it is not a settings read."
        }
        StageId::SettingsRead => {
            "High risk. This is higher than the inventory check. The read-session messages go to the instrument and can change session state. The metadata is a partial current reading, not a backup."
        }
        StageId::SettingChange => {
            "Highest risk. This is higher than the settings read. You change the named setting on the instrument, then the app reads again. The second read happens while the instrument is in your hands. A changed value means the second reading differed. An equal value means the reading stayed the same. The step is complete when the second read finishes."
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub guitar: String,
    pub product_id: u32,
    pub setting: &'static str,
    pub value: String,
    pub body_model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub usb_vid: u16,
    pub usb_pid: u16,
    pub identity_product: u32,
    pub before: [u8; 192],
    pub after: [u8; 192],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pass {
    stage: StageId,
    decision: &'static str,
    engine: &'static str,
    engine_result: &'static str,
    opened_device: &'static str,
    message_sent: &'static str,
    synthetic_send: &'static str,
    detail: String,
}

pub struct Session {
    inventory: Inventory,
    pending: Option<StageId>,
    passes: Vec<Pass>,
    bridge: Option<String>,
    product_id: Option<u32>,
    guitar: Option<String>,
    profile: Option<ProductProfile>,
    reading: Option<Reading>,
}

impl Session {
    pub fn start() -> Result<Self, String> {
        Ok(Self {
            inventory: Inventory::parse(crate::device_inventory::COMMITTED_TABLE)?,
            pending: Some(StageId::UsbRecognition),
            passes: Vec::new(),
            bridge: None,
            product_id: None,
            guitar: None,
            profile: None,
            reading: None,
        })
    }

    pub fn pending(&self) -> Option<StageId> {
        self.pending
    }

    pub fn disclaimer(&self) -> Option<Disclaimer> {
        self.pending
            .map(|stage| disclaimer_for(stage, self.reading.as_ref()))
    }

    pub fn rail(&self) -> [RailState; 5] {
        let mut states = [RailState::Locked; 5];
        if let Some(pending) = self.pending {
            for (index, stage) in StageId::LADDER.iter().enumerate() {
                if *stage == pending {
                    states[index] = RailState::Current;
                    return states;
                }
                states[index] = RailState::Passed;
            }
        }
        for (index, stage) in StageId::LADDER.iter().enumerate() {
            if stage_passed(*stage, &self.passes) {
                states[index] = RailState::Passed;
            } else if self.passes.iter().any(|pass| pass.stage == *stage) {
                states[index] = RailState::Stopped;
                return states;
            } else {
                return states;
            }
        }
        states
    }

    pub fn decline(&mut self) {
        let Some(stage) = self.pending else {
            return;
        };
        self.passes.push(Pass {
            stage,
            decision: "no",
            engine: "none",
            engine_result: "declined",
            opened_device: "no",
            message_sent: "no",
            synthetic_send: "no",
            detail: "stop=declined".to_string(),
        });
        self.pending = None;
    }

    pub fn proceed(&mut self, scene: &Scene) {
        let Some(stage) = self.pending else {
            return;
        };
        let green = match stage {
            StageId::UsbRecognition => self.usb_stage(scene),
            StageId::Identification => self.identity_stage(scene),
            StageId::InventoryCheck => self.inventory_stage(),
            StageId::SettingsRead => self.settings_stage(&scene.before),
            StageId::SettingChange => self.change_stage(&scene.after),
        };
        self.pending = if green { stage.next() } else { None };
    }

    pub fn report(&self) -> String {
        let mut lines = vec![
            "probe=device-check".to_string(),
            "schema=1".to_string(),
            "writes=no".to_string(),
        ];
        for pass in &self.passes {
            lines.push(format!(
                "stage={} risk={} decision={} engine={} engine_result={} opened_device={} message_sent={} synthetic_send={} {}",
                stage_token(pass.stage),
                pass.stage.risk(),
                pass.decision,
                pass.engine,
                pass.engine_result,
                pass.opened_device,
                pass.message_sent,
                pass.synthetic_send,
                pass.detail,
            ));
        }
        lines.join("\n")
    }

    fn usb_stage(&mut self, scene: &Scene) -> bool {
        let (engine, engine_result) = live_engine_result();
        let live_ok = live_gate_ok(engine_result);
        self.passes.push(Pass {
            stage: StageId::UsbRecognition,
            decision: "yes",
            engine,
            engine_result,
            opened_device: "no",
            message_sent: "no",
            synthetic_send: "no",
            detail: "host_to_device=none".to_string(),
        });
        if !live_ok {
            return false;
        }
        match self.inventory.find_usb(scene.usb_vid, scene.usb_pid) {
            NameMatch::One(row) => {
                self.bridge = Some(row.name.clone());
                self.passes.push(Pass {
                    stage: StageId::UsbRecognition,
                    decision: "yes",
                    engine: "device_inventory::find_usb",
                    engine_result: "one_match",
                    opened_device: "no",
                    message_sent: "no",
                    synthetic_send: "no",
                    detail: format!("registry_match={} role=bridge guitar=none", row.name),
                });
                true
            }
            NameMatch::Ambiguous => {
                self.fail_match(StageId::UsbRecognition, "ambiguous");
                false
            }
            NameMatch::None => {
                self.fail_match(StageId::UsbRecognition, "not-in-registry");
                false
            }
        }
    }

    fn identity_stage(&mut self, scene: &Scene) -> bool {
        let parsed = identity_from_engine(scene.identity_product);
        match parsed {
            Ok(identity) if matches!(identity.product_id(), 0x0007_0000 | 0x0007_0004) => {
                let product_id = identity.product_id();
                self.product_id = Some(product_id);
                self.passes.push(Pass {
                    stage: StageId::Identification,
                    decision: "yes",
                    engine: "midi::Identity::parse",
                    engine_result: "identity_ok",
                    opened_device: "no",
                    message_sent: "no",
                    synthetic_send: "yes",
                    detail: format!("product={product_id:08x} read_allowlist_only=yes"),
                });
                true
            }
            Ok(_) => {
                self.passes.push(Pass {
                    stage: StageId::Identification,
                    decision: "yes",
                    engine: "midi::Identity::parse",
                    engine_result: "not_legacy_variax",
                    opened_device: "no",
                    message_sent: "no",
                    synthetic_send: "yes",
                    detail: "read_allowlist_only=yes".to_string(),
                });
                false
            }
            Err(_) => {
                self.passes.push(Pass {
                    stage: StageId::Identification,
                    decision: "yes",
                    engine: "midi::Identity::parse",
                    engine_result: "identity_rejected",
                    opened_device: "no",
                    message_sent: "no",
                    synthetic_send: "yes",
                    detail: "read_allowlist_only=yes".to_string(),
                });
                false
            }
        }
    }

    fn inventory_stage(&mut self) -> bool {
        let Some(product_id) = self.product_id else {
            self.fail_match(StageId::InventoryCheck, "not-in-registry");
            return false;
        };
        match self.inventory.find_l6(product_id) {
            NameMatch::One(row) if row.role == Role::Guitar => {
                let capability = row.engine_profile.map(|profile| {
                    self.profile = Some(profile);
                    resolve(
                        profile,
                        Operation::EnumerateUsb,
                        Some(CapabilityState::Offline),
                    )
                });
                let capability_token = match capability {
                    Some(CapabilityState::Locked) => "Locked",
                    Some(CapabilityState::Offline) => "Offline",
                    Some(CapabilityState::Unverified) => "Unverified",
                    Some(CapabilityState::Unsupported) => "Unsupported",
                    Some(CapabilityState::Planned { .. }) => "Planned",
                    None => "none",
                };
                self.guitar = Some(row.name.clone());
                let gaps = if row.gaps.is_empty() {
                    "none".to_string()
                } else {
                    row.gaps.join(",")
                };
                self.passes.push(Pass {
                    stage: StageId::InventoryCheck,
                    decision: "yes",
                    engine: "rig::capability::resolve",
                    engine_result: "inventory_match",
                    opened_device: "no",
                    message_sent: "no",
                    synthetic_send: "no",
                    detail: format!(
                        "guitar={} product={product_id:08x} capability={capability_token} gaps={gaps} next_process_change=existing-engine",
                        row.name
                    ),
                });
                true
            }
            NameMatch::One(_) => {
                self.fail_match(StageId::InventoryCheck, "not_a_guitar");
                false
            }
            NameMatch::Ambiguous => {
                self.fail_match(StageId::InventoryCheck, "ambiguous");
                false
            }
            NameMatch::None => {
                self.fail_match(StageId::InventoryCheck, "not-in-registry");
                false
            }
        }
    }

    fn settings_stage(&mut self, raw: &[u8; 192]) -> bool {
        if !self.record_live_gate(StageId::SettingsRead) {
            return false;
        }
        match read_metadata(raw) {
            Ok(current) => self.store_reading(StageId::SettingsRead, current, None),
            Err(result) => {
                self.passes
                    .push(read_failure(StageId::SettingsRead, result));
                false
            }
        }
    }

    fn change_stage(&mut self, raw: &[u8; 192]) -> bool {
        if !self.record_live_gate(StageId::SettingChange) {
            return false;
        }
        let Some(before) = self.reading.clone() else {
            self.passes
                .push(read_failure(StageId::SettingChange, "missing_setting"));
            return false;
        };
        match read_metadata(raw) {
            Ok(current) => self.store_reading(StageId::SettingChange, current, Some(before)),
            Err(result) => {
                self.passes
                    .push(read_failure(StageId::SettingChange, result));
                false
            }
        }
    }

    fn store_reading(
        &mut self,
        stage: StageId,
        current: crate::read_session::CurrentPreset,
        previous: Option<Reading>,
    ) -> bool {
        let product_id = current.identity.product_id();
        if self.product_id != Some(product_id) {
            self.passes
                .push(read_failure(stage, "rejection_different_model"));
            return false;
        }
        let Some(reading) = reading_from(&current.metadata, self.guitar.as_deref(), product_id)
        else {
            self.passes.push(read_failure(stage, "missing_setting"));
            return false;
        };
        let detail = match &previous {
            None => format!(
                "guitar={} product={product_id:08x} setting={} value={} body_model={} read_allowlist_only=yes next_process_change=fixture",
                reading.guitar, reading.setting, reading.value, reading.body_model
            ),
            Some(previous) => {
                let change = if previous.value == reading.value {
                    "unchanged"
                } else {
                    "changed"
                };
                format!(
                    "guitar={} product={product_id:08x} setting={} before={} after={} change={change} body_model={} read_allowlist_only=yes next_process_change=fixture",
                    reading.guitar,
                    reading.setting,
                    previous.value,
                    reading.value,
                    reading.body_model
                )
            }
        };
        self.reading = Some(reading);
        self.passes.push(Pass {
            stage,
            decision: "yes",
            engine: "read_session::read_current_on",
            engine_result: "metadata_ok",
            opened_device: "no",
            message_sent: "no",
            synthetic_send: "yes",
            detail,
        });
        true
    }

    fn record_live_gate(&mut self, stage: StageId) -> bool {
        let (engine, engine_result) = live_engine_result();
        let live_ok = live_gate_ok(engine_result);
        self.passes.push(Pass {
            stage,
            decision: "yes",
            engine,
            engine_result,
            opened_device: "no",
            message_sent: "no",
            synthetic_send: "no",
            detail: "host_to_device=none next_process_change=existing-engine".to_string(),
        });
        live_ok
    }

    fn fail_match(&mut self, stage: StageId, engine_result: &'static str) {
        self.passes.push(Pass {
            stage,
            decision: "yes",
            engine: "device_inventory",
            engine_result,
            opened_device: "no",
            message_sent: "no",
            synthetic_send: "no",
            detail: "guitar=none next_process_change=table".to_string(),
        });
    }
}

fn reading_from(
    metadata: &PresetMetadata,
    guitar: Option<&str>,
    product_id: u32,
) -> Option<Reading> {
    let setting = metadata.parameters.get(WATCHED_SETTING)?;
    let body = metadata.parameters.get(MODEL_FIELD)?;
    Some(Reading {
        guitar: guitar.unwrap_or("none").to_string(),
        product_id,
        setting: WATCHED_SETTING,
        value: parameter_token(setting),
        body_model: parameter_token(body),
    })
}

fn parameter_token(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Integer(value) => value.to_string(),
        ParameterValue::Boolean(true) => "1".to_string(),
        ParameterValue::Boolean(false) => "0".to_string(),
        ParameterValue::Float(value) => format!("{value:.6}"),
    }
}

fn read_metadata(raw: &[u8; 192]) -> Result<crate::read_session::CurrentPreset, &'static str> {
    let mut rig = VirtualControl::new(*raw).map_err(|_| "unclassified-engine-error")?;
    let current = match read_current_on(&mut rig, &AtomicBool::new(false)) {
        Ok(current) => current,
        Err(error) => return Err(classify_read_error(&error)),
    };
    if !rig
        .sent_messages()
        .iter()
        .all(|message| read_session::is_read_session_message(message))
    {
        return Err("unclassified-engine-error");
    }
    Ok(current)
}

fn read_failure(stage: StageId, engine_result: &'static str) -> Pass {
    Pass {
        stage,
        decision: "yes",
        engine: "read_session::read_current_on",
        engine_result,
        opened_device: "no",
        message_sent: "no",
        synthetic_send: "yes",
        detail: "read_allowlist_only=yes next_process_change=fixture".to_string(),
    }
}

fn classify_read_error(error: &str) -> &'static str {
    if error.contains("identical") {
        "ambiguous_metadata"
    } else if error.contains("did not identify") {
        "not_legacy_variax"
    } else if error.contains("timed out") {
        "timeout"
    } else {
        "unclassified-engine-error"
    }
}

fn identity_from_engine(product: u32) -> Result<Identity, ()> {
    let mut rig = VirtualControl::new([b' '; 192])
        .map_err(|_| ())?
        .with_declared_product(product)
        .map_err(|_| ())?;
    rig.send(&read_session::IDENTITY_REQUEST).map_err(|_| ())?;
    let reply = rig
        .next_message(&AtomicBool::new(false), Duration::from_secs(0))
        .map_err(|_| ())?
        .ok_or(())?;
    Identity::parse(&reply).map_err(|_| ())
}

fn live_gate_ok(engine_result: &str) -> bool {
    matches!(engine_result, "live_access_locked" | "usb_feature_off")
}

fn live_engine_result() -> (&'static str, &'static str) {
    #[cfg(feature = "usb")]
    {
        if !crate::usb::LIVE_ACCESS_LOCKED {
            return ("usb::read_current", "live_call_refused");
        }
        match crate::usb::read_current(&AtomicBool::new(false)) {
            Err(error)
                if error.contains("No device was opened")
                    && error.contains("no message was sent") =>
            {
                ("usb::read_current", "live_access_locked")
            }
            _ => ("usb::read_current", "unclassified-engine-error"),
        }
    }
    #[cfg(not(feature = "usb"))]
    {
        ("usb::read_current", "usb_feature_off")
    }
}

fn stage_passed(stage: StageId, passes: &[Pass]) -> bool {
    let code = match stage {
        StageId::UsbRecognition => "one_match",
        StageId::Identification => "identity_ok",
        StageId::InventoryCheck => "inventory_match",
        StageId::SettingsRead | StageId::SettingChange => "metadata_ok",
    };
    passes
        .iter()
        .any(|pass| pass.stage == stage && pass.engine_result == code)
}

fn stage_token(stage: StageId) -> &'static str {
    match stage {
        StageId::UsbRecognition => "usb-recognition",
        StageId::Identification => "identification",
        StageId::InventoryCheck => "inventory",
        StageId::SettingsRead => "settings-read",
        StageId::SettingChange => "setting-change",
    }
}

pub fn scene(tuning_before: i8, tuning_after: i8, body: i32) -> Scene {
    Scene {
        usb_vid: crate::linux_transport::VARIAX_PODXT_LIVE.vendor_id,
        usb_pid: crate::linux_transport::VARIAX_PODXT_LIVE.product_id,
        identity_product: crate::firmware::VARIAX_300_600_PRODUCT_ID,
        before: metadata_bytes(tuning_before, body),
        after: metadata_bytes(tuning_after, body),
    }
}

fn metadata_bytes(tuning: i8, body: i32) -> [u8; 192] {
    let mut raw = [0; 192];
    raw[..14].copy_from_slice(b"ZzNameSentinel");
    raw[18..22].copy_from_slice(&body.to_be_bytes());
    raw[69] = tuning as u8;
    raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firmware::VARIAX_300_600_PRODUCT_ID;

    fn run_all(scene: &Scene) -> Session {
        let mut session = Session::start().expect("inventory");
        for _ in 0..5 {
            assert!(session.disclaimer().is_some());
            session.proceed(scene);
        }
        assert!(session.pending().is_none());
        session
    }

    #[test]
    fn risk_rises_and_every_stage_states_intent_and_risk() {
        let mut previous = 0;
        for stage in StageId::LADDER {
            let copy = disclaimer_for(stage, None);
            assert!(copy.risk > previous);
            previous = copy.risk;
            assert_eq!(copy.title, stage.title());
            assert!(copy.intent.contains("does not send a preset"));
            assert!(!copy.risk_text.is_empty());
        }
        assert!(
            disclaimer_for(StageId::UsbRecognition, None)
                .risk_text
                .starts_with("Low risk")
        );
        assert!(
            disclaimer_for(StageId::SettingsRead, None)
                .risk_text
                .starts_with("High risk")
        );
        assert!(
            disclaimer_for(StageId::SettingChange, None)
                .risk_text
                .starts_with("Highest risk")
        );
        let reading = Reading {
            guitar: "Variax 300/600".to_string(),
            product_id: VARIAX_300_600_PRODUCT_ID,
            setting: WATCHED_SETTING,
            value: "-12".to_string(),
            body_model: "51380227".to_string(),
        };
        let last = disclaimer_for(StageId::SettingChange, Some(&reading));
        assert!(last.intent.contains("Variax 300/600"));
        assert!(last.intent.contains("string1_tuning = -12"));
        assert!(
            last.intent
                .contains("Change string1_tuning on the instrument")
        );
        assert!(last.intent.contains("51380227"));
    }

    #[test]
    fn decline_stops_and_the_next_stage_is_not_offered() {
        let mut session = Session::start().expect("inventory");
        let scene = scene(-12, -10, 0x0310_0003);
        assert_eq!(session.pending(), Some(StageId::UsbRecognition));
        session.proceed(&scene);
        assert_eq!(session.pending(), Some(StageId::Identification));
        session.decline();
        assert!(session.pending().is_none());
        assert!(session.disclaimer().is_none());
        let report = session.report();
        assert!(report.contains("stage=usb-recognition"));
        assert!(report.contains("stage=identification"));
        assert!(report.contains("decision=no"));
        assert!(report.contains("engine_result=declined"));
        assert!(!report.contains("engine=midi::Identity::parse"));
        assert!(!report.contains("stage=settings-read"));
        assert!(!report.contains("stage=setting-change"));
        session.proceed(&scene);
        assert!(!session.report().contains("engine=midi::Identity::parse"));
    }

    #[test]
    fn second_read_reports_the_named_setting_changed() {
        let session = run_all(&scene(-12, -10, 0x0310_0003));
        let report = session.report();
        assert!(report.contains("writes=no"));
        assert!(report.contains("registry_match=PODxt Live"));
        assert!(report.contains("guitar=none"));
        assert!(report.contains("guitar=Variax 300/600"));
        assert!(report.contains("capability=Locked"));
        assert!(report.contains("engine=usb::read_current"));
        assert!(report.contains("engine_result=live_access_locked"));
        assert!(report.contains("opened_device=no"));
        assert!(!report.contains("message_sent=yes"));
        assert!(report.contains("setting=string1_tuning value=-12 body_model=51380227"));
        assert!(report.contains(
            "setting=string1_tuning before=-12 after=-10 change=changed body_model=51380227"
        ));
        assert!(!report.contains("ZzNameSentinel"));
        assert!(report.contains("read_allowlist_only=yes"));
        assert_eq!(
            session
                .reading
                .as_ref()
                .map(|reading| reading.value.as_str()),
            Some("-10")
        );
    }

    #[test]
    fn second_read_reports_when_the_setting_stayed() {
        let session = run_all(&scene(-12, -12, 0x0310_0003));
        let report = session.report();
        assert!(report.contains("setting=string1_tuning before=-12 after=-12 change=unchanged"));
        assert!(!report.contains("change=changed"));
        assert!(session.pending().is_none());
        assert_eq!(
            session
                .passes
                .iter()
                .find(|pass| pass.stage == StageId::SettingChange
                    && pass.engine_result == "metadata_ok")
                .map(|pass| pass.engine_result),
            Some("metadata_ok")
        );
    }

    #[test]
    fn a_non_variax_identity_stops_before_the_settings_read() {
        let mut scene = scene(-12, -10, 0x0310_0003);
        scene.identity_product = crate::firmware::PODXT_LIVE_PRODUCT_ID;
        let mut session = Session::start().expect("inventory");
        session.proceed(&scene);
        session.proceed(&scene);
        assert!(session.pending().is_none());
        let report = session.report();
        assert!(report.contains("engine_result=not_legacy_variax"));
        assert!(!report.contains("stage=settings-read"));
        assert!(!report.contains("ZzNameSentinel"));
    }

    #[test]
    fn identical_metadata_keeps_the_earlier_passes_and_skips_the_change_check() {
        let mut scene = scene(-12, -10, 0x0310_0003);
        let first = scene.before[..96].to_vec();
        scene.before[96..].copy_from_slice(&first);
        let mut session = Session::start().expect("inventory");
        while session.pending().is_some() {
            session.proceed(&scene);
        }
        let report = session.report();
        assert!(report.contains("engine_result=ambiguous_metadata"));
        assert!(report.contains("stage=usb-recognition"));
        assert!(report.contains("stage=identification"));
        assert!(report.contains("stage=inventory"));
        assert!(!report.contains("stage=setting-change"));
        assert!(!report.contains("ZzNameSentinel"));
        assert!(!report.contains("message_sent=yes"));
    }

    #[test]
    fn change_check_names_the_setting_from_the_first_read() {
        let mut session = Session::start().expect("inventory");
        let scene = scene(-12, -7, 0x0310_0003);
        for _ in 0..4 {
            let copy = session.disclaimer().expect("stage");
            assert!(copy.risk < 5 || copy.stage == StageId::SettingsRead);
            session.proceed(&scene);
        }
        let copy = session.disclaimer().expect("change check");
        assert_eq!(copy.stage, StageId::SettingChange);
        assert_eq!(copy.risk, 5);
        assert!(copy.intent.contains("string1_tuning = -12"));
        assert!(copy.intent.contains("body model 51380227"));
        assert!(
            copy.intent
                .contains("Change string1_tuning on the instrument")
        );
        session.proceed(&scene);
        assert!(
            session
                .report()
                .contains("before=-12 after=-7 change=changed")
        );
    }
}
