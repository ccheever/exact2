# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `b460f39bb5df58c1266226e53360a6cbd2b89de6e212afce1f26c9856ca54fbc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (retained colours stale; wide colours to the fallback; the view's tint ignored; off-main resolution; one observer a binding; hatched views no place; dead places cached; Android accent).
- **Disposition:** in the first-round commit.

---

**Blocking issues found.**

- **P1 — Retained Swift colours stay stale.** [ExactTamber.swift:90](/tmp/rv-s5c5-1/host/apple/modules/ExactTamber.swift:90): cached sides refresh only through an observer on that same binding. Retain a colour, change `setMode`, then resolve it again: it still returns the old colour. The [in-process hook](/tmp/rv-s5c5-1/host/apple/Sources/ExactKit/Tamber.swift:140) also never invalidates the view. The smoke masks this by creating a fresh binding and colour on every callback.

- **P1 — Wide-gamut colours become transparent.** [ExactTamber.swift:41](/tmp/rv-s5c5-1/host/apple/modules/ExactTamber.swift:41): the library requires four components, but `hatchTamber` sends `textChannels`, which returns nine for wide colours. A valid `color(display-p3 …)` token therefore returns the fallback, normally `.clear`.

- **P1 — Swift accent tokens use the wrong tint.** [NativeHatches.swift:169](/tmp/rv-s5c5-1/host/apple/Sources/ExactKit/NativeHatches.swift:169) and [Tamber.swift:56](/tmp/rv-s5c5-1/host/apple/Sources/ExactKit/Tamber.swift:56) omit the view’s tint when resolving `@tint`. Under an authored red `accent-color`, an `AccentColor` token resolves through the global/system tint instead. This also loses distinct embedded sessions’ theme tints.

- **P2 — Background colour resolution returns fallback.** [ExactTamber.swift:110](/tmp/rv-s5c5-1/host/apple/modules/ExactTamber.swift:110): a cold dynamic-provider resolution calls a hatch that rejects non-main threads. Passing the colour to a background renderer and calling `resolvedColor(with:)` therefore produces clear, although [`UIColor` supports use across threads](https://developer.apple.com/documentation/uikit/uicolor/).

- **P2 — Only the first observer on a binding fires.** [ExactTamber.swift:143](/tmp/rv-s5c5-1/host/apple/modules/ExactTamber.swift:143): registering two observers on one `ExactTamber` shares its `TamberPlace`. The first `inputs()` call consumes the change; the second returns false, suppressing that observer on every subsequent change.

- **P2 — Element hatches cannot resolve their place.** [ExactTamber.swift:29](/tmp/rv-s5c5-1/host/apple/modules/ExactTamber.swift:29): lookup requires an ancestor registered by `Tambers.made`. Host-created `ExactElement.view` objects never enter that registry, so `ExactTamber(element.view!).color(...)` always falls back and observation does nothing. D12 explicitly includes hatch objects.

- **P2 — Destroyed places remain cached and polled indefinitely.** [Tamber.swift:140](/tmp/rv-s5c5-1/host/apple/Sources/ExactKit/Tamber.swift:140): failed lookups merely `continue`; nothing removes dead node IDs or unused bindings from `known`. Repeatedly opening and closing themed views leaves their values retained and adds an owner-thread round trip per historical node to every batch—even after all observations stop.

- **P2 — Android misses inherited accent changes.** [tamber.rs:53](/tmp/rv-s5c5-1/host/linux/src/tamber.rs:53): colours depend on the authored accent, but the returned `inputs` fingerprint does not. Changing only an ancestor’s `accent-color` makes [Tamber.kt:82](/tmp/rv-s5c5-1/host/android/canvas/src/main/java/dev/exact/canvas/Tamber.kt:82) skip refresh, leaving the cached colour and observers stale.

Static review; platform tests were not run. Kotlin remains uncompiled. Worktree and index unchanged.
