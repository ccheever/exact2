//! Delivery baking (LLP 1030 D3a, D8; 1030.000 D4, D7): the compatibility id
//! a build writes beside its plan, and the embedded receipt it checks against
//! the publisher's. Build-side only: app build scripts, `js/bake` and the dev
//! server link it. The compiler (`contract`) and the runtimes do not, so they
//! carry neither ed25519 (through `exact-update`) nor naga (through
//! `exact-gpu-reflect`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod compat;
mod receipt;

pub use compat::{compatibility_id, compatibility_id_sources, Compat};
pub use receipt::write_development_artifacts;

/// The Bun a bake spawns: `BUN` names one, else the first on PATH. The scripts
/// put the Bun they checked against package.json's pin first (`developmentBuildEnv`).
pub fn bun() -> std::process::Command {
    std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
}
