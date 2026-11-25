//! Audio/DAW steps (#264, #265).
//!
//! Step 8.1 reports what the operating system already exposes about the
//! PODxt Live's audio device **without opening any stream**: it reads only
//! text the kernel publishes (`/proc/asound`) and lists device nodes and
//! sockets by name through a [`FileSource`]. Steps 8.2-8.5 (play, record,
//! latency, record each test model) are designed in
//! `docs/rig-check/audio-steps.md`; their code paths exist but stop before
//! any stream is opened until the owner approves audio streams (AGENTS.md).

use super::approvals::AUDIO_STREAMS_APPROVED;
use super::computer::FileSource;
use super::runner::Ctx;
use crate::pod_audio::{BYTES_PER_CHANNEL, DECLARED_CHANNELS, RATE_DENOMINATOR, RATE_NUMERATOR};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PcmInfo {
    pub card: u32,
    pub device: String,
    pub direction: &'static str,
    pub name: Option<String>,
    pub subdevices: Option<String>,
    /// `closed` unless another program is using it.
    pub state: Option<String>,
}

/// `Format:`, `Channels:` and `Rates:` lines of a USB-audio `stream0` file.
pub fn parse_stream(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "Playback:" || trimmed == "Capture:" {
            section = trimmed.trim_end_matches(':').to_owned();
        }
        for key in ["Format", "Channels", "Rates"] {
            if let Some(value) = trimmed.strip_prefix(&format!("{key}:")) {
                out.push((
                    format!("{section} {key}").trim().to_owned(),
                    value.trim().to_owned(),
                ));
            }
        }
    }
    out
}

fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(&format!("{key}:"))
            .map(|v| v.trim().to_owned())
    })
}

/// PCM devices of one card from `proc/asound/cardN/pcmXp|c/info`.
pub fn pcm_devices(source: &dyn FileSource, card: u32) -> Vec<PcmInfo> {
    let dir = format!("proc/asound/card{card}");
    source
        .list(&dir)
        .into_iter()
        .filter(|name| name.starts_with("pcm") && (name.ends_with('p') || name.ends_with('c')))
        .map(|name| {
            let info = source
                .read(&format!("{dir}/{name}/info"))
                .unwrap_or_default();
            let state = source
                .read(&format!("{dir}/{name}/sub0/hw_params"))
                .map(|t| t.lines().next().unwrap_or("").trim().to_owned());
            PcmInfo {
                card,
                direction: if name.ends_with('p') {
                    "playback"
                } else {
                    "capture"
                },
                device: name,
                name: field(&info, "name"),
                subdevices: field(&info, "subdevices_count"),
                state,
            }
        })
        .collect()
}

/// The audio system, from sockets and files by name only (nothing is opened).
pub fn audio_system(source: &dyn FileSource) -> Vec<String> {
    let mut found = vec!["ALSA (kernel)".to_string()];
    for user in source.list("run/user") {
        let sockets = source.list(&format!("run/user/{user}"));
        if sockets.iter().any(|s| s.starts_with("pipewire-")) {
            found.push(format!("PipeWire (socket for user id {user})"));
        }
        if source
            .list(&format!("run/user/{user}/pulse"))
            .iter()
            .any(|s| s == "native")
        {
            found.push(format!("PulseAudio protocol (user id {user})"));
        }
    }
    if source.list("dev/shm").iter().any(|s| s.starts_with("jack")) {
        found.push("JACK (shared memory present)".into());
    }
    found
}

