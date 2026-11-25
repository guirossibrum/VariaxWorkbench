use std::io::{self, Write};

const ATTEMPTS: usize = 3;
const DISPLAY_LIMIT: usize = 256;

enum Key {
    Byte(u8),
    Idle,
    End,
}

trait Input {
    fn discard_pending(&mut self, stopping: &mut dyn FnMut() -> bool) -> io::Result<()>;
    fn next(&mut self) -> io::Result<Key>;
}

fn confirm_with(
    input: &mut impl Input,
    output: &mut impl Write,
    phrase: &str,
    mut stopping: impl FnMut() -> bool,
) -> io::Result<()> {
    for attempt in 1..=ATTEMPTS {
        check_stopping(&mut stopping)?;
        input.discard_pending(&mut stopping)?;
        check_stopping(&mut stopping)?;
        if phrase.is_empty() {
            writeln!(output, "Press Enter to continue.")?;
        } else {
            writeln!(output, "Type this exact line and press Enter.")?;
            writeln!(output, "{phrase}")?;
        }
        output.flush()?;

        let mut received = Vec::new();
        let mut truncated = false;
        loop {
            check_stopping(&mut stopping)?;
            match input.next()? {
                Key::Idle => {}
                Key::End => return Err(io::Error::other("Confirmation ended before Enter.")),
                Key::Byte(b'\n') => break,
                Key::Byte(byte) if received.len() < DISPLAY_LIMIT => received.push(byte),
                Key::Byte(_) => truncated = true,
            }
        }
        check_stopping(&mut stopping)?;
        // Treat one CRLF as a line ending. Every other byte must match.
        if !truncated && received.ends_with(b"\r") {
            received.pop();
        }
        if !truncated && received == phrase.as_bytes() {
            return Ok(());
        }

        let escaped: String = received
            .iter()
            .flat_map(|&byte| std::ascii::escape_default(byte))
            .map(char::from)
            .collect();
        let tail = if truncated { " (truncated)" } else { "" };
        writeln!(output, "Received \"{escaped}\"{tail}. Expected {phrase:?}.")?;
        if attempt < ATTEMPTS {
            let remaining = ATTEMPTS - attempt;
            let label = if remaining == 1 {
                "attempt"
            } else {
                "attempts"
            };
            writeln!(output, "{remaining} {label} left.")?;
        }
        output.flush()?;
    }
    Err(io::Error::other(
        "Confirmation did not match after three attempts.",
    ))
}

fn check_stopping(stopping: &mut impl FnMut() -> bool) -> io::Result<()> {
    if stopping() {
        Err(io::Error::other("Confirmation cancelled."))
    } else {
        Ok(())
    }
}

#[cfg(all(not(test), target_os = "linux"))]
struct Stdin;

#[cfg(all(not(test), target_os = "linux"))]
impl Stdin {
    fn poll_byte(wait_ms: libc::c_int) -> io::Result<Key> {
        let mut fd = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            match unsafe { libc::poll(&mut fd, 1, wait_ms) } {
                0 => return Ok(Key::Idle),
                n if n < 0 => {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error);
                }
                _ => {}
            }
            let mut byte = 0u8;
            match unsafe { libc::read(fd.fd, (&mut byte as *mut u8).cast(), 1) } {
                0 => return Ok(Key::End),
                1 => return Ok(Key::Byte(byte)),
                _ => {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }
}

#[cfg(all(not(test), target_os = "linux"))]
impl Input for Stdin {
    fn discard_pending(&mut self, stopping: &mut dyn FnMut() -> bool) -> io::Result<()> {
        if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
            // Clear complete lines and an unfinished line in the terminal driver.
            if unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) } == -1 {
                return Err(io::Error::last_os_error());
            }
        } else {
            while !stopping() && matches!(Self::poll_byte(0)?, Key::Byte(_)) {}
        }
        Ok(())
    }

    fn next(&mut self) -> io::Result<Key> {
        // Give the caller a chance to process Ctrl+C even without input.
        Self::poll_byte(100)
    }
}

