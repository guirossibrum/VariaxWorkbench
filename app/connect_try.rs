//! Small window for the one name question. The usual Workbench window does not
//! use this. Nothing here sends a settings read, a MIDI enable, or interface 0
//! traffic, and nothing asks for a password.
#![forbid(unsafe_code)]

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

const IDENTITY: &[u8] = &[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];
const READ_BYTES: usize = 16;
const WRITE_WAIT: Duration = Duration::from_millis(1000);
const CHUNK_GAP: Duration = Duration::from_millis(5);
const READ_WAIT: Duration = Duration::from_millis(100);
const LISTEN_FOR: Duration = Duration::from_secs(3);

const USAGE: &str =
    "Usage: variax-connect-try [--display auto|x11|wayland]\n       variax-connect-try --version";

fn main() -> eframe::Result {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["--version"] {
        println!("Connection try");
        return Ok(());
    }
    let display = match display_choice(&arguments) {
        Ok(display) => display,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id("variax-connect-try")
            .with_inner_size([720.0, 520.0])
            .with_min_inner_size([480.0, 360.0])
            .with_title("Connection try"),
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    let options = {
        let mut options = options;
        let backend = display;
        options.event_loop_builder = Some(Box::new(move |builder| match backend.as_str() {
            "x11" => {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
            "wayland" => {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                builder.with_wayland();
            }
            _ => {}
        }));
        options
    };
    #[cfg(not(target_os = "linux"))]
    let _ = display;
    eframe::run_native(
        "Connection try",
        options,
        Box::new(|_cc| Ok(Box::new(ConnectTry::default()))),
    )
}

fn display_choice(arguments: &[String]) -> Result<String, String> {
    match arguments {
        [] => Ok("auto".into()),
        [flag, value]
            if flag == "--display" && matches!(value.as_str(), "auto" | "x11" | "wayland") =>
        {
            Ok(value.clone())
        }
        _ => Err("That argument is not used by Connection try.".into()),
    }
}

struct ConnectTry {
    report: String,
    running: bool,
    incoming: Option<Receiver<String>>,
}

impl Default for ConnectTry {
    fn default() -> Self {
        Self {
            report: "Nothing has been asked yet.".into(),
            running: false,
            incoming: None,
        }
    }
}

impl eframe::App for ConnectTry {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if let Some(incoming) = &self.incoming {
            match incoming.try_recv() {
                Ok(report) => {
                    self.report = report;
                    self.running = false;
                    self.incoming = None;
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                Err(TryRecvError::Disconnected) => {
                    self.report = "Connection try\nThe try stopped before it could report.".into();
                    self.running = false;
                    self.incoming = None;
                }
            }
        }
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connection try");
            ui.label(
                "This only asks the guitar its name, then puts the driver back. It does not \
                 change the guitar. There is no password. Later checks stay off.",
            );
            ui.add_space(8.0);
            let button = eframe::egui::Button::new(if self.running {
                "Asking…"
            } else {
                "Ask the guitar its name"
            });
            if ui.add_enabled(!self.running, button).clicked() {
                self.running = true;
                self.report = "Asking now. Leave this window open.".into();
                let (sender, incoming) = mpsc::channel();
                self.incoming = Some(incoming);
                std::thread::spawn(move || {
                    let _ = sender.send(try_identity());
                });
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            ui.add_space(8.0);
            ui.add(
                eframe::egui::TextEdit::multiline(&mut self.report)
                    .desired_rows(14)
                    .desired_width(f32::INFINITY)
                    .font(eframe::egui::TextStyle::Monospace),
            );
        });
    }
}

fn try_identity() -> String {
    format!("Connection try\n{}", ask_once())
}

fn ask_once() -> String {
    let device = match find_one() {
        Ok(device) => device,
        Err(text) => return text,
    };
    let handle = match device.open() {
        Ok(handle) => handle,
        Err(error) => {
            let text = error.to_string();
            if text.contains("Access denied") {
                return "Linux did not allow this window to open the PODxt Live. The question \
                        was not sent, and the guitar driver was not detached."
                    .into();
            }
            return format!(
                "The PODxt Live could not be opened, so the question was not sent. {text}"
            );
        }
    };
    let mut link = Link {
        handle,
        detached: false,
        claimed: false,
        closed: false,
    };
    let body = (|| {
        prepare(&mut link)?;
        ask_and_listen(&mut link)
    })();
    let cleanup = link.finish();
    match body {
        Ok(text) => join_report(text, cleanup),
        Err(text) => join_report(text, cleanup),
    }
}

fn join_report(body: String, cleanup: String) -> String {
    if cleanup.is_empty() {
        body
    } else {
        format!("{body}\n{cleanup}")
    }
}

fn find_one() -> Result<rusb::Device<rusb::GlobalContext>, String> {
    let list = rusb::devices().map_err(|error| {
        format!("The PODxt Live could not be looked up, so the question was not sent. {error}")
    })?;
    let mut found = Vec::new();
    for device in list.iter() {
        let descriptor = device.device_descriptor().map_err(|error| {
            format!("A USB device could not be read, so the question was not sent. {error}")
        })?;
        if descriptor.vendor_id() == 0x0e41 && descriptor.product_id() == 0x4650 {
            found.push(device);
        }
    }
    match found.len() {
        0 => Err("The PODxt Live was not found. Nothing was sent.".into()),
        1 => Ok(found.remove(0)),
        _ => Err("More than one PODxt Live was found. Nothing was sent.".into()),
    }
}

struct Link {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    detached: bool,
    claimed: bool,
    closed: bool,
}

impl Link {
    fn finish(&mut self) -> String {
        if self.closed {
            return String::new();
        }
        self.closed = true;
        let mut lines = Vec::new();
        if self.claimed {
            match self.handle.release_interface(1) {
                Ok(()) => self.claimed = false,
                Err(error) => {
                    lines.push(format!(
                        "The guitar interface could not be released. {error}"
                    ));
                }
            }
        }
        if self.detached {
            match self.handle.attach_kernel_driver(1) {
                Ok(()) => {
                    self.detached = false;
                    lines.push("The guitar driver is back.".into());
                }
                Err(error) => {
                    lines.push(format!("The guitar driver could not be put back. {error}"));
                }
            }
        }
        lines.join("\n")
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn prepare(link: &mut Link) -> Result<(), String> {
    match link.handle.kernel_driver_active(1) {
        Ok(true) => {
            link.handle.detach_kernel_driver(1).map_err(|error| {
                format!(
                    "The guitar driver could not be moved aside, so the question was not sent. {error}"
                )
            })?;
            link.detached = true;
        }
        Ok(false) => {}
        Err(error) => {
            return Err(format!(
                "The guitar driver could not be checked, so the question was not sent. {error}"
            ));
        }
    }
    link.handle.claim_interface(1).map_err(|error| {
        format!("The guitar interface could not be claimed, so the question was not sent. {error}")
    })?;
    link.claimed = true;
    link.handle.set_alternate_setting(1, 1).map_err(|error| {
        format!(
            "The guitar interface could not be made ready, so the question was not sent. {error}"
        )
    })?;
    Ok(())
}

fn ask_and_listen(link: &mut Link) -> Result<String, String> {
    let sent = link
        .handle
        .write_interrupt(0x05, IDENTITY, WRITE_WAIT)
        .map_err(|error| format!("The identity question was not accepted. {error}"))?;
    if sent != IDENTITY.len() {
        return Err("The identity question was not accepted.".into());
    }
    std::thread::sleep(CHUNK_GAP);
    let mut lines = vec![format!("Question sent: {}", hex_bytes(IDENTITY))];
    let mut assembler = Assembler::default();
    let deadline = Instant::now() + LISTEN_FOR;
    let mut heard_any = false;
    while Instant::now() < deadline {
        let mut packet = [0u8; READ_BYTES];
        match link.handle.read_interrupt(0x86, &mut packet, READ_WAIT) {
            Ok(length) => {
                heard_any = true;
                let piece = &packet[..length];
                lines.push(format!("Heard: {}", hex_bytes(piece)));
                for message in assembler.push(piece)? {
                    lines.push(format!("Complete message: {}", hex_bytes(&message)));
                    if is_identity(&message) {
                        lines.push(describe_identity(&message));
                        return Ok(lines.join("\n"));
                    }
                }
            }
            Err(rusb::Error::Timeout) => {}
            Err(error) => {
                return Err(format!("The identity answer could not be read. {error}"));
            }
        }
    }
    lines.push(if heard_any {
        "Packets arrived, but no complete name answer arrived in time. Stopped.".into()
    } else {
        "No identity answer arrived in time. Stopped.".into()
    });
    Ok(lines.join("\n"))
}

fn hex_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".into();
    }
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_identity(message: &[u8]) -> bool {
    message.starts_with(&[0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c])
}

fn describe_identity(message: &[u8]) -> String {
    if message.len() >= 16 && message[8..10] == [0x07, 0x00] {
        let version = std::str::from_utf8(message.get(12..16).unwrap_or_default()).unwrap_or("");
        if message.get(10..12) == Some(&[0x04, 0x00]) && version == "0310" {
            return "The guitar answered. It matches a Variax 300 or 600 at version 0310.".into();
        }
        return format!(
            "The guitar answered. It is in the Variax family. Version text: {version}."
        );
    }
    "The guitar answered, but the name was not the expected Variax family.".into()
}

#[derive(Default)]
struct Assembler {
    buffer: Vec<u8>,
}

impl Assembler {
    fn push(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut done = Vec::new();
        for &byte in data {
            if byte >= 0xf8 {
                continue;
            }
            if byte == 0xf0 {
                self.buffer.clear();
            }
            if byte == 0xf0 || !self.buffer.is_empty() {
                self.buffer.push(byte);
            }
            if self.buffer.len() > 8192 {
                return Err("The answer was unexpectedly large. Stopped.".into());
            }
            if byte == 0xf7 && !self.buffer.is_empty() {
                done.push(std::mem::take(&mut self.buffer));
            }
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_name_answer_joins_across_two_pieces() {
        let mut assembler = Assembler::default();
        let first = assembler
            .push(&[
                0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x04, 0x00, 0x30, 0x33,
                0x31, 0x30,
            ])
            .unwrap();
        assert!(first.is_empty());
        let second = assembler.push(&[0xf7]).unwrap();
        assert_eq!(second.len(), 1);
        assert!(is_identity(&second[0]));
        assert_eq!(
            describe_identity(&second[0]),
            "The guitar answered. It matches a Variax 300 or 600 at version 0310."
        );
    }

    #[test]
    fn realtime_bytes_and_bytes_before_the_start_are_ignored() {
        let mut assembler = Assembler::default();
        let messages = assembler
            .push(&[
                0x11, 0xf8, 0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0xf8, 0x07, 0x00, 0x04,
                0x00, 0x30, 0x33, 0x31, 0x30, 0xf7,
            ])
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert!(is_identity(&messages[0]));
        assert_eq!(messages[0][8..10], [0x07, 0x00]);
    }

    #[test]
    fn a_new_start_drops_the_unfinished_message() {
        let mut assembler = Assembler::default();
        let messages = assembler
            .push(&[0xf0, 0x01, 0xf0, 0x7e, 0x7f, 0xf7])
            .unwrap();
        assert_eq!(messages, vec![vec![0xf0, 0x7e, 0x7f, 0xf7]]);
    }

    #[test]
    fn an_end_with_no_start_is_ignored() {
        let mut assembler = Assembler::default();
        assert!(assembler.push(&[0xf7, 0x01]).unwrap().is_empty());
    }

    #[test]
    fn a_different_message_is_not_the_name_answer() {
        assert!(!is_identity(&[0xf0, 0x00, 0x01, 0x0c, 0xf7]));
    }

    #[test]
    fn the_only_question_is_the_six_byte_name_request() {
        assert_eq!(IDENTITY, &[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7]);
    }

    #[test]
    fn an_overlong_message_stops() {
        let mut assembler = Assembler::default();
        let mut data = vec![0xf0];
        data.extend(std::iter::repeat_n(0x01, 8192));
        assert!(assembler.push(&data).is_err());
    }
}
