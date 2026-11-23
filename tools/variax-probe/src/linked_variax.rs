use super::{Capture, OwnerGrants, describe_identity};
use line6_devices::family::podxt::LinkedVariaxModel::{self, FactorySpank1, FactoryTModel1};
use line6_devices::family::{podxt, variax};
use line6_devices::session::{Link, Log, Path, Session};
use line6_devices::transport::{rawmidi, usb};
use std::time::Duration;

const LINKED_MODEL_CC: u8 = 120;
const RECALL_SETTLE: Duration = Duration::from_secs(10);

struct Readback {
    tone: podxt::EditBuffer,
    guitar: variax::PresetMetadata,
}

fn observe(
    podxt_link: &mut impl Link,
    capture: &mut Capture,
    stage: &str,
) -> Result<Readback, String> {
    let tone = podxt::read_current(&mut Session::new(podxt_link, &OwnerGrants, capture))?;
    if tone.identity.member != podxt::PODXT_LIVE_MEMBER {
        return Err("The tone read requires a PODxt Live.".into());
    }
    capture.file(&format!("{stage}-podxt.syx"), &tone.tone.raw)?;

    let mut link = usb::UsbInterfaceLink::open(usb::VARIAX_VIA_PODXT_LIVE)?;
    let result = variax::read_current(&mut Session::new(&mut link, &OwnerGrants, capture));
    let cleanup = link.finish();
    let preset = match (result, cleanup) {
        (Ok(preset), Ok(())) => preset,
        (Err(read), Err(cleanup)) => return Err(format!("{read} USB cleanup: {cleanup}")),
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };
    capture.line(describe_identity(
        &preset.identified.reply,
        Some(preset.identified.requests),
    ));
    if preset.identified.identity.member != 4 {
        return Err("The guitar read requires the Variax 300/600 identity.".into());
    }
    for (index, reply) in preset.replies.iter().enumerate() {
        capture.file(&format!("{stage}-variax-{}.syx", index + 1), reply)?;
    }
    Ok(Readback {
        tone: tone.tone,
        guitar: preset.metadata,
    })
}

fn confirm(
    capture: &mut Capture,
    stage: &str,
    read: &Readback,
    model: LinkedVariaxModel,
) -> Result<(), String> {
    let (label, name, body) = match model {
        FactoryTModel1 => ("F-T-Model 1", "T-Model-1", 0x03100000),
        FactorySpank1 => ("F-Spank 1", "Spank-1", 0x03100003),
    };
    let assignment = read.tone.cc(LINKED_MODEL_CC);
    let guitar_body = read.guitar.parameters.get("body_model");
    capture.line(format!(
        "{stage}: PODxt linked model value {assignment}, Variax preset {}.",
        read.guitar.name
    ));
    if assignment != model as u8
        || read.guitar.name != name
        || guitar_body != Some(&variax::ParameterValue::Integer(body))
    {
        return Err(format!(
            "{stage}: expected {label} in the PODxt tone and {name} on the guitar."
        ));
    }
    Ok(())
}

fn edit_and_read(
    podxt_link: &mut impl Link,
    capture: &mut Capture,
    stage: &str,
    model: LinkedVariaxModel,
) -> Result<Readback, String> {
    podxt::set_linked_variax_model(
        &mut Session::new(&mut *podxt_link, &OwnerGrants, capture),
        model,
    )?;
    capture.line(format!("{stage}: waiting 10 seconds before readback."));
    std::thread::sleep(RECALL_SETTLE);
    let read = observe(podxt_link, capture, stage)?;
    confirm(capture, stage, &read, model)?;
    Ok(read)
}

fn round_trip<T>(
    mut edit: impl FnMut(&str, LinkedVariaxModel) -> Result<T, String>,
) -> (Result<T, String>, Result<T, String>) {
    let changed = edit("changed", FactorySpank1);
    // A failed send or read can leave the selected model changed.
    let restored = edit("restored", FactoryTModel1);
    (changed, restored)
}

pub(super) fn run(capture: &mut Capture) -> Result<(), String> {
    // The ALSA reader keeps a cloned descriptor open while it waits for
    // input. Use one PODxt port throughout the test.
    let mut podxt_link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    capture.line(format!(
        "Opened PODxt MIDI port {}",
        podxt_link.device.display()
    ));
    let baseline = observe(&mut podxt_link, capture, "before")?;
    confirm(capture, "before", &baseline, FactoryTModel1)
        .map_err(|err| format!("{err} No edits were sent."))?;

    let (changed, restored) =
        round_trip(|stage, model| edit_and_read(&mut podxt_link, capture, stage, model));
    if let Err(error) = &changed {
        capture.line(format!("Target change did not verify: {error}"));
    }
    let restored =
        restored.map_err(|error| format!("Original model undo did not verify: {error}"))?;
    if !variax::same_preset(&baseline.guitar, &restored.guitar) {
        return Err("The restored guitar metadata differs from the baseline.".into());
    }
    if baseline.tone.raw != restored.tone.raw {
        return Err(format!(
            "The guitar was restored. PODxt edit-buffer differences after undo: {:?}",
            baseline.tone.differences(&restored.tone)
        ));
    }
    capture.line("Original guitar metadata and all 168 PODxt tone bytes restored.".to_owned());
    changed?;
    capture.line(
        "PASS: linked model changed to Spank-1 and returned to T-Model-1 without saving."
            .to_owned(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_to_t_model_is_attempted_after_failed_target_readback() {
        let mut models = Vec::new();
        let (changed, restored) = round_trip(|_, model| {
            models.push(model);
            match model {
                FactorySpank1 => Err("target readback unavailable".into()),
                FactoryTModel1 => Ok(()),
            }
        });
        assert!(changed.is_err() && restored.is_ok());
        assert_eq!(models, [FactorySpank1, FactoryTModel1]);
    }
}
