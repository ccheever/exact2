# Web grant tests trust a stale exact-web-js binary

**Status:** Open
**Systems:** host/web, verification
**Severity:** P3
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** LLP 1012.001.000; host/web/request-refusal.test.mjs normalized()

`host/web/request-refusal.test.mjs` `normalized()` runs `target/debug/exact-web-js normalize-grants` whenever that binary exists, and only falls through to `cargo run` when it does not. It does not compare the binary to its sources.

During this review the binary's CLI was still `exact-web-js js <app> -o <dir>` and rejected `normalize-grants`. Every test that seals a grant set failed with that usage line, including ones whose assertion never ran. A rebuild at 09:59 made the same path print a sealed grant set. The agent driver already refuses a build older than its sources (LLP 1012.001.000). This helper does not, so a stale debug binary looks like a grant-admission failure.

Check freshness the way the driver does, or always `cargo run` when the binary predates `host/web-js`. A usage error from an old binary should say to rebuild, not fail the assertion.
