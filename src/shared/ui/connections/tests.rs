use super::*;
use eframe::egui::{self, Pos2, Rect, vec2};

const POD: Attachment = Attachment {
    device: 1,
    generation: 1,
};
const GUITAR: Attachment = Attachment {
    device: 2,
    generation: 1,
};
const HD: Attachment = Attachment {
    device: 3,
    generation: 1,
};
const OTHER: Attachment = Attachment {
    device: 4,
    generation: 1,
};

fn device(name: &str) -> Device {
    Device {
        name: name.into(),
        artwork: None,
        offline_editor: None,
        upstream: None,
    }
}

fn pod() -> Device {
    Device {
        artwork: Some("shell/podxt-live.png"),
        offline_editor: Some("podxt"),
        ..device("PODxt Live")
    }
}

fn guitar(upstream: Option<Attachment>) -> Device {
    Device {
        upstream,
        artwork: Some("shell/variax.png"),
        offline_editor: Some("variax"),
        ..device("Variax 300/600")
    }
}

fn status(connections: &Connections, attachment: Attachment) -> &Status {
    &connections
        .rows
        .iter()
        .find(|row| row.attachment == attachment)
        .unwrap()
        .status
}

fn started(connections: &mut Connections, attachment: Attachment, total: usize) -> Read {
    connections
        .started(attachment, NonZeroUsize::new(total).unwrap())
        .unwrap()
}

#[test]
fn simultaneous_arrivals_keep_transport_waiting_and_use_the_selected_read_totals() {
    let mut connections = Connections::default();
    connections.detection(Detection::Checking);
    connections.arrived(POD, pod());
    connections.arrived(GUITAR, guitar(Some(POD)));
    connections.detection(Detection::Complete);
    assert_eq!(status(&connections, POD), &Status::Checking);
    assert!(
        connections
            .started(POD, NonZeroUsize::new(128).unwrap())
            .is_none()
    );

    for attachment in [POD, GUITAR] {
        connections.verified(attachment, Verification::Verified);
    }
    connections.waiting(GUITAR);
    let read = started(&mut connections, POD, 299);
    connections.progress(read, 128);
    connections.progress(read, 299);
    connections.progress(read, 300);
    connections.progress(read, 127);
    assert!(matches!(
        status(&connections, POD),
        Status::Reading { done: 299, total, .. } if total.get() == 299
    ));
    assert_eq!(status(&connections, GUITAR), &Status::Waiting);

    let now = Instant::now();
    connections.finished(read, Ok(()));
    assert_eq!(connections.advance(now), None);
    let read = started(&mut connections, GUITAR, 32);
    connections.progress(read, 32);
    connections.finished(read, Ok(()));
    assert_eq!(connections.advance(now), Some(COMPLETE_PAUSE));
    assert_eq!(connections.advance(now + COMPLETE_PAUSE), None);
    assert!(!connections.visible);
}

#[test]
fn arrival_at_the_close_deadline_starts_a_new_completion_interval() {
    let mut connections = Connections::default();
    connections.detection(Detection::Complete);
    connections.arrived(POD, pod());
    connections.verified(POD, Verification::Verified);
    let read = started(&mut connections, POD, 128);
    connections.finished(read, Ok(()));
    let now = Instant::now();
    connections.advance(now);

    connections.arrived(HD, device("POD HD500X"));
    connections.verified(HD, Verification::Unsupported);
    assert_eq!(
        connections.advance(now + COMPLETE_PAUSE),
        Some(COMPLETE_PAUSE)
    );
    assert!(connections.visible);
    assert_eq!(connections.rows[0].attachment, POD);
    assert_eq!(connections.rows[1].attachment, HD);
    assert_eq!(connections.advance(now + COMPLETE_PAUSE * 2), None);
    assert!(!connections.visible);

    // A later unsupported-only arrival should stay visible for its action.
    connections.arrived(OTHER, device("Other Line 6 device"));
    connections.verified(OTHER, Verification::Unsupported);
    assert_eq!(connections.advance(now + COMPLETE_PAUSE * 3), None);
    assert!(connections.visible);
}

