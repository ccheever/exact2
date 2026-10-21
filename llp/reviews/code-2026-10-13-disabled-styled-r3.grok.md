# Code review: disabled look round 3, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `111ca92db7b366a286ed253d53584a205bb769018faf668038361e08d4cd6a26`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Grok's NIT (a test for a match binding spelled like the state) left: the walker subtracts it, and the callback case pins the subtraction.

---

## NIT

- `contract/cli/tests/it/disabled_look.rs:91` — The negative list locks a callback parameter spelled like the state. It does not lock a `match` arm binding spelled the same way. The walker subtracts that binding only in the `some` arm (`contract/lower/src/disabled_look.rs:105`). Freshen renames match binders only inside derive bodies, so a view-level `color=(match item as busy then busy else "black")` with `disabled=busy` stays unmarked only because of that subtraction. Deleting it would mark the node and skip the dim, and `disabled_look::` would still pass. The walker handles this form today, so nothing paints wrong until that branch regresses. Add that button next to `shadowed` and assert it is not marked.

Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-43a1f77a.md`  
Summary: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-43a1f77a.md`

**READY**
