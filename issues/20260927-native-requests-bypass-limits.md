# The web's `native.later` skips the request scope check and the 1 MiB response bound, and a Swift module never receives its grants

**Status:** Open
**Systems:** Web host (`host/web/glue.js`), JS prelude (`js/src/prelude.js`), native executor (`js/src/swift.rs`, `js/src/native.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067, LLP 1027 request policy

On the web, the `exact-native:` branch of the request path returns before `scopeValid` and before bounded response handling (`host/web/glue.js:680-681`). The caller asks for a 1 MiB ceiling (`js/src/prelude.js:309`). Apple applies that bound (`host/apple/src/executor_core.rs:383`), so a large reply succeeds on the web and fails on Apple.

Separately, the Swift factory discards its grants (`native_module(_grants)`, `js/src/swift.rs:154`), although the native-module contract says the implementation enforces them (`js/src/native.rs:41`). A Swift module therefore cannot enforce grants it never receives.

**Fix:** route native requests through the same admission and response-limit path as other requests on every host, and pass the effective grants into the module instance.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max (design). Verification: confirmed by reading.
