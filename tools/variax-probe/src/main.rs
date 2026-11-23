//! variax-probe: lean, owner-run, read-only probe built on `line6-devices`.
//!
//!   variax identity        identity request on PODxt interface 1
//!   variax read            identity, session on, two metadata reads, session off
//!   variax listen [SECS]   identity, session on, listen only, session off
//!   podxt read             PODxt identity and current-tone (edit buffer) read
//!
//! Only the requests defined by the `line6-devices` families can be sent, and
//! only those this file's policy allows. Every packet in and out is printed
//! and saved, with each reply, to a new folder under `--out` (default
//! `analysis/captures`). Nothing is overwritten.

#![forbid(unsafe_code)]

use line6_devices::family::{Family, podxt, variax};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::identity::Identity;
use line6_devices::session::{Log, Path, Session};
use line6_devices::transport::{rawmidi, usb};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path as FsPath, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The owner's direct permissions for this tool (2026-11-22). Nothing else.
struct OwnerGrants;

impl GatePolicy for OwnerGrants {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            // G2: Variax 600 identity through the PODxt Live.
            Operation::VariaxIdentity => true,
            // G3: Variax current-preset metadata read (session on/off, two blocks).
            Operation::VariaxRead => true,
            // "test my podxtlive same way": PODxt identity and current-tone read.
            Operation::PodxtIdentity | Operation::PodxtRead => true,
        }
    }
}

const USAGE: &str = "Usage: variax-probe variax identity|read|listen [SECONDS] [--out DIR]
       variax-probe podxt read [--out DIR]
Read-only. Run it yourself; it contacts the connected PODxt Live and Variax.";

/// Printed lines plus saved files, in one new folder.
struct Capture {
    dir: PathBuf,
    lines: Vec<String>,
}

impl Capture {
    fn create(root: &FsPath, name: &str) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
        let dir = root.join(name);
        fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self {
            dir,
            lines: Vec::new(),
        })
    }

    fn file(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.dir.join(name);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    fn finish(mut self, result: &Result<(), String>) -> Result<(), String> {
        if let Err(error) = result {
            self.line(format!("STOPPED: {error}"));
        }
        let mut text = self.lines.join("\n");
        text.push('\n');
        self.file("log.txt", text.as_bytes())?;
        println!("Capture: {}", self.dir.display());
        Ok(())
    }
}

impl Log for Capture {
    fn line(&mut self, text: String) {
        println!("{text}");
        self.lines.push(text);
    }
}

#[derive(Debug, PartialEq)]
enum Command {
    VariaxIdentity,
    VariaxRead,
    VariaxListen(u64),
    PodxtRead,
}

impl Command {
    fn slug(&self) -> &'static str {
        match self {
            Command::VariaxIdentity => "variax-identity",
            Command::VariaxRead => "variax-read",
            Command::VariaxListen(_) => "variax-listen",
            Command::PodxtRead => "podxt-read",
        }
    }

    fn plan(&self) -> &'static str {
        match self {
            Command::VariaxIdentity => {
                "identity (sent once more only if the reply's start is lost)"
            }
            Command::VariaxRead => {
                "identity, Variax session on, metadata blocks 1 and 2, session off"
            }
            Command::VariaxListen(_) => "identity, Variax session on, listen only, session off",
            Command::PodxtRead => {
                "PODxt identity, then the PODxt edit-buffer (current tone) request"
            }
        }
    }
}

fn parse(args: &[String]) -> Result<(Command, PathBuf), String> {
    let mut out = PathBuf::from("analysis/captures");
    let mut words = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--out" {
            out = PathBuf::from(iter.next().ok_or("--out needs a folder")?);
        } else {
            words.push(arg.as_str());
        }
    }
    let max = variax::LISTEN_MAX.as_secs();
    let command = match words.as_slice() {
        ["variax", "identity"] => Command::VariaxIdentity,
        ["variax", "read"] => Command::VariaxRead,
        ["variax", "listen"] => Command::VariaxListen(10),
        ["variax", "listen", seconds] => match seconds.parse::<u64>() {
            Ok(seconds) if (1..=max).contains(&seconds) => Command::VariaxListen(seconds),
            _ => return Err(format!("listen SECONDS must be a whole number 1..={max}")),
        },
        ["podxt", "read"] => Command::PodxtRead,
        _ => return Err(USAGE.into()),
    };
    Ok((command, out))
}

fn describe_identity(reply: &[u8], requests: Option<u8>) -> String {
    let Some(identity) = Identity::parse(reply) else {
        return "Identity: (unparsed)".into();
    };
    let device = Family::from_identity(&identity).map_or("unknown Line 6 device".to_owned(), |f| {
        f.member_name(identity.member).to_owned()
    });
    let requests = requests.map_or(String::new(), |n| format!(", identity requests sent: {n}"));
    format!(
        "Identity: {device} (family {:#06x}, member {:#06x}), version {}{requests}",
        identity.family,
        identity.member,
        identity.version_text()
    )
}

