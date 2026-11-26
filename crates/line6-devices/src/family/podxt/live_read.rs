//! PODxt Live editor reads. The caller owns the rig lock and establishes
//! automatic identity before opening the port. Each operation also confirms
//! the identity of the open port before sending a domain request.

use super::procedures::resend_identity;
use super::*;
use crate::midi::Event;
use crate::session::POLL;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadCommand {
    Current,
    Library,
    ToneBank,
    Packs,
    Globals,
    Tuner,
    Follow,
}

pub enum ReadUpdate {
    Current(EditBuffer),
    LibraryStarted,
    Stored(StoredTone),
    Packs(Packs),
    Globals(GlobalValues),
    Tuner(TunerReading),
    TunerMode(bool),
    Controller { number: u8, value: u8 },
    Clip,
    Tap,
    Channel(Location),
    FollowWarning(String),
}

enum Notice {
    Controller(u8, u8),
    Save(Location),
    Clip,
    Channel(Location),
}

fn notice(event: &Event) -> Option<Notice> {
    match event {
        Event::Short(message) => match message.as_slice() {
            [status, number, value] if status & 0xf0 == 0xb0 => {
                Some(Notice::Controller(*number, *value))
            }
            [status, program] if status & 0xf0 == 0xc0 => {
                Location::new(BankKind::Tones, u16::from(*program)).map(Notice::Channel)
            }
            _ => None,
        },
        Event::Sysex(frame) => match frame.as_slice() {
            // Line 6 Edit's OnSysex passes the controller and value to OnCC.
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x10, _, number, value, 0xf7] => {
                Some(Notice::Controller(*number, *value))
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0f, 0xf7] => Some(Notice::Clip),
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x24, bank, slot, 0xf7] => {
                let bank = BankKind::ALL.get(usize::from(bank.checked_sub(1)?))?;
                Location::new(*bank, u16::from(*slot)).map(Notice::Save)
            }
            _ => None,
        },
    }
}

/// Decode a physical controller notification using the editor's Follow parser.
pub fn controller_notification(event: &Event) -> Option<(u8, u8)> {
    match notice(event) {
        Some(Notice::Controller(number, value)) => Some((number, value)),
        _ => None,
    }
}

const DEVICE_SAVE_SETTLE: Duration = Duration::from_millis(2500);
const CURRENT_SETTLE: Duration = Duration::from_millis(100);
const IDENTITY_WINDOW: Duration = Duration::from_secs(3);
const IDENTITY_ORPHAN_QUIET: Duration = Duration::from_millis(300);

/// Run a read through a session with the caller's gate policy. Stop checks
/// occur before writes and between MIDI polls. A tuner run always attempts
/// its off command after entering tuner mode, including on cancellation.
pub fn read_live<L: Link>(
    session: &mut Session<L>,
    command: ReadCommand,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    read_live_on(session, command, None, true, cancelled, &mut publish)
}

/// Run the editor Follow reader for a bounded interval on the caller's locked link.
pub fn follow_window<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    read_live_on(
        session,
        ReadCommand::Follow,
        Some(window),
        true,
        cancelled,
        &mut publish,
    )
}

/// Run the editor tuner with a shorter diagnostic window, including cleanup.
pub fn tuner_window<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    read_live_on(
        session,
        ReadCommand::Tuner,
        Some(window),
        true,
        cancelled,
        &mut publish,
    )
}

/// Use Follow on the same already-verified, locked PODxt Live link.
pub fn follow_action<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    if session.link.path() != crate::session::Path::PodxtMidi
        || !session.allows(crate::gate::Operation::PodxtIdentity)
        || !session.allows(crate::gate::Operation::PodxtRead)
    {
        return Err(
            "PODxt Follow requires a PODxt MIDI link and identity and read permissions.".into(),
        );
    }
    read_live_on(
        session,
        ReadCommand::Follow,
        None,
        false,
        cancelled,
        &mut publish,
    )
}

