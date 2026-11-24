//! One native file or folder picker per window, polled without blocking egui.

use eframe::egui;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

pub(super) type PickFuture = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

#[derive(Clone)]
pub(super) struct FileFilter {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

pub(super) enum PickRequest {
    OpenEditorFile(Vec<FileFilter>),
    #[allow(dead_code)] // Editor-lane migration, see docs/non-blocking-pickers.md.
    OpenWorkbenchFile,
    SaveWorkbenchFile {
        name: String,
        extension: String,
    },
    #[allow(dead_code)]
    Artwork,
    OpenWav,
    SaveAudio {
        name: String,
        aiff: bool,
    },
    #[allow(dead_code)]
    OpenTone,
    #[allow(dead_code)]
    SaveTone {
        name: String,
    },
    Folder,
}

#[cfg(not(test))]
impl PickRequest {
    pub fn native(self) -> PickFuture {
        let dialog = rfd::AsyncFileDialog::new();
        match self {
            Self::OpenEditorFile(filters) => {
                let all: Vec<_> = filters
                    .iter()
                    .flat_map(|filter| filter.extensions)
                    .copied()
                    .collect();
                let mut dialog = dialog.add_filter("Editor files", &all);
                for filter in filters {
                    dialog = dialog.add_filter(filter.name, filter.extensions);
                }
                file_path(dialog.add_filter("All files", &["*"]).pick_file())
            }
            Self::OpenWorkbenchFile => file_path(
                dialog
                    .add_filter(
                        "Workbench model, bank or collection",
                        &["vwm", "vwb", "l6c"],
                    )
                    .pick_file(),
            ),
            Self::SaveWorkbenchFile { name, extension } => file_path(
                dialog
                    .add_filter("Workbench document", &[extension])
                    .set_file_name(name)
                    .save_file(),
            ),
            Self::Artwork | Self::Folder => file_path(dialog.pick_folder()),
            Self::OpenWav => file_path(dialog.add_filter("PCM WAV", &["wav"]).pick_file()),
            Self::SaveAudio { name, aiff } => {
                let (label, extensions): (_, &[_]) = if aiff {
                    ("Exact-clock AIFF", &["aiff", "aif"])
                } else {
                    ("PCM WAV", &["wav"])
                };
                file_path(
                    dialog
                        .add_filter(label, extensions)
                        .set_file_name(name)
                        .save_file(),
                )
            }
            Self::OpenTone => file_path(dialog.pick_file()),
            Self::SaveTone { name } => file_path(dialog.set_file_name(name).save_file()),
        }
    }
}

#[cfg(not(test))]
fn file_path(pick: impl Future<Output = Option<rfd::FileHandle>> + 'static) -> PickFuture {
    Box::pin(async move { pick.await.map(|file| file.path().to_path_buf()) })
}

pub(super) struct Picker<A> {
    pending: Option<Pending<A>>,
}

struct Pending<A> {
    action: Option<A>,
    future: PickFuture,
}

impl<A> Default for Picker<A> {
    fn default() -> Self {
        Self { pending: None }
    }
}

struct Repaint(egui::Context);

impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}

impl<A> Picker<A> {
    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Reject a second request before constructing its native future.
    pub fn start(&mut self, action: A, launch: impl FnOnce() -> PickFuture) -> bool {
        if self.is_open() {
            return false;
        }
        self.pending = Some(Pending {
            action: Some(action),
            future: launch(),
        });
        true
    }

    /// Drop the completion action while keeping the slot until the native picker closes.
    pub fn discard_if(&mut self, matches: impl FnOnce(&A) -> bool) {
        if let Some(pending) = self.pending.as_mut()
            && pending.action.as_ref().is_some_and(matches)
        {
            pending.action = None;
        }
    }

    pub fn poll(&mut self, egui: &egui::Context) -> Option<(A, Option<PathBuf>)> {
        let pending = self.pending.as_mut()?;
        let waker = Waker::from(Arc::new(Repaint(egui.clone())));
        let Poll::Ready(path) = pending
            .future
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
        else {
            return None;
        };
        let pending = self.pending.take().expect("the ready picker");
        pending.action.map(|action| (action, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn pending_picker_ignores_duplicates_and_returns_one_result() {
        let context = egui::Context::default();
        let ready = Rc::new(Cell::new(false));
        let ready_to_poll = ready.clone();
        let mut picker = Picker::default();
        assert!(picker.start(7, || Box::pin(std::future::poll_fn(move |_| {
            if ready_to_poll.get() {
                Poll::Ready(Some("destination.vwm".into()))
            } else {
                Poll::Pending
            }
        }))));
        assert!(picker.poll(&context).is_none());
        assert!(!picker.start(8, || panic!("would launch a duplicate picker")));
        ready.set(true);
        assert_eq!(
            picker.poll(&context),
            Some((7, Some("destination.vwm".into())))
        );
        assert!(picker.poll(&context).is_none());
        assert!(!picker.is_open());
    }

    #[test]
    fn cancel_completes_and_a_discarded_action_ignores_a_late_path() {
        let context = egui::Context::default();
        let mut picker = Picker::default();
        assert!(picker.start(1, || Box::pin(std::future::ready(None))));
        assert_eq!(picker.poll(&context), Some((1, None)));
        assert!(picker.start(2, || Box::pin(std::future::ready(Some("stale".into())))));
        picker.discard_if(|action| *action == 2);
        assert!(!picker.start(3, || panic!("the old dialog is still open")));
        assert!(picker.poll(&context).is_none());
        assert!(!picker.is_open());
    }
}
