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
pub use receipt::write_development_artifacts;

/// The Bun a bake spawns: `BUN` names one, else the first on PATH. The scripts
/// put the Bun they checked against package.json's pin first (`developmentBuildEnv`).
pub fn bun() -> std::process::Command {
    let mut command = std::process::Command::new(bun_program());
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

fn bun_program() -> std::ffi::OsString {
    std::env::var_os("BUN").unwrap_or_else(|| "bun".into())
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
