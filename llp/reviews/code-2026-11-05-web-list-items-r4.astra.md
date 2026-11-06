# Code review: web list item roles, round 4, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `e3bbb59783` (one source commit on `99fd8ca92c`).
- **Method:** one brief (sha256 `e2267565a55907de73ec855c25eee18a74a244e56837d838f088663a9bf72c83`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

Reviewed `e3bbb5978353f0a974225abd27e7b4a440c51485` against `99fd8ca92c`.

**Adversarial correctness:** Explicitly **accept** all four R3 settlements:

1. Native CSS and layer metadata.
2. Logical Back-proxy lookup.
3. Modal-aware whole-region removal.
4. The narrowly updated icon-path allowlist.

The compiled regressions, valid removal controls and read-only production probes support these fixes. The remaining **R1 navigation settlement is accepted**, and the **R2 navigation settlement is accepted**.

**Simplification:** Accepted. The existing marker and shared traversal helpers handle the changes without another abstraction.

**Writing:** **P3 — Narrow the commit’s Test Plan claim.** “Sheet replacement on both targets” exceeds the supplied coverage: the whole-region replacement regression in [sheet-leave.test.mjs:133](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r4/host/web-js/sheet-leave.test.mjs:133) runs the JS target. Separate that claim from the cases exercised on both targets. The remaining added prose and amended Cargo statement are accurate.

**Verification:** Two new P2 harness regressions remain:

- **P2 — Supply the imported traversal helper to the agent test.** [agent-bare.test.mjs:10](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r4/host/web/agent-bare.test.mjs:10) extracts `kids` into a VM without `viewChildren`, causing `ReferenceError`. Supply the real helper and retain the hatch-filter assertions.
- **P2 — Update the header-geometry rule extraction.** [neutral-look.test.mjs:27](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r4/host/web/tests/neutral-look.test.mjs:27) requires the old selector text and now dereferences a null match. Update extraction while preserving the schema comparison.

I executed both exact test bodies against main and candidate sources: main passes; candidate fails. Their existing assertions provide meaningful regressions for these harness repairs.

All recorded Cargo/compiler checks pass. The focused browser run passes **103 tests**, the native accessibility collector passes, and item/route comparisons pass against wasm and WebKit. Structure conformance retains the main-reproduced cached-panel discrepancy; caps retains only main’s foldables violation.

The [native-related run](/tmp/exact-list-r4-native-related.log) finishes with **121 passes, 10 failures and 2 errors**. Beyond the two confirmed regressions above, the symbol and touch-press failures still require comparable main results.

No files changed or browsers launched. Local execution remained limited by missing `target`/`node_modules`; the standalone neutral suite could not load `rolldown/utils`. VoiceOver evidence remains prototype-only.

**Not ready. Round 4 is not clean.**