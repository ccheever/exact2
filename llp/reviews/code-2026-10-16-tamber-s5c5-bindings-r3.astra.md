# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `2fc030f3a199b1c75feb732246b8f9216817679288fec2c1d2901dbf9ad5698b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (off-main dynamic tint; UIKit colour appearance; a first unmounted observer; tint sides; Android scroll lends; a binding made in the factory).
- **Disposition:** applied without re-review (the last round).

---

Blocking issues remain:

- **P1 — Off-main tint resolution still races.** [Tamber.swift:64](/tmp/rv-s5c5-3/host/apple/Sources/ExactKit/Tamber.swift:64): the cached tint is a dynamic colour whose provider reads `NodeView.style` (`nearestAccent`/`applyAccentTint`). Resolving an accent-backed token off-main while main applies an accent change accesses that dictionary unsynchronized. Locking the answer cache does not protect this read.

- **P2 — UIKit colour invalidation is incomplete.** Both [Tamber.swift:173](/tmp/rv-s5c5-3/host/apple/Sources/ExactKit/Tamber.swift:173) and [ExactTamber.swift:163](/tmp/rv-s5c5-3/host/apple/modules/ExactTamber.swift:163) omit `affectsColorAppearance = true`. A colour assigned once to `UIView.backgroundColor` can remain stale after `setMode` or a live batch; the custom trait change does not trigger UIKit’s colour refresh. [Apple documents the required flag](https://developer.apple.com/documentation/uikit/uitraitdefinition-64c15).

- **P2 — A first observer registered unmounted never starts.** [Tamber.swift:241](/tmp/rv-s5c5-3/host/apple/Sources/ExactKit/Tamber.swift:241): registering while detached creates no `bySession` entry. After mounting, every batch returns here before checking observers. The new test masks this by first creating another colour binding.

- **P2 — Tint fingerprints miss other appearances.** [Tamber.swift:216](/tmp/rv-s5c5-3/host/apple/Sources/ExactKit/Tamber.swift:216): `extended(tint)` samples only the ambient appearance. In a light app with a dark native view, an action changing an ancestor’s accent from `light-dark(red, blue)` to `light-dark(red, green)` leaves that fingerprint unchanged. The library retains its cached blue dark side.

- **P2 — Android misses placement changes during scrolling.** [ExactView.kt:464](/tmp/rv-s5c5-3/host/android/canvas/src/main/java/dev/exact/canvas/ExactView.kt:464): `placeFeed()` can lend a native view through `Natives.near()` on a movement-only frame, when `painted` is false. `placed()` is skipped, while `check()` continues querying the old node. An observer-managed colour keeps the previous row’s mode until a full paint.

- **P2 — Library bindings created during initialization stay disconnected.** [ExactTamber.swift:31](/tmp/rv-s5c5-3/host/apple/modules/ExactTamber.swift:31): constructing a binding inside the native factory captures `place = nil`, because `Tambers.made` runs only after the factory returns. Retained colours permanently use fallback, and observations never register—even after mounting.

Source review only; no builds/tests run. Worktree unchanged.
