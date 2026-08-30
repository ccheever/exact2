# Agent instructions

Read `rules/RULES.md` and `rules/NOT-DOING.md` first; they bind and this file does not.
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

- `kernel/tables/schema.json` is the one declaration authority; `kernel/build.rs`
  generates from it. Edit the table, never generated code.
- Every source file ≤ 1,500 lines. Stage (`git add -A`) and run `node scripts/caps.mjs`.
- The five checks: `cargo build --workspace` · `cargo test --workspace` ·
  `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check` ·
  `node scripts/caps.mjs` · `node scripts/boot.mjs`.
- An app outside this repo (weird-castle, `~/projects/weird-castle`) builds, runs, and is
  driven through these same scripts with `EXACT_APP_DIR` set — `scripts/app.mjs` is the
  one place that knows; its `exact.mjs` sets it. exact2 is consumed there by path.
- The dev loop is `node host/web/dev.mjs`: edit `apps/caltrain/app.contract`, the page
  restarts from the new plan in ~20 ms; edit Rust under the wasm's crates, it rebuilds
  and the page reloads. `node scripts/metrics.mjs` prints every number
  (`--long` adds the macOS build and boot). macOS: `node host/apple/build.mjs --run`;
  iOS: `node host/apple/build.mjs --ios --run` (a simulator; `--sim` or `EXACT_SIM` picks one);
  `--device --run` on a connected iPhone (signed with a team profile on this Mac).
- Verify by running, never by grepping. Fix loops get three rounds, then stop and say so.
- To see a change work, drive the app: `node scripts/agent.mjs <web|macos|ios|linux> tree
  "tap change-station" "type station-search Palo" "clock +60000" state logs "screenshot
  out.png"` — the eight operations of LLP 1012, the same on every host, with the clock
  in your hands (`clock settle` instead of waiting). `node scripts/smoke.mjs
  <web|macos|ios|linux>` is the whole app driven that way. The Linux host
  (`cargo build --release -p caltrain-linux`) runs headless anywhere, macOS included.
- `QUEUE.md` is what would make sense to do next. Add a line when you find something
  worth doing; delete it when it lands. It decides nothing.
- Never `git stash`. Kill only PIDs you recorded. Agents remove apparatus freely and add
  none without a human saying so.
- Optional capability is a separate artifact loaded on demand (the GPU module) or another
  executor (the browser, for motion on the web) — never a cargo feature on a core crate.
  Each crate depends on strictly less than the one above it; there is no build matrix.
