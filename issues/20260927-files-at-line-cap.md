# Seven sources sit at the 1,500-line cap and are being compressed onto single lines to stay under it

**Status:** Open
**Systems:** `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift`, `scripts/agent.mjs`, `host/web/glue.js`, `host/linux/src/content_region/presenter_tests.rs`, `kernel/build.rs`, `kernel/src/kernel.rs`, `host/linux/src/text/residency_tests.rs`
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** rules/RULES.md (the file cap), `bun scripts/caps.mjs`

At `c74615a3` these files are at 1,499, 1,499, 1,499, 1,499, 1,498, 1,496 and 1,496 lines. Code is being squeezed rather than split: `host/web/glue.js` has 300-character one-liners (for example `:307`, `:748`, `:805`), `host/apple/Sources/ExactKit/Bridge.swift:231-232`, and commit `0eeb76a2` "agent.mjs back under the cap". The next feature that touches any of them fails `caps` or adds more one-liners, which defeats the cap's purpose (files a reader can hold).

**Fix:** split each along its seams now. `glue.js` has clear candidates: the request path, the agent seek, and batch apply.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max. Verification: confirmed with `wc -l`.
