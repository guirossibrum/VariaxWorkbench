use super::*;
use line6_devices::{identity::REQUEST, session::Path};
use std::{cell::RefCell, collections::VecDeque};
use variax_workbench::identity_stage::{bridge_reply, guitar_reply};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Open(Interface),
    Identity(Interface, Duration),
    Finish(Interface),
    Drop(Interface),
}

#[derive(Default)]
struct Rig {
    pod_replies: Option<Vec<Vec<u8>>>,
    guitar_replies: Vec<Vec<Vec<u8>>>,
    open_failure: Option<(Interface, &'static str)>,
    fail_finish: Option<Interface>,
    cancel_guitar_at: Option<Duration>,
    cancel: AtomicBool,
    events: RefCell<Vec<Event>>,
}

struct FakeLink<'a> {
    rig: &'a Rig,
    side: Interface,
    now: Duration,
    script: VecDeque<Vec<Vec<u8>>>,
    inbox: VecDeque<Vec<u8>>,
}

impl Rig {
    fn run(&self, kind: JobKind, silence_delay: Option<Duration>) -> Outcome {
        identify_on(
            kind,
            &self.cancel,
            silence_delay,
            |side| {
                self.events.borrow_mut().push(Event::Open(side));
                if let Some((expected, error)) = self.open_failure
                    && side == expected
                {
                    return Err(error.into());
                }
                let script = match side {
                    Interface::Podxt => vec![
                        self.pod_replies
                            .clone()
                            .unwrap_or_else(|| vec![bridge_reply()]),
                    ],
                    Interface::Variax => self.guitar_replies.clone(),
                };
                Ok(FakeLink {
                    rig: self,
                    side,
                    now: Duration::ZERO,
                    script: script.into(),
                    inbox: VecDeque::new(),
                })
            },
            |link| {
                self.events.borrow_mut().push(Event::Finish(link.side));
                if self.fail_finish == Some(link.side) {
                    return Err("release failed".into());
                }
                Ok(())
            },
        )
    }

    fn guitar_requests(&self) -> Vec<Duration> {
        self.events
            .borrow()
            .iter()
            .filter_map(|event| match event {
                Event::Identity(Interface::Variax, time) => Some(*time),
                _ => None,
            })
            .collect()
    }
}

