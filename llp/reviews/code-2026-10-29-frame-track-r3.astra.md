# Code review: the Apple agent's bounded scroll track, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `1464cd9eee`.
- **Method:** one brief (sha256 `dc901ebeb9b9ad2d679ad5ec8efc2a27fe4d24c8db34333038a617b4de3e3545`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The five round-2 fixes have regression coverage. All 3 targeted Bun tests passed; native XCTests were inspected but not rerun in this read-only checkout.

VERDICT: CHANGES

1. **MAJOR — Stop discards a tail spanning multiple pages.** [FrameHealthTrack.swift:124](host/apple/Sources/ExactKit/FrameHealthTrack.swift:124) releases the ring after returning one page. If more than 4000 bytes of rows arrive between the driver’s final read and stop, the remaining rows become unrecoverable. [agent-inspect.mjs:539](scripts/agent-inspect.mjs:539) returns `more: true`, but another read returns `no-track`; the discarded tail is also absent from `dropped`. Reproduced through the driver with mocked bounded host replies. Stop collection first, retain the bounded ring until its remaining pages drain, and cover a multi-page stop tail.
