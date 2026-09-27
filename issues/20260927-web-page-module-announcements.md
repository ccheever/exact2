# A web page module's announcements connect only after a `native.later`, are applied once per announcement, and a failed load is cached until reload

**Status:** Fixed: page modules connect after first paint without a later call, coalesce announcements by topic and generation, and retry rejected loads with cleared caches; loader, activation, and coalescing regressions pass.
**Systems:** Web host (`host/web/native-glue.js`, `host/web/glue.js`), runner announcements (`runner/src/runner/time.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1016.002, LLP 1067 D5

- **Announcements need a `native.later` first.** `connect` is called only inside `pageNative` (`host/web/native-glue.js:129-133`), and that is reached only from an `exact-native:` request (`glue.js:681`). A page module that exports `connect` to announce, say, connectivity is never connected unless something also calls `native.later`. A module that exports only `connect` is refused with "exports no later".
- **No coalescing on the web.** Every announcement calls `wasm.exact_changed` and then `applyBatch` synchronously (`glue.js:41`). Native hosts queue announcements, deduplicate them, and drain once (`runner/src/runner/time.rs:97-137`), as LLP 1016.002 describes. A burst of identical announcements re-asks the resource N times on the web and once on native.
- **A failed load is cached forever.** `globalThis.exact.nativeArtifact ??= new Promise(…)` (`native-glue.js:30`) and `pageNativeModule ??=` (`glue.js:41`) keep a rejected promise. After one slow or failed first load (the 10 s timeout), every later native view and every `native.later` fails until the page reloads.

**Fix:**
- Register announcements independently of long calls: after first paint, or at the first `native.watch`.
- Queue and deduplicate web announcements, and drain them at the same boundary native hosts use.
- Clear both caches when a load is rejected.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max (code and design), Opus 5.5 max. Verification: confirmed by reading.

## Fix verification (2026-09-27)

`glue.js` connects the page module after activation through the same paint gate
as native views. `native-glue.js` accepts connect-only modules, queues distinct
topics until the next host turn, holds them until ready, and discards retired
generations. Both rejected promise caches reset. Retries use a fresh module URL,
ignore late results from older attempts, and remove their load hooks; subsequent
native views can retry too.

Before the fix, the new tests failed for a connect-only module and a rejected
artifact cache. `host/web/native-glue.test.mjs` now exercises activation without
requests, both caches, error/timeout cleanup, coalescing, and generation retirement.
The relevant Bun suite: `45 pass; 0 fail`. The native-view browser smoke also
caught an initial first-paint regression; routing both callers through the native
paint gate fixes that ordering. Required host-check limitations are recorded below.

The full required web smoke reports `web tests: 3 passed, 0 failed` and exits
with `web smoke: 1 failure(s)` solely for Chrome diagnostics (keychain access,
password-store encryption, and first-paint metrics). The native-module smoke,
`bun scripts/smoke.mjs web --app native-fixture --app-only`, reports
`web native: 32 of 32 checks passed`; its overall smoke also exits with one
failure for Chrome keychain/encryption diagnostics. No page/runtime failure is
reported. The required macOS suite ran twice with the same unchanged raster-loader
test failure: `Executed 442 tests, with 1 failure` (`seen.count` 1 versus 240).
Those host/environment findings are outside these ticket fixes.

Root verification: build, Clippy (`-D warnings`), format, staged caps and boot
passed; the Cargo test run totaled 1,714 passed, 0 failed and 8 ignored across
71 test binaries. The two changed JS crates also pass Clippy with
`EXACT_JS_ENGINE=stub`. `glue.js` stays below 1,500 lines; boot still reaches
only `glue.js` and `navigation.js` before first pixel.
