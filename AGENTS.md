# Agent instructions

**Building an app with Exact rather than working on it?** Make it with `bun scripts/exact.mjs
new <path>`. Its own `AGENTS.md` has the commands. Read `docs/contract-for-agents.md` first,
then `docs/agent-pitfalls.md`; `contract vocab` lists every tag and property Contract accepts.

Read `rules/RULES.md` and `rules/DEFERRED.md` first; they bind and this file does not.
`llp/1000-exact2-root.explainer.md` is the map. `docs/agent-pitfalls.md` lists verified
footguns in writing apps here; add to it when you hit one. Design documents under `llp/research/`
are the predecessor's — research, never authority.

## No tells (first)

James's directive, and it outranks the web standard below (Charlie, 2026-10-09): an
Exact app on iPhone must feel as if it were built by hand with UIKit and/or SwiftUI,
following Apple's Human Interface Guidelines. No tells: nothing a person who knows iOS
could point at and say "this isn't native" — not a control, tint, font, metric, inset,
gesture, transition, presentation, haptic, keyboard behaviour, or accessibility answer.
Where following CSS (a default, an inherited value, a behaviour) would leave a tell, the
platform's default wins; what the author sets explicitly on a node still wins over both.
The same holds for each platform's own conventions (macOS and AppKit, and so on).
When a change could leave a tell, compare against what a hand-built UIKit/SwiftUI screen
does on the same iOS version, not against Chrome.

## The web is the standard (second to no tells)

