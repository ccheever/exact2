# Code review: ResetGate knownLoss follow-up, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `23c3395155`.
- **Method:** one brief (sha256 `2280a0f608e2d63551314054d6351833a9d361206401ece87a8abb09a2795db1`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [ResetGate.swift:153](host/apple/Sources/ExactKit/ResetGate.swift:153): Async arrivals cannot be declared unreachable. macOS reset calls `dialogs.reset()` → `close()` → `end()` → [`endSheet`](host/apple/Sources/ExactKit/Mac/SheetsMac.swift:274); alerts also call `endSheet`. Existing comments document sheet animation waits and main-queue reentrancy. Furthermore, [`applyTogether`](host/apple/Sources/ExactKit/Session.swift:279) holds every session before presenting them sequentially, so later sessions remain held during earlier sessions’ sheet presentation. Remove the blanket exclusion from the comment, QUEUE, and commit message; keep those async losses open for triage.

**Non-blocking**

1. [QUEUE.md:4](QUEUE.md:4): “Replay the release” alone will not clear a carried answer. The new runner initializes `surface_records` empty, and [`replace_surface_record`](runner/src/runner/surface_record.rs:47) immediately returns for an absent record plus `nil`, although the resource answer can survive through carry. Specify invalidating/re-querying carried `exactSurface` answers. “Games only” also overstates the restriction: `Surface.published()` is a general GPU-surface API.

The `fulfillSurface` move is correct: generation increments before teardown, and `completeSurface` rejects old-generation replies. `KNOWN_LOSS` matches all 18 table entries. The `canvases.reset()` → `releasePublisher` → `surfaceRecord(name, nil)` path is correctly identified.