/// Step 8.1 on Linux: all lines for the report.
pub fn detect_linux(source: &dyn FileSource) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    lines.push(format!("Audio system: {}", audio_system(source).join(", ")));
    lines.push("Audio system versions: not collected (that needs the servers' own tools).".into());
    let cards_text = source
        .read("proc/asound/cards")
        .ok_or("The kernel lists no sound cards.")?;
    let cards = crate::device_probe::parse_sound_cards(&cards_text).map_err(|e| e.to_string())?;
    for card in &cards {
        lines.push(format!(
            "Sound card {}: {} ({})",
            card.index, card.id, card.name
        ));
    }
    let pod = cards
        .iter()
        .find(|card| card.id == "PODxtLive" || card.name.contains("PODxt Live"));
    let Some(pod) = pod else {
        return Err("No PODxt Live sound card: is the PODxt Live connected and its driver (snd-usb-pod) loaded?".into());
    };
    lines.push(format!(
        "PODxt Live audio device: card {} ({})",
        pod.index, pod.id
    ));
    for pcm in pcm_devices(source, pod.index) {
        lines.push(format!(
            "  {} {}: name {}, subdevices {}, state {}",
            pcm.device,
            pcm.direction,
            pcm.name.as_deref().unwrap_or("?"),
            pcm.subdevices.as_deref().unwrap_or("?"),
            pcm.state.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(stream) = source.read(&format!("proc/asound/card{}/stream0", pod.index)) {
        for (key, value) in parse_stream(&stream) {
            lines.push(format!("  {key}: {value}"));
        }
    } else {
        lines.push(format!(
            "  Formats (from the snd-usb-pod driver declaration, not read from the device): S24_3LE, {DECLARED_CHANNELS} channels, {} bytes per channel, rate {RATE_NUMERATOR}/{RATE_DENOMINATOR} Hz",
            BYTES_PER_CHANNEL
        ));
    }
    let defaults = source
        .read("etc/asound.conf")
        .map(|t| {
            t.lines()
                .filter(|l| {
                    l.contains("defaults.pcm.card") || l.trim_start().starts_with("pcm.!default")
                })
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|t| !t.is_empty());
    lines.push(format!(
        "Default device: {}",
        defaults.unwrap_or_else(|| "not set in /etc/asound.conf (the audio server chooses)".into())
    ));
    lines.push("No audio stream was opened.".into());
    Ok(lines)
}

pub fn detect(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let lines = if cfg!(target_os = "linux") || ctx.mode == super::record::Mode::Practice {
        let system = ctx.port.system();
        detect_linux(&*system)?
    } else if cfg!(target_os = "windows") {
        vec![
            "Audio system: WASAPI (ASIO through the Line 6 driver if installed)".into(),
            "Device formats: not available on this OS yet (needs the #262 adapter); no stream was opened.".into(),
        ]
    } else {
        vec![
            "Audio system: Core Audio".into(),
            "Device formats: not available on this OS yet (needs the #262 adapter); no stream was opened.".into(),
        ]
    };
    for line in lines {
        ctx.rec.note(line);
    }
    Ok(())
}

/// Steps 8.2-8.5: the code path a future approved build takes. It stops
/// before any stream is opened while audio streams are not approved.
pub fn record_or_play(ctx: &mut Ctx<'_>) -> Result<(), String> {
    if !AUDIO_STREAMS_APPROVED {
        return Err("Not available in this version — needs the owner's audio approval (AGENTS.md, #265). No audio stream was opened.".into());
    }
    let _ = ctx;
    Err("Audio streams are approved, but the recorder integration (#265) is not built yet.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::computer::linux::synthetic_machine;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    #[test]
    fn the_podxt_audio_device_is_listed_without_a_stream() {
        let mut fs = synthetic_machine(true, true);
        fs.file(
            "proc/asound/card1/pcm0p/info",
            "card: 1\ndevice: 0\nid: PODxt Live\nname: PODxt Live\nsubdevices_count: 1\n",
        )
        .file("proc/asound/card1/pcm0p/sub0/hw_params", "closed\n")
        .file(
            "proc/asound/card1/pcm0c/info",
            "name: PODxt Live\nsubdevices_count: 1\n",
        )
        .node("run/user/1000/pipewire-0", 0o666, 1000, 1000)
        .file("etc/asound.conf", "defaults.pcm.card 1\n");
        let lines = detect_linux(&fs).unwrap();
        let text = lines.join("\n");
        assert!(text.contains("PipeWire (socket for user id 1000)"));
        assert!(text.contains("PODxt Live audio device: card 1 (PODxtLive)"));
        assert!(text.contains("pcm0p playback: name PODxt Live, subdevices 1, state closed"));
        assert!(text.contains("pcm0c capture"));
        assert!(text.contains("78125/2 Hz"));
        assert!(text.contains("Default device: defaults.pcm.card 1"));
        assert!(text.ends_with("No audio stream was opened."));
        assert!(
            detect_linux(&synthetic_machine(false, true))
                .unwrap_err()
                .contains("No PODxt Live sound card")
        );
    }

    #[test]
    fn usb_audio_stream_files_are_parsed() {
        let stream = "Line 6 Device at usb-1, full speed : USB Audio\n\nPlayback:\n  Status: Stop\n  Interface 1\n    Altset 1\n    Format: S24_3LE\n    Channels: 2\n    Rates: 48000\n\nCapture:\n  Interface 2\n    Format: S24_3LE\n    Channels: 2\n    Rates: 44100, 48000\n";
        let parsed = parse_stream(stream);
        assert_eq!(parsed[0], ("Playback Format".into(), "S24_3LE".into()));
        assert_eq!(parsed[5], ("Capture Rates".into(), "44100, 48000".into()));
    }

    #[test]
    fn audio_steps_stay_off_without_the_owner_approval() {
        let mut person = Script::run_all();
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(run.step(StepId(8, 1)).outcome.passed());
        for id in [StepId(8, 2), StepId(8, 3), StepId(8, 4), StepId(8, 5)] {
            assert!(
                matches!(&run.step(id).outcome, Outcome::NotAvailable(r) if r.contains("audio approval")),
                "{id}"
            );
        }
    }
}
