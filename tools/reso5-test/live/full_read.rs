use super::{
    DeviceTask, Gate, Path, Trace, combine, install_cancellation, run_device, run_if_unlocked,
};
use crate::procedure::full_read;

// The owner directly approved one #234 run on 2026-11-24, after reviewer check.
const OWNER_RUN_APPROVED: bool = true;

pub(super) fn run(offline: bool, archive: &Path, log_root: &Path) -> Result<(), String> {
    if !offline && !OWNER_RUN_APPROVED {
        return Err("The #234 stepped buffer read has no owner approval for a live run.".into());
    }
    let trace = Trace::create(log_root, "full-read")?;
    let result = (|| {
        let factory = full_read::prepare(archive)?;
        trace.record("Verified the archived Reso-5 factory program.".into());
        if offline {
            return Ok(());
        }

        install_cancellation()?;
        run_if_unlocked(Gate::CurrentSettingsRead, || {
            run_if_unlocked(Gate::IdentityRequest, || {
                run_if_unlocked(Gate::OsDiscovery, || {
                    run_device(DeviceTask::FullRead(&factory), &trace)
                })
            })
        })
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: factory reference verified offline.".into(),
        Ok(()) => "Captured all five active-buffer replies.".into(),
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())?;
    if offline {
        println!("Offline check passed. Verified the factory Reso-5 program.");
    }
    Ok(())
}
