# Code review: a game with data bakes with it in the web dev loop (host/web red #7, tables), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `22f8baf93e`.
- **Method:** one brief (sha256 `3ffb4540ad46d16ccae840f00cb85baa3f37abdd3171dac5caa2515a4cb938f0`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

Clean. The newest commit fixes the round-1 note, and the three commits hold together.

The queue line at `QUEUE.md:3` now matches Cargo. A game web shell is `crate-type = ["cdylib"]` only (`game/app/shells.mjs:275`). Cargo links a same-package bin to the lib only when that lib is linkable (`rlib` / `lib` / `dylib`), and a `cdylib` is not, so `cargo run --bin <name>-dev` rebuilds the bin and the package build script. An app web crate is `["cdylib", "rlib"]` (for example `apps/geofence-fixture/web/Cargo.toml:12`), so its bin does link the rlib. The parenthetical says both.

The generated shell is the same shape as those app crates, on the paths that already bake a game with data:

- `[[bin]]` is `name = "<name>-dev"`, `path = "src/bin/<name>-dev.rs"`, and only when `kind === 'web' && data` (`game/app/shells.mjs:278`). A game with no data concatenates nothing and adds no file, so that shell stays as it was.
- The bin is `exact_web::dev::main::<app_data::<type>>()` (`game/app/shells.mjs:289`). `app-data` is already the dependency alias, so the extern crate is `app_data`. `data.type` is the same Rust path the build script already passes to `bake_data_declaration`.
- `exact_web::dev::main` is `D: DataSource + Default` (`host/web/src/dev.rs:563`), and `dev` is compiled only off wasm (`host/web/src/lib.rs:46`). Beacons' `BeaconsData` and Tennis' `Jev` both derive `Default` and implement `DataSource`. The wasm bake passes `--lib` (`scripts/app.mjs:1297`), so the host-only bin is not built for `wasm32`.
- Staleness is the whole `files` map, including the new bin (`game/app/shells.mjs:345`). A miss stages a new tree and renames it over the shell, so a shell that gains or loses data gains or loses `src/bin` with the `Cargo.toml`.

Apple, Linux, Android, and Windows do not need this bin. Their shells already bake with `bake_data_declaration::<app_data::<type>>` in `build.rs`. A live dev client follows `EXACT_DEV_PLAN` (Android: `x_dev_plan`) to the web dev server, whose producer is this bin. `host/web/dev.mjs:534` selects `<name>-dev` and runs it with `cargo run --release`.

`game/new.test.mjs:1007` would fail on the old generator: with data it requires the `[[bin]]` stanza and `exact_web::dev::main::<app_data::Readings>()`, which the old shell did not emit. The no-data half still requires no `[[bin]]` and no `src/bin`.

The 300 s wait does not hide a hang. After `rust: rebuilt`, `current` is cleared and `/exact.json` is 404 until the new producer publishes (`host/web/dev.mjs:1460`). `until` still throws, naming that step (`host/web/tests/tables.test.mjs:153`); a compiler exit fails immediately.
EXIT=0
