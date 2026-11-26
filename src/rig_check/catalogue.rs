//! The Rig Check step catalogue, mirrored in `docs/rig-check/catalogue.md`.
//!
//! A test checks that every step ID and the catalogue version below appear in
//! that document. Change both together and bump the version.

use crate::hardware_gates::Gate;
use std::fmt;

pub const CATALOGUE_VERSION: &str = "1.4";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepId(pub u8, pub u8);

impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}

impl StepId {
    pub fn parse(text: &str) -> Option<Self> {
        let (a, b) = text.split_once('.')?;
        let id = StepId(a.parse().ok()?, b.parse().ok()?);
        step(id).map(|_| id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Nothing is sent to any device.
    None,
    /// Questions only; nothing on the device changes (preset switches return).
    Read,
    /// The playing sound changes for a moment and is put back. Nothing saved.
    Live,
    /// A stored slot is written (with its own bytes).
    Save,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::None => "None",
            Risk::Read => "Read",
            Risk::Live => "Live",
            Risk::Save => "Save",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// This computer only. No device approval needed.
    Computer,
    /// Sends messages to the PODxt Live or the Variax.
    Device,
    /// Asks the person to act on the rig while Rig Check listens.
    Guided,
    /// Audio/DAW.
    Audio,
}

pub fn asks_for_action(id: StepId) -> bool {
    step(id).is_some_and(|step| step.kind == Kind::Guided)
}

pub struct Step {
    pub id: StepId,
    pub title: &'static str,
    pub kind: Kind,
    pub needs: &'static [StepId],
    pub gates: &'static [Gate],
    pub risk: Risk,
    pub does: &'static str,
    pub you_do: &'static str,
    pub see: &'static str,
    pub hear: &'static str,
    pub evidence: &'static str,
    pub proof: &'static str,
    /// `Some(reason)`: listed but not available in this version.
    pub not_in_this_version: Option<&'static str>,
    /// Live and save steps run only on verified device models and firmware.
    pub verified_devices_only: bool,
}

impl Step {
    pub fn starting_buffer_note(&self) -> Option<&'static str> {
        match (self.id.0, self.id.1) {
            (3, 4 | 5) | (4, 2..=5) | (5, 1 | 3 | 4) | (7, 1) => {
                Some("Rig Check keeps your captured Variax edits and puts them back in step 6.2.")
            }
            (4, 1 | 6 | 7) | (7, 3) => Some(
                "Rig Check puts back any captured PODxt Live edits after the check and verifies the starting sound in step 6.2.",
            ),
            _ => None,
        }
    }

    pub fn gate_labels(&self) -> String {
        if self.gates.is_empty() {
            return "none".into();
        }
        self.gates
            .iter()
            .map(|gate| gate_label(*gate))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Device, guided and audio steps need their own owner approval in real
    /// mode ([`super::approvals`]). Computer steps do not touch a device.
    pub fn needs_device_approval(&self) -> bool {
        self.kind != Kind::Computer
    }
}

pub fn gate_label(gate: Gate) -> &'static str {
    match gate {
        Gate::OsDiscovery => "G1",
        Gate::IdentityRequest => "G2",
        Gate::CurrentSettingsRead | Gate::DeviceCheckCurrentSettingsRead => "G3",
        Gate::AudioCapture => "G4",
        Gate::FullBackupRead | Gate::DeviceCheckBackup => "G5",
        Gate::EditBufferWrite
        | Gate::DeviceCheckLiveTest
        | Gate::KeepLiveEdit
        | Gate::DeviceCheckChannel => "G6",
        Gate::StoreRestore => "G7",
    }
}

const G1: Gate = Gate::OsDiscovery;
const G2: Gate = Gate::IdentityRequest;
const G3: Gate = Gate::CurrentSettingsRead;
const G4: Gate = Gate::AudioCapture;
const G5: Gate = Gate::FullBackupRead;
const G6: Gate = Gate::EditBufferWrite;
const G7: Gate = Gate::StoreRestore;

const AUDIO_LATER: &str = "Not available in this version: opening audio streams needs the owner's separate audio approval (AGENTS.md, #265).";

macro_rules! step {
    (($a:literal, $b:literal), $title:literal, $kind:ident, [$(($na:literal, $nb:literal)),*], [$($gate:ident),*], $risk:ident,
     does: $does:literal, you_do: $you:literal, see: $see:literal, hear: $hear:literal,
     evidence: $evidence:literal, proof: $proof:literal $(, later: $later:expr)? $(, verified_only: $vo:literal)?) => {
        Step {
            id: StepId($a, $b),
            title: $title,
            kind: Kind::$kind,
            needs: &[$(StepId($na, $nb)),*],
            gates: &[$($gate),*],
            risk: Risk::$risk,
            does: $does,
            you_do: $you,
            see: $see,
            hear: $hear,
            evidence: $evidence,
            proof: $proof,
            not_in_this_version: step!(@later $($later)?),
            verified_devices_only: step!(@vo $($vo)?),
        }
    };
    (@later) => { None };
    (@later $later:expr) => { Some($later) };
    (@vo) => { false };
    (@vo $vo:literal) => { $vo };
}

pub static STEPS: &[Step] = &[
    step!((0, 1), "Computer and operating system", Computer, [], [], None,
        does: "Reads the operating system name and version, kernel, processor model and features (for example FMA), and how this copy of Rig Check was built (target, compiler, maths profile).",
        you_do: "Nothing.", see: "A short summary of this computer.", hear: "Nothing.",
        evidence: "OS, kernel, architecture, CPU model and flags, build target, compiler, maths profile.",
        proof: "-"),
    step!((0, 2), "USB devices and drivers on this computer", Computer, [], [G1], None,
        does: "Reads what the operating system already knows about connected USB devices: IDs, port, speed, the driver holding each PODxt Live interface, and permissions. No device is opened.",
        you_do: "Nothing.", see: "The USB device list.", hear: "Nothing.",
        evidence: "Device list (serial numbers hashed), interface drivers, permissions, libusb version.",
        proof: "#239, g1.md"),
    step!((0, 3), "Engine check (no device)", Computer, [], [], None,
        does: "Builds reference guitar models on this computer and compares all 1,092 program words with the expected result. Differences are listed by word, bit and sound part.",
        you_do: "Nothing, or choose a folder with your own reference models.", see: "Results for the reference models and a count of the synthetic cases.", hear: "Nothing.",
        evidence: "Per model: words compared, differences with position, expected/actual hex, XOR mask and sound part.",
        proof: "#238 sweep, synthetic host fixtures"),
    step!((1, 1), "See the PODxt Live on USB", Device, [(0, 2)], [G1], None,
        does: "Looks for exactly one PODxt Live in the USB list and notes its port and speed. USB discovery cannot show the firmware. Step 2.1 checks the PODxt Live firmware and step 3.1 checks the Variax firmware.",
        you_do: "Make sure the PODxt Live is on and connected by USB.", see: "PODxt Live found on port ...", hear: "Nothing.",
        evidence: "Vendor/product ID, port path, speed, address, interface drivers.",
        proof: "#213"),
    step!((1, 2), "Open and give back the guitar interface", Device, [(1, 1), (2, 1)], [G1, G2], Read,
        does: "Opens the PODxt Live's guitar interface (interface 1), closes it again and checks that the operating system gave it back to its driver (snd-usb-variax). If not, it waits and retries once.",
        you_do: "Nothing.", see: "Guitar interface opened and given back.", hear: "Nothing.",
        evidence: "Open/release result, driver binding before and after.",
        proof: "#239, variax-driver-rebind-test-card.md"),
    step!((2, 1), "PODxt Live identity and firmware", Device, [(1, 1)], [G2], Read,
        does: "Asks the PODxt Live who it is. A lost first reply packet is retried once.",
        you_do: "Nothing.", see: "The reported PODxt model and firmware, with verification status.", hear: "Nothing.",
        evidence: "Identity reply in hex, model, firmware, verified or unverified device.",
        proof: "#213, automatic-identity-owner-test-card.md"),
    step!((2, 2), "PODxt Live current tone", Device, [(2, 1)], [G3], Read,
        does: "Reads the tone you are playing (168 bytes). This is also the starting tone the rig must return to.",
        you_do: "Nothing.", see: "The tone name.", hear: "Nothing changes.",
        evidence: "Dump in hex, SHA-256, tone name, amp and effects.",
        proof: "#220, podxt-live-reads-test-card.md"),
    step!((2, 3), "PODxt Live one stored channel", Device, [(2, 2)], [G5], Read,
        does: "Reads one saved channel by memory location. The tone you are playing does not change.",
        you_do: "Nothing.", see: "The channel name.", hear: "Nothing changes.",
        evidence: "Dump in hex, SHA-256, shape check.", proof: "#220"),
    step!((2, 4), "PODxt Live one bank (A-D)", Device, [(2, 3)], [G5], Read,
        does: "Reads the four saved channels A-D of one bank.",
        you_do: "Nothing.", see: "Four channel names.", hear: "Nothing changes.",
        evidence: "Four dumps, SHA-256 each.", proof: "#220"),
    step!((2, 5), "PODxt Live full library backup", Device, [(2, 2)], [G5], Read,
        does: "Reads all 128 tones, 107 amp setups and 64 FX setups (299 entries) into the evidence folder, compares the reads from 2.3 and 2.4 with it, and reads the playing tone again to show it did not change.",
        you_do: "Nothing. Takes about a minute.", see: "The number of entries read out of 299.", hear: "Nothing changes.",
        evidence: "Entry count, SHA-256 per entry and manifest, before/after current tone.",
        proof: "#220, #222"),
    step!((2, 6), "PODxt Live editor Get, packs and globals", Device, [(2, 2)], [G3], Read,
        does: "Uses the editor Get procedure and reads installed packs and global values.",
        you_do: "Leave the footswitches and controls alone.", see: "The current channel, tone name and returned globals.", hear: "Your current tone.",
        evidence: "Channel, tone and global replies.", proof: "#246", verified_only: true),
    step!((2, 7), "Compatibility check for other firmware versions", Computer, [(2, 1)], [], Read,
        does: "Compares the PODxt Live identity already read in 2.1 with the tested version 3.01. The regular PODxt checks require 3.01.",
        you_do: "Nothing.", see: "The reported version and the tested version.", hear: "Nothing changes.",
        evidence: "Identity version field, expected and actual bytes, XOR and bit counts.", proof: "#277"),
    step!((3, 1), "Variax identity, model and firmware", Device, [(1, 2), (2, 1)], [G2], Read,
        does: "Asks the guitar who it is, through the PODxt Live. A lost first packet or a silent first try is retried once.",
        you_do: "Nothing.", see: "The reported Variax model family and firmware, with verification status.", hear: "Nothing.",
        evidence: "Identity reply, model (300/500/600/700), firmware, VDI link answering, verified or unverified.",
        proof: "#213, G2 owner run (build 12)"),
    step!((3, 2), "Variax current settings", Device, [(3, 1), (2, 1)], [G3], Read,
        does: "Reads the 192 bytes of settings of the preset you are playing (name, pickups, tuning, controls).",
        you_do: "Nothing.", see: "The preset name.", hear: "Nothing.",
        evidence: "Both replies in hex, decoded settings.", proof: "#176, #225"),
    step!((3, 3), "Variax full current program (35 reads)", Device, [(3, 1), (2, 1)], [G3], Read,
        does: "Reads all 1,092 words of the sound the guitar is playing in 35 reads of 32 words (the PODxt Live drops larger replies). A damaged reply is retried up to twice, 100 ms apart.",
        you_do: "Nothing.", see: "The number of chunks read out of 35 and the checksum result.", hear: "Nothing.",
        evidence: "Per read: request, reply, retries, lost bytes; SHA-256 and checksum word of the program.",
        proof: "#234, reso5-full-read-test-card.md"),
    step!((3, 4), "Variax all 60 presets (backup), and back", Device, [(3, 3), (2, 1)], [G5], Read,
        does: "Reads all 60 stored presets (1,092 words each) into the evidence folder, finds the one you started on, switches back to it and checks all words. Rig Check keeps the captured starting buffer for 6.2. If the bytes match no slot or multiple slots, it asks you to move the selector away and back so it can identify the original position.",
        you_do: "Nothing. Takes about 4 minutes.", see: "The number of presets read out of 60 and whether the guitar returned to its starting preset.", hear: "The sound changes 60 times, then comes back.",
        evidence: "60 SHA-256 values, manifest, the starting preset, return check.", proof: "#236, variax-backup-test-card.md"),
    step!((3, 5), "Editor: Read unmodified from device", Device, [(3, 4), (2, 1)], [G5], Read,
        does: "Tests the same read the Variax editor uses for Read unmodified from device and Sync revert. It selects one other stored preset, compares all 1,092 words with the backup from 3.4, then returns to your starting preset and verifies it.",
        you_do: "Nothing.", see: "Which preset was read, whether it matches the backup, and whether the guitar returned to its starting preset.", hear: "The sound changes for a few seconds and comes back.",
        evidence: "Program SHA-256, backup comparison, return check.", proof: "#238, variax-any-selection-test-card.md"),
    step!((3, 7), "Compatibility check for other firmware versions", Device, [(2, 1)], [G1, G2, G3], Read,
        does: "Records identity, current settings and 35 current-program replies. The guitar stays on the same preset. If the PODxt Live firmware is unverified, this is the only Variax step offered because the relay was verified on 3.01.",
        you_do: "Keep the guitar connected and leave the selector and knobs alone.",
        see: "The reported firmware and the evidence collected for comparison with 3.10.",
        hear: "The current sound should stay the same.",
        evidence: "Raw replies, message sizes, and version and checksum differences.",
        proof: "#271"),
    step!((4, 1), "PODxt Live: change one control, read back, undo", Device, [(2, 2), (2, 5), (2, 6)], [G6], Live,
        does: "Switches the stomp of the tone you are playing, reads the tone back, switches it back and checks the whole tone matches the start. Also checks Drive and tempo through the editor and undoes both. Nothing is saved.",
        you_do: "Nothing.", see: "The stomp setting read back and whether the starting tone was restored.", hear: "The stomp switches for a moment.",
        evidence: "Before, after and restored dumps, differences.", proof: "#221, podxt-live-edit-test-card.md",
        verified_only: true),
    step!((4, 2), "Variax: send the current preset's own bytes to the working buffer", Device, [(3, 3), (2, 1)], [G6], Live,
        does: "Sends the 1,092 words read in 3.3 back to the guitar's working sound and reads them back. Nothing is saved and no save command is sent.",
        you_do: "Nothing.", see: "Sent and read back: identical.", hear: "No change (a very short drop-out is possible).",
        evidence: "Upload SHA-256, read-back SHA-256, word differences.", proof: "#176, reso5-test-card.md",
        verified_only: true),
    step!((4, 3), "Variax: listen to a test model, then return", Device, [(4, 2), (3, 4), (2, 1)], [G6], Live,
        does: "Plays a test model in the working buffer, asks what you hear against the sound you just played, then switches back to the starting preset and checks all words. Nothing is saved.",
        you_do: "Play the original first, then play the test model and answer.", see: "Your answers for each test model and whether the guitar returned to its starting preset.", hear: "The test model, then your own sound again.",
        evidence: "Upload and read-back SHA-256, answers next to the byte checks.", proof: "#238 listen-list, variax-listen-list-test-card.md",
        verified_only: true),
    step!((4, 4), "Variax: lower output trim and return", Device, [(4, 2), (3, 4), (2, 1)], [G6], Live,
        does: "Lowers the starting preset's output trim by 6 dB with one difference message, reads the working buffer and reloads your starting preset.",
        you_do: "Play the same short passage before and after the prompt. Leave the knobs alone.", see: "The quieter program read back and the original verified.", hear: "The same sound, quieter, then the starting level again.",
        evidence: "Full read-back, including the retained checksum, and verified return.", proof: "#245", verified_only: true),
    step!((4, 5), "Variax: change alternate tuning and return", Device, [(4, 2), (3, 4), (2, 1)], [G6], Live,
        does: "Sends the Workbench muted transition and then the unmuted alternate-tuning program. Reads it back and reloads your starting preset.",
        you_do: "Play the low E string before and after the prompt. Leave the knobs alone.", see: "The test tuning read back and the original verified.", hear: "A short mute, then the low string at a different pitch, then your original tuning.",
        evidence: "Transition and full upload, read-back and verified return.", proof: "#245", verified_only: true),
    step!((4, 6), "PODxt Live: send the playing tone's own bytes", Device, [(2, 5), (2, 6)], [G6], Live,
        does: "Verifies the saved channel against the backup, sends the playing tone to the edit buffer and checks the full reply.",
        you_do: "Leave the footswitches and controls alone.", see: "The full tone verified on your original channel.", hear: "Your original sound throughout.",
        evidence: "Whole-tone write and exact read-back.", proof: "#246", verified_only: true),
    step!((4, 7), "PODxt Live: recall another channel and return", Device, [(2, 5), (2, 6)], [G6], Live,
        does: "Recalls the next saved channel using the editor procedure, verifies it, then recalls and verifies the original and puts back captured unsaved edits.",
        you_do: "Leave the footswitches and controls alone.", see: "The recalled channel and the verified return.", hear: "The other channel briefly, then your original tone.",
        evidence: "Both program changes and their read-backs.", proof: "#246", verified_only: true),
    step!((4, 8), "PODxt Live: tuner on, read, then off", Device, [(2, 6)], [G3], Live,
        does: "Opens the POD tuner briefly, polls note and tuning deviation, then switches the tuner off.",
        you_do: "Pluck one open string when asked.", see: "Whether tuner values arrived and the tuner was switched off.", hear: "The tuner may mute the sound briefly, then your current tone returns.",
        evidence: "Tuner mode and note/deviation messages.", proof: "#246", verified_only: true),
    step!((5, 1), "Variax: save a preset's own bytes back to its slot", Device, [(3, 4), (4, 2), (2, 1)], [G7], Save,
        does: "Checks the slot still matches the backup, sends its own program, waits 100 ms, sends one save command, waits up to 15 s for one confirmation, then reads the slot back and compares it with the backup. Never repeats the save.",
        you_do: "Read the save notice and type the confirmation.", see: "Saved and verified.", hear: "A short drop-out, then the same sound.",
        evidence: "Pre-check, store messages, confirmation time, read-back comparison.", proof: "#177, #238 4c, reso5-store-test-card.md",
        verified_only: true),
    step!((5, 2), "PODxt Live: save a channel's own bytes back to it", Device, [(2, 5), (4, 1)], [G7], Save,
        does: "Reads the channel and requires all 160 tone bytes to match the backup, stores the same bytes, waits for the confirmation and reads the channel back.",
        you_do: "Read the save notice and type the confirmation.", see: "Saved and verified.", hear: "Nothing changes.",
        evidence: "Pre-check, store messages, receipt, read-back.", proof: "#222 stage B (owner run pending), podxt-store-test-card.md",
        verified_only: true),
    step!((5, 3), "Variax: save a quieter preset and restore it", Device, [(5, 1), (4, 4), (2, 1)], [G7], Save,
        does: "Saves the quieter variant to the starting slot, reads it back, then restores and verifies the backup.",
        you_do: "Read the save notice and type the confirmation.", see: "The quieter save and the verified restoration.", hear: "The sound becomes quieter and then returns to the original level.",
        evidence: "Both store receipts, read-backs and return.", proof: "#238 4b/4c", verified_only: true),
    step!((5, 4), "Variax: Sync two presets' own bytes", Device, [(5, 1), (3, 4), (2, 1)], [G7], Save,
        does: "Checks two saved presets against the backup, saves each one's own program once, then reloads and verifies your starting preset.",
        you_do: "Read the save notice for both slots and type the confirmation.", see: "Each slot verified, then the verified return.", hear: "The two saved sounds as they are checked, then your original sound.",
        evidence: "Both stored-image checks and the return.", proof: "#205", verified_only: true),
    step!((6, 1), "VDI cable unplug and replug noticed", Guided, [(3, 1), (2, 1)], [G2], Read,
        does: "Asks you to unplug the VDI cable, waits until the guitar stops answering, asks you to plug it back, and reads the identity again. Nothing is sent while you handle the cable.",
        you_do: "Unplug the VDI cable, wait for the screen, plug it back in.", see: "The observed disconnect and reconnect, and the guitar firmware after reconnecting.", hear: "The guitar goes silent while unplugged.",
        evidence: "Times of loss and return, identity after.", proof: "#235", verified_only: true),
    step!((6, 2), "Rig back to its starting state, driver given back", Device, [], [G1, G3, G6], Read,
        does: "Reads the PODxt Live tone and the Variax program again and compares them with the start. If the channel or preset differs it switches back and checks again. It restores captured unsaved edits to the working buffers, then gives the guitar interface back to the operating system and checks the driver.",
        you_do: "Nothing.", see: "What matched the starting state, and the guitar driver status if checked.", hear: "Your starting sound.",
        evidence: "Start and end SHA-256, any switch-back, driver binding.", proof: "#239, #238"),
    step!((7, 1), "Turn the Variax selector to a named preset", Guided, [(3, 4), (2, 1)], [G3], Read,
        does: "Opens a listening window; nothing is sent while you turn the selector. When the guitar reports the new preset, Rig Check checks it against the backup and asks you to turn back.",
        you_do: "Turn the selector to the named preset, then back.", see: "The preset the guitar reported and the return to the starting preset.", hear: "The named preset, then your start sound.",
        evidence: "Program change seen, preset matched, return seen.", proof: "#202 Follow, #238", verified_only: true),
    step!((7, 2), "Sweep the Variax volume and tone knobs", Guided, [(3, 1), (2, 1)], [G3], Read,
        does: "Opens a listening window and watches the volume (CC 7) and tone (CC 79) knobs.",
        you_do: "Turn the volume knob down and up, then the tone knob down and up. Leave them where they were.", see: "Volume and tone ranges.", hear: "The volume and tone change while you turn.",
        evidence: "Lowest and highest value seen per knob.", proof: "live-control-midi (CC 7, CC 79)", verified_only: true),
    step!((7, 3), "Switch the PODxt Live to a named channel", Guided, [(2, 5)], [G3, G6], Read,
        does: "Opens a listening window and waits for the channel change, checks the new tone against the library backup, then waits for you to switch back and restores any captured unsaved PODxt edits.",
        you_do: "Press the footswitch for the named channel, then switch back.", see: "The channel and tone the PODxt Live reported and the return to the starting channel.", hear: "The named channel, then your start tone.",
        evidence: "Program change seen, tone matched, return seen.", proof: "#220", verified_only: true),
    step!((7, 4), "Move the PODxt Live pedal", Guided, [(2, 1)], [G3], Read,
        does: "Opens a listening window and watches the pedal (CC 7, or CC 4 in wah mode).",
        you_do: "Rock the pedal from heel to toe and back, then leave it where it was.", see: "Pedal range.", hear: "The volume or wah follows the pedal.",
        evidence: "Lowest and highest value seen.", proof: "#220", verified_only: true),
    step!((7, 5), "Turn the PODxt Live Drive knob", Guided, [(2, 2)], [G3], Read,
        does: "Watches the decoded Drive knob change and reads the value from the playing tone. Then it checks that the original value is back.",
        you_do: "Turn Drive to a different setting. Then return it to the starting value shown on screen.",
        see: "The new Drive value and the original value verified after the return.",
        hear: "The drive changes when you turn the knob, then returns to its starting setting.",
        evidence: "Decoded Drive value and both tone read-backs.", proof: "#246", verified_only: true),
    step!((8, 1), "Audio device and formats (no sound)", Computer, [(0, 1)], [], None,
        does: "Lists what the operating system says about the PODxt Live's audio device: channels, sample rates, formats, the audio system and the default devices. No audio stream is opened.",
        you_do: "Nothing.", see: "The audio device listing.", hear: "Nothing.",
        evidence: "Audio system, PODxt Live device, channels, rates, formats, defaults.", proof: "#264"),
    step!((8, 2), "Play a test tone", Audio, [(8, 1)], [G4], Live,
        does: "Plays a short test tone through the PODxt Live outputs and asks whether you heard it.",
        you_do: "-", see: "-", hear: "-", evidence: "-", proof: "#265", later: AUDIO_LATER),
    step!((8, 3), "Record 10 seconds while you strum", Audio, [(8, 1)], [G4], None,
        does: "Records 10 s from the PODxt Live with a level meter and reports levels, clipping, drop-outs and the real sample rate.",
        you_do: "-", see: "-", hear: "-", evidence: "-", proof: "#265", later: AUDIO_LATER),
    step!((8, 4), "Measure latency", Audio, [(8, 2)], [G4], Live,
        does: "Measures the round trip of the test tone, or asks you to tap along.",
        you_do: "-", see: "-", hear: "-", evidence: "-", proof: "#265", later: AUDIO_LATER),
    step!((8, 5), "Record each test model", Audio, [(4, 3), (8, 3)], [G4], Live,
        does: "Records a strum for each test model from 4.3 into the evidence folder.",
        you_do: "-", see: "-", hear: "-", evidence: "-", proof: "#265", later: AUDIO_LATER),
];

/// Reads first, then guided actions, live changes, saves, recovery, audio,
/// and the rig guard last.
pub const RUN_ORDER: &[StepId] = &[
    StepId(0, 1),
    StepId(0, 2),
    StepId(0, 3),
    StepId(1, 1),
    StepId(2, 1),
    PODXT_COMPATIBILITY,
    StepId(1, 2),
    StepId(2, 2),
    StepId(2, 3),
    StepId(2, 4),
    StepId(2, 5),
    StepId(2, 6),
    StepId(3, 1),
    VARIAX_EVIDENCE,
    StepId(3, 2),
    StepId(3, 3),
    StepId(3, 4),
    StepId(3, 5),
    StepId(6, 1),
    StepId(7, 1),
    StepId(7, 2),
    StepId(7, 3),
    StepId(7, 4),
    StepId(7, 5),
    StepId(8, 1),
    StepId(4, 1),
    StepId(4, 2),
    StepId(4, 3),
    StepId(4, 4),
    StepId(4, 5),
    StepId(4, 6),
    StepId(4, 7),
    StepId(4, 8),
    StepId(5, 1),
    StepId(5, 2),
    StepId(5, 3),
    StepId(5, 4),
    StepId(8, 2),
    StepId(8, 3),
    StepId(8, 4),
    StepId(8, 5),
    StepId(6, 2),
];

/// Variax-directed checks, excluding the mixed final cleanup step.
pub const fn is_variax_step(id: StepId) -> bool {
    matches!(
        (id.0, id.1),
        (1, 2) | (3, _) | (4, 2..=5) | (5, 1 | 3 | 4) | (6, 1) | (7, 1 | 2)
    )
}

pub const PODXT_COMPATIBILITY: StepId = StepId(2, 7);

/// Evidence branch for an unverified Variax 300/600 or PODxt Live relay.
pub const VARIAX_EVIDENCE: StepId = StepId(3, 7);

/// The step that always closes a run, including after Stop.
pub const RIG_GUARD: StepId = StepId(6, 2);

pub fn step(id: StepId) -> Option<&'static Step> {
    STEPS.iter().find(|step| step.id == id)
}

