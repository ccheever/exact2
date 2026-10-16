//! Delivery baking (LLP 1030 D3a, D8; 1030.000 D4, D7): the compatibility id
//! a build writes beside its plan, and the embedded receipt it checks against
//! the publisher's. Build-side only: app build scripts, `js/bake` and the dev
//! server link it. The compiler (`contract`) and the runtimes do not, so they
//! carry neither ed25519 (through `exact-update`) nor naga (through
//! `exact-gpu-reflect`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod colors;
pub mod compat;
mod link;
mod reach;
mod receipt;

pub use compat::{compatibility_id, compatibility_id_sources, Compat};
pub use link::{ahead, apple_link};
pub use receipt::{plan_graph, write_development_artifacts};

/// The Bun a bake spawns: an explicit `BUN`, the versioned home install,
/// then PATH. Refuse an older executable before it can compile the app.
pub fn bun() -> std::process::Command {
    let program = bun_program();
    let output = std::process::Command::new(&program)
        .arg("--version")
        .output();
    if let Ok(output) = output {
        let have = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        assert!(output.status.success() && bun_version(&have) >= bun_version(bun_pin()),
            "Bun {have} is older than {}; install it with `curl -fsSL https://bun.sh/install | BUN_INSTALL=~/.bun-{} bash -s bun-v{}` and set BUN=~/.bun-{}/bin/bun", bun_pin(), bun_pin(), bun_pin(), bun_pin());
    }
    let mut command = std::process::Command::new(program);
    if let Some(helper) = filesystem_helper() {
        command.env("EXACT_FILESYSTEM_HELPER", helper);
    }
    command
}

/// The tooling filesystem helper (`scripts/filesystem.mjs`), checked current once per
/// process and named to every Bun this process spawns, each of which would otherwise run
/// Cargo to check it again; a process that was named one passes it on. `None` when it
/// cannot be resolved: each Bun then checks it for itself, and reports any failure.
fn filesystem_helper() -> Option<&'static std::ffi::OsString> {
    static HELPER: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
    HELPER
        .get_or_init(|| {
            if let Some(inherited) = std::env::var_os("EXACT_FILESYSTEM_HELPER") {
                return Some(inherited);
            }
            let script =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/filesystem.mjs");
            let output = std::process::Command::new(bun_program())
                .arg(script)
                .arg("--helper")
                .output()
                .ok()?;
            (output.status.success() && !output.stdout.is_empty())
                .then(|| String::from_utf8(output.stdout).ok().map(Into::into))
                .flatten()
        })
        .as_ref()
}

fn bun_pin() -> &'static str {
    include_str!("../../package.json")
        .split("bun@")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
}

fn bun_version(version: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = version.split('.').map(str::parse::<u32>).collect();
    match parts.as_slice() {
        [Ok(a), Ok(b), Ok(c)] => Some([*a, *b, *c]),
        _ => None,
    }
}

fn bun_program() -> std::ffi::OsString {
    if let Some(program) = std::env::var_os("BUN") {
        return program;
    }
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let path = std::path::PathBuf::from(home).join(format!(
            ".bun-{}/bin/{}",
            bun_pin(),
            if cfg!(windows) { "bun.exe" } else { "bun" }
        ));
        if path.is_file() {
            return path.into_os_string();
        }
    }
    "bun".into()
}

/// Why a `bun()` spawn failed, naming the program and the fix: the OS's own
/// error is a bare "No such file or directory", which reads as a missing
/// source file.
pub fn bun_error(e: std::io::Error) -> String {
    let bun = bun_program();
    let bun = bun.to_string_lossy();
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("{bun}: not found; the bake runs Bun (install it, put ~/.bun/bin on PATH, or name it with BUN)")
    } else {
        format!("{bun}: {e}")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bun_versions_compare_numerically_and_reject_unknown_versions() {
        assert!(super::bun_version("1.3.99") < super::bun_version("1.4.2"));
        assert!(super::bun_version("1.10.0") > super::bun_version("1.4.2"));
        assert!(super::bun_version("unknown") < super::bun_version("1.4.2"));
    }

    /// A missing Bun is named with its fix, not reported as the OS's bare error.
    #[test]
    fn a_missing_bun_is_named() {
        let missing = super::bun_error(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(
            missing.contains("not found; the bake runs Bun"),
            "{missing}"
        );
        let other = super::bun_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(!other.contains("not found"), "{other}");
    }
}
