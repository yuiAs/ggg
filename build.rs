use std::process::Command;

fn main() {
    // Embed the short git commit hash so the app can display its exact build revision.
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    // Rebuild when HEAD moves so the embedded hash stays current.
    println!("cargo:rerun-if-changed=.git/HEAD");
}
