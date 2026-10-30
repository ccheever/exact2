# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `01279c4e98`.
- **Method:** one brief (sha256 `3c6cb334ca72276ea5c79da50d1b9d7ae97f8762ef257958b85580a758d6a649`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [js/src/binary.js:450](js/src/binary.js:450) — The ordinary function wrapper changes construction semantics. `Reflect.construct(Proxy, [target, handler], newTarget)` now reads `newTarget.prototype` before entering the wrapper. A callable proxy whose getter throws for `"prototype"` therefore makes construction fail; the engine’s Proxy ignores that property. Also, assigning `Proxy.prototype = undefined` creates an own property that hardening locks, breaking subclassing through a proxy that supplies a virtual prototype. **Fix:** wrap the native constructor with a native Proxy `construct` trap that records and returns the native result, preserving its construction behavior and property layout.

**Non-blocking**

1. [js/src/binary.js:457](js/src/binary.js:457) — `Proxy.revocable` becomes enumerable and constructible: `new Proxy.revocable({}, {})` succeeds instead of throwing. **Fix:** preserve its native descriptor and wrap the native method with an `apply` trap, retaining its lack of construction support.

The round-3 serialization findings and listed fixture gaps are resolved by inspection. `Proxy.length`/`name` remain correct, and hardening freezes the replacement and `revocable`. I found no reachable untracked proxy path before `binary.js` in the current bootstrap. No files changed; no network, tests, or builds run.
