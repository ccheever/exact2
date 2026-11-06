# Code review: web list item roles, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `9e1976d509` (one source commit on `99fd8ca92c`).
- **Method:** one brief (sha256 `a68a676c63363318b13abd967682ca1cee365bc5c4828f181ce20e904fbb1d95`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

Reviewed only `9e1976d509` against `99fd8ca92c`. Four regression classes remain.

**1. Adversarial correctness**

- **P2 — Wrappers break authored radio groups and tablists.** [navigation.js:40](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r1/host/web/navigation.js:40) inserts wrappers even when the author overrides the list role. `input-glue.js:36–55` and `navigation.js:17` still inspect physical parents/children. Production-helper probes changed radio discovery from **2 to 0**, ArrowRight handling from **true to false**, and tab-panel discovery from **2 to 0**. The segmented-control CSS has the same assumption. Update these consumers to traverse logical children. Regression coverage should exercise keyboard selection, tab navigation and selected styling on both targets, including adopted pages.

- **P2 — Wrappers break JS text flow.** [flow.js:59](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r1/host/web-js/flow.js:59) groups exclusions by their physical parent; its ancestor walk also treats the wrapper as a layout box. With the new wrappers, a source probe changed a valid sibling paragraph list from **one paragraph to none**. A paragraph nested under a list instead received a structural refusal. The wasm implementation uses kernel parents and remains unaffected. Use logical parent/child traversal; add direct-list and nested-list wrapping regressions comparing both targets.

- **P2 — Removing a resizing row can bypass its exit animation.** [navigation.js:42](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r1/host/web/navigation.js:42) assumes the wrapper’s first element is its row. During resizing, `presence-glue.js:220–229` inserts an animation surface before that row. The new `rt.js:1172` removal path consequently extracts the surface and deletes the real row before calling its exit handler. The probe confirmed **row disconnected, zero exit calls**. Sweeping this assumption also found that `syncListItemRoles` can assign a second `listitem` role during resizing. Select the actual row consistently and preserve presence cleanup ordering. Regressions should interrupt a resize with removal and with an authored-role change.

- **P2 — Grouped-list styling misses the new DOM structure.** [neutral.css:516](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r1/host/web/neutral.css:516) still expects `list > section > div`. Wrapping sections makes that selector match **zero cards instead of one**. Empty cards retain their compiler-provided border; the corner rules and `index.html:91` grouped-row press exclusion also miss. Update the selectors. Add compiled grouped-list regressions for empty cards, corners and pressed rows on both targets. The passing native-style tests currently construct the old, unwrapped structure.

These are introduced by this commit’s wrappers, rather than existing failures. Each fix needs a regression demonstrated to fail before the fix and pass afterward.

**2. Simplification**

Use `viewParent`/`viewChildren` consistently for logical traversal, and centralize identification of the real row inside a wrapper. This addresses the repeated DOM assumptions behind the findings without adding another wrapper abstraction.

**3. Writing**

The recipe and commit-message structure follow the writing rules. One **P3** correction: [navigation.js:1032](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r1/host/web/navigation.js:1032) should name `kernel/src/layout/fit.rs`. Refresh the commit’s Test Plan with the final Cargo failures below. Its description of the Safari/VoiceOver result correctly limits it to the prototype.

**4. Verification**

The supplied logs show 55 related JS tests, 21 native-style tests, 33 host-related tests and the actual wasm accessibility test passing. Wasm conformance matched all 12 comparisons; WebKit reported no unexpected failures. The wrapper, role, fitting, focus and renderer negative controls reached assertions. The earlier wasm receipt-validation failure provides no behavioral proof.

[Final check results](/tmp/exact-list-final-checks.json) are complete: build, clippy, formatting, workspace, compiler tests/clippy, boot and semantics passed. [Full Cargo testing](/tmp/exact-list-final-cargo-tests.log) failed on Apple worker-start and photo-upload timeouts. Isolated executor suites passed on both revisions, but that does not establish the final full-run failures as pre-existing. The caps failure is pre-existing: `foldables.md` is already 3,254 bytes on main.

I ran read-only production-source/DOM probes and selector checks. This snapshot has neither `target/` nor `node_modules`, so I could not rerun builds or browser suites. Both server renderers have JS-runtime adoption evidence; list-specific **wasm-runtime adoption remains unverified**. James’s VoiceOver report is prototype evidence only. No files were changed, and no Firefox, GUI apps or other agents were started.

**Verdict: not ready.** The four P2 regressions and the validation gaps above remain unresolved.