# The web's `native.later` skips the request scope check and the 1 MiB response bound, and a Swift module never receives its grants

**Status:** Closed
**Resolution:** Fixed: web native requests share scope admission and bounded response handling, and each Swift factory receives its effective grants; UTF-8 success/error limits and per-session grant regressions pass. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Web host (`host/web/glue.js`), JS prelude (`js/src/prelude.js`), native executor (`js/src/swift.rs`, `js/src/native.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067, LLP 1027 request policy

On the web, the `exact-native:` branch of the request path returns before `scopeValid` and before bounded response handling (`host/web/glue.js:680-681`). The caller asks for a 1 MiB ceiling (`js/src/prelude.js:309`). Apple applies that bound (`host/apple/src/executor_core.rs:383`), so a large reply succeeds on the web and fails on Apple.

Separately, the Swift factory discards its grants (`native_module(_grants)`, `js/src/swift.rs:154`), although the native-module contract says the implementation enforces them (`js/src/native.rs:41`). A Swift module therefore cannot enforce grants it never receives.

**Fix:** route native requests through the same admission and response-limit path as other requests on every host, and pass the effective grants into the module instance.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: web native requests share scope admission and bounded response handling, and each Swift factory receives its effective grants; UTF-8 success/error limits and per-session grant regressions pass.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max (design). Verification: confirmed by reading.

## Fix verification (2026-09-27)

The regression first demonstrated a native call executing under an ungranted
source scope. Request execution now lives in `host/web/http-body.js`, called by
`glue.js` after the batch; native and HTTP requests share admission and bounded
body handling. Native success, module errors, and load errors all respect the
byte ceiling. A retired generation cannot start a delayed request. Swift's
`exactNativeModule(grants:)` receives the activation's effective grants at creation.

`host/web/request-refusal.test.mjs` checks scope refusal before module entry,
oversized UTF-8 success and error bodies, and an exactly 1 MiB reply. The real
Swift fixture checks two sessions receiving different grants. The relevant Bun
suite (including agent and HTTP regressions): `45 pass; 0 fail`; Swift bridge:
`3 passed; 0 failed`. Required host-check limitations are recorded below.

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