impl Link for FakeLink<'_> {
    fn path(&self) -> Path {
        match self.side {
            Interface::Podxt => Path::PodxtMidi,
            Interface::Variax => Path::VariaxViaPodxt,
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        assert_eq!(bytes, REQUEST);
        self.rig
            .events
            .borrow_mut()
            .push(Event::Identity(self.side, self.now));
        self.inbox
            .extend(self.script.pop_front().unwrap_or_default());
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let packet = self.inbox.pop_front();
        if packet.is_none() {
            self.pause(wait);
        }
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.now
    }

    fn pause(&mut self, duration: Duration) {
        self.now += duration;
        if self.side == Interface::Variax
            && self
                .rig
                .cancel_guitar_at
                .is_some_and(|limit| self.now >= limit)
        {
            self.rig.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for FakeLink<'_> {
    fn drop(&mut self) {
        self.rig.events.borrow_mut().push(Event::Drop(self.side));
    }
}

const RIG_CHECK_DELAY: Option<Duration> = Some(Duration::from_secs(2));

fn split_reply(reply: Vec<u8>) -> Vec<Vec<u8>> {
    reply.chunks(16).map(<[u8]>::to_vec).collect()
}

#[test]
fn rig_check_policy_retries_first_silence_on_the_same_guitar_link() {
    for (kind, pod) in [
        (JobKind::Initial, Some("3.01")),
        (JobKind::RefreshGuitar, None),
    ] {
        let rig = Rig {
            guitar_replies: vec![vec![], split_reply(guitar_reply())],
            ..Default::default()
        };
        assert!(matches!(
            rig.run(kind, RIG_CHECK_DELAY),
            Outcome::Finished { pod_firmware, guitar: Guitar::Verified("3.10") } if pod_firmware == pod
        ));
        assert_eq!(
            rig.guitar_requests(),
            [Duration::ZERO, Duration::from_secs(5)]
        );
        let mut expected = vec![
            Event::Open(Interface::Variax),
            Event::Identity(Interface::Variax, Duration::ZERO),
            Event::Identity(Interface::Variax, Duration::from_secs(5)),
            Event::Finish(Interface::Variax),
            Event::Drop(Interface::Variax),
        ];
        if kind == JobKind::Initial {
            expected.splice(
                0..0,
                [
                    Event::Open(Interface::Podxt),
                    Event::Identity(Interface::Podxt, Duration::from_millis(200)),
                    Event::Finish(Interface::Podxt),
                    Event::Drop(Interface::Podxt),
                ],
            );
        }
        assert_eq!(*rig.events.borrow(), expected);
    }
}

#[test]
fn pod_startup_distinguishes_silent_and_unaccepted_receive_data() {
    let valid = bridge_reply();
    let truncated = valid[..16].to_vec();
    let mut short_identity = valid[..15].to_vec();
    short_identity.push(0xf7);
    for (packets, expected_detail) in [
        (vec![], "No receive chunks in three seconds."),
        (
            vec![vec![0xf7]],
            "Receive chunks: 1, orphan bytes: 1, malformed identity frames: 0, other messages: 0.",
        ),
        (
            vec![truncated],
            "Receive chunks: 1, orphan bytes: 0, malformed identity frames: 0, other messages: 0.",
        ),
        (
            vec![short_identity],
            "Receive chunks: 1, orphan bytes: 0, malformed identity frames: 1, other messages: 0.",
        ),
        (
            vec![vec![0xf0, 0, 1, 0x0c, 3, 0x11, 0xf7], vec![0xb0, 7, 10]],
            "Receive chunks: 2, orphan bytes: 0, malformed identity frames: 0, other messages: 2.",
        ),
    ] {
        let rig = Rig {
            pod_replies: Some(packets),
            ..Default::default()
        };
        assert!(matches!(
            rig.run(JobKind::Initial, DISCOVERY_SILENCE_DELAY),
            Outcome::Finished { pod_firmware: None, guitar: Guitar::Unavailable(reason) }
                if reason.strip_prefix("No PODxt identity answered. ") == Some(expected_detail)
        ));
        assert_eq!(
            *rig.events.borrow(),
            [
                Event::Open(Interface::Podxt),
                Event::Identity(Interface::Podxt, Duration::from_millis(200)),
                Event::Finish(Interface::Podxt),
                Event::Drop(Interface::Podxt),
            ],
        );
    }
}

#[test]
fn no_answer_is_bounded_by_the_selected_silence_policy() {
    for (policy, count) in [(DISCOVERY_SILENCE_DELAY, 1), (RIG_CHECK_DELAY, 2)] {
        let rig = Rig::default();
        assert!(matches!(
            rig.run(JobKind::RefreshGuitar, policy),
            Outcome::Finished {
                guitar: Guitar::Silent,
                ..
            }
        ));
        assert_eq!(rig.guitar_requests().len(), count);
    }
}

#[test]
fn existing_orphan_tail_resend_is_still_available() {
    let rig = Rig {
        guitar_replies: vec![vec![vec![0xf7]], split_reply(guitar_reply())],
        ..Default::default()
    };
    assert!(matches!(
        rig.run(JobKind::RefreshGuitar, None),
        Outcome::Finished {
            guitar: Guitar::Verified("3.10"),
            ..
        }
    ));
    assert_eq!(
        rig.guitar_requests(),
        [Duration::ZERO, Duration::from_millis(300)]
    );
}

#[test]
fn malformed_or_incomplete_reply_does_not_trigger_a_silence_resend() {
    for reply in [vec![0xf0, 0x7e, 0xf7], guitar_reply()[..16].to_vec()] {
        let rig = Rig {
            guitar_replies: vec![vec![reply], split_reply(guitar_reply())],
            ..Default::default()
        };
        assert!(matches!(
            rig.run(JobKind::RefreshGuitar, RIG_CHECK_DELAY),
            Outcome::Finished {
                guitar: Guitar::Unavailable(reason),
                ..
            } if reason.starts_with("Variax identity exchange failed:")
        ));
        assert_eq!(rig.guitar_requests(), [Duration::ZERO]);
    }
}

#[test]
fn unverified_identity_is_reported_without_a_resend() {
    let mut old_guitar = guitar_reply();
    old_guitar[14..16].copy_from_slice(b"06");
    for (reply, is_variax) in [(old_guitar, true), (bridge_reply(), false)] {
        let rig = Rig {
            guitar_replies: vec![split_reply(reply)],
            ..Default::default()
        };
        assert!(matches!(
            rig.run(JobKind::RefreshGuitar, RIG_CHECK_DELAY),
            Outcome::Finished { guitar: Guitar::Unverified(reply), .. } if reply.is_variax == is_variax
        ));
        assert_eq!(rig.guitar_requests(), [Duration::ZERO]);
    }
}

#[test]
fn cancellation_before_open_reads_nothing() {
    let rig = Rig::default();
    rig.cancel.store(true, Ordering::Relaxed);
    assert!(matches!(rig.run(JobKind::Initial, None), Outcome::Deferred));
    assert!(rig.events.borrow().is_empty());
}

#[test]
fn cancellation_during_wait_or_backoff_releases_without_resending() {
    for time in [100, 3100, 5000] {
        let rig = Rig {
            cancel_guitar_at: Some(Duration::from_millis(time)),
            ..Default::default()
        };
        assert!(matches!(
            rig.run(JobKind::RefreshGuitar, RIG_CHECK_DELAY),
            Outcome::Finished { guitar: Guitar::Unavailable(reason), .. }
                if reason == "Identification cancelled."
        ));
        assert_eq!(rig.guitar_requests(), [Duration::ZERO]);
        assert!(rig.events.borrow().ends_with(&[
            Event::Finish(Interface::Variax),
            Event::Drop(Interface::Variax),
        ]));
    }
}

#[test]
fn stale_attachment_is_rejected_before_sending_on_interface_one() {
    for kind in [JobKind::Initial, JobKind::RefreshGuitar] {
        let rig = Rig {
            open_failure: Some((Interface::Variax, "The PODxt Live USB attachment changed.")),
            ..Default::default()
        };
        assert!(matches!(
            rig.run(kind, RIG_CHECK_DELAY),
            Outcome::Finished { guitar: Guitar::Unavailable(reason), .. }
                if reason.contains("attachment changed")
        ));
        assert!(rig.guitar_requests().is_empty());
    }
}

#[test]
fn guitar_open_refusals_name_the_stage_without_sending_identity() {
    for (kind, pod_firmware) in [
        (JobKind::Initial, Some("3.01")),
        (JobKind::RefreshGuitar, None),
    ] {
        for error in [
            "PODxt Live USB configuration descriptor: Entity not found",
            "Claim interface 1: Access denied (insufficient permissions)",
        ] {
            let rig = Rig {
                open_failure: Some((Interface::Variax, error)),
                ..Default::default()
            };
            let outcome = rig.run(kind, DISCOVERY_SILENCE_DELAY);
            assert!(matches!(
                outcome,
                Outcome::Finished { pod_firmware: actual, guitar: Guitar::Unavailable(reason) }
                    if actual == pod_firmware
                        && reason.strip_prefix("Variax USB interface 1 open failed: ") == Some(error)
            ));
            assert!(rig.guitar_requests().is_empty());
            assert_eq!(
                rig.events.borrow().last(),
                Some(&Event::Open(Interface::Variax))
            );
        }
    }
}

#[test]
fn failed_release_prevents_the_next_open_or_a_verified_result() {
    for side in [Interface::Podxt, Interface::Variax] {
        let rig = Rig {
            guitar_replies: vec![split_reply(guitar_reply())],
            fail_finish: Some(side),
            ..Default::default()
        };
        assert!(matches!(
            rig.run(JobKind::Initial, None),
            Outcome::Finished {
                guitar: Guitar::CleanupFailed,
                ..
            }
        ));
        assert_eq!(
            rig.events
                .borrow()
                .contains(&Event::Open(Interface::Variax)),
            side == Interface::Variax
        );
        assert!(
            rig.events
                .borrow()
                .ends_with(&[Event::Finish(side), Event::Drop(side)])
        );
    }
}
