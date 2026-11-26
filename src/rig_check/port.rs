//! Where device links come from: the practice rig or the real rig.
//!
//! The shared family procedures use these links through the same sessions
//! on both platforms.

use super::approvals;
use super::catalogue::StepId;
use super::computer::{FileSource, MemFs, RootFs, linux};
use super::practice::{SimLink, SimRig, UserAction};
use super::record::Mode;
use line6_devices::session::Link;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub trait RigPort {
    fn mode(&self) -> Mode;
    /// The PODxt Live MIDI link, opened on first use by an approved step.
    fn podxt(&mut self, step: StepId) -> Result<&mut dyn Link, String>;
    /// The guitar interface (PODxt Live interface 1), claimed on first use.
    fn variax(&mut self, step: StepId) -> Result<&mut dyn Link, String>;
    /// Whether the guitar interface is currently claimed by Rig Check.
    fn variax_claimed(&self) -> bool;
    /// Give the guitar interface back to the operating system and report the
    /// driver binding afterwards.
    fn release_variax(&mut self) -> Result<Vec<String>, String>;
    fn direct_usb(&self) -> bool {
        false
    }
    fn direct_usb_release_failed(&self) -> bool {
        false
    }
    fn finish_direct_usb(&mut self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn direct_usb_inventory(&mut self) -> Result<Vec<super::computer::UsbDevice>, String> {
        Err("No macOS USB inventory for this port.".into())
    }
    /// The computer's own files for steps 0.1, 0.2 and 8.1 (read-only).
    fn system(&self) -> Box<dyn FileSource + '_>;
    /// The libusb version this build links, if any.
    fn libusb(&self) -> String {
        super::computer::libusb_version()
    }
    /// Practice only: what the simulated person does during a guided step.
    fn user_acts(&mut self, _action: UserAction, _after: Duration) {}
    /// Practice only: the simulated rig.
    fn simulation(&self) -> Option<Rc<RefCell<SimRig>>> {
        None
    }
}

/// Finish the last claimed interface even if the return procedure failed.
pub(super) fn finish_direct_usb(
    port: &mut dyn RigPort,
    rec: &mut super::record::StepRecord,
    result: Result<(), String>,
) -> Result<(), String> {
    match port.finish_direct_usb() {
        Ok(lines) => {
            for line in lines {
                rec.show(format!("Rig: {line}"));
            }
            result
        }
        Err(error) => {
            rec.show(format!("Rig: USB cleanup failed: {error}"));
            Err(result.err().map_or_else(
                || error.clone(),
                |previous| format!("{previous} USB cleanup failed: {error}"),
            ))
        }
    }
}

pub struct PracticePort {
    rig: Rc<RefCell<SimRig>>,
    podxt: Option<SimLink>,
    variax: Option<SimLink>,
}

impl PracticePort {
    pub fn new(rig: Rc<RefCell<SimRig>>) -> Self {
        Self {
            rig,
            podxt: None,
            variax: None,
        }
    }
}

impl RigPort for PracticePort {
    fn mode(&self) -> Mode {
        Mode::Practice
    }

    fn podxt(&mut self, _step: StepId) -> Result<&mut dyn Link, String> {
        if self.rig.borrow().faults.podxt_absent {
            return Err("No PODxt Live on USB.".into());
        }
        let rig = &self.rig;
        Ok(self.podxt.get_or_insert_with(|| SimLink::podxt(rig)))
    }

    fn variax(&mut self, _step: StepId) -> Result<&mut dyn Link, String> {
        {
            let faults = &self.rig.borrow().faults;
            if faults.podxt_absent {
                return Err("Expected one USB device 0e41:4650, found 0.".into());
            }
            if faults.interface_busy {
                return Err("claim interface 1: Resource busy".into());
            }
            if faults.permission_denied {
                return Err("open: Access denied (insufficient permissions)".into());
            }
        }
        {
            let mut rig = self.rig.borrow_mut();
            rig.variax_claimed = true;
            rig.driver_bound = false;
        }
        let rig = &self.rig;
        Ok(self.variax.get_or_insert_with(|| SimLink::variax(rig)))
    }

    fn variax_claimed(&self) -> bool {
        self.variax.is_some()
    }

    fn release_variax(&mut self) -> Result<Vec<String>, String> {
        let was_claimed = self.variax.take().is_some();
        let mut rig = self.rig.borrow_mut();
        rig.variax_claimed = false;
        if was_claimed {
            rig.driver_bound = !rig.faults.driver_not_returned;
        }
        if rig.driver_bound {
            Ok(vec![
                "Guitar interface released.".into(),
                "Interface 1 driver: snd-usb-variax (expected).".into(),
            ])
        } else {
            Err("Driver not returned: interface 1 has no driver after one retry. Unplug and replug the PODxt Live USB cable, or use Refresh guitar in Variax Workbench.".into())
        }
    }

    fn user_acts(&mut self, action: UserAction, after: Duration) {
        self.rig.borrow_mut().user_acts(action, after);
    }

    fn system(&self) -> Box<dyn FileSource + '_> {
        let rig = self.rig.borrow();
        Box::new(linux::synthetic_machine(
            !rig.faults.podxt_absent,
            rig.driver_bound,
        ))
    }

    fn libusb(&self) -> String {
        "1.0.0 (simulated)".into()
    }

    fn simulation(&self) -> Option<Rc<RefCell<SimRig>>> {
        Some(self.rig.clone())
    }
}

