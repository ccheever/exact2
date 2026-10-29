# Code review: a game with data bakes with it in the web dev loop (host/web red #7, tables), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `2382287979`.
- **Method:** one brief (sha256 `db5f4215cf970795ac36e2abdf6eb2888653724b1453b5ebc02f7fb30bad0495`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

One finding. The shell generator, the dev bin, and the 300 s wait behave as intended.

1. [P3] The queue item says a game's dev bin rebuilds the web lib, and this shell does not link that lib — `QUEUE.md:3`

The new web target is still `crate-type = ["cdylib"]` only (`game/app/shells.mjs:275`). Cargo links a same-package lib into a bin only when that lib is an rlib, dylib, or lib. A cdylib is not linkable, so `cargo run --bin <name>-dev` builds the bin and not the lib. App web crates are different: they declare `["cdylib", "rlib"]`, and that rlib is what makes an app's `-dev` bin rebuild the lib. The queue's follow-up, a bin crate that does not link the app lib, is already the game case, so it would not change the Beacons timing it records. A dev profile for the bin still would: the bin depends on `exact-web` directly and is what `cargo run --release` relinks.

The rest of the checklist holds.

The generated package is `<name>-web`, the bin is `<name>-dev` at `src/bin/<name>-dev.rs`, and the dependency rename `app-data` is the `app_data` crate, the same path `build.rs` already uses for `app_data::<type>`. `exact_web::dev::main` is `D: DataSource + Default` (`host/web/src/dev.rs:563`). Beacons' `BeaconsData` and Tennis' `Jev` both derive `Default` and implement `DataSource`. The host bake passes that same raw value into `contract::bake`; `Storage<T>` is only the runtime `AppData`. The bin never names the cdylib, and the wasm bake invokes Cargo with `--lib` (`scripts/app.mjs:1297`), so the wasm build does not compile this bin. `exact_web::dev` exists only off wasm (`host/web/src/lib.rs:46`), which is the host target `cargo run` uses.

`gameShells` regenerates when any desired file is missing or different, writes the new tree under a stage directory, and renames it over the shell (`game/app/shells.mjs:345`). Adding `src/bin/<name>-dev.rs` misses the staleness check, so an existing data game's shell is replaced wholesale. With no `game.data`, both new strings are empty, so that shell stays byte for byte what it was. The dev loop's `--check` path calls `prepareGame`, which calls `gameShells`, before the compiler starts.

Apple, Linux, Windows, and Android do not bake. They take `EXACT_DEV_PLAN` (a file or the web dev server's URL) through `exact_linux::Config::from_env`, which Windows uses too, or through the Apple and Android launch environment. The web loop is the only baker, so only the web shell needs the bin.

The new test fails on the old generator: after `game.data` is set it requires the `[[bin]]` stanza and `exact_web::dev::main::<app_data::Readings>()`. It does not compile the bin. Beacons is the compile check, in the dev test.

The 300 s wait does not turn a hang into a pass. After `rust: rebuilt`, `current` is already null, and `/exact.json` is 404 until the new producer publishes (`host/web/dev.mjs:1458`). A live process that never publishes fails at 300 s with the step named; a dead process fails as soon as it exits. The test's own cap is 1500 s, the same as the 600 s plan wait plus the 600 s rebuild wait plus this 300 s, so a hang after both earlier waits ran long can surface as the harness timeout instead of that named error.
EXIT=0