#[test]
fn disconnect_at_the_close_deadline_is_sticky_through_quick_replug() {
    let mut connections = Connections::default();
    connections.detection(Detection::Complete);
    connections.arrived(POD, pod());
    connections.verified(POD, Verification::Verified);
    let old_read = started(&mut connections, POD, 128);
    connections.finished(old_read, Ok(()));
    let now = Instant::now();
    connections.advance(now);
    connections.disconnected(POD);
    assert_eq!(connections.advance(now + COMPLETE_PAUSE), None);
    assert_eq!(status(&connections, POD), &Status::Disconnected);
    assert!(connections.visible);

    let replug = Attachment {
        generation: 2,
        ..POD
    };
    connections.arrived(replug, pod());
    connections.arrived(POD, pod());
    connections.verified(POD, Verification::Unsupported);
    connections.disconnected(POD);
    connections.verified(replug, Verification::Verified);
    let read = started(&mut connections, replug, 299);
    connections.progress(old_read, 99);
    connections.finished(old_read, Err("late failure".into()));
    assert!(matches!(
        status(&connections, replug),
        Status::Reading { done: 0, .. }
    ));
    connections.finished(read, Ok(()));
    assert_eq!(connections.advance(now + COMPLETE_PAUSE * 2), None);
    assert!(connections.visible);
    assert!(connections.rows[0].reconnected);

    connections.dismiss();
    connections.detection(Detection::Complete);
    connections.arrived(replug, pod());
    assert!(!connections.visible);
    connections.disconnected(replug);
    let after_dismissal = Attachment {
        generation: 3,
        ..POD
    };
    connections.arrived(after_dismissal, pod());
    assert!(connections.visible);
    assert!(!connections.manual_close);
    assert_eq!(connections.rows.len(), 1);
}

#[test]
fn upstream_disconnect_leaves_other_transports_and_rejects_old_linked_results() {
    let mut connections = Connections::default();
    connections.arrived(POD, pod());
    connections.arrived(GUITAR, guitar(Some(POD)));
    connections.arrived(OTHER, guitar(None));
    for attachment in [POD, GUITAR, OTHER] {
        connections.verified(attachment, Verification::Verified);
    }
    let linked = started(&mut connections, GUITAR, 32);
    let independent = started(&mut connections, OTHER, 32);
    let new_pod = Attachment {
        generation: 2,
        ..POD
    };
    connections.arrived(new_pod, pod());

    connections.finished(linked, Ok(()));
    assert_eq!(status(&connections, GUITAR), &Status::Disconnected);
    connections.progress(independent, 17);
    assert!(matches!(
        status(&connections, OTHER),
        Status::Reading { done: 17, .. }
    ));
    let new_guitar = Attachment {
        generation: 2,
        ..GUITAR
    };
    connections.arrived(new_guitar, guitar(Some(POD)));
    assert_eq!(connections.rows[1].attachment, GUITAR);
    connections.arrived(new_guitar, guitar(Some(new_pod)));
    assert_eq!(connections.rows[1].attachment, new_guitar);
    assert_eq!(status(&connections, new_guitar), &Status::Checking);
    assert!(connections.manual_close);
}

#[test]
fn failed_read_waits_for_retry_and_ignores_the_previous_attempt() {
    let mut connections = Connections::default();
    connections.detection(Detection::Complete);
    connections.arrived(POD, pod());
    connections.arrived(OTHER, guitar(None));
    connections.verified(POD, Verification::Verified);
    connections.verified(OTHER, Verification::Verified);
    let first = started(&mut connections, POD, 299);
    let unrelated = started(&mut connections, OTHER, 32);
    connections.finished(first, Err("Reader stalled".into()));
    connections.finished(unrelated, Ok(()));
    let now = Instant::now();
    assert_eq!(connections.advance(now), None);
    assert_eq!(
        status(&connections, POD),
        &Status::Failed("Reader stalled".into())
    );
    assert!(
        connections
            .started(POD, NonZeroUsize::new(299).unwrap())
            .is_none()
    );

    assert!(connections.accept_retry(POD));
    assert!(!connections.accept_retry(POD));
    let retry = started(&mut connections, POD, 299);
    connections.progress(first, 128);
    connections.finished(first, Ok(()));
    assert!(matches!(
        status(&connections, POD),
        Status::Reading { done: 0, .. }
    ));
    connections.finished(retry, Ok(()));
    assert_eq!(connections.advance(now), Some(COMPLETE_PAUSE));
}