fn read_live_on<L: Link>(
    session: &mut Session<L>,
    command: ReadCommand,
    window: Option<Duration>,
    verify: bool,
    cancelled: impl Fn() -> bool,
    publish: &mut dyn FnMut(ReadUpdate),
) -> Result<(), String> {
    let mut reader = Reader {
        session,
        cancelled: &cancelled,
        publish,
        follow: command == ReadCommand::Follow,
        saves: VecDeque::new(),
        current_due: None,
        after_reply: Vec::new(),
    };
    if verify {
        reader.identify(command == ReadCommand::Tuner)?;
    }
    match command {
        ReadCommand::Current => reader.current(),
        ReadCommand::Library => reader.library(&BankKind::ALL),
        ReadCommand::ToneBank => reader.library(&[BankKind::Tones]),
        ReadCommand::Packs => reader.packs(),
        ReadCommand::Globals => reader.globals(),
        ReadCommand::Tuner => reader.tuner(window.unwrap_or(TUNER_MAX).min(TUNER_MAX)),
        ReadCommand::Follow => reader.follow(window),
    }
}

struct Reader<'a, 's, L: Link> {
    session: &'a mut Session<'s, L>,
    cancelled: &'a dyn Fn() -> bool,
    publish: &'a mut dyn FnMut(ReadUpdate),
    follow: bool,
    saves: VecDeque<(Location, Duration)>,
    current_due: Option<Duration>,
    after_reply: Vec<Event>,
}

impl<L: Link> Reader<'_, '_, L> {
    fn check_cancel(&self) -> Result<(), String> {
        if (self.cancelled)() {
            Err("PODxt read cancelled.".into())
        } else {
            Ok(())
        }
    }

    fn identify(&mut self, tuner: bool) -> Result<(), String> {
        // This short drain discards events queued before the port was verified.
        let before = self.session.link.elapsed() + Duration::from_millis(200);
        while self.session.link.elapsed() < before {
            self.check_cancel()?;
            self.session.poll(POLL)?;
        }
        self.check_cancel()?;
        self.session.send(PodxtRequest::Identity)?;
        let mut last_packet = self.session.link.elapsed();
        let mut end = last_packet + IDENTITY_WINDOW;
        let initial_orphans = self.session.assembler.orphans;
        let mut resent = false;
        let mut identity = None;
        while let Some(left) = self.remaining(end) {
            self.check_cancel()?;
            let now = self.session.link.elapsed();
            if identity.is_none()
                && !resent
                && self.session.assembler.orphans > initial_orphans
                && now - last_packet >= IDENTITY_ORPHAN_QUIET
            {
                resend_identity(self.session)?;
                resent = true;
                end = self.session.link.elapsed() + IDENTITY_WINDOW;
                continue;
            }
            let Some(events) = self.session.poll(left.min(POLL))? else {
                continue;
            };
            last_packet = self.session.link.elapsed();
            for event in events {
                if let Event::Sysex(frame) = event
                    && let Some(reply) = Identity::parse(&frame)
                    && identity.replace(reply).is_some()
                {
                    return Err("More than one MIDI identity answered.".into());
                }
            }
        }
        self.check_cancel()?;
        let missing = "The open MIDI port did not verify as PODxt Live 3.01.";
        let identity = identity.ok_or(missing)?;
        check_live_identity(&identity).map_err(|error| if tuner { error } else { missing.into() })
    }

    fn remaining(&self, end: Duration) -> Option<Duration> {
        end.checked_sub(self.session.link.elapsed())
            .filter(|duration| !duration.is_zero())
    }

    fn request(
        &mut self,
        request: PodxtRequest,
        window: Duration,
        matches: impl Fn(&[u8]) -> bool,
    ) -> Result<Option<Vec<u8>>, String> {
        self.check_cancel()?;
        self.session.send(request)?;
        let end = self.session.link.elapsed() + window;
        while let Some(left) = self.remaining(end) {
            self.check_cancel()?;
            let mut answer = None;
            for event in self.session.poll(left.min(POLL))?.into_iter().flatten() {
                match event {
                    Event::Sysex(frame) if answer.is_none() && matches(&frame) => {
                        answer = Some(frame);
                    }
                    other if answer.is_some() && self.follow => self.after_reply.push(other),
                    other => self.incoming(&other),
                }
            }
            self.check_cancel()?;
            if answer.is_some() {
                return Ok(answer);
            }
        }
        Ok(None)
    }

    fn current(&mut self) -> Result<(), String> {
        let tone = request_tone(None, |request, window| {
            self.request(request, window, is_edit_buffer)
        })?
        .ok_or("No 168-byte edit-buffer dump arrived.")?;
        (self.publish)(ReadUpdate::Current(tone));
        self.replay_after_reply();
        Ok(())
    }

