# Code review: the Apple agent's bounded scroll track, round 3, narrow re-check (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `4e185b32b8`.
- **Method:** one brief (sha256 `aa61d36f4ff0f85e4bec6fafcb4425747997da830b4f184d48c3bcefb0dff2bf`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The fix matches the ruled behavior. `stop` freezes the ring and returns the first page with `more`; the ring stays readable until a read answers `more: false`, and it is also dropped when the lease is observed on a later track request or when a new `start` replaces it. The driver reads that tail to the end. The new host and driver tests cover the whole tail with exact `dropped` (0 for 600 rows, 904 for 5,000; 12 rows and `dropped` 1 in the driver), release after `more: false`, no feed after `stop`, and release by lease expiry and by a new start. `bun test scripts/agent-inspect.test.mjs` passed (27 tests).

The ring is still one 4096-row allocation. A read after release answers `track: false, reason: no-track`. Cursors stay row counts, kept per session and cleared when a start succeeds or the stop finishes. Pages still fit the 4000-byte envelope (`stopped` is inside the measured header), with no timer and no new sampler.

VERDICT: APPROVE

1. NIT — `docs/contract-for-agents.md:3217` — The memory bullet says a frozen ring is freed by the first feed or read after the lease ends. `trackScrolled` returns on `frozen` before the lease check (`FrameHealthTrack.swift:146` and `:158`), so a feed does not release it. Release happens on the next track request (`expireTrack` at `FrameHealthTrack.swift:139`) or on teardown. That matches the ruling (checked on read, no timer) and stays one ring; the bullet overstates the feed.
