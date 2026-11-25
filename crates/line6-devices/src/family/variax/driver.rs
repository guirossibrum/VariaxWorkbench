//! Expected Linux binding for the PODxt Live's Variax interface.

pub const EXPECTED_VARIAX_DRIVER: &str = "snd-usb-variax";

pub fn is_variax_driver(name: &str) -> bool {
    name.replace('_', "-") == EXPECTED_VARIAX_DRIVER
}

#[cfg(all(feature = "usb", target_os = "linux"))]
pub(crate) mod linux {
    use super::*;
    use std::fs;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::time::Duration;

    #[derive(Debug, PartialEq, Eq)]
    pub enum Binding {
        Expected,
        Unbound,
        Other(String),
    }

    pub fn check(root: &Path, port: &str, address: u8) -> Result<Binding, String> {
        let number = |part: &str| part.parse::<u8>().is_ok_and(|n| n != 0);
        if !port
            .split_once('-')
            .is_some_and(|(bus, ports)| number(bus) && ports.split('.').all(number))
        {
            return Err("No Linux USB port path for the PODxt Live.".into());
        }

        let devices = root.join("sys/bus/usb/devices");
        let device = devices.join(port);
        let check_address = || {
            if fs::read_to_string(device.join("devnum"))
                .ok()
                .and_then(|text| text.trim().parse::<u8>().ok())
                == Some(address)
            {
                Ok(())
            } else {
                Err(
                    "The PODxt Live USB attachment changed. Wait for automatic identity."
                        .to_string(),
                )
            }
        };

        check_address()?;
        // PODxt Live uses configuration 1. Check the interface itself exists
        // before treating a missing driver link as an unbound interface.
        let interface = devices.join(format!("{port}:1.1"));
        let binding = match fs::read_link(interface.join("driver")) {
            Ok(target) => {
                let name = target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                if is_variax_driver(name) {
                    Binding::Expected
                } else {
                    Binding::Other(name.into())
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound && interface.is_dir() => {
                Binding::Unbound
            }
            Err(error) => {
                return Err(format!(
                    "Could not verify the guitar interface driver: {error}."
                ));
            }
        };
        check_address()?;
        Ok(binding)
    }

    pub fn restore(
        mut check: impl FnMut() -> Result<Binding, String>,
        mut attach: impl FnMut() -> Result<(), String>,
        detached: bool,
        wait: impl FnOnce(Duration),
        log: impl Fn(&str),
    ) -> Result<(), String> {
        let mut check = || {
            let result = check();
            log(&match &result {
                Ok(Binding::Expected) => {
                    format!("PODxt Live interface 1: {EXPECTED_VARIAX_DRIVER} binding verified.")
                }
                Ok(Binding::Unbound) => "PODxt Live interface 1: guitar driver is unbound.".into(),
                Ok(Binding::Other(driver)) => {
                    format!("PODxt Live interface 1: unexpected driver {driver}.")
                }
                Err(error) => format!("PODxt Live interface 1: {error}"),
            });
            match result? {
                Binding::Other(driver) => {
                    Err(format!("Unexpected guitar interface driver {driver}."))
                }
                binding => Ok(binding),
            }
        };

        if check()? == Binding::Expected {
            return Ok(());
        }
        let mut attach = || {
            let attempt = attach();
            log(&match &attempt {
                Ok(()) => "PODxt Live interface 1: kernel reattach returned success.".into(),
                Err(error) => format!("PODxt Live interface 1: kernel reattach returned {error}."),
            });
            attempt
        };
        if detached {
            let _ = attach();
            if check()? == Binding::Expected {
                return Ok(());
            }
        }
        wait(Duration::from_millis(150));
        if check()? == Binding::Expected {
            return Ok(());
        }
        let attempt = attach();
        if check()? == Binding::Expected {
            return Ok(());
        }
        let detail = attempt
            .err()
            .map_or_else(String::new, |error| format!(" ({error})"));
        Err(format!(
            "The guitar interface driver is still unbound{detail}. Replug PODxt Live USB as a last resort."
        ))
    }
}

#[cfg(all(test, feature = "usb", target_os = "linux"))]
mod tests {
    use super::linux::{Binding, check, restore};
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::Duration;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "variax-driver-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap()
            ));
            let fixture = Self(root);
            fs::create_dir_all(fixture.sysfs()).unwrap();
            fs::create_dir(fixture.sysfs().join("8-2.1")).unwrap();
            fs::create_dir(fixture.sysfs().join("8-2.1:1.1")).unwrap();
            fixture.address(11);
            fixture
        }

        fn sysfs(&self) -> PathBuf {
            self.0.join("sys/bus/usb/devices")
        }

        fn address(&self, address: u8) {
            fs::write(self.sysfs().join("8-2.1/devnum"), format!("{address}\n")).unwrap();
        }

        fn bind(&self, name: &str) {
            symlink(
                format!("../../../../bus/usb/drivers/{name}"),
                self.sysfs().join("8-2.1:1.1/driver"),
            )
            .unwrap();
        }

        fn check(&self) -> Result<Binding, String> {
            check(&self.0, "8-2.1", 11)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn verifies_and_logs_a_binding_already_present_after_release() {
        let fixture = Fixture::new();
        fixture.bind("snd_usb_variax");
        let log = RefCell::new(Vec::new());
        restore(
            || fixture.check(),
            || panic!("already bound"),
            true,
            |_| panic!("already bound"),
            |line| log.borrow_mut().push(line.to_string()),
        )
        .unwrap();
        assert_eq!(
            log.into_inner(),
            ["PODxt Live interface 1: snd-usb-variax binding verified."]
        );
    }

    #[test]
    fn a_successful_attach_result_without_a_binding_gets_one_delayed_retry() {
        let fixture = Fixture::new();
        let calls = Cell::new(0);
        let waited = Cell::new(false);
        let log = RefCell::new(Vec::new());
        restore(
            || fixture.check(),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    assert!(waited.get());
                    fixture.bind("snd-usb-variax");
                }
                Ok(())
            },
            true,
            |duration| {
                assert_eq!(duration, Duration::from_millis(150));
                waited.set(true);
            },
            |line| log.borrow_mut().push(line.to_string()),
        )
        .unwrap();
        assert_eq!(calls.get(), 2);
        let log = log.into_inner();
        assert!(log.first().unwrap().contains("unbound"));
        assert!(log.last().unwrap().contains("binding verified"));
    }

    #[test]
    fn a_persistent_failure_suggests_replugging_only_after_the_retry() {
        let fixture = Fixture::new();
        let calls = Cell::new(0);
        let error = restore(
            || fixture.check(),
            || {
                calls.set(calls.get() + 1);
                Err("fake attach failure".into())
            },
            true,
            |_| {},
            |_| {},
        )
        .unwrap_err();
        assert_eq!(calls.get(), 2);
        assert!(error.contains("fake attach failure"));
        assert!(error.contains("Replug PODxt Live USB as a last resort"));
    }

    #[test]
    fn a_replacement_attachment_receives_no_retry() {
        let fixture = Fixture::new();
        let calls = Cell::new(0);
        let error = restore(
            || fixture.check(),
            || {
                calls.set(calls.get() + 1);
                Ok(())
            },
            true,
            |_| fixture.address(12),
            |_| {},
        )
        .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert!(error.contains("USB attachment changed"));
    }

    #[test]
    fn manual_repair_of_an_unbound_interface_tries_once_and_checks_the_actual_binding() {
        let fixture = Fixture::new();
        let calls = Cell::new(0);
        restore(
            || fixture.check(),
            || {
                calls.set(calls.get() + 1);
                fixture.bind("snd-usb-variax");
                Err("fake attach error despite kernel binding".into())
            },
            false,
            |_| {},
            |_| {},
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn leaves_an_unexpected_driver_in_place() {
        let fixture = Fixture::new();
        fixture.bind("snd-usb-pod");
        let error = restore(
            || fixture.check(),
            || panic!("do not attach over a different driver"),
            false,
            |_| panic!("the interface is already bound"),
            |_| {},
        )
        .unwrap_err();
        assert!(error.contains("Unexpected guitar interface driver snd-usb-pod"));
    }
}
