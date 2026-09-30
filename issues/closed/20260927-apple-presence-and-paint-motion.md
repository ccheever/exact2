# Apple motion leftovers: a removed layout-transition keeps its offset, exit colours don't show, inherited colour is cached past retirement, and more

**Status:** Closed
**Resolution:** Fixed: retire layout and inherited paint, emit exit text color, clamp Apple/Linux surfaces, clear exits and tab projections on reset, and broadcast preferences; all 13 regressions pass. Root checks and iOS pass; baseline macOS test/smoke failures and the Keychain test restriction are recorded below. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host (`host/apple/src/presence.rs`, `style.rs`, `paragraph.rs`, `Sources/ExactKit/…`), Linux host (`host/linux/src/paint/presented.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062, LLP 1063, LLP 1059, LLP 1061

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: retire layout and inherited paint, emit exit text color, clamp Apple/Linux surfaces, clear exits and tab projections on reset, and broadcast preferences; all 13 regressions pass. Root checks and iOS pass; baseline macOS test/smoke failures and the Keychain test restriction are recorded below.

Each item is confirmed by reading.

1. **A removed `layout-transition` leaves its last offset and scale.** `presence.rs:185-188` calls `remove_property` without presenting identity, so the presenter keeps `layoutOffset`/`layoutScale` (`PresenterIOS.swift:719`). It also happens when an ancestor becomes `display:none` mid-move.
2. **An exit animation's `color` never shows.** `paint_over` changes `text_color` without setting its mask bit (`style.rs:211`), and `restyle_presented` starts from an empty mask (`:243`), so the colour is never sent.
3. **Inherited colour is cached past retirement.** When a parent's colour transition is cancelled, its engine property is removed, and `present_colors` skips the descendant walk that clears `paint.runs` (`paragraph.rs:330`). Inline text and inheriting children keep the mid-transition colour. Destroying a node doesn't clear its entry.
4. **Size can go negative (P3).** A layout spring that overshoots gives a negative width or height on Apple (`presence.rs:263`, `Surface.swift:58`) and Linux (`presented.rs:86-87`). The web clamps to 0. Fix: clamp on every host.
5. **Presenter reset keeps exiting views (P3).** Reset never clears `leaving` (`PresenterIOS.swift:271`, `PresenterMac.swift:710`), and `.destroy` checks `endExit(id)` first (`:701`). After a reload that reuses an id, a new view's destroy can be consumed.
6. **Tab-bar state survives a reset (P3).** `SegmentHost.reset()` restores only `controls`, and `bars`/`members`/`hidden` survive a dev reload (`SegmentsIOS.swift:298`).
7. **The agent's `prefer` reaches only one session (P3).** It sets the static `DisplayPreferences.agent` and tells only the addressed session (`Agent.swift:237-248`). Another session's runner keeps the old bits while its press feedback follows the new ones.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max, Grok 4.7 xhigh, Opus 5.5 max. Verification: confirmed by reading.

## Fix and regression coverage (Astra, 2026-09-27)

All seven findings were reproduced with failing tests before their fixes. No design
ruling is needed for this ticket.

- `host/apple/src/presence.rs` and `layout.rs` send identity when a layout
  transition retires, including a hidden ancestor, and clamp presented size to zero.
  `tests/it/presence.rs` covers removal midway through movement and resizing,
  hide/show during movement, and a spring overshooting below zero.
- `host/apple/src/style.rs` sets the text-color mask for presented paint.
  `tests/it/paint.rs` checks the color of an exiting text view halfway through its exit.
- `host/apple/src/paint.rs`, `paragraph.rs`, and `host.rs` carry color retirement
  through the descendant repaint even after the engine drops its slot, and remove
  destroyed runs from the paint cache. Integration and unit regressions cover
  inheriting views, inline runs, and destruction during a color transition.
- `host/apple/Sources/ExactKit/Surface.swift` and
  `host/linux/src/paint/presented.rs` clamp surface dimensions at the paint boundary.
  Both Apple presenter suites and a Linux unit regression check negative dimensions.
- `PresenterIOS.swift` and `PresenterMac.swift` finish every retained exit on reset.
  Both presenter suites reset during an exit, reuse the id, then destroy the new view.
- `IOS/SegmentsIOS.swift` resets every projected tablist's membership and decisions,
  restoring hidden tabs and dropping both tab bars and segmented controls. The iOS
  regression checks visibility restoration, fresh projection, and presenter reset.
- `DisplayPreferences.swift` broadcasts agent overrides through the existing
  preference observers; `Agent.swift` no longer updates only its addressed session.
  `SessionClockTimerTests.swift` checks two live runners and a newly booted session.

There are 13 new regression tests, beside the existing tests. `NodeViewIOS.swift`
was not changed; no scripts, generated files, or declaration tables were added.

## Verification

The required root commands all passed:

- `cargo build --all-targets --keep-going`: exit 0.
- `cargo test --lib --bins --tests --no-fail-fast`: exit 0; 1,714 passed,
  0 failed, 8 ignored across 71 test summaries.
- `cargo clippy --all-targets --keep-going -- -D warnings`: exit 0.
- `cargo fmt --all -- --check`: exit 0.
- `git add -A && bun scripts/caps.mjs`: `All budgets within cap. 6 categories inspected.`
- `bun scripts/boot.mjs`: `modules reachable before first pixel: 2
  (host/web/glue.js, host/web/navigation.js); wasm references: 8`.

Host checks:

- `cargo clippy -p exact-apple -p exact-linux --all-targets --keep-going -- -D warnings`: exit 0.
- Apple Rust integration tests: `110 passed; 0 failed`.
- Apple Rust unit tests: `138 passed; 1 failed`. The unrelated
  `abi::tests::fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit`
  cannot write to this session's Keychain: `User interaction is not allowed.`
  A subsequent run excluding only that test reported `138 passed; 0 failed;
  1 filtered out`; the integration tests passed again. The new destruction/cache
  regression passed in both runs.
- Linux paint presentation tests: `2 passed; 0 failed`, including the new clamp regression.
- `MACOSX_DEPLOYMENT_TARGET=14.0 bun host/apple/build.mjs --test`:
  `Executed 445 tests, with 1 failure (0 unexpected)`. Every new regression passed.
  The unrelated `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  failed with `1` loaded instead of `240`, identically before and after these fixes.
  The deployment target keeps the Rust archive and Swift tests on the same macOS floor.
- `bun host/apple/build.mjs --ios --test`:
  `Executed 52 tests, with 0 failures (0 unexpected)`; `** TEST SUCCEEDED **`.
- `bun scripts/smoke.mjs linux`: `linux smoke: ok in 37.0 s`.
- `bun scripts/smoke.mjs macos`: `macos smoke: 3 failure(s) in 14.5 s`.
  With the production changes temporarily reversed to `e6e34c9e` and the app
  rebuilt, the same command reported `3 failure(s) in 17.6 s`, with identical
  failures: scroll limit `667` instead of `652`, `screencapture exited 1`, and a
  tap on `card-mv-north-131` focusing `null`. The fixes were restored afterward.
  The Caltrain Contract tests passed in both runs: `3 passed, 0 failed`.

Nothing in this ticket remains for Charlie to decide. The baseline smoke failures and unrelated test
failures above were left outside this branch's assigned scope.
