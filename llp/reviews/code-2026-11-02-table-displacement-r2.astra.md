# Code review: a table row's cells follow the engine's displacement (Taffy patch 69), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `1bb377e36c`.
- **Method:** one brief (sha256 `bcbdddfd8ba4335078c895800a9416d7241946df42ac6b1b9f2da177a6d4649f`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** the finding (shared by both reviewers) is addressed in `01efbc5155`; a confirmation round follows.

- **MAJOR — [kernel/src/layout/publication.rs:207](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/layout/publication.rs:207): ordinary cell updates now traverse the entire row group.** The unconditional `tree.tables.flat(slot)` applies even when publication visits a clean `tbody` solely to reach one dirty descendant. It enqueues every row; each row then enqueues every cell. For example, changing a small child’s relative offset inside one fixed-size cell, while leaving tracks and other frames unchanged, increases publication from the dirty path to Θ(rows × columns). This affects ordinary static tables too.

  **Fix:** expand a flat part’s children when its effective child origin changes, retaining sparse traversal when it is merely an ancestor of a dirty node. Keep the popover regression test and add a `publication_visits` test showing that one isolated descendant update does not visit unrelated rows and cells.

Both round-one correctness fixes look sound by inspection: the in-flow origin now includes alignment, margins and baseline shims while excluding relative offsets; origin-change reporting and child descent address stale publication beneath a popover row with its own block. The repeated alignment calculation also preserves RTL overflow behavior.

I found no additional layout-result or memo/replay defect. `Event::Origin` participates in recording, comparison and replay, and the added writes do not feed Taffy’s layout calculations. The new tests cover the reported correctness regressions but miss the publication-cost regression above.

I could not run `cargo test -p exact-kernel --test it table::`: the read-only sandbox denied Cargo’s temporary target-directory creation (`Operation not permitted`).

VERDICT: CHANGES