pub const MAC_START_NOTICE: &str = "Before you start. Rig Check can check this Mac and run the offline engine comparison. The Mac connection isn't available in this version yet. The audio check lists devices and opens no stream. Each check waits for you to choose Run.\n\nRig Check writes a text report on this computer. It does not use the internet. Your user name, computer name and serial numbers are removed from the report.";

pub const START_NOTICE: &str = "Before you start. Rig Check tests your PODxt Live and Variax with the same code the editor uses. Each step tells you what it will do and waits for you. Nothing runs until you choose Run.\n\nMost steps only ask the devices questions. Some steps briefly change the sound you hear and put it back. Saving steps come later and have their own warning.\n\nYour PODxt Live and Variax are old and cannot be replaced. Keep both powered and connected until Rig Check says it has finished. Do not touch the selector, knobs or footswitches unless a step asks you to.\n\nRig Check writes a text report on this computer. It does not use the internet. Your user name, computer name and serial numbers are removed from the report.";

/// Only the run-start notices can be reopened from the first card.
pub fn accepted_start_notice(notice: &str) -> Option<&'static str> {
    [START_NOTICE, MAC_START_NOTICE]
        .into_iter()
        .find(|text| *text == notice)
}

/// The second notice, before any save step.
pub struct SaveNotice {
    pub device: &'static str,
    pub slot: String,
    pub backup_step: StepId,
    pub backup_file: String,
    pub backup_sha256: String,
    pub confirmation: String,
}