CSS stays the authoring vocabulary, the layout model and the dev loop; "No tells" above
decides where a platform default and a CSS default disagree on an Apple surface. Within that:
where a default, a property name, a value vocabulary, or a behavior could follow CSS or
something else (React Native/Yoga, UIKit, AppKit), it follows CSS. The point is to be
familiar to agents trained on a vast number of web pages, and most of those pages load
a reset (normalize.css, Tailwind's preflight). So where that common reset convention
differs from the browser's raw default, the reset's behaviour is also a familiar path
and may be chosen, declared with the reason (a form control inherits the page's font,
LLP 1104 D4). Otherwise the raw default stands: a bare node is `display: block`, `box-sizing:
content-box`, `flex-direction: row`, `flex-shrink: 1`. Rows are named `object-fit`,
`text-overflow`, `line-clamp`, not `resizeMode`, `ellipsizeMode`, `numberOfLines`.
The web is the dev loop and the layout parity oracle for every other surface; a kernel that
disagrees with a bare `<div>` reintroduces the four-disagreeing-default-layers bug
class the predecessor had. An unavoidable deviation (Taffy has no `position: static`)
is declared in `llp/1001-kernel-v1.spec.md` with the reason.

## Working here

- Tooling runs on Bun (pinned in `package.json`); `bun install --frozen-lockfile`.
  Rolldown remains the app bundler. Node and npm are not required.
- The web artifacts (`app.wasm`, the Markdown editor, text flow) build with a pinned
  nightly (`WEB_TOOLCHAIN` in `scripts/app.mjs`) that builds std for size; everything
  else is `rust-toolchain.toml`'s stable, which the five checks use. `build.mjs`
  names the install commands when the nightly or its `rust-src` is missing.

- `kernel/tables/schema.json` is the one declaration authority; `kernel/build.rs`
  generates from it. Edit the table, never generated code.
- Every source file ≤ 1,500 lines. Stage (`git add -A`) and run `bun scripts/caps.mjs`.
- Writing or changing a test: read `docs/testing.md` (waits, isolation, coverage, measuring).
- The five checks: `cargo build --all-targets --keep-going` ·
  `cargo test --lib --bins --tests --no-fail-fast` · `cargo clippy --all-targets
  --keep-going -- -D warnings` and `cargo fmt --all -- --check` (run both) ·
  `bun scripts/caps.mjs` · `bun scripts/boot.mjs`. The flags keep Cargo from
  stopping at the first failing crate or test binary. Cargo's scope is the root
  `default-members`; the async lane (`bun scripts/async.mjs`, per commit on
  origin/main) runs the same with `--workspace` plus the `async lane:` ignored tests,
  the web JS target's conformance run (`host/web-js/conform.mjs --strict`) and its
  Chrome-oracle Firefox/WebKit steps (`conform-firefox`, `conform-webkit`),
  the UIKit XCTests for commits under `host/apple` (`build.mjs --test --ios`), the
  Contract semantics (`semantics/README.md`: the Lean proofs, then `contract-difftest`
  running the runner against the Lean semantics over `semantics/corpus` and a fixed
  random sweep), then `metrics.mjs --long`. A second, hourly tier (`bun scripts/async.mjs --tier 2`) builds
  the platforms that ride on another host's code: tvOS on the UIKit presenter. Building
  `--all-targets` resolves features as `cargo test` does, so the two share artifacts.
- A green `--workspace` build proves the apps compile, not that they work: their Apple
  crates are rlibs to Cargo, and the archive an app links is built only by
  `host/apple/build.mjs`, when the app is built to run. When a change can affect how an
  app runs (the kernel, the runner, a host, the app itself), build and launch the apps it
  touches (`bun host/apple/build.mjs --ios <app>-apple --run`, `bun host/web/dev.mjs --app
  <app>`) and use them as a person would, with `scripts/agent.mjs` or by hand.
- An unset `EXACT_UPDATE_TRUST` bakes development trust, which checks only an
  origin named by `EXACT_UPDATE_ORIGIN`. `EXACT_UPDATE_TRUST=production` native
  bakes require an authenticated `EXACT_UPDATE_RECEIPT` or explicit new-stream
  `EXACT_UPDATE_GENESIS=1` (README).
- An app outside this repo (one `bun scripts/exact.mjs new <path>` made) builds, runs, and is
  driven through these same scripts with `EXACT_APP_DIR` set — `scripts/app.mjs` is the
  one place that knows; its `exact.mjs` sets it. exact2 is consumed there by path. What
  `exact new` writes (its `AGENTS.md`, `exact.mjs` verbs, `app.json` `$schema`) is LLP 1086.
- The web build, `bun host/web/build.mjs <app>`, makes the JS target (LLP 1071: the
  plan compiled to one ES module over a ~20 KB runtime, `host/web-js`); what it refuses
  fails the build. A game builds the wasm target (LLP 1071 §8), and `--wasm` is the
  internal flag for the uses below. The dev loop, the agent's web host, the smoke's app drive and
  metrics run what the build makes: on the JS target an edit rebuilds and reloads the
  page (~0.1 s, no state carried); a native client opening the dev URL is forwarded to
  the resident loop's producers (`dev.mjs --wasm`, started at its first request).
  Delivery bakes the web crate without its wasm for the native streams and publishes
  the JS build of that bake's plan as the web root; the smoke's bare-plan fixtures and
  conformance's oracle stay `--wasm` (LLP 1071 §7, "Retiring the wasm target on the web").
- The dev loop is `bun host/web/dev.mjs`: edit `apps/caltrain/app.contract` or its data
  crate, the page rebuilds and reloads (`--wasm`: the resident loop restarts the plan in
  place in ~20 ms). `bun scripts/metrics.mjs` prints every number
  (`--long` adds the macOS build and boot, and the loop's own budgets: the
  warm gate, touch one line, test what you changed). macOS: `bun host/apple/build.mjs --run`;
  iOS: `bun host/apple/build.mjs --ios --run` (a simulator; `--sim` or `EXACT_SIM` picks one);
  tvOS: `bun host/apple/build.mjs --tvos --run` (an Apple TV simulator; LLP 1008 §9);
  `--device --run` on a connected iPhone (signed with a team profile on this Mac);
  `--host` also builds the sample host (LLP 1031 D10), the native app that embeds two
  sessions, which `bun scripts/smoke.mjs host` drives and `scripts/agent.mjs host
  --session a …` addresses; `--ios --host` is the same fixture on a simulator, driven
  by `smoke.mjs host-ios` and `agent.mjs host-ios`. The Swift is one package, `host/apple/Package.swift`:
  `ExactKit` (session, view, app owner — what an embedder links) and the executables
  as adapters over it. `apps/<name>/app.json` is the app manifest (LLP 1030 D2): the
  bundle id, name, host files, and deploy policy come from it, never from a crate name.
- When you touch `contract/`, `runner/`, `plan/` or `semantics/`, run `cargo run -p
  contract-difftest -- quick` before landing. It checks the semantics on what you changed,
  takes about 10 s warm, and is advice, not a check (`semantics/README.md`, "Using it day to
  day"; `contract verify <app>` is the app author's version).
- The gate tests only `default-members`. Of the hosts it holds the web host
  (`exact-web`) and the Apple host's Rust (`exact-apple`; its tests that drive a real
  socket through URLSession, wait on the wall clock or launch Bun, Swift or a nested
  cargo build are `async lane:`, and its Swift is the XCTests). The
  others (`exact-web-js`, `exact-web-capabilities`, `exact-linux`, `exact-windows`,
  `exact-render`, …), `js/`, `gpu/` and most apps are
  not tested by it (one is compiled when a member depends on it, and its tests still do
  not run), and the async lane reports them only after the push. When you touch one, or
  what its tests read, run its tests before landing: `cargo test -p exact-linux --lib
  --tests --no-fail-fast`, or `--bins` for a bin-only crate such as `exact-web-js`.
  Advice, not a check.
- Verify by running, never by grepping. Fix loops get three rounds, then stop and say so.
- To see a change work, drive the app: `bun scripts/agent.mjs <web|macos|ios|linux> tree
  "tap change-station" "type station-search Palo" "clock +60000" state logs "screenshot
  out.png"` — the ten operations of LLP 1012 and LLP 1079 (`perf`: a subtree's work by
  plan site, `perf <target> during "<op>" …` for the difference a drive made, and
  `perf frames`, `perf frames live <ms>` for a game's frame pacing; a development build's ⌥⇧T, Save Trace or `SIGUSR1` writes a trace
  that `bun scripts/agent.mjs trace <file>` reads, `trace --phone` a phone's), the same on every host, with the clock
  in your hands (`clock settle` instead of waiting; `"screenshot out.png over 600 every
  50"` films motion on it as a contact sheet, `.apng` to play). The driver refuses a
  build older than its sources and names the rebuild (LLP 1012.001.000). `bun scripts/smoke.mjs
  <web|macos|ios|linux|host>` is the whole app driven that way. The Linux host
  (`cargo build --profile host-dev -p caltrain-linux`: a development build, as
  `build.mjs` makes for Apple; `--release` is the one that ships) runs headless
  anywhere, macOS included. `host-dev` compiles incrementally unless the shell
  exports `CARGO_INCREMENTAL=0`, which a hand-run cargo obeys (a touched kernel
  line is then 17 s, not 6) and `build.mjs` overrides, saying so.
- The terminal host (LLP 1101): `cargo build --profile host-dev -p harness-terminal`,
  then `./target/host-dev/harness` runs the coding harness in this terminal (inline;
  `--fullscreen` for the alternate screen), or `exact-terminal <entry.contract>` any
  terminal entry. With operations it runs headless (`--size 80x24 type prompt "hi" key
  Enter until "…" print`); headless, the frames go into a terminal emulator (`vt100`)
  and `print`, `screenshot` read its screen (`print --all` the scrollback, `document`
  the whole laid-out document). Develop it in `host-dev`: a debug build lays out a long
  document hundreds of times slower. `exact-terminal` is in `default-members`
  (`tests/screen.rs` drives inline mode through the emulator); `harness-data` is not,
  so when you touch the harness run `cargo test -p harness-data`.
  `examples/replay.rs` replays a recorded session through the same emulator.
  The LLP reader is the other terminal app (LLP 1101.004): `cargo build --profile
  host-dev -p llp-terminal`, then `./target/host-dev/llp [llp/ or a document]`.
- Delivery (LLP 1030.000): `bun scripts/deploy.mjs <app> [--origin <dir>]` prints the
  classifier's table (a dry run); `--yes` publishes the web root and signed bundles per stream through
  `scripts/origin.mjs`; `keygen <id>` makes a signing key (the private half never enters
  the repo). `bun scripts/smoke.mjs deploy` drives it; on a Mac it signs production
  macOS Rust modules, so set `EXACT_RUST_SIGN_IDENTITY` to an identity listed by
  `security find-identity -v -p codesigning` (an Apple Development one does). Its first
  deploy builds every release bundle cold; the rest reuse that cache. A native host
  opens its update store at launch and checks after first pixel
  (`EXACT_UPDATE_ORIGIN=<url>` points a dev build at a directory `serve.mjs` serves;
  `state.delivery` shows what it did).
- `QUEUE.md` is what would make sense to do next. Add a line when you find something
  worth doing; delete it when it lands. It decides nothing.
- Each worktree builds into its own `target/`: never symlink or share another
  checkout's (the scripts refuse); a private `CARGO_TARGET_DIR` outside every
  checkout is fine. What a development Apple build takes from the machine
  instead: the host's two Rust modules, by the SHA-256 of every input
  (`~/.cache/exact/apple-modules`, `host/apple/modules.mjs`), and, into a
  `target/` that has compiled nothing, the registry crates another build
  compiled, which Cargo then accepts or not by its own fingerprints
  (`~/.cache/exact/apple-crates`, `host/apple/crates.mjs`). A development web
  build takes the compiler and the leaf wasm modules the same way
  (`~/.cache/exact/web-modules`, `host/web-js/module.mjs`; `bun
  host/web-js/module.mjs --prebuild` warms a builder for its checkout's
  sources). Deleting any of these directories only costs the next first build
  its time.
- Never `git stash`. Kill only PIDs you recorded. Agents remove apparatus freely and add
  none without a human saying so.
- Optional capability is a separate artifact loaded on demand (the GPU module) or another
  executor (the browser, for motion on the web) — never a cargo feature on a core crate.
  Each crate depends on strictly less than the one above it; there is no build matrix.
