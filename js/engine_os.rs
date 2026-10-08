/// The operating systems whose builds link the Hermes engine (`build.rs`
/// builds the shim and sets `exact_js_engine`). Every target with a pinned
/// Hermes bundle must be one of these (`tests/it/pinned_engines.rs`): a target
/// left out builds, then refuses every TypeScript app at run time.
pub(crate) const ENGINE_OS: &[&str] = &["macos", "ios", "tvos", "linux", "windows", "android"];