    fn stored(&mut self, location: Location) -> Result<(), String> {
        let tone = request_tone(Some(location), |request, window| {
            self.request(request, window, is_edit_buffer)
        })?
        .ok_or_else(|| {
            format!(
                "{} {}: no stored dump.",
                location.kind().name(),
                location.label()
            )
        })?;
        (self.publish)(ReadUpdate::Stored(StoredTone { location, tone }));
        self.replay_after_reply();
        self.wait(STORED_GAP)
    }

    fn replay_after_reply(&mut self) {
        for event in std::mem::take(&mut self.after_reply) {
            self.incoming(&event);
        }
    }

    fn wait(&mut self, duration: Duration) -> Result<(), String> {
        let end = self.session.link.elapsed() + duration;
        while let Some(left) = self.remaining(end) {
            self.check_cancel()?;
            for event in self.session.poll(left.min(POLL))?.into_iter().flatten() {
                self.incoming(&event);
            }
        }
        self.check_cancel()
    }

    fn library(&mut self, banks: &[BankKind]) -> Result<(), String> {
        self.check_cancel()?;
        (self.publish)(ReadUpdate::LibraryStarted);
        for location in Location::all(banks) {
            self.stored(location)?;
        }
        Ok(())
    }

    fn packs(&mut self) -> Result<(), String> {
        let frame = self
            .request(PodxtRequest::InstalledPacks, WINDOW, is_packs_reply)?
            .ok_or("No installed-pack reply arrived.")?;
        (self.publish)(ReadUpdate::Packs(Packs(frame[7])));
        Ok(())
    }

    fn global(&mut self, field: Global) -> Result<Option<u16>, String> {
        let id = field.id();
        Ok(self
            .request(PodxtRequest::QueryGlobal(field), GLOBAL_WINDOW, |msg| {
                global_value(msg, id).is_some()
            })?
            .and_then(|reply| global_value(&reply, id)))
    }

    fn globals(&mut self) -> Result<(), String> {
        let values = Global::STATUS
            .into_iter()
            .map(|field| self.global(field).map(|value| (field, value)))
            .collect::<Result<Vec<_>, _>>()?;
        (self.publish)(ReadUpdate::Globals(values));
        Ok(())
    }

    fn tuner(&mut self, window: Duration) -> Result<(), String> {
        self.check_cancel()?;
        let on = self.session.send(PodxtRequest::TunerMode(true));
        let start = self.session.link.elapsed();
        let result = on.and_then(|()| {
            while self.remaining(start + window).is_some() {
                let note = self.global(Global::TunerNote)?;
                let deviation = self.global(Global::TunerDeviation)?;
                (self.publish)(ReadUpdate::Tuner(TunerReading {
                    at: self.session.link.elapsed().saturating_sub(start),
                    note,
                    deviation,
                }));
                self.wait(TUNER_POLL)?;
            }
            Ok(())
        });
        match (result, self.session.send(PodxtRequest::TunerMode(false))) {
            (result, Ok(())) => result,
            (Ok(()), Err(off)) => Err(format!("Tuner off failed: {off}")),
            (Err(error), Err(off)) => Err(format!("{error} Tuner off also failed: {off}")),
        }
    }

    fn incoming(&mut self, event: &Event) {
        if !self.follow {
            return;
        }
        let now = self.session.link.elapsed();
        match notice(event) {
            Some(Notice::Controller(64, _)) => {
                (self.publish)(ReadUpdate::Tap);
                self.current_due = Some(now + CURRENT_SETTLE);
            }
            Some(Notice::Controller(TUNER_CC, value)) => {
                (self.publish)(ReadUpdate::TunerMode(value > 63));
            }
            Some(Notice::Controller(19, _)) => {
                // Line 6 Edit refreshes the edit buffer after an FX setup recall.
                self.current_due = Some(now + CURRENT_SETTLE);
            }
            Some(Notice::Controller(number, value)) => {
                (self.publish)(ReadUpdate::Controller { number, value });
                if matches!(number, 11 | 12 | 37 | 58 | 71 | 75 | 88 | 91) {
                    // Model selection can change companion controls without
                    // sending separate controller feedback for every value.
                    self.current_due = Some(now + CURRENT_SETTLE);
                }
            }
            Some(Notice::Channel(location)) => {
                (self.publish)(ReadUpdate::Channel(location));
                self.current_due = Some(now + CURRENT_SETTLE);
            }
            Some(Notice::Clip) => (self.publish)(ReadUpdate::Clip),
            // Line 6 Edit does not request a stored dump for an amp-setup save.
            Some(Notice::Save(location)) if location.kind() != BankKind::AmpSetups => {
                // Repeated saves to one slot refresh its latest value once.
                self.saves.retain(|(pending, _)| *pending != location);
                self.saves.push_back((location, now + DEVICE_SAVE_SETTLE));
            }
            _ => {}
        }
    }

