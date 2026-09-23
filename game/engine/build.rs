//! RUSTFLAGS silently replaces a workspace's `[build] rustflags`, including
//! `-C llvm-args=-fp-contract=off`, which keeps floating point unfused on every
//! host. The game workspace and every generated game shell set
//! EXACT_GAME_FP_CONTRACT=off in `.cargo/config.toml` `[env]` (which RUSTFLAGS
//! does not replace); under that promise a build without the flag is refused.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#rustflags-keep-contraction-off-2026-09-23
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=EXACT_GAME_FP_CONTRACT");
    let env = |name| std::env::var(name).unwrap_or_default();
    let flags = env("CARGO_ENCODED_RUSTFLAGS");
    // With --target, host units (a shell build script's copy of this crate,
    // which runs no simulation) get no rustflags at all; target units build
    // under target/<triple>/. Without --target every unit gets the flags.
    let (target, out) = (env("TARGET"), env("OUT_DIR"));
    let target_unit = std::path::Path::new(&out)
        .components()
        .any(|part| part.as_os_str() == target.as_str());
    if env("EXACT_GAME_FP_CONTRACT") == "off"
        && (target_unit || !flags.is_empty())
        && !flags
            .split('\x1f')
            .any(|flag| flag.ends_with("llvm-args=-fp-contract=off"))
    {
        panic!(
            "RUSTFLAGS replaced the game workspace's `-C llvm-args=-fp-contract=off`, so floating \
             point may fuse differently per host. Append it: \
             RUSTFLAGS=\"$RUSTFLAGS -C llvm-args=-fp-contract=off\", or unset RUSTFLAGS \
             (got {:?})",
            flags.replace('\x1f', " ")
        );
    }
}