struct Borrowed<'a>(&'a dyn FileSource);

impl FileSource for Borrowed<'_> {
    fn read(&self, path: &str) -> Option<String> {
        self.0.read(path)
    }
    fn link_name(&self, path: &str) -> Option<String> {
        self.0.link_name(path)
    }
    fn list(&self, dir: &str) -> Vec<String> {
        self.0.list(dir)
    }
    fn mode(&self, path: &str) -> Option<(u32, u32, u32)> {
        self.0.mode(path)
    }
}

#[cfg(all(target_os = "macos", feature = "usb"))]
use line6_devices::{
    device_lock::DeviceLock,
    transport::usb::{UsbInterfaceLink, macos, macos_inventory},
};

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Interface {
    Podxt,
    Variax,
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
impl Interface {
    fn number(self) -> u8 {
        match self {
            Self::Podxt => 0,
            Self::Variax => 1,
        }
    }
}

/// Keep only one live handle. A release error leaves interface ownership
/// uncertain, so the rest of this run cannot reopen either interface.
#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
pub(super) struct DirectUsb<L> {
    active: Option<(Interface, L)>,
    release_error: Option<String>,
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
impl<L> Default for DirectUsb<L> {
    fn default() -> Self {
        Self {
            active: None,
            release_error: None,
        }
    }
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
impl<L> DirectUsb<L> {
    pub(super) fn active(&self) -> Option<Interface> {
        self.active.as_ref().map(|(side, _)| *side)
    }

    pub(super) fn release_failed(&self) -> bool {
        self.release_error.is_some()
    }

    pub(super) fn link(
        &mut self,
        side: Interface,
        open: impl FnOnce() -> Result<L, String>,
        finish: impl FnOnce(&mut L) -> Result<(), String>,
    ) -> Result<&mut L, String> {
        if self.active() != Some(side) {
            self.release(finish)?;
            self.active = Some((side, open()?));
        }
        Ok(&mut self.active.as_mut().expect("link opened").1)
    }

    pub(super) fn release(
        &mut self,
        finish: impl FnOnce(&mut L) -> Result<(), String>,
    ) -> Result<Vec<String>, String> {
        if let Some(error) = &self.release_error {
            return Err(error.clone());
        }
        let Some((side, mut link)) = self.active.take() else {
            return Ok(Vec::new());
        };
        let number = side.number();
        if let Err(error) = finish(&mut link) {
            let error = format!("PODxt Live interface {number} release failed: {error}");
            self.release_error = Some(error.clone());
            return Err(error);
        }
        Ok(vec![format!("PODxt Live interface {number} released.")])
    }
}

/// Owns the macOS lock outside the port, so its link can borrow that lock.
/// The scope lives through an ordinary run and any restore offered first.
#[derive(Default)]
pub struct RigScope {
    #[cfg(all(target_os = "macos", feature = "usb"))]
    matched: std::cell::OnceCell<LockedAttachment>,
}

#[cfg(all(target_os = "macos", feature = "usb"))]
struct LockedAttachment {
    lock: DeviceLock,
    attachment: macos::Attachment,
    observed: macos_inventory::ObservedPodxt,
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
fn select_attachment<'a, A: Eq>(
    attachments: &'a [A],
    expected: Option<&A>,
) -> Result<&'a A, String> {
    let [attachment] = attachments else {
        return Err(format!(
            "Expected one PODxt Live on USB, found {}.",
            attachments.len()
        ));
    };
    if expected.is_some_and(|observed| observed != attachment) {
        return Err("The PODxt Live USB attachment changed. Start a new run.".into());
    }
    Ok(attachment)
}

/// The real rig. Every open first checks the step's owner approval and its
/// hardware gates. Test builds return before opening a device.
pub struct RealPort<'rig> {
    _scope: &'rig RigScope,
    system: Box<dyn FileSource>,
    #[cfg(target_os = "linux")]
    podxt: Option<line6_devices::transport::rawmidi::RawMidiLink>,
    #[cfg(all(target_os = "macos", feature = "usb"))]
    mac_usb: DirectUsb<UsbInterfaceLink<&'rig DeviceLock>>,
    #[cfg(all(target_os = "macos", feature = "usb"))]
    mac_inventory: Option<Vec<macos_inventory::ObservedPodxt>>,
    #[cfg(all(feature = "usb", not(target_os = "macos")))]
    variax: Option<line6_devices::transport::usb::UsbInterfaceLink>,
    #[cfg(all(feature = "usb", not(target_os = "macos")))]
    lock: Option<line6_devices::device_lock::DeviceLock>,
}

impl<'rig> RealPort<'rig> {
    /// The real rig and this computer's real files. Test builds get an
    /// empty tree instead, so they can never read `/proc` or `/sys`.
    pub fn new(scope: &'rig RigScope) -> Self {
        let system: Box<dyn FileSource> = if cfg!(test) {
            Box::new(MemFs::default())
        } else {
            Box::new(RootFs { root: "/".into() })
        };
        Self::with_system(scope, system)
    }

    /// The real rig with an injected file tree (tests).
    pub fn with_system(scope: &'rig RigScope, system: Box<dyn FileSource>) -> Self {
        Self {
            _scope: scope,
            system,
            #[cfg(target_os = "linux")]
            podxt: None,
            #[cfg(all(target_os = "macos", feature = "usb"))]
            mac_usb: DirectUsb::default(),
            #[cfg(all(target_os = "macos", feature = "usb"))]
            mac_inventory: None,
            #[cfg(all(feature = "usb", not(target_os = "macos")))]
            variax: None,
            #[cfg(all(feature = "usb", not(target_os = "macos")))]
            lock: None,
        }
    }
    #[cfg(all(target_os = "macos", feature = "usb"))]
    fn open_direct_usb(&mut self, side: Interface) -> Result<&mut dyn Link, String> {
        let scope = self._scope;
        if scope.matched.get().is_none() {
            let lock = DeviceLock::acquire().map_err(|error| error.to_string())?;
            let devices = macos_inventory::inventory()?;
            let observed = select_attachment(&devices, None)?;
            if let Some(previous) = &self.mac_inventory {
                let expected = select_attachment(previous, None)?;
                if !expected.matches_generation(observed) {
                    return Err("The PODxt Live USB attachment changed. Start a new run.".into());
                }
            }
            let candidate = observed
                .attachment(&devices)
                .ok_or("PODxt Live detected, but its USB interfaces are unavailable.")?;
            let attachments = macos::attachments()?;
            let attachment = select_attachment(&attachments, Some(&candidate))?;
            let _ = scope.matched.set(LockedAttachment {
                lock,
                attachment: attachment.clone(),
                observed: observed.clone(),
            });
        }
        let matched = scope
            .matched
            .get()
            .expect("rig lock and attachment selected");
        let spec = match side {
            Interface::Podxt => macos::PODXT_MIDI,
            Interface::Variax => line6_devices::transport::usb::VARIAX_VIA_PODXT_LIVE,
        };
        Ok(self.mac_usb.link(
            side,
            || {
                macos_inventory::verify_candidate(
                    &matched.lock,
                    matched.observed.registry_id,
                    &matched.attachment,
                )?;
                UsbInterfaceLink::open_attachment_borrowed(
                    spec,
                    &matched.attachment.path,
                    matched.attachment.address,
                    &matched.lock,
                )
            },
            UsbInterfaceLink::finish,
        )?)
    }
}

fn refuse_in_tests() -> Result<(), String> {
    if cfg!(test) {
        return Err("Test builds never open a real device.".into());
    }
    Ok(())
}

impl RigPort for RealPort<'_> {
    fn mode(&self) -> Mode {
        Mode::Real
    }

    fn podxt(&mut self, step: StepId) -> Result<&mut dyn Link, String> {
        approvals::real_device_check(step)?;
        refuse_in_tests()?;
        #[cfg(target_os = "linux")]
        {
            use line6_devices::family::podxt::transport::PODXT_LIVE_CARD;
            use line6_devices::session::Path;
            use line6_devices::transport::rawmidi::RawMidiLink;
            if self.podxt.is_none() {
                self.podxt = Some(RawMidiLink::open(PODXT_LIVE_CARD, Path::PodxtMidi)?);
            }
            Ok(self.podxt.as_mut().expect("opened above"))
        }
        #[cfg(all(target_os = "macos", feature = "usb"))]
        {
            self.open_direct_usb(Interface::Podxt)
        }
        #[cfg(not(any(target_os = "linux", all(target_os = "macos", feature = "usb"))))]
        {
            Err(
                "The PODxt Live MIDI link is not available on this operating system yet (#262)."
                    .into(),
            )
        }
    }

    fn variax(&mut self, step: StepId) -> Result<&mut dyn Link, String> {
        approvals::real_device_check(step)?;
        refuse_in_tests()?;
        #[cfg(all(feature = "usb", not(target_os = "macos")))]
        {
            use line6_devices::device_lock::DeviceLock;
            use line6_devices::transport::usb::{UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE};
            if self.lock.is_none() {
                self.lock = Some(DeviceLock::acquire().map_err(|error| error.to_string())?);
            }
            if self.variax.is_none() {
                self.variax = Some(UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE)?);
            }
            Ok(self.variax.as_mut().expect("opened above"))
        }
        #[cfg(all(target_os = "macos", feature = "usb"))]
        {
            self.open_direct_usb(Interface::Variax)
        }
        #[cfg(not(feature = "usb"))]
        {
            Err("This build has no USB support (feature `usb`).".into())
        }
    }

    fn direct_usb(&self) -> bool {
        crate::macos_usb_available()
    }

    #[cfg(all(target_os = "macos", feature = "usb"))]
    fn direct_usb_release_failed(&self) -> bool {
        self.mac_usb.release_failed()
    }

    fn finish_direct_usb(&mut self) -> Result<Vec<String>, String> {
        #[cfg(all(target_os = "macos", feature = "usb"))]
        {
            self.mac_usb.release(UsbInterfaceLink::finish)
        }
        #[cfg(not(all(target_os = "macos", feature = "usb")))]
        {
            Ok(Vec::new())
        }
    }

    #[cfg(all(target_os = "macos", feature = "usb"))]
    fn direct_usb_inventory(&mut self) -> Result<Vec<super::computer::UsbDevice>, String> {
        refuse_in_tests()?;
        let devices = macos_inventory::inventory()?;
        self.mac_inventory = Some(devices.clone());
        Ok(devices
            .into_iter()
            .map(|device| super::computer::UsbDevice {
                port: device
                    .route()
                    .map(str::to_owned)
                    .unwrap_or_else(|| device.label()),
                address: device.address,
                vendor: 0x0e41,
                product: 0x4650,
                name: Some("PODxt Live".into()),
                manufacturer: None,
                serial: None,
                speed: None,
                bus: None,
                interfaces: vec![],
                node: None,
            })
            .collect())
    }

    fn system(&self) -> Box<dyn FileSource + '_> {
        Box::new(Borrowed(&*self.system))
    }

    fn variax_claimed(&self) -> bool {
        #[cfg(all(target_os = "macos", feature = "usb"))]
        {
            self.mac_usb.active() == Some(Interface::Variax)
        }
        #[cfg(all(feature = "usb", not(target_os = "macos")))]
        {
            self.variax.is_some()
        }
        #[cfg(not(feature = "usb"))]
        {
            false
        }
    }

    fn release_variax(&mut self) -> Result<Vec<String>, String> {
        #[cfg(all(target_os = "macos", feature = "usb"))]
        if self.variax_claimed() {
            return self.finish_direct_usb();
        }
        #[cfg(all(feature = "usb", not(target_os = "macos")))]
        if let Some(mut link) = self.variax.take() {
            link.finish()?;
            return Ok(vec![
                "Guitar interface released; the driver binding was verified by the shared release (#239).".into(),
            ]);
        }
        Ok(vec!["The guitar interface was not open.".into()])
    }
}

#[cfg(test)]
mod tests {
    use super::select_attachment;

    #[test]
    fn attachment_selection_rejects_missing_ambiguous_or_changed_path_and_address() {
        let original = ("4-2", 17);
        let new_path = ("4-3", 17);
        let new_address = ("4-2", 18);

        assert_eq!(select_attachment(&[original], None), Ok(&original));
        assert_eq!(
            select_attachment(&[original], Some(&original)),
            Ok(&original)
        );
        assert!(
            select_attachment(&[], Some(&original))
                .unwrap_err()
                .contains("found 0")
        );
        assert!(
            select_attachment(&[original, new_path], Some(&original))
                .unwrap_err()
                .contains("found 2")
        );
        for changed in [new_path, new_address] {
            assert!(
                select_attachment(&[changed], Some(&original))
                    .unwrap_err()
                    .contains("attachment changed")
            );
        }
    }
}
