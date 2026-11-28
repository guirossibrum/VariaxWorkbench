use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RIG_CHECK_GIT_COMMIT");
    let revision = std::env::var("RIG_CHECK_GIT_COMMIT")
        .ok()
        .or_else(checkout_revision)
        .filter(|revision| {
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    println!(
        "cargo:rustc-env=VARIAX_LOG_REVISION={}",
        revision.as_deref().unwrap_or("unknown")
    );
}

fn checkout_revision() -> Option<String> {
    // Watch both HEAD and its ref so an incremental development build reflects
    // checkouts and commits, including in a linked worktree.
    let branch = git(&["symbolic-ref", "-q", "HEAD"]);
    for file in ["HEAD", "packed-refs"].into_iter().chain(branch.as_deref()) {
        if let Some(path) = git(&["rev-parse", "--git-path", file])
            && std::path::Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    git(&["rev-parse", "HEAD"])
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