impl SaveNotice {
    pub fn text(&self) -> String {
        format!(
            "This step saves to your {device}. It writes slot {slot} with the bytes it already holds, so the sound should not change. The backup taken in step {step} protects it: {file} (SHA-256 {sha}).\n\nIf the power fails or a cable is pulled during the save, that slot can stop working until it is saved again from the backup. Rig Check never repeats a save by itself. If the device does not confirm the save, keep everything powered and untouched for 2 minutes and follow the steps on screen.\n\nType {confirmation} to save, or choose Skip.",
            device = self.device,
            slot = self.slot,
            step = self.backup_step,
            file = self.backup_file,
            sha = self.backup_sha256.chars().take(12).collect::<String>(),
            confirmation = self.confirmation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const DOC: &str = include_str!("../../docs/rig-check/catalogue.md");

    #[test]
    fn run_order_lists_every_step_once_and_after_its_needs() {
        let ids: BTreeSet<_> = STEPS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), STEPS.len(), "duplicate step ID");
        let order: BTreeSet<_> = RUN_ORDER.iter().copied().collect();
        assert_eq!(order, ids);
        assert_eq!(RUN_ORDER.len(), STEPS.len());
        assert_eq!(*RUN_ORDER.last().unwrap(), RIG_GUARD);
        for (position, id) in RUN_ORDER.iter().enumerate() {
            for need in step(*id).unwrap().needs {
                let at = RUN_ORDER.iter().position(|x| x == need).unwrap();
                assert!(at < position, "{id} runs before its need {need}");
            }
        }
    }

    #[test]
    fn the_document_lists_every_step_and_the_version() {
        assert!(DOC.contains(&format!("Catalogue version: {CATALOGUE_VERSION}")));
        for step in STEPS {
            assert!(
                DOC.contains(&format!("| {} | ", step.id)),
                "{} missing from the step table",
                step.id
            );
        }
    }

    #[test]
    fn every_catalogue_area_is_present() {
        for area in 0..=8 {
            assert!(STEPS.iter().any(|s| s.id.0 == area), "area {area}");
        }
        assert!(StepId::parse("0.3").is_some());
        assert!(StepId::parse("9.9").is_none());
    }

    #[test]
    fn save_notice_names_the_slot_and_backup() {
        let notice = SaveNotice {
            device: "Variax",
            slot: "Spank 1".into(),
            backup_step: StepId(3, 4),
            backup_file: "14-Spank-1.bin".into(),
            backup_sha256: "abcd".into(),
            confirmation: "STORE VARIAX Spank 1".into(),
        }
        .text();
        for part in ["Spank 1", "3.4", "14-Spank-1.bin", "abcd", "never repeats"] {
            assert!(notice.contains(part), "{part}");
        }
    }
}
