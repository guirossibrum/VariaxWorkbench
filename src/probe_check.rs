#![forbid(unsafe_code)]
//! Device-check screen and the offline self-check.
//!
//! The screen drives the existing probe session. It does not open USB, MIDI,
//! or audio. Audio capture stays out of this build.

use crate::device_probe::{self, Disclaimer, RailState, Scene, Session, StageId};
use crate::document;
use std::path::Path;

pub const FOOTER: &str = "This build does not open USB, MIDI, or audio devices.";
pub const STAGE_LIMIT: &str = "This stage sends no USB, MIDI, or audio request.";
pub const AUDIO_PIN: &str =
    "Audio capture is not in this build. It waits for the family capture tools.";

pub struct CheckScreen {
    session: Session,
    scene: Scene,
    notice: String,
}

impl CheckScreen {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            session: Session::start()?,
            scene: device_probe::scene(-12, -10, 0x0310_0003),
            notice: String::new(),
        })
    }

    pub fn rail(&self) -> [RailState; 5] {
        self.session.rail()
    }

    pub fn current(&self) -> Option<StageId> {
        self.session.pending()
    }

    pub fn disclaimer(&self) -> Option<Disclaimer> {
        self.session.disclaimer()
    }

    pub fn buttons_for(&self, stage: StageId) -> bool {
        self.session.pending() == Some(stage)
    }

    pub fn notice(&self) -> &str {
        &self.notice
    }

    pub fn choose_no(&mut self) {
        self.session.decline();
        self.notice.clear();
    }

    pub fn choose_yes(&mut self) {
        self.session.proceed(&self.scene);
        self.notice.clear();
    }

    pub fn new_check(&mut self) -> Result<(), String> {
        *self = Self::new()?;
        Ok(())
    }

    pub fn report(&self) -> String {
        sanitize_report(&self.session.report())
    }

    pub fn save(&mut self, path: &Path) -> Result<(), &'static str> {
        let bytes = sanitize_report(&self.session.report());
        match document::save_new(path, bytes.as_bytes()) {
            Ok(()) => {
                self.notice = "Saved".to_string();
                Ok(())
            }
            Err(_) => {
                self.notice = "SaveFailed".to_string();
                Err("SaveFailed")
            }
        }
    }
}

pub fn sanitize_report(text: &str) -> String {
    text.lines()
        .filter(|line| !forbidden_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn forbidden_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("zznamesentinel")
        || line.contains('\\')
        || line.contains('@')
        || lower.contains("serial=")
    {
        return true;
    }
    line.replace("300/600", "")
        .replace("500/700", "")
        .contains('/')
}

pub fn synthetic_self_check() -> Result<String, String> {
    let mut failure_scene = device_probe::scene(-12, -10, 0x0310_0003);
    failure_scene.identity_product = crate::firmware::PODXT_LIVE_PRODUCT_ID;
    let failure = run_scene(&failure_scene)?;
    if !failure.contains("engine_result=not_legacy_variax")
        || failure.contains("stage=settings-read")
        || failure.contains("message_sent=yes")
        || failure.contains("opened_device=yes")
        || failure.contains("ZzNameSentinel")
    {
        return Err("failure self-check did not keep the locked read boundary".into());
    }
    let success = run_scene(&device_probe::scene(-12, -10, 0x0310_0003))?;
    if !success.contains("change=changed")
        || !success.contains("writes=no")
        || !success.contains("guitar=Variax 300/600")
        || success.contains("message_sent=yes")
        || success.contains("opened_device=yes")
        || success.contains("ZzNameSentinel")
        || !(success.contains("engine_result=live_access_locked")
            || success.contains("engine_result=usb_feature_off"))
    {
        return Err("partial-success self-check did not name the in-memory reading".into());
    }
    Ok(format!(
        "check=failure\n{failure}\ncheck=partial-success\n{success}"
    ))
}

fn run_scene(scene: &Scene) -> Result<String, String> {
    let mut session = Session::start()?;
    while session.pending().is_some() {
        session.proceed(scene);
    }
    Ok(sanitize_report(&session.report()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_probe::RailState;

    #[test]
    fn the_next_stage_has_no_yes_until_the_previous_stage_is_green() {
        let mut screen = CheckScreen::new().expect("screen");
        assert!(screen.buttons_for(StageId::UsbRecognition));
        assert!(!screen.buttons_for(StageId::Identification));
        assert_eq!(screen.rail()[1], RailState::Locked);
        screen.choose_yes();
        assert!(!screen.buttons_for(StageId::UsbRecognition));
        assert!(screen.buttons_for(StageId::Identification));
        assert!(!screen.report().contains("engine=midi::Identity::parse"));
        assert_eq!(screen.current(), Some(StageId::Identification));
    }

    #[test]
    fn a_new_check_drops_the_previous_consent() {
        let mut screen = CheckScreen::new().expect("screen");
        screen.choose_yes();
        screen.new_check().expect("reset");
        assert_eq!(screen.current(), Some(StageId::UsbRecognition));
        assert!(!screen.report().contains("decision=yes"));
    }

    #[test]
    fn the_report_drops_names_paths_and_serials() {
        let dirty = "stage=usb-recognition\nname=ZzNameSentinel\npath=/tmp/secret\nwin=C:\\Users\\a\nmail=a@b\nserial=ABC\nwrites=no";
        let clean = sanitize_report(dirty);
        assert_eq!(clean, "stage=usb-recognition\nwrites=no");
        let screen = CheckScreen::new().expect("screen");
        let report = screen.report();
        assert!(!report.contains("ZzNameSentinel"));
        assert!(!report.contains('/'));
        assert!(!report.contains('\\'));
        assert!(report.contains("writes=no"));
    }

    #[test]
    fn save_failure_has_no_path_and_a_real_save_keeps_the_report() {
        let mut screen = CheckScreen::new().expect("screen");
        let missing = std::env::temp_dir().join("variax-probe-missing-parent");
        let bad = missing.join("no-such-dir").join("report.txt");
        assert_eq!(screen.save(&bad), Err("SaveFailed"));
        assert_eq!(screen.notice(), "SaveFailed");
        assert!(!screen.notice().contains("no-such-dir"));
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("report.txt");
        screen.save(&path).expect("save");
        let saved = std::fs::read_to_string(&path).expect("read");
        assert!(saved.contains("writes=no"));
        assert!(!saved.contains("ZzNameSentinel"));
    }

    #[test]
    fn synthetic_self_check_covers_one_failure_and_one_partial_success() {
        let text = synthetic_self_check().expect("self-check");
        let failure = text.split("check=partial-success").next().expect("failure");
        let success = text.split("check=partial-success").nth(1).expect("success");
        assert!(failure.contains("engine_result=not_legacy_variax"));
        assert!(!failure.contains("stage=settings-read"));
        assert!(success.contains("change=changed"));
        assert!(success.contains("opened_device=no"));
        assert!(!text.contains("ZzNameSentinel"));
        assert!(!text.contains("message_sent=yes"));
    }
}
