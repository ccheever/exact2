# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `2c5ed80bfa10134b92376d4bb901710130b75ef471f9e4e663f23a0e2b8ef9a7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (views not redrawn; Android no-stream turns; off-main stale; tint not in the inputs; off-main tint; observe froze its place; Kotlin's place walk off the main thread; unsynchronized caches).
- **Disposition:** in the second-round commit.

---

**Blocking issues remain.**

- **P1 — Swift views still are not invalidated.** [Tamber.swift:186](/tmp/rv-s5c5-2/host/apple/Sources/ExactKit/Tamber.swift:186), [ExactTamber.swift:161](/tmp/rv-s5c5-2/host/apple/modules/ExactTamber.swift:161): a native view drawing a retained color without an observer stays visually unchanged after `setMode` or a live edit. Lazy cache refresh only helps after something else requests a redraw.

- **P1 — Android skips refresh without a paint stream.** [Owner.kt:403](/tmp/rv-s5c5-2/host/android/canvas/src/main/java/dev/exact/canvas/Owner.kt:403): `Tambers.turned()` now runs only when `fresh != null`. Editing a token consumed solely by native code can leave the kernel unchanged, producing no stream; cached colors and observers then never update.

- **P2 — Background library resolution still becomes stale or fallback.** [ExactTamber.swift:105](/tmp/rv-s5c5-2/host/apple/modules/ExactTamber.swift:105): after a mode change, an unobserved retained color keeps its old sides off-main. With an observer, `inputs()` clears those sides, and background resolution returns fallback until a main-thread read repopulates them.

- **P2 — Apple’s tint-dependent cache omits tint from its inputs.** [NativeHatches.swift:174](/tmp/rv-s5c5-2/host/apple/Sources/ExactKit/NativeHatches.swift:174): changing only an ancestor’s `accent-color` changes the resolved `@tint` but leaves `answer.inputs` unchanged. Retained library colors remain the previous accent and observers stay silent.

- **P2 — In-process background resolution still loses the place’s tint.** [Tamber.swift:58](/tmp/rv-s5c5-2/host/apple/Sources/ExactKit/Tamber.swift:58): resolving an `AccentColor` token off-main explicitly supplies `nil` tint. A view under a red accent therefore uses the global/system tint instead, despite prefetching.

- **P2 — In-process observations still freeze their place.** [Tamber.swift:143](/tmp/rv-s5c5-2/host/apple/Sources/ExactKit/Tamber.swift:143): observing before mounting silently registers nothing; observing before reparenting retains the old session/node. Colors now follow the new place, but notifications do not.

- **P2 — Kotlin rechecks the hierarchy before it reflects the new frame.** [Tamber.kt:79](/tmp/rv-s5c5-2/host/android/canvas/src/main/java/dev/exact/canvas/Tamber.kt:79): the owner thread walks the main-thread view map before publication. When a frame recycles a view into a differently pinned element, it checks the old node; after ingestion, no further check is guaranteed, leaving observer-applied colors stale.

- **P2 — Background providers race mutable Swift caches.** [Tamber.swift:134](/tmp/rv-s5c5-2/host/apple/Sources/ExactKit/Tamber.swift:134), [ExactTamber.swift:106](/tmp/rv-s5c5-2/host/apple/modules/ExactTamber.swift:106): background resolution reads dictionaries that main-thread batch handling clears or replaces without synchronization. Concurrent rendering and live edits can crash; prefetching does not remove that race.

Static review; platform tests were not run. Worktree and index unchanged.