struct Screen {
    ctx: egui::Context,
    connections: Connections,
    now: Instant,
    artwork: Vec<&'static str>,
    action: Option<Action>,
}

impl Screen {
    fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            connections: Connections::default(),
            now: Instant::now(),
            artwork: Vec::new(),
            action: None,
        }
    }

    fn draw(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.artwork.clear();
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1120.0, 760.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                self.action = show(ctx, &mut self.connections, self.now, |_, path, _| {
                    self.artwork.push(path);
                    true
                });
            },
        )
    }

    fn click_text(&mut self, text: &str) {
        let pos = text_shape(&self.draw(vec![]), text).center();
        self.draw(vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, true),
            pointer(pos, false),
        ]);
    }
}

fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn text_shape(output: &egui::FullOutput, expected: &str) -> Rect {
    fn find(shape: &egui::Shape, expected: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == expected => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, expected)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, expected))
        .unwrap_or_else(|| panic!("missing text: {expected}"))
}

#[test]
fn startup_no_device_offline_action_and_distinct_detection_errors_render() {
    let mut screen = Screen::new();
    screen.connections.detection(Detection::Checking);
    screen.draw(vec![]);
    text_shape(&screen.draw(vec![]), "Checking for devices…");
    screen.connections.detection(Detection::Complete);
    let output = screen.draw(vec![]);
    text_shape(&output, "No device connected");
    text_shape(&output, "No supported device is connected");
    screen.click_text("Edit offline");
    assert_eq!(screen.action, Some(Action::EditOffline));
    assert!(!screen.connections.visible);

    screen
        .connections
        .detection(Detection::Unavailable("No USB backend".into()));
    let output = screen.draw(vec![]);
    text_shape(&output, "Device detection unavailable");
    text_shape(&output, "No USB backend");
    screen
        .connections
        .detection(Detection::Failed("Permission denied".into()));
    let output = screen.draw(vec![]);
    text_shape(&output, "Device check failed");
    text_shape(&output, "Permission denied");
}

#[test]
fn startup_window_replaces_no_device_status_in_place_on_first_arrival() {
    let mut screen = Screen::new();
    screen.connections.detection(Detection::Complete);
    screen.draw(vec![]);
    text_shape(&screen.draw(vec![]), "No supported device is connected");
    let id = egui::Id::new(view::WINDOW_ID);
    let rect = |screen: &Screen| egui::AreaState::load(&screen.ctx, id).unwrap().rect();
    let initial_corner = rect(&screen).min;
    screen.connections.arrived(POD, pod());
    let output = screen.draw(vec![]);
    text_shape(&output, "PODxt Live");
    text_shape(&output, "Checking connection…");
    assert_eq!(rect(&screen).min, initial_corner);
}

