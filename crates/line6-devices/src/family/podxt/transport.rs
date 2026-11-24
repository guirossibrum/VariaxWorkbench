//! Endpoints for the PODxt family.
use crate::session::Path;
use crate::transport::RawMidiSpec;

impl Path {
    // Keep the existing request and tool path name (#214).
    #[allow(non_upper_case_globals)]
    pub const PodxtMidi: Self = Self::new("PodxtMidi");
}

/// ALSA card id of the PODxt Live (`snd_usb_pod`).
pub const PODXT_LIVE_CARD: &str = "PODxtLive";

pub const PODXT_LIVE_MIDI: RawMidiSpec = RawMidiSpec {
    card_id: PODXT_LIVE_CARD,
    device: 0,
    path: Path::PodxtMidi,
};
