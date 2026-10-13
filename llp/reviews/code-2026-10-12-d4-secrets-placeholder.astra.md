# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) (26c76fe56), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4e8b4caa9f2d44c8671889c46d74089152c370ce9cb58b569b6e47ca0d8e6639`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's two MINORs (a secret written or forgotten before readiness flipping the choice and its flags) are taken by deciding on the launch's snapshot (`Store::opened_with_secrets`); Grok's two NITs (comments) are taken, and a test that never saw its entry is replaced.

---

- **MINOR — [settlement.rs:528](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:528): Pending flags disagree after removing the last secret before readiness.** The new guard allows a resource to switch from its placeholder back to its compiled answer. This branch clears `awaiting[i]` but leaves the pass-local `pending_res[i]` true. Consequently, a derive using `pending(resource)` retains true while the final runner flag becomes false through `sync_pending_flags()`. Clear the settlement flag too, and test this transition.

- **MINOR — [settlement.rs:735](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:735): An equal-valued placeholder retains compiled provenance.** Boot without secrets, then write a secret before readiness, with the declared placeholder equal to the baked answer. The guard selects the placeholder, but value deduplication preserves the old compiled `Held`. At activation, `revalidated()` consequently emits a build-time-answer line despite `placeholder == true`. Preserve placeholder provenance or prioritize that flag when journaling.

These findings follow from code-path inspection. Existing prebuilt kept-answer, settlement, carry, store, and placeholder tests passed; the new tests cover neither transition and do not assert pending state.

The ordinary boot guard matches the amendment. Unknown dependent identities are intentional under LLP 1027.005. Granted-secret detection excludes runner-owned entries; files/SQLite remain outside its scope. The journal exposes no secret names or values, although collection itself is unconditional rather than development-build-only. The deferred `rt.js` work is explicitly documented.

NOT READY
