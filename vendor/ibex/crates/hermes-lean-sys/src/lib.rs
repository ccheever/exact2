//! Link metadata for the repository's pinned, unmodified Hermes engine.
//!
//! Resolution prefers a caller-supplied install, then this repository's local
//! development layout, then a SHA-256-pinned release bundle in the per-user
//! Cargo cache.

// @ref LLP 0057.000#l1--the-bindings-door — R-e requires one linked archive
// identity per process; Cargo feature unification must not select both VMs.
#[cfg(all(feature = "link", feature = "link-lean"))]
compile_error!(
    "hermes-lean-sys features `link` and `link-lean` are mutually exclusive; select exactly one VM archive"
);

/// Full VM archive selected for this target. The owning Ibex runtime needs
/// its source entrance; the `link` feature controls only link-line emission.
pub const ARCHIVE: &str = env!("HERMES_LEAN_ARCHIVE");

/// Digest of [`ARCHIVE`], independent of the `link` feature.
pub const ENGINE_DIGEST: &str = env!("HERMES_LEAN_ENGINE_DIGEST");

/// Lean VM archive selected for this target, if the install contains one.
/// When a receipt is present, this is exported only after its archive manifest
/// authenticates the lean bytes. It is `None` when no lean archive exists;
/// requesting `link-lean` makes that absence a resolution error.
pub const LEAN_ARCHIVE: Option<&str> = option_env!("HERMES_LEAN_LEAN_ARCHIVE");

/// Digest of [`LEAN_ARCHIVE`] when present and, if a receipt exists,
/// authenticated by that receipt's archive manifest.
pub const LEAN_ENGINE_DIGEST: Option<&str> = option_env!("HERMES_LEAN_LEAN_ENGINE_DIGEST");

/// HBC version reported by the matching `hermesc`.
pub const BYTECODE_VERSION: &str = env!("HERMES_LEAN_BYTECODE_VERSION");

/// Compiler paired with the selected bundle, verified by the resolver.
pub const HERMESC_PATH: &str = env!("HERMES_LEAN_HERMESC_PATH");

/// HBC version consumed by [`LEAN_ARCHIVE`], or `None` when no lean archive is
/// present in the selected install.
pub const LEAN_BYTECODE_VERSION: Option<&str> = option_env!("HERMES_LEAN_LEAN_BYTECODE_VERSION");

/// Archive actually selected by `link` or `link-lean`, if either is enabled.
pub const LINKED_ARCHIVE: Option<&str> = option_env!("HERMES_LEAN_LINKED_ARCHIVE");

/// Digest of [`LINKED_ARCHIVE`], naming the archive this feature context links.
pub const LINKED_ENGINE_DIGEST: Option<&str> = option_env!("HERMES_LEAN_LINKED_ENGINE_DIGEST");

/// Receipt-bound trimmed root+en ICU data archive available on Linux.
pub const ICU_DATA_ARCHIVE: Option<&str> = option_env!("HERMES_LEAN_ICU_DATA_ARCHIVE");

/// Digest of [`ICU_DATA_ARCHIVE`].
pub const ICU_DATA_DIGEST: Option<&str> = option_env!("HERMES_LEAN_ICU_DATA_DIGEST");

/// Receipt-bound full ICU data archive available on Linux.
pub const ICU_FULL_DATA_ARCHIVE: Option<&str> = option_env!("HERMES_LEAN_ICU_FULL_DATA_ARCHIVE");

/// Digest of [`ICU_FULL_DATA_ARCHIVE`].
pub const ICU_FULL_DATA_DIGEST: Option<&str> = option_env!("HERMES_LEAN_ICU_FULL_DATA_DIGEST");

/// ICU data archive selected by `icu` or `icu-full-data` on Linux.
pub const LINKED_ICU_DATA_ARCHIVE: Option<&str> =
    option_env!("HERMES_LEAN_LINKED_ICU_DATA_ARCHIVE");

/// Digest of [`LINKED_ICU_DATA_ARCHIVE`], kept separate from the VM digest so
/// the linked identity names the exact locale-data variant.
pub const LINKED_ICU_DATA_DIGEST: Option<&str> = option_env!("HERMES_LEAN_LINKED_ICU_DATA_DIGEST");

/// Windows only: the operating-system ICU DLL behind `icu` (`icu.dll`). The
/// link names only its frozen `icuuc`/`icuin` forwarders; `ibex2` binds the
/// rest from System32's copy at run time. There is no archive and no digest:
/// the bytes belong to Windows and change with Windows Update, so the version
/// a process uses is an observed runtime fact (`ibex2::bindings::os_icu`),
/// never a pinned identity.
/// Absent elsewhere; [`LINKED_ICU_DATA_ARCHIVE`] is absent on Windows.
pub const LINKED_OS_ICU: Option<&str> = option_env!("HERMES_LEAN_LINKED_OS_ICU");

/// Keep this native-link dependency in binaries that call into Hermes through
/// a sibling C++ shim rather than through Rust FFI declared in this crate.
#[inline(never)]
pub fn ensure_linked() {}
