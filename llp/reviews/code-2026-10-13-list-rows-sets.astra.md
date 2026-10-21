# Code review: onlyListRows decides a batch's creates once, and QUEUE's one-pass entry, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `300ef82ad197f669a05bd912deb5272bab70e0443b239322dd043b543593a574`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The MINOR (both: the QUEUE entry's "each non-empty one" overlooks `applySnapshots`' collections-only path and `applyUnlessEmpty`'s skip) taken in the landing commit.

---

- **MINOR — [QUEUE.md:827](/Users/admin/x2p2/apps/exact2-comp/QUEUE.md:827):** “Each non-empty one pays … fixed pass whatever it holds” overlooks the snapshot fast path. [PresenterIOS.swift:737](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:737) accepts non-empty `.collections`-only batches when nothing awaits finalization, then returns before navigation, `onlyListRows`, controls and accessibility work. Qualify the claim as batches that miss the existing fast paths, so the entry does not overstate potential savings.

No correctness findings in `HEAD~1`. All three former dictionary reads remain equivalent: membership preserves the nil check, and `createdProjected` preserves both `inRows`’s `own` check and the children predicate. Insert/remove preserves last-create-wins in both directions. `projected` is pure over its arguments and immutable `back`/`scrolls` captures, so evaluating it earlier is safe.

The queue entry otherwise matches `landFill`’s ordering and reads clearly. No measured speed-up is established or assumed.

Read-only review; `git diff --check` passed. Builds, tests and simulator profiling were not run.

READY
