# Code review: the Apple dev client waiting for its program instead of stopping, round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the change and the first round.
- **Method:** one brief (sha256 `a63ab2e0045bdce337c146aead6612784e2831dbd43e9eca157f5d43c74a54ff`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

Nothing blocking on the live connection these commits describe. A host that has already taken a program stays on its stream, stays registered, and will not apply a generation for a different program. One replacement connection does not.

**[P1] A replacement connection never subscribes or retries when the server names another program — `host/apple/Sources/ExactKit/PlanURL.swift:655`**

`adopted` starts nil on every new `PlanURL`, while `Session.connect` keeps the process pin in `devProgram` (`Session.swift:232`). Open the dev link again, or Dev Menu → Open Project, after this process has already taken program A and the server is announcing B. The envelope check at `PlanURL.swift:655` only refuses when `adopted` is set, so it falls through. `observe` (`PlanURL.swift:553`) rejects B via `acceptProgram`, and `waitForProgram` bumps `resolution`. `failed` (`PlanURL.swift:855`) then drops its retry because it captured the pre-bump generation. `subscribe` never runs, and `reload()` (`PlanURL.swift:445`) only clears `terminal` and hits the same hole. The build drops off the channel and does not come back when the server returns to A, unless a later manual reload happens to see A. Round 1 stayed registered here because it asked `acceptProgram` before subscribing.

The in-place path holds. `{rebuilt}` and a foreign announcement call `waitForProgram` without cancelling the stream (`PlanURL.swift:874`, `PlanURL.swift:553`). A restart’s envelope still has `adopted` set, so `PlanURL.swift:655` subscribes and does not fetch. The next frame or envelope that names `adopted` clears `terminal` and is fetched. A server that keeps announcing the other program does not resubscribe, re-register, or spin: those frames return in `observe`, and backoff still caps a real disconnect. In-flight generation fetches cannot commit afterward: `waitForProgram` always bumps `resolution`, and `fetch` completions, `pump`, and `commitCandidate` all require that generation (`PlanURL.swift:501`, `PlanURL.swift:698`, `PlanURL.swift:715`). `close()` still stops retries. `connectionStatus` still shows “(rebuild the host)” while `terminal` is set (`Session.swift:252`). Live batches and snapshots still go through `design()` (`PlanURL.swift:879`); a different design structure is refused by the runtime, and a snapshot cancelled by an in-stream refusal is started again (`PlanURL.swift:845`).

Round 2’s three fixes hold on that same object: a waiting host accepts only `adopted`, the refused-envelope path does not call `acceptProgram`, and an in-stream refusal restarts a snapshot that was actually in flight. No test covers the program gate.
