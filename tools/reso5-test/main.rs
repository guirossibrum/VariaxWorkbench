#[cfg(any(test, target_os = "linux"))]
mod procedure;

#[cfg(all(not(test), target_os = "linux"))]
mod live;

const CARD: &str = include_str!("../../docs/reso5-test-card.md");

fn main() {
    if let Err(error) = run() {
        eprintln!("STOP: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--card"] {
        print!("{CARD}");
        return Ok(());
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        live::run(&args)
    }
    #[cfg(any(test, not(target_os = "linux")))]
    {
        Err("This build prints the test card only. No device was opened.".into())
    }
}
