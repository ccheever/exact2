surfaces: windows, android
agent: Claude Code, Claude Opus 5.5
task: follow a change to the data module's native-module trait

### Rough

`exact_js::NativeModule::configure_storage` now takes an `exact_js::NativeStorage` (the app's three roots and its
SQLite files admitted under its grants) instead of three paths (LLP 1154 D3). The Windows and Android `Host`
modules keep no storage, so each takes the handle and ignores it, as it ignored the paths.

### Lean in

Nothing new: one signature in each crate.

### Needed

- none

### Checkpoints

- compiling: `cargo check --workspace --all-targets`
