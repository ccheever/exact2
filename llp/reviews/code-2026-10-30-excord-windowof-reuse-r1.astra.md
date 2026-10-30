# Code review: Excord's windowOf row reuse, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `b202ad0ca0`.
- **Method:** one brief (sha256 `e3ccb18812775dc032614210af81c7e3b25fc334114d0d53f9dbe48711fac86e`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Reviewed only `b202ad0ca0`. No BLOCKER or MAJOR found.

The window-cache key covers its added computation. Renderer validation still runs first; unread metadata, anchors, locale/time zone, and jump highlighting remain outside the cache. WeakMap entries do not independently retain evicted rows. I found no returned-row mutation or runtime equality regression.

Existing renderer gaps for directory-only changes reproduce with window caching disabled; they predate HEAD.

Validation: **138 tests passed**, plus in-memory checks for prepend identity, changed grouping/day inputs, and renderer eviction. No files changed.

VERDICT: APPROVE

1. **MINOR — [apps/excord/render.test.ts:618](apps/excord/render.test.ts:618): Randomized coverage misses older-page arrivals and day boundaries.** It creates only newer messages and resizes `latestWindow`. The existing fixed prepend test checks deep equality, not shared-object identity. Add older-history ingestion and identity assertions for retained rows—including the New-divider row—and predecessor changes across midnight to cover the approved conditions.
