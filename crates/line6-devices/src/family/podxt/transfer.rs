//! The full PODxt Live tone payload used by Line 6 Edit 3.06 EncodePatch.

use super::*;

const TONE_BYTES: usize = 160;
const FAMILY: [u8; 5] = [0xf0, 0x00, 0x01, 0x0c, 0x03];
const DEVICE: u8 = PODXT_LIVE_MEMBER as u8;

/// The 160 payload bytes, including tone name, link fields and reserved bytes.
/// Construct from a PODxt Live dump or a complete, seven-bit tone payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TonePayload([u8; TONE_BYTES]);

impl TonePayload {
    pub fn from_bytes(payload: [u8; TONE_BYTES]) -> Result<Self, String> {
        if payload.iter().any(|byte| *byte > 0x7f) {
            return Err("PODxt Live tone payload contains non-MIDI data bytes.".into());
        }
        Ok(Self(payload))
    }

    pub fn from_dump(tone: &EditBuffer) -> Result<Self, String> {
        if !is_edit_buffer(&tone.raw) || tone.raw[6] != DEVICE || tone.raw.last() != Some(&0xf7) {
            return Err("Expected a complete PODxt Live 03 74 dump.".into());
        }
        let payload = tone.raw[7..7 + TONE_BYTES]
            .try_into()
            .expect("frame length checked");
        Self::from_bytes(payload)
    }

    pub fn as_bytes(&self) -> &[u8; TONE_BYTES] {
        &self.0
    }

    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.0[..16])
            .trim_end_matches([' ', '\0'])
            .to_owned()
    }

    pub(super) fn edit_dump(&self) -> EditBuffer {
        EditBuffer {
            raw: EditFrame(self).bytes(),
        }
    }

    pub(super) fn frame(&self, opcode: u8, address: Option<u16>) -> Vec<u8> {
        let mut bytes = Vec::from(FAMILY);
        bytes.extend([opcode, DEVICE]);
        if let Some(addr) = address {
            bytes.extend([(addr >> 7) as u8, (addr & 0x7f) as u8]);
        }
        bytes.extend(self.0);
        bytes.push(0xf7);
        bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EditFrame<'a>(pub(super) &'a TonePayload);

impl Request for EditFrame<'_> {
    fn bytes(self) -> Vec<u8> {
        self.0.frame(0x74, None)
    }

    fn path(self) -> Path {
        Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        Operation::PodxtEditBufferWrite
    }
}

pub enum WholeToneSource<'a> {
    /// Send the most recent verified read of the playing tone.
    Current,
    /// Send this complete PODxt Live payload, including its link fields.
    Payload(&'a TonePayload),
}

/// Send a complete tone to the playing buffer under the G6 policy, using the
/// snapshot taken by Get as the expected current state. The caller holds the
/// rig lock and checks the automatic identity of the attachment. The saved
/// channel must still match its verified backup before dispatch.
pub fn send_whole_tone<L: Link>(
    session: &mut Session<L>,
    expected: &EditorBuffer,
    source: WholeToneSource<'_>,
    backup: &VerifiedToneBackup,
    cancelled: impl Fn() -> bool,
) -> Result<EditorBuffer, String> {
    use live_edit::{
        check_channel, check_stop, editor_tone, open_editor, read_after_write, still_current,
    };

    for op in [
        Operation::PodxtEditBufferWrite,
        Operation::PodxtKeepEditBufferChanges,
        Operation::PodxtLibraryRead,
    ] {
        if !session.allows(op) {
            return Err("Whole-tone send is closed. No edit was sent.".into());
        }
    }
    if backup.slot() != expected.channel {
        return Err("Verify a backup for the playing channel before sending.".into());
    }
    TonePayload::from_dump(&expected.tone)?;
    open_editor(session, &cancelled)?;
    check_channel(session, expected.channel)?;
    let before = editor_tone(session)?;
    if !still_current(&expected.tone, &before) {
        return Err("The playing tone changed. Get edit buffer again before sending.".into());
    }
    let current;
    let payload = match source {
        WholeToneSource::Current => {
            current = TonePayload::from_dump(&before)?;
            &current
        }
        WholeToneSource::Payload(payload) => payload,
    };
    let candidate = payload.edit_dump();
    let packs = query_packs(session)?;
    for setting in Setting::ALL
        .into_iter()
        .filter(|setting| setting.model_list().is_some())
    {
        let model = LiveControl::Additional(setting);
        setting.check_model(model.read(&before), packs, "original")?;
        setting.check_model(model.read(&candidate), packs, "requested")?;
    }
    check_stop(&cancelled)?;
    backup.verify_slot(session)?;
    check_channel(session, expected.channel)?;
    check_stop(&cancelled)?;
    session
        .send(EditFrame(payload))
        .map_err(|error| format!("{error} Edit-buffer state unknown."))?;
    let tone = read_after_write(session, expected.channel)
        .map_err(|error| format!("{error} Edit-buffer state unknown."))?;
    if candidate.raw != tone.raw {
        return Err("Whole-tone readback differs from the sent tone. Get edit buffer to inspect the result.".into());
    }
    Ok(EditorBuffer {
        channel: expected.channel,
        tone,
        packs,
    })
}

#[cfg(test)]
pub(super) mod tests;