#[test]
fn window_keeps_its_dragged_position_and_draws_progress_error_and_offline_rows() {
    let mut screen = Screen::new();
    let c = &mut screen.connections;
    c.detection(Detection::Checking);
    c.arrived(POD, pod());
    c.arrived(GUITAR, guitar(Some(POD)));
    c.detection(Detection::Complete);
    c.verified(POD, Verification::Verified);
    c.verified(GUITAR, Verification::Verified);
    c.waiting(GUITAR);
    let read = started(c, POD, 299);
    c.progress(read, 65);

    screen.draw(vec![]);
    let output = screen.draw(vec![]);
    text_shape(&output, "Model Sync 65 of 299");
    text_shape(&output, "Waiting to sync");
    let title = text_shape(&output, "Device connections").center();
    let id = egui::Id::new(view::WINDOW_ID);
    let rect = |screen: &Screen| egui::AreaState::load(&screen.ctx, id).unwrap().rect();
    assert!(rect(&screen).center().distance(egui::pos2(560.0, 380.0)) < 2.0);
    screen.draw(vec![egui::Event::PointerMoved(title), pointer(title, true)]);
    let dragged = title + vec2(-140.0, 50.0);
    screen.connections.progress(read, 128);
    let output = screen.draw(vec![egui::Event::PointerMoved(dragged)]);
    text_shape(&output, "Model Sync 128 of 299");
    screen.draw(vec![pointer(dragged, false)]);
    assert!(
        (rect(&screen).center() - egui::pos2(560.0, 380.0) - vec2(-140.0, 50.0)).length() < 2.0
    );

    let previous = rect(&screen).min;
    let c = &mut screen.connections;
    c.arrived(
        HD,
        Device {
            offline_editor: Some("hd500x"),
            ..device("POD HD500X")
        },
    );
    c.verified(HD, Verification::Unsupported);
    c.finished(read, Err("Read timed out".into()));
    let guitar_read = started(c, GUITAR, 32);
    c.progress(guitar_read, 1);
    let output = screen.draw(vec![]);
    let failed = text_shape(&output, "Sync failed");
    let progress = text_shape(&output, "Model Sync 1 of 32");
    let unsupported = text_shape(&output, "POD HD500X Connected");
    assert!(failed.top() < progress.top() && progress.top() < unsupported.top());
    text_shape(&output, "Live connection not supported");
    assert_eq!(screen.artwork, ["shell/podxt-live.png", "shell/variax.png"]);
    assert_eq!(rect(&screen).min, previous);
    screen.click_text("Retry");
    assert_eq!(screen.action, Some(Action::Retry(POD)));
    assert!(screen.connections.accept_retry(POD));

    screen.connections.disconnected(POD);
    text_shape(&screen.draw(vec![]), "Disconnected");
    let replug = Attachment {
        generation: 2,
        ..POD
    };
    screen.connections.arrived(replug, pod());
    screen.connections.verified(replug, Verification::Verified);
    let fresh = started(&mut screen.connections, replug, 299);
    screen.connections.finished(fresh, Ok(()));
    screen.connections.finished(guitar_read, Ok(()));
    screen.now += COMPLETE_PAUSE * 2;
    let output = screen.draw(vec![]);
    text_shape(&output, "Reconnected after disconnect");
    text_shape(&output, "Model Sync Complete");
    assert_eq!(rect(&screen).min, previous);
    screen.click_text("Close");
    assert!(!screen.connections.visible);
}

#[test]
fn unverified_and_unsupported_rows_can_offer_offline_editing_without_reading() {
    let mut screen = Screen::new();
    let c = &mut screen.connections;
    c.detection(Detection::Complete);
    c.arrived(
        HD,
        Device {
            offline_editor: Some("hd500x"),
            ..device("POD HD500X")
        },
    );
    c.verified(HD, Verification::Unsupported);
    c.arrived(POD, pod());
    c.verified(POD, Verification::Unsupported);
    c.arrived(GUITAR, guitar(None));
    c.verified(
        GUITAR,
        Verification::UnverifiedFirmware("Firmware 3.09".into()),
    );
    assert!(c.started(GUITAR, NonZeroUsize::new(32).unwrap()).is_none());

    screen.draw(vec![]);
    let output = screen.draw(vec![]);
    text_shape(&output, "POD HD500X Connected");
    text_shape(&output, "PODxt Live Connected");
    text_shape(&output, "Unverified firmware");
    text_shape(&output, "Firmware 3.09");
    assert_eq!(screen.artwork, ["shell/podxt-live.png", "shell/variax.png"]);
    screen.click_text("Open offline editor");
    assert_eq!(screen.action, Some(Action::OpenOfflineEditor("hd500x")));
}
