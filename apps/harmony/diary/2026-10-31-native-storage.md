surfaces: ios, macos, linux
agent: Claude Code, Claude Opus 5.5
task: follow a change to the data module's native-module trait

## Rough

`exact_js::NativeModule::configure_storage` now takes an `exact_js::NativeStorage`, the app's three roots and
its SQLite files admitted under its grants, instead of three paths (LLP 1154 D3). The Apple and Linux wrappers
pass the roots on to `exact_snapback4::Module::configure_storage`, which is unchanged: the adapter still admits
its partition against its own parse of the grants, until it takes `NativeStorage::sqlite` when Harmony moves to
the published package (LLP 1154 §5 step 3). Nothing else in the app changed.

## Lean in

The handle's roots are what the wrappers passed before, so the change is two lines in each.

## Needed

- local database: provided

## Checkpoints

- compiling the app's workspace: `cargo check --manifest-path apps/harmony/app/Cargo.toml --all-targets`
