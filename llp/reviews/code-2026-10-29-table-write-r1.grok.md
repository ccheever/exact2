# Code review: game dev loop: identical table asset not rewritten, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e95fbfa507`.
- **Method:** one brief (sha256 `9fd1a0d186f59bce38ebff2527058d1c1f3af92246216cb334967fb0a6b4b1e2`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `game/app/tables.mjs:56` compares decoded UTF-8 strings. `write_changed` compares raw bytes (`game/bake/src/files.rs:6`). Those agree for well-formed UTF-8 this writer produces, so an identical rewrite of its own file is skipped. The baker (`files.rs:42-49`) encodes a `BTreeMap` with `exact_game::json::to_string`, and `quote_into` (`game/engine/src/data/json.rs:595-611`) emits `\u0008` / `\u000c` and raw U+2028 / U+2029. `JSON.stringify` (`tables.mjs:67`) emits `\b` / `\f` and `\u2028` / `\u2029`, with keys in `TABLES` order. `exact_game_tables` returns that string unchanged (`game/render/src/lib.rs:568-569`). A table that hits the difference is still renamed after the bake, and the next logic edit rebuilds the app wasm. Beacons has one table (`game/games/beacons/logic/src/lib.rs:27`) and `movement.json:1` matches both encoders. Comparing the JS string's UTF-8 would not close that. Have this loop write the baker's bytes.

2. `host/web/tests/tables.test.mjs:182` sees the wasm build's `Compiling` line on the real pipeline. `buildBake` passes no `-q` (`scripts/app.mjs:1301`) and inherits cargo stderr (`:1310`); `scripts/app-bake.test.mjs:521-523` shows `Compiling` on that stderr. `host/web/dev.mjs:1241` forwards `build.mjs` stderr, `game/dev.mjs:78` forwards that to the test, and the test records both streams (`tables.test.mjs:128-129`). `--quiet` on the per-edit check (`shells.mjs:616`, `:633`) is the logic crate and is written to stdout (`dev.mjs:890`). Two holes remain. Beacons has `game.data` (`game/games/beacons/app.json:3`), so the restart runs `cargo run -q` of `beacons-web` (`scripts/app.mjs:1255`, `host/web/dev.mjs:535`); a dev-bin rebuild prints no `Compiling` line. `CARGO_TERM_COLOR=always` in the inherited env puts a reset between `Compiling` and the package name, and `not.toContain('Compiling beacons-web')` passes while the wasm crate compiles. The pipe's default is uncolored, which is why the test was red on main. Set `CARGO_TERM_COLOR=never` in the test env.

3. `host/web/tests/tables.test.mjs:177-178` says the second edit "compiles nothing of the app's own". The assertion is the `beacons-web` substring only. "The reload above" is `tuning.reload()` inside the first rebuild (`host/web/dev.mjs:1255`), before `rust: rebuilt` (`:1260`). The `40-72 s` figure is the plan's range; this commit measures the wasm build at 34–58 s wall.

4. `QUEUE.md:3` is the right mechanism (`game/app/tables.mjs`, `assets/` mtime) and `wasm-opt -Oz` does run on every non-bake web build with no byte skip (`host/web/build.mjs:166-168`). The headline `12-18 s` is wider than this commit's `rust: rebuilt in` 11.1–11.4 s and wider than the line's own 12.4 s sample; the commit's first rebuild was 23.7 s. `40-72 s` is again the plan range, against the commit's 34–58 s wasm wall. In `host/web/build.mjs` the order is app `wasm-opt` (`:168`), then instantiate and plan extract (`:232-242`), then GPU `wasm-bindgen` (`:490`, `scripts/app.mjs:417`), then the tail including `gzipSync` (`:530`). The line places the ~4 s after `wasm-bindgen`; the instantiate work sits before that call.
