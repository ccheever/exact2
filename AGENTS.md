# Agent instructions

Read `rules/RULES.md` and `rules/DEFERRED.md` first; they bind and this file does not.
`llp/1000-exact2-root.explainer.md` is the map. Design documents under `llp/research/`
are the predecessor's — research, never authority.

## The web is the standard

Where a default, a property name, a value vocabulary, or a behavior could follow CSS or
something else (React Native/Yoga, UIKit, AppKit), it follows CSS — even where a CSS
reset would usually override it. A bare node is `display: block`, `box-sizing:
content-box`, `flex-direction: row`, `flex-shrink: 1`. Rows are named `object-fit`,
`text-overflow`, `line-clamp`, not `resizeMode`, `ellipsizeMode`, `numberOfLines`.
The web is the dev loop and the parity oracle for every other surface; a kernel that
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
  random sweep), then `metrics.mjs --long`. Building
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
- An app outside this repo (weird-castle, `~/projects/weird-castle`) builds, runs, and is
  driven through these same scripts with `EXACT_APP_DIR` set — `scripts/app.mjs` is the
  one place that knows; its `exact.mjs` sets it. exact2 is consumed there by path.
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
  `--device --run` on a connected iPhone (signed with a team profile on this Mac);
  `--host` also builds the sample host (LLP 1031 D10), the native app that embeds two
  sessions, which `bun scripts/smoke.mjs host` drives and `scripts/agent.mjs host
  --session a …` addresses; `--ios --host` is the same fixture on a simulator, driven
  by `smoke.mjs host-ios` and `agent.mjs host-ios`. The Swift is one package, `host/apple/Package.swift`:
  `ExactKit` (session, view, app owner — what an embedder links) and the executables
  as adapters over it. `apps/<name>/app.json` is the app manifest (LLP 1030 D2): the
  bundle id, name, host files, and deploy policy come from it, never from a crate name.
- Verify by running, never by grepping. Fix loops get three rounds, then stop and say so.
- To see a change work, drive the app: `bun scripts/agent.mjs <web|macos|ios|linux> tree
  "tap change-station" "type station-search Palo" "clock +60000" state logs "screenshot
  out.png"` — the ten operations of LLP 1012 and LLP 1079 (`perf`: a subtree's work by
  plan site, `perf <target> during "<op>" …` for the difference a drive made, and
  `perf frames`; a development build's ⌥⇧T, Save Trace or `SIGUSR1` writes a trace
  that `bun scripts/agent.mjs trace <file>` reads), the same on every host, with the clock
  in your hands (`clock settle` instead of waiting; `"screenshot out.png over 600 every
  50"` films motion on it as a contact sheet, `.apng` to play). The driver refuses a
  build older than its sources and names the rebuild (LLP 1012.001.000). `bun scripts/smoke.mjs
  <web|macos|ios|linux|host>` is the whole app driven that way. The Linux host
  (`cargo build --release -p caltrain-linux`) runs headless anywhere, macOS included.
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
  checkout is fine.
- Never `git stash`. Kill only PIDs you recorded. Agents remove apparatus freely and add
  none without a human saying so.
- Optional capability is a separate artifact loaded on demand (the GPU module) or another
  executor (the browser, for motion on the web) — never a cargo feature on a core crate.
  Each crate depends on strictly less than the one above it; there is no build matrix.
