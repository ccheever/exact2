# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 3 (abcd157b9), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `257f05878f19f0f8b92c5d2ea45a4eb408de1c0116e169c3d5ad4a3c949e92a7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra's MAJOR is taken: a passed-over compiled answer is not reused at equal arguments; the test covers equal and changed arguments.

---

No findings. **READY.**

`origin/main..HEAD` is four commits (`00dfae39..3c3792ddd`). The fourth, `3c3792ddd`, landed during this review. It is the piece that finishes the round-2 fix.

The third commit's conjunct at `runner/src/runner/settlement.rs:745` is real: once the placeholder arm has built a new cell, an equal compiled object is not kept, so `data_ready` does not journal a build-time revalidation. That conjunct never runs when reuse returns the standing cell. Reuse still did that when an action made the arguments read a store reader and the argument bytes stayed the same (`store_readers[i]` flips at `settlement.rs:403`, then the old predicate treated equal args and an unchanged store revision as a standing answer). `consume(use ? account : "")` with `account` showing `""` kept the compiled cell.

`3c3792ddd` closes it. Reuse now also requires that the standing cell not be a compiled answer `secrets_unseen` would pass over (`settlement.rs:473`). The test loops `account` over `"x"` and `""`. All seven `kept_tests::secrets` tests passed.

The rest of the rule holds on this tree:

- `opened_with_secrets` is set once in `Store::from_names` (`store.rs:194`). `new`, `new_linked`, and `module_mirror` all go through it. Boot, reload carry, and delivery boot pass a snapshot into `new_linked`, so each boot's flag is the snapshot that boot opened with. A checkpoint seed is a `Held::new` answer, and a transaction restore does not touch the flag. A write or a forget before readiness on the same store does not flip the choice.
- An `else source()` owner waits until that row is in `settled_res` (`settlement.rs:392`). `secrets_unseen` then sees the row, so the compiled arm is not taken while the placeholder row is still empty.
- An admitted kept seed is not compiled, so the `kept_seed` reuse short-circuit does not preserve a bake cell. A refused seed is not reused, and the placeholder arm takes it.
- The LLP paragraph matches: the launch snapshot decides, a missing `else` or an empty grant keeps the build-time answer, and the journal line is the one `lines::secrets_unseen` writes. `rt.js` is untouched and still queued.

Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-7c0f963c.md` and `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-7c0f963c.md`.
