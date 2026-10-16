# Code review: the Apple dev client waiting for its program instead of stopping, round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the whole range.
- **Method:** one brief (sha256 `90e25f829462b92c64dc66972f75d370218b4446cf0f664c4f7df88250134fd9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** design sync cancelled when the running generation is announced again.
- **Disposition:** applied without re-review (the last round).

---

**P2 — Resuming can strand live-design synchronization:** [PlanURL.swift:897](/tmp/rv-s4-20/host/apple/Sources/ExactKit/PlanURL.swift:897). While waiting for program A, suppose a design snapshot is fetching and newer design frames are queued. If A returns announcing the already-running generation, `observe` clears `terminal`, then line 898 calls `beginResolution()`, cancelling the snapshot and discarding queued frames. No generation applies, so synchronization never restarts; a held design preview can remain stale until another event. Preserve or restart design synchronization on this path.

The other stated fixes check out. I found no new foreign-program application or tight reconnect/re-registration loop. Swift execution was blocked by the read-only sandbox’s module-cache restrictions.
