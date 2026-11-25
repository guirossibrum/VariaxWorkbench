use std::ffi::OsString;

#[cfg(any(test, target_os = "linux"))]
mod confirmation;
#[cfg(any(test, target_os = "linux"))]
mod procedure;

#[cfg(all(not(test), target_os = "linux"))]
mod live;

const LISTEN_LIST_CARD: &str = include_str!("../../docs/variax-listen-list-test-card.md");
const SELECTION_CARD: &str = include_str!("../../docs/variax-any-selection-test-card.md");
const FULL_READ_CARD: &str = include_str!("../../docs/reso5-full-read-test-card.md");
const SLOT_READ_CARD: &str = include_str!("../../docs/reso5-slot-read-test-card.md");
const STORE_CARD: &str = include_str!("../../docs/reso5-store-test-card.md");
const CARD: &str = include_str!("../../docs/reso5-test-card.md");

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("STOP: {error}");
        std::process::exit(1);
    }
}

fn run(args: &[OsString]) -> Result<(), String> {
    if args == ["--listen-list-card"] {
        print!("{LISTEN_LIST_CARD}");
        return Ok(());
    }
    if args == ["--any-selection-card"] {
        print!("{SELECTION_CARD}");
        return Ok(());
    }
    if args == ["--full-read-card"] {
        print!("{FULL_READ_CARD}");
        return Ok(());
    }
    if args == ["--slot-read-card"] {
        print!("{SLOT_READ_CARD}");
        return Ok(());
    }
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
        live::run(args)
    }
    #[cfg(any(test, not(target_os = "linux")))]
    {
        Err("This build prints the test card only. No device was opened.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cannot_open_a_device_for_listen_list() {
        let args = [
            "listen-list",
            "backup",
            "archive",
            "models",
            "logs",
            "A.vwm",
        ]
        .map(OsString::from);
        assert_eq!(
            run(&args).unwrap_err(),
            "This build prints the test card only. No device was opened."
        );
    }

    #[test]
    fn test_build_cannot_open_a_device_for_full_read() {
        let args = ["full-read", "archive", "logs"].map(OsString::from);
        assert_eq!(
            run(&args).unwrap_err(),
            "This build prints the test card only. No device was opened."
        );
    }

    #[test]
    fn test_build_cannot_open_a_device_for_slot_read() {
        let args = [
            "slot-read",
            "variax-backup",
            "podxt-backup",
            "archive",
            "logs",
        ]
        .map(OsString::from);
        assert_eq!(
            run(&args).unwrap_err(),
            "This build prints the test card only. No device was opened."
        );
    }
}
