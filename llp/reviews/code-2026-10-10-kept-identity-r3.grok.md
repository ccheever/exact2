# Code review: a kept answer's identity read from a fallback (LLP 1027.005 D3) round 3 (019b88ead), 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f74e50777a88693c372dd61809980a7314af3524d720c59628708e2467f06e82`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

The round-2 fix is right and complete. No findings.

The only change in behavior is at the object-preserving swap in `runner/src/runner/settlement.rs:721`. A fresh value equal to the last one keeps the old object only when that object is compiled, or the new one is not. A refused kept seed is a non-compiled `Held`. When its value equals the compiled fallback, the fallback's compiled object is the one that is published, so a dependent still sees an unknown identity.

The other fallback paths keep a mark a dependent can see:

- An admitted seed, a carried answer, and a checkpoint answer are reused before the swap, so an equal value stays a non-compiled object and remains a known identity.
- A compiled value replaced by an equal live answer still keeps its object: the old object is compiled, so the new conjunct stays true.
- The placeholder flag lives on `ResourceState`, not on `Held`. The swap does not clear it, and `identity_unknown` treats placeholder and compiled as unknown.
- A refused pass never publishes the tentative states. A commit that fails after a successful settle restores the checkpoint's `ResourceState`s, marks included.
- A later pass in the same boot, including the stale-store repeat, either reuses the compiled object or builds another compiled value and keeps the old one because it is compiled.
- `release_compiled`'s released form still has compiled bytes, so `is_compiled` stays true, and `Held::same` treats it as the same value as a fresh compiled fallback with those bytes.

A legitimate seed is not refused. Reuse of a matching kept, carried, or checkpoint answer never enters the swap, including when that answer equals the compiled value. A resource that reads no store is not treated as unknown. After readiness the check is off.

READY
