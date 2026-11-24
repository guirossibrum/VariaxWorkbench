use super::{
    CANCELLED, DevicePath, Gate, Log, Ordering, PODXT_LIVE_CARD, Path, RawMidiLink, Trace,
    TracedLink, UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE, combine, install_cancellation, now,
    run_if_unlocked,
};
use crate::procedure::slot_read::{self, PreparedSlotRead};
use line6_devices::device_lock::DeviceLock;

pub(super) fn run(
    offline: bool,
    variax_backup: &Path,
    podxt_backup: &Path,
    archive: &Path,
    log_root: &Path,
) -> Result<(), String> {
    let trace = Trace::create(log_root, "slot-read")?;
    let result = (|| {
        let (_, started) = now()?;
        trace.record(format!("Reso-5 0x44 slot read at {started}."));
        let prepared = slot_read::prepare(variax_backup, podxt_backup, archive)?;
        let evidence = "Verified backup evidence for Variax 300/600 firmware 3.10 and PODxt Live firmware 3.01, and the factory Reso-5 fingerprint.";
        trace.record(evidence.into());
        println!("{evidence}");
        if offline {
            return Ok(());
        }

        install_cancellation()?;
        run_if_unlocked(Gate::FullBackupRead, || {
            run_if_unlocked(Gate::IdentityRequest, || {
                run_if_unlocked(Gate::OsDiscovery, || read_device(&prepared, &trace))
            })
        })
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline preflight. No device was opened.".into(),
        Ok(()) => "Captured three replies. Review the comparison results.".into(),
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())?;
    if offline {
        println!("Offline check passed. No device was opened.");
    }
    Ok(())
}

fn read_device(prepared: &PreparedSlotRead, trace: &Trace) -> Result<(), String> {
    let stopping = || CANCELLED.load(Ordering::Relaxed) || trace.failed();
    if stopping() {
        return Err(
            "Cancelled or unable to write the log before checking the PODxt Live identity.".into(),
        );
    }
    let _rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
    let podxt = RawMidiLink::open(PODXT_LIVE_CARD, DevicePath::PodxtMidi)?;
    let mut log = trace;
    slot_read::after_podxt_identity(
        TracedLink {
            inner: podxt,
            trace,
            label: "PODxt",
        },
        &mut log,
        |log| read_variax(prepared, trace, log, stopping),
    )
}

fn read_variax(
    prepared: &PreparedSlotRead,
    trace: &Trace,
    log: &mut dyn Log,
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    if stopping() {
        return Err(
            "Cancelled or unable to write the log before opening the Variax interface.".into(),
        );
    }
    let mut usb = UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE)?;
    let captured = slot_read::capture(
        TracedLink {
            inner: &mut usb,
            trace,
            label: "Variax",
        },
        log,
        prepared,
        &trace.path,
        stopping,
    );
    let cleanup = usb.finish();
    trace.record(match &cleanup {
        Ok(()) => "Variax USB interface released to its driver.".into(),
        Err(error) => format!("Variax USB interface cleanup failed: {error}"),
    });
    let result = captured.map(|replies| {
        for line in slot_read::summarize(prepared, &replies) {
            println!("{line}");
            trace.record(line);
        }
    });
    combine(result, cleanup)
}
