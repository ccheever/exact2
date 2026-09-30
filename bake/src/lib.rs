//! Delivery baking (LLP 1030 D3a, D8; 1030.000 D4, D7): the compatibility id
//! a build writes beside its plan, and the embedded receipt it checks against
//! the publisher's. Build-side only: app build scripts, `js/bake` and the dev
//! server link it. The compiler (`contract`) and the runtimes do not, so they
//! carry neither ed25519 (through `exact-update`) nor naga (through
//! `exact-gpu-reflect`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod compat;
mod reach;
mod receipt;

pub use compat::{compatibility_id, compatibility_id_sources, Compat};
pub use receipt::write_development_artifacts;

/// The Bun a bake spawns: `BUN` names one, else the first on PATH. The scripts
/// put the Bun they checked against package.json's pin first (`developmentBuildEnv`).
pub fn bun() -> std::process::Command {
    std::process::Command::new(bun_program())
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
