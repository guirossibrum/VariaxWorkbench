//! Where device links come from: the practice rig or the real rig.
//!
//! This is Rig Check's small internal adapter. The shared procedures from
//! #245 (Variax) and #246 (PODxt) will replace the procedure wrappers in
//! `procedures.rs`; the links here stay.

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

/// The real rig. Every open first checks the step's owner approval and its
/// hardware gates. Test builds return before opening a device.
pub struct RealPort {
    system: Box<dyn FileSource>,
    #[cfg(target_os = "linux")]
    podxt: Option<line6_devices::transport::rawmidi::RawMidiLink>,
    #[cfg(feature = "usb")]
    variax: Option<line6_devices::transport::usb::UsbInterfaceLink>,
    #[cfg(feature = "usb")]
    lock: Option<line6_devices::device_lock::DeviceLock>,
}

impl RealPort {
    /// The real rig and this computer's real files. Test builds get an
    /// empty tree instead, so they can never read `/proc` or `/sys`.
    pub fn new() -> Self {
        let system: Box<dyn FileSource> = if cfg!(test) {
            Box::new(MemFs::default())
        } else {
            Box::new(RootFs { root: "/".into() })
        };
        Self::with_system(system)
    }

    /// The real rig with an injected file tree (tests).
    pub fn with_system(system: Box<dyn FileSource>) -> Self {
        Self {
            system,
            #[cfg(target_os = "linux")]
            podxt: None,
            #[cfg(feature = "usb")]
            variax: None,
            #[cfg(feature = "usb")]
            lock: None,
        }
    }
}

impl Default for RealPort {
    fn default() -> Self {
        Self::new()
    }
}

fn refuse_in_tests() -> Result<(), String> {
    if cfg!(test) {
        return Err("Test builds never open a real device.".into());
    }
    Ok(())
}

impl RigPort for RealPort {
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
        #[cfg(not(target_os = "linux"))]
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
        #[cfg(feature = "usb")]
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
        #[cfg(not(feature = "usb"))]
        {
            Err("This build has no USB support (feature `usb`).".into())
        }
    }

    fn system(&self) -> Box<dyn FileSource + '_> {
        Box::new(Borrowed(&*self.system))
    }

    fn variax_claimed(&self) -> bool {
        #[cfg(feature = "usb")]
        {
            self.variax.is_some()
        }
        #[cfg(not(feature = "usb"))]
        {
            false
        }
    }

    fn release_variax(&mut self) -> Result<Vec<String>, String> {
        #[cfg(feature = "usb")]
        if let Some(mut link) = self.variax.take() {
            link.finish()?;
            return Ok(vec![
                "Guitar interface released; the driver binding was verified by the shared release (#239).".into(),
            ]);
        }
        Ok(vec!["The guitar interface was not open.".into()])
    }
}
