# Review round 3, LLP 1121 stage 10 A1, Astra (gpt-6-astra, max), security pass

Reviewed: e6f255b0a6 (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

Three blockers remain:

- **BLOCKING — [scripts/tamber-server.mjs:575](/tmp/rv-rl-a1c/scripts/tamber-server.mjs:575):** An undefined result from `scopeOf(req)` becomes `write` through the default argument. An in-memory probe forwarded `transaction` and returned 200. Remove the default or reject missing scopes explicitly.
- **BLOCKING — [scripts/tamber-history.mjs:90](/tmp/rv-rl-a1c/scripts/tamber-history.mjs:90):** Read-scope `history` invokes Git without `--no-textconv`. `--no-ext-diff` still permits configured textconv helpers; enabling `cachetextconv` also writes Git objects/refs. A harmless Git probe confirmed helper execution. Add `--no-textconv`.
- **BLOCKING — [host/web-js/dev.mjs:798](/tmp/rv-rl-a1c/host/web-js/dev.mjs:798):** Unheld live `propose` reaches `inlineWhy()` → `toolFresh()` → `buildTool()` when the reader is stale. This starts Cargo build, potentially recovery clean, below write scope. Make the classifier’s freshness check free of side effects.
- **SHOULD-FIX — [scripts/tamber-server.mjs:247](/tmp/rv-rl-a1c/scripts/tamber-server.mjs:247):** The selection cache ignores accepted `selected: null` reports. Reproduced: refresh still returns the previous node after inspect exits. Clear that entry; the [current test:93](/tmp/rv-rl-a1c/scripts/tamber-pairing.test.mjs:93) exercises only an empty cache.

The round-2 fixes check out: atomic owner-only cap eviction, failed-hold preservation, scope propagation through import/revert, forced unkept exports, bounded/pruned selections, and explicit local handling. Earlier method and stream guards remain intact.

Read-only harness checks passed for denials, cap concurrency, export overrides, and disconnected-build pruning. Full integration tests weren’t run because they write fixtures. No files modified.