fn variax_lines(metadata: &variax::PresetMetadata) -> Vec<String> {
    let mut lines = vec![format!("Preset name : {}", metadata.name)];
    for (key, value) in &metadata.parameters {
        let shown = match value {
            variax::ParameterValue::Integer(v) if key.ends_with("_model") => format!("{v:#010x}"),
            variax::ParameterValue::Integer(v) => v.to_string(),
            variax::ParameterValue::Boolean(v) => v.to_string(),
            variax::ParameterValue::Float(v) => v.to_string(),
        };
        lines.push(format!("  {key} = {shown}"));
    }
    lines
}

fn run_variax(command: &Command, capture: &mut Capture) -> Result<(), String> {
    let mut link = usb::UsbInterfaceLink::open(usb::VARIAX_VIA_PODXT_LIVE)?;
    let (detached, claimed) = link.state();
    let mut saved: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut summary = Vec::new();
    let (result, cleanup) = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!(
            "interface 1: kernel driver detached={detached}, claimed={claimed}, altsetting 1"
        ));
        let result = match command {
            Command::VariaxIdentity => variax::identify(&mut session).map(|id| {
                summary.push(describe_identity(&id.reply, Some(id.requests)));
                saved.push(("identity.syx", id.reply));
            }),
            Command::VariaxRead => variax::read_current(&mut session).map(|preset| {
                summary.push(describe_identity(
                    &preset.identified.reply,
                    Some(preset.identified.requests),
                ));
                summary.extend(variax_lines(&preset.metadata));
                let [first, second] = preset.replies;
                saved.extend([
                    ("identity.syx", preset.identified.reply),
                    ("metadata-1.syx", first),
                    ("metadata-2.syx", second),
                ]);
            }),
            Command::VariaxListen(seconds) => {
                variax::listen(&mut session, Duration::from_secs(*seconds)).map(|id| {
                    summary.push(describe_identity(&id.reply, Some(id.requests)));
                    saved.push(("identity.syx", id.reply));
                })
            }
            Command::PodxtRead => unreachable!(),
        };
        let cleanup = session.link.finish();
        session.note(match &cleanup {
            Ok(()) => "interface 1 released and given back to the kernel driver".to_owned(),
            Err(error) => format!("CLEANUP FAILED: {error}"),
        });
        (result, cleanup)
    };
    for line in summary {
        capture.line(line);
    }
    for (name, bytes) in &saved {
        capture.file(name, bytes)?;
    }
    result.and(cleanup)
}

fn run_podxt(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::read_current(&mut session)
    };
    let tone = result?;
    capture.line(describe_identity(&tone.identity_reply, None));
    for line in tone.tone.describe() {
        capture.line(line);
    }
    capture.file("identity.syx", &tone.identity_reply)?;
    capture.file("editbuffer.syx", &tone.tone.raw)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut capture = match Capture::create(&out, &format!("probe-{}-{stamp}", command.slug())) {
        Ok(capture) => capture,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    capture.line(format!(
        "variax-probe {} — will send: {}",
        command.slug(),
        command.plan()
    ));
    let result = match command {
        Command::PodxtRead => run_podxt(&mut capture),
        _ => run_variax(&command, &mut capture),
    };
    let saved = capture.finish(&result);
    if let Err(error) = result.and(saved) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn commands_parse_and_unknown_ones_do_not() {
        assert_eq!(parse(&args("variax read")).unwrap().0, Command::VariaxRead);
        assert_eq!(
            parse(&args("variax listen 5")).unwrap().0,
            Command::VariaxListen(5)
        );
        assert_eq!(
            parse(&args("podxt read --out /x")).unwrap(),
            (Command::PodxtRead, PathBuf::from("/x"))
        );
        for bad in [
            "",
            "variax write",
            "podxt store",
            "variax listen 0",
            "variax listen 31",
            "podxt identity",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
    }

    /// Guitar-side encoding of 96 metadata bytes into one 200-byte reply.
    fn encode_reply(bytes: &[u8]) -> Vec<u8> {
        let mut reply = vec![0xf0, 0x00, 0x01, 0x0c, 0x07, 0x04, 0x3c];
        for word in bytes.chunks(3) {
            let value = u32::from_be_bytes([0, word[0], word[1], word[2]]);
            reply.extend((0..6).map(|i| ((value >> (4 * i)) & 15) as u8));
        }
        reply.push(0xf7);
        reply
    }

    #[test]
    fn library_variax_decoder_matches_the_main_crate() {
        for seed in 0u32..64 {
            let mut raw = [0u8; 192];
            raw[..6].copy_from_slice(b"Seed  ");
            let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
            for byte in &mut raw[18..] {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let (first, second) = (encode_reply(&raw[..96]), encode_reply(&raw[96..]));
            let ours = variax::PresetMetadata::from_replies(&first, &second).unwrap();
            let main = variax_workbench::PresetMetadata::from_replies(&first, &second).unwrap();
            assert_eq!(ours.name, main.name);
            assert_eq!(ours.raw, main.raw);
            assert_eq!(
                format!("{:?}", ours.parameters),
                format!("{:?}", main.parameters),
                "seed {seed}"
            );
        }
    }
}