#[cfg(all(not(test), target_os = "linux"))]
pub fn confirm(phrase: &str, stopping: impl FnMut() -> bool) -> Result<(), String> {
    confirm_with(&mut Stdin, &mut io::stdout(), phrase, stopping)
        .map_err(|error| format!("Cannot confirm slot action: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;

    const PHRASE: &str = "RESTORE SLOT 14 Spank 1";

    #[derive(Default)]
    struct Fake {
        pending: VecDeque<Key>,
        after_prompt: VecDeque<Vec<Key>>,
        discarded: Vec<usize>,
    }

    impl Input for Fake {
        fn discard_pending(&mut self, _: &mut dyn FnMut() -> bool) -> io::Result<()> {
            self.discarded.push(self.pending.len());
            self.pending.clear();
            Ok(())
        }

        fn next(&mut self) -> io::Result<Key> {
            if self.pending.is_empty() {
                self.pending
                    .extend(self.after_prompt.pop_front().unwrap_or_default());
            }
            Ok(self.pending.pop_front().unwrap_or(Key::End))
        }
    }

    fn keys(bytes: &[u8]) -> Vec<Key> {
        bytes.iter().copied().map(Key::Byte).collect()
    }

    #[test]
    fn enter_prompt_discards_stale_enter_and_waits_for_a_new_empty_line() {
        let mut fake = Fake {
            pending: keys(b"\n").into(),
            after_prompt: [keys(b"next\n"), keys(b"\n")].into(),
            ..Fake::default()
        };
        let mut output = Vec::new();
        confirm_with(&mut fake, &mut output, "", || false).unwrap();
        assert_eq!(fake.discarded, [1, 0]);
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.matches("Press Enter to continue.").count(), 2);
        assert!(output.contains("Received \"next\". Expected \"\"."));
    }

    #[test]
    fn clears_typeahead_before_each_prompt_and_prints_the_phrase_on_its_own_line() {
        let mut fake = Fake {
            pending: keys(b"RESTORE SLOT 14 Spank 1\nunfinished").into(),
            after_prompt: [
                keys(b"RESTORE SLOT Spank 1\nRESTORE SLOT 14 Spank 1\n"),
                keys(b"RESTORE SLOT 14 Spank 1\n"),
            ]
            .into(),
            ..Fake::default()
        };
        let mut output = Vec::new();
        confirm_with(&mut fake, &mut output, PHRASE, || false).unwrap();
        assert_eq!(
            fake.discarded,
            [
                b"RESTORE SLOT 14 Spank 1\nunfinished".len(),
                PHRASE.len() + 1
            ],
        );
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.lines().filter(|line| *line == PHRASE).count(), 2);
        assert!(
            output.contains(
                "Received \"RESTORE SLOT Spank 1\". Expected \"RESTORE SLOT 14 Spank 1\"."
            )
        );
    }

    #[test]
    fn requires_every_byte_to_match_and_allows_success_on_the_third_attempt() {
        let mut fake = Fake {
            after_prompt: [
                keys(b"RESTORE SLOT 14 Spank 1 \n"),
                keys(b"restore SLOT 14 Spank 1\n"),
                keys(b"RESTORE SLOT 14 Spank 1\r\n"),
            ]
            .into(),
            ..Fake::default()
        };
        let mut output = Vec::new();
        confirm_with(&mut fake, &mut output, PHRASE, || false).unwrap();
        assert_eq!(fake.discarded.len(), 3);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("2 attempts left."));
        assert!(output.contains("1 attempt left."));
    }

    #[test]
    fn escapes_control_and_invalid_bytes_and_refuses_a_fourth_attempt() {
        let mut fake = Fake {
            after_prompt: [
                keys(b"\x1b[H\xff\n"),
                keys(b"RESTORE SLOT 14 Spank 1\r\r\n"),
                keys(b"\n"),
                keys(b"RESTORE SLOT 14 Spank 1\n"),
            ]
            .into(),
            ..Fake::default()
        };
        let mut output = Vec::new();
        let error = confirm_with(&mut fake, &mut output, PHRASE, || false).unwrap_err();
        assert!(error.to_string().contains("after three attempts"));
        assert_eq!(fake.after_prompt.len(), 1);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(r"\x1b[H\xff"));
        assert!(output.contains(r"Spank 1\r"));
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn bounded_diagnostic_drains_the_rest_of_an_overlong_line() {
        let mut long = PHRASE.as_bytes().to_vec();
        long.extend([b' '; DISPLAY_LIMIT]);
        long.push(b'\n');
        let mut fake = Fake {
            after_prompt: [keys(&long), keys(b"RESTORE SLOT 14 Spank 1\n")].into(),
            ..Fake::default()
        };
        let mut output = Vec::new();
        confirm_with(&mut fake, &mut output, PHRASE, || false).unwrap();
        assert_eq!(fake.discarded, [0, 0]);
        assert!(String::from_utf8(output).unwrap().contains("(truncated)"));
    }

    #[test]
    fn eof_and_cancellation_stop_without_accepting_anything() {
        for cancel in [false, true] {
            let mut fake = Fake {
                after_prompt: [vec![Key::Idle, Key::End]].into(),
                ..Fake::default()
            };
            let checks = Cell::new(0);
            let error = confirm_with(&mut fake, &mut Vec::new(), PHRASE, || {
                checks.set(checks.get() + 1);
                cancel && checks.get() > 3
            })
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(if cancel { "cancelled" } else { "before Enter" })
            );
            assert_eq!(fake.discarded.len(), 1);
        }
    }
}