    fn follow(&mut self, window: Option<Duration>) -> Result<(), String> {
        self.current()?;
        let end = window.map(|window| self.session.link.elapsed() + window);
        loop {
            let now = self.session.link.elapsed();
            if end.is_some_and(|end| end <= now) {
                return Ok(());
            }
            self.check_cancel()?;
            if self.current_due.is_some_and(|due| due <= now) {
                self.current_due = None;
                self.current()?;
            }
            if let Some(&(location, due)) = self.saves.front()
                && due <= now
            {
                self.saves.pop_front();
                if self.session.allows(Operation::PodxtLibraryRead) {
                    self.stored(location)?;
                } else {
                    (self.publish)(ReadUpdate::FollowWarning(
                        "A PODxt save was detected. The library read gate is closed.".into(),
                    ));
                }
            }
            let now = self.session.link.elapsed();
            let wait = self
                .saves
                .front()
                .map(|&(_, due)| due)
                .into_iter()
                .chain(self.current_due)
                .chain(end)
                .min()
                .map_or(POLL, |due| due.saturating_sub(now).min(POLL));
            if !wait.is_zero() {
                for event in self.session.poll(wait)?.into_iter().flatten() {
                    self.incoming(&event);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::GatePolicy;
    use crate::identity::tests::PODXT_LIVE;
    use crate::session::{
        Log, Quiet,
        testing::{Fake, packets},
    };
    use std::cell::Cell;

    struct ReadsOnly {
        stored: bool,
    }

    impl GatePolicy for ReadsOnly {
        fn allows(&self, operation: Operation) -> bool {
            match operation {
                Operation::PodxtIdentity
                | Operation::PodxtRead
                | Operation::PodxtPackQuery
                | Operation::PodxtGlobalRead
                | Operation::PodxtTunerMode => true,
                Operation::PodxtLibraryRead => self.stored,
                _ => false,
            }
        }
    }

    fn identity() -> Vec<u8> {
        let mut reply = PODXT_LIVE.to_vec();
        reply[12..16].copy_from_slice(&[0, 3, 0, 1]);
        reply
    }

    fn tone(name: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; EDIT_BUFFER_LEN];
        bytes[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
        bytes[7..7 + name.len()].copy_from_slice(name);
        bytes[167] = 0xf7;
        bytes
    }

    fn global_reply(field: Global, raw: u16) -> Vec<u8> {
        let [high, low] = raw.to_be_bytes();
        vec![
            0xf0,
            0,
            1,
            0x0c,
            3,
            0x57,
            field.id(),
            high >> 4,
            high & 15,
            low >> 4,
            low & 15,
            0xf7,
        ]
    }

    fn run(
        fake: &mut impl Link,
        command: ReadCommand,
        cancelled: impl Fn() -> bool,
        update: impl FnMut(ReadUpdate),
    ) -> Result<(), String> {
        let mut log = Quiet;
        let mut session = Session::new(fake, &ReadsOnly { stored: true }, &mut log);
        read_live(&mut session, command, cancelled, update)
    }

    #[test]
    fn current_tone_and_packs_use_the_verified_requests() {
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity())),
                (PodxtRequest::EditBuffer, packets(&tone(b"Current"))),
            ],
        );
        let mut updates = Vec::new();
        run(
            &mut fake,
            ReadCommand::Current,
            || false,
            |event| updates.push(event),
        )
        .unwrap();
        assert!(
            matches!(updates.as_slice(), [ReadUpdate::Current(tone)] if tone.name() == "Current")
        );
        fake.assert_finished();

        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity())),
                (
                    PodxtRequest::InstalledPacks,
                    vec![vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7]],
                ),
            ],
        );
        let mut updates = Vec::new();
        run(
            &mut fake,
            ReadCommand::Packs,
            || false,
            |event| updates.push(event),
        )
        .unwrap();
        assert!(matches!(updates.as_slice(), [ReadUpdate::Packs(Packs(4))]));
        fake.assert_finished();
    }

    #[test]
    fn all_twenty_one_globals_are_polled_and_timeouts_stay_unknown() {
        let mut script = vec![(PodxtRequest::Identity, packets(&identity()))];
        for field in Global::STATUS {
            let replies = if field == Global::ClipStatus {
                vec![]
            } else {
                vec![global_reply(field, 0xff00 | u16::from(field.id()))]
            };
            script.push((PodxtRequest::QueryGlobal(field), replies));
        }
        let mut fake = Fake::new(Path::PodxtMidi, script);
        run(
            &mut fake,
            ReadCommand::Globals,
            || false,
            |update| {
                let ReadUpdate::Globals(values) = update else {
                    panic!("unexpected update");
                };
                assert_eq!(values.len(), 21);
                assert_eq!(values[1], (Global::ClipStatus, None));
                assert_eq!(values[6], (Global::UiMode, Some(0xff12)));
            },
        )
        .unwrap();
        fake.assert_finished();
    }

    #[test]
    fn library_reads_all_299_locations_and_cancels_after_the_last_published_entry() {
        for count in [7, 299] {
            let expected: Vec<_> = Location::all(&BankKind::ALL).take(count).collect();
            let mut script = vec![(PodxtRequest::Identity, packets(&identity()))];
            script.extend(
                expected
                    .iter()
                    .map(|&location| (PodxtRequest::Stored(location), packets(&tone(b"Saved")))),
            );
            let mut fake = Fake::new(Path::PodxtMidi, script);
            let stop = Cell::new(false);
            let mut locations = Vec::new();
            let mut starts = 0;
            let result = run(
                &mut fake,
                ReadCommand::Library,
                || stop.get(),
                |update| match update {
                    ReadUpdate::LibraryStarted => starts += 1,
                    ReadUpdate::Stored(stored) => {
                        locations.push(stored.location);
                        if count == 7 && locations.len() == count {
                            stop.set(true);
                        }
                    }
                    _ => panic!("unexpected update"),
                },
            );
            assert_eq!(result.is_ok(), count == 299);
            assert_eq!(starts, 1);
            assert_eq!(locations, expected);
            assert_eq!(fake.writes.len(), count + 1);
            fake.assert_finished();
        }
    }

    #[test]
    fn librarian_reads_only_the_128_tone_channels() {
        let expected: Vec<_> = Location::all(&[BankKind::Tones]).collect();
        let mut script = vec![(PodxtRequest::Identity, packets(&identity()))];
        script.extend(
            expected
                .iter()
                .map(|&slot| (PodxtRequest::Stored(slot), packets(&tone(b"Librarian")))),
        );
        let mut fake = Fake::new(Path::PodxtMidi, script);
        let mut seen = Vec::new();
        run(
            &mut fake,
            ReadCommand::ToneBank,
            || false,
            |update| {
                if let ReadUpdate::Stored(stored) = update {
                    seen.push(stored.location);
                }
            },
        )
        .unwrap();
        assert_eq!(seen, expected);
        assert_eq!(seen.len(), 128);
        fake.assert_finished();
    }

    struct Disconnect {
        fake: Fake,
        on_write_count: usize,
    }

    impl Link for Disconnect {
        fn path(&self) -> Path {
            self.fake.path()
        }
        fn elapsed(&self) -> Duration {
            self.fake.elapsed()
        }
        fn pause(&mut self, duration: Duration) {
            self.fake.pause(duration);
        }
        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.fake.write(bytes)
        }
        fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
            if self.fake.writes.len() == self.on_write_count {
                Err("MIDI port disconnected".into())
            } else {
                self.fake.read(wait)
            }
        }
    }

    #[test]
    fn disconnect_stops_the_library_without_requesting_another_slot() {
        let addresses: Vec<_> = Location::all(&BankKind::ALL).take(2).collect();
        let mut fake = Disconnect {
            fake: Fake::new(
                Path::PodxtMidi,
                vec![
                    (PodxtRequest::Identity, packets(&identity())),
                    (
                        PodxtRequest::Stored(addresses[0]),
                        packets(&tone(b"First slot")),
                    ),
                    (PodxtRequest::Stored(addresses[1]), vec![]),
                ],
            ),
            on_write_count: 3,
        };
        let mut received = Vec::new();
        let error = run(
            &mut fake,
            ReadCommand::Library,
            || false,
            |event| {
                if let ReadUpdate::Stored(stored) = event {
                    received.push(stored.location);
                }
            },
        )
        .unwrap_err();
        assert_eq!(error, "MIDI port disconnected");
        assert_eq!(received, addresses[..1]);
        fake.fake.assert_finished();
    }

    #[test]
    fn tuner_stays_within_its_commands_and_sends_off_on_cancel_or_read_failure() {
        for (disconnected, off_fails) in [(false, false), (true, false), (true, true)] {
            let mut script = vec![
                (PodxtRequest::Identity, packets(&identity())),
                (PodxtRequest::TunerMode(true), vec![]),
                (PodxtRequest::QueryGlobal(Global::TunerNote), vec![]),
            ];
            if !disconnected {
                script[2].1.push(global_reply(Global::TunerNote, 1));
                script.push((
                    PodxtRequest::QueryGlobal(Global::TunerDeviation),
                    vec![global_reply(Global::TunerDeviation, 0xfffc)],
                ));
            }
            script.push((PodxtRequest::TunerMode(false), vec![]));
            let off_index = script.len() - 1;
            let mut fake = Disconnect {
                fake: Fake::new(Path::PodxtMidi, script),
                on_write_count: if disconnected { 3 } else { usize::MAX },
            };
            if off_fails {
                fake.fake.fail_at.push(off_index);
            }
            let stop = Cell::new(false);
            let error = run(
                &mut fake,
                ReadCommand::Tuner,
                || stop.get(),
                |event| {
                    let ReadUpdate::Tuner(reading) = event else {
                        panic!("unexpected update");
                    };
                    assert_eq!(reading.note, Some(1));
                    assert_eq!(reading.deviation.map(tuner_cents), Some(-4.0));
                    stop.set(true);
                },
            )
            .unwrap_err();
            assert_eq!(stop.get(), !disconnected);
            assert!(error.contains(if disconnected {
                "disconnected"
            } else {
                "cancelled"
            }));
            assert_eq!(error.contains("Tuner off also failed"), off_fails);
            assert_eq!(fake.fake.writes.last().unwrap(), &[0xb0, 69, 0]);
            fake.fake.assert_finished();
        }
    }

    #[test]
    fn guided_follow_times_out_when_a_channel_notice_arrives_without_the_startup_dump() {
        let target = Location::new(BankKind::Tones, 41).unwrap();
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![(
                PodxtRequest::EditBuffer,
                vec![vec![0xc0, 41], vec![0xb0, 7, 50]],
            )],
        );
        let mut log = Quiet;
        let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut log);
        let mut updates = Vec::new();
        let error =
            follow_action(&mut session, || false, |update| updates.push(update)).unwrap_err();

        assert_eq!(error, "No 168-byte edit-buffer dump arrived.");
        assert_eq!(session.link.elapsed(), WINDOW);
        assert!(matches!(
            updates.as_slice(),
            [ReadUpdate::Channel(channel), ReadUpdate::Controller { number: 7, value: 50 }]
                if *channel == target
        ));
        assert_eq!(session.link.writes, [PodxtRequest::EditBuffer.bytes()]);
        session.link.assert_finished();
    }

    #[test]
    fn guided_follow_reads_the_new_tone_when_the_channel_notice_follows_startup() {
        let target = Location::new(BankKind::Tones, 41).unwrap();
        let starting_tone = tone(b"Starting tone");
        let target_tone = tone(b"Target tone");
        let mut startup_packets = packets(&starting_tone);
        startup_packets.extend([vec![0xc0, 41], vec![0xb0, 7, 50]]);
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::EditBuffer, startup_packets),
                (PodxtRequest::EditBuffer, packets(&target_tone)),
            ],
        );
        let mut log = Quiet;
        let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut log);
        let stop = Cell::new(false);
        let mut updates = Vec::new();
        let result = follow_action(
            &mut session,
            || stop.get(),
            |update| {
                if matches!(&update, ReadUpdate::Current(tone) if tone.raw == target_tone) {
                    stop.set(true);
                }
                updates.push(update);
            },
        );

        assert_eq!(result, Err("PODxt read cancelled.".into()));
        assert!(matches!(
            updates.as_slice(),
            [ReadUpdate::Current(first), ReadUpdate::Channel(channel),
             ReadUpdate::Controller { number: 7, value: 50 }, ReadUpdate::Current(last)]
                if first.raw == starting_tone && *channel == target && last.raw == target_tone
        ));
        assert_eq!(
            session.link.writes,
            [
                PodxtRequest::EditBuffer.bytes(),
                PodxtRequest::EditBuffer.bytes()
            ]
        );
        session.link.assert_finished();
    }

    #[test]
    fn follow_delivers_controller_clip_tap_and_rereads_saved_slot_without_writing_tones() {
        let slot = Location::new(BankKind::Tones, 80).unwrap();
        let mut current = tone(b"Current");
        current.extend([0xb4, 13, 37, 0xb4, 11, 20, 0xb4, 64, 127]);
        current.extend([0xf0, 0, 1, 0x0c, 3, 0x0f, 0xf7]);
        current.extend([0xf0, 0, 1, 0x0c, 3, 0x24, 1, 80, 0xf7]);
        current.extend([0xc2, 80]);
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity())),
                (PodxtRequest::EditBuffer, vec![current]),
                (PodxtRequest::EditBuffer, packets(&tone(b"New current"))),
                (PodxtRequest::Stored(slot), packets(&tone(b"Stored 80"))),
            ],
        );
        let stop = Cell::new(false);
        let mut events = Vec::new();
        let error = run(
            &mut fake,
            ReadCommand::Follow,
            || stop.get(),
            |event| {
                if matches!(event, ReadUpdate::Stored(_)) {
                    stop.set(true);
                }
                events.push(event);
            },
        )
        .unwrap_err();
        assert!(error.contains("cancelled"));
        assert!(matches!(events.as_slice(), [
            ReadUpdate::Current(first), ReadUpdate::Controller { number: 13, value: 37 },
            ReadUpdate::Controller { number: 11, value: 20 },
            ReadUpdate::Tap, ReadUpdate::Clip, ReadUpdate::Channel(channel),
            ReadUpdate::Current(next), ReadUpdate::Stored(stored)
        ] if first.name() == "Current" && *channel == slot && next.name() == "New current"
            && stored.location == slot && stored.tone.name() == "Stored 80"));
        fake.assert_finished();
    }

    #[test]
    fn follow_decodes_plain_and_wrapped_cc_and_refreshes_after_fx_setup_recall() {
        // Test plain and wrapped FX recall separately so each must request its own dump.
        for fx_recall in [
            vec![0xb4, 19, 3],
            vec![0xf0, 0, 1, 0x0c, 3, 0x10, 5, 19, 3, 0xf7],
        ] {
            let mut current = tone(b"Before FX recall");
            current.extend([0xb4, 69, 64, 0xb4, 69, 63]);
            current.extend([0xf0, 0, 1, 0x0c, 3, 0x10, 2, 69, 127, 0xf7]);
            current.extend([0xf0, 0, 1, 0x0c, 3, 0x10, 2, 69, 0, 0xf7]);
            current.extend([0xf0, 0, 1, 0x0c, 3, 0x10, 2, 13, 21, 0xf7]);
            // Wrong manufacturer or length is not a PODxt controller frame.
            current.extend([0xf0, 0, 1, 0x0d, 3, 0x10, 2, 13, 99, 0xf7]);
            current.extend([0xf0, 0, 1, 0x0c, 3, 0x10, 2, 13, 99, 0, 0xf7]);
            current.extend(fx_recall);
            let mut fake = Fake::new(
                Path::PodxtMidi,
                vec![
                    (PodxtRequest::Identity, packets(&identity())),
                    (PodxtRequest::EditBuffer, vec![current]),
                    (PodxtRequest::EditBuffer, packets(&tone(b"After FX recall"))),
                ],
            );
            let mut log = Quiet;
            let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut log);
            let mut updates = Vec::new();
            follow_window(
                &mut session,
                Duration::from_millis(400),
                || false,
                |update| updates.push(update),
            )
            .unwrap();
            assert!(matches!(updates.as_slice(), [
                ReadUpdate::Current(first),
                ReadUpdate::TunerMode(true), ReadUpdate::TunerMode(false),
                ReadUpdate::TunerMode(true), ReadUpdate::TunerMode(false),
                ReadUpdate::Controller { number: 13, value: 21 },
                ReadUpdate::Current(next),
            ] if first.name() == "Before FX recall" && next.name() == "After FX recall"));
            fake.assert_finished();
        }
    }

    #[derive(Default)]
    struct Traffic(Vec<String>);

    impl Log for Traffic {
        fn line(&mut self, line: String) {
            self.0.push(line);
        }
    }

    impl Traffic {
        fn time_of(&self, kind: &str, bytes: &[u8]) -> Duration {
            let message = format!("{kind} {}", crate::midi::hex(bytes));
            self.0
                .iter()
                .find_map(|line| {
                    let (timestamp, payload) = line.trim_start().split_once("s ")?;
                    (payload == message)
                        .then(|| Duration::from_secs_f64(timestamp.parse().unwrap()))
                })
                .unwrap_or_else(|| panic!("missing {message}: {:?}", self.0))
        }
    }

    #[test]
    fn follow_rereads_tone_and_fx_saves_at_2500ms_and_skips_amp_setup_saves() {
        for (kind, bank, slot) in [(BankKind::Tones, 1, 80), (BankKind::FxSetups, 3, 11)] {
            let location = Location::new(kind, slot.into()).unwrap();
            let request = PodxtRequest::Stored(location);
            let saved = [0xf0, 0, 1, 0x0c, 3, 0x24, bank, slot, 0xf7];
            let amp_saved = [0xf0, 0, 1, 0x0c, 3, 0x24, 2, 12, 0xf7];
            let mut current = packets(&tone(b"Current"));
            current.extend([saved.to_vec(), amp_saved.to_vec()]);
            let mut fake = Fake::new(
                Path::PodxtMidi,
                vec![
                    (PodxtRequest::Identity, packets(&identity())),
                    (PodxtRequest::EditBuffer, current),
                    (request, packets(&tone(b"Saved"))),
                ],
            );
            let mut log = Traffic::default();
            let mut session = Session::new(&mut fake, &ReadsOnly { stored: true }, &mut log);
            let mut updated_slots = Vec::new();
            follow_window(
                &mut session,
                Duration::from_secs(3),
                || false,
                |update| {
                    if let ReadUpdate::Stored(stored) = update {
                        updated_slots.push(stored.location);
                    }
                },
            )
            .unwrap();
            assert_eq!(updated_slots, [location]);
            assert_eq!(
                log.time_of("OUT", &request.bytes()) - log.time_of("IN ", &saved),
                Duration::from_millis(2500)
            );
            fake.assert_finished();
        }
    }

    #[test]
    fn follow_window_ending_before_save_settles_sends_no_addressed_read() {
        let mut current = tone(b"Current");
        current.extend([0xf0, 0, 1, 0x0c, 3, 0x24, 1, 80, 0xf7]);
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity())),
                (PodxtRequest::EditBuffer, vec![current]),
            ],
        );
        let mut log = Quiet;
        let mut session = Session::new(&mut fake, &ReadsOnly { stored: true }, &mut log);
        follow_window(
            &mut session,
            Duration::from_millis(2499),
            || false,
            |update| assert!(matches!(update, ReadUpdate::Current(_))),
        )
        .unwrap();
        fake.assert_finished();
    }

    #[test]
    fn an_unverified_or_ambiguous_port_sends_identity_only_and_cancel_can_stop_before_identity() {
        for replies in [
            vec![],
            vec![PODXT_LIVE.to_vec()],
            vec![identity(), identity()],
        ] {
            let mut fake = Fake::new(Path::PodxtMidi, vec![(PodxtRequest::Identity, replies)]);
            assert!(
                run(
                    &mut fake,
                    ReadCommand::Current,
                    || false,
                    |_| panic!("unexpected update")
                )
                .is_err()
            );
            assert_eq!(fake.writes, [identity::REQUEST]);
            fake.assert_finished();
        }
        let mut fake = Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]);
        assert!(
            run(
                &mut fake,
                ReadCommand::Current,
                || true,
                |_| panic!("unexpected update")
            )
            .is_err()
        );
        assert!(fake.writes.is_empty());
    }

    #[test]
    fn a_save_notification_obeys_the_library_read_gate_even_while_following() {
        let mut current = tone(b"Current");
        current.extend([0xf0, 0, 1, 0x0c, 3, 0x24, 1, 80, 0xf7]);
        let mut fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity())),
                (PodxtRequest::EditBuffer, vec![current]),
            ],
        );
        let stop = Cell::new(false);
        let mut quiet = Quiet;
        let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut quiet);
        assert!(
            read_live(
                &mut session,
                ReadCommand::Follow,
                || stop.get(),
                |update| {
                    if matches!(update, ReadUpdate::FollowWarning(_)) {
                        stop.set(true);
                    }
                }
            )
            .is_err()
        );
        fake.assert_finished();
    }
}
