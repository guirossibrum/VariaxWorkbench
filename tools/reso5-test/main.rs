#[cfg(any(test, target_os = "linux"))]
mod procedure;

#[cfg(all(not(test), target_os = "linux"))]
mod live;

const STORE_CARD: &str = include_str!("../../docs/reso5-store-test-card.md");
const CARD: &str = include_str!("../../docs/reso5-test-card.md");

fn main() {
    if let Err(error) = run() {
        eprintln!("STOP: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--store-card"] {
        print!("{STORE_CARD}");
        return Ok(());
    }
    if args == ["--card"] {
        print!("{CARD}");
        return Ok(());
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        let _rig =
            line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string())?;
        live::run(&args)
    }
    #[cfg(any(test, not(target_os = "linux")))]
    {
        Err("This build prints the test card only. No device was opened.".into())
    }
}
