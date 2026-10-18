# Code review: Apple's absolute press opacity and looks (LLP 1121 stage 8 B2), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `e99eda450`, the Apple host.
- **Method:** one brief (sha256 `a99d2f2635b82c222bf9cecc242ad2436e884e9920a7cfc8b66b151bb205985f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: disabled changes leaving stale looks, a look's explicit resets lost, a look's currentcolor transparent, a pressed share ignoring a look's opacity; should-fix: a re-aimed fade jumping, a superseded hover path's looks).
- **Disposition:** in the round-1 commit, with tests (`StateLooksTests`: disabled flipped under a resting pointer, the share of a hover's opacity, a re-entrant hover; `looks.rs`: currentcolor and clears). 1: `relook` on every props change. 2: a cleared row crosses null (an opacity 1); Swift removes the key. 3: currentcolor resolved against the node's colour in Rust. 4: the share is of what shows. 5: the re-aimed segment is tracked (`pressShown`). 6: the defer publishes only for the generation it owns, and only views in the viewport.

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Disabled changes leave stale looks.** [PressPaint.swift:302](/tmp/rv-s8b2-r1/host/apple/Sources/ExactKit/PressPaint.swift:302) recomposes only when hover/focus flags change. Hover a node, then change only `disabled`: the props path never recomposes its look, and subsequent hover updates hit the unchanged-flags guard. Its hover paint remains; enabling an already-hovered disabled node has the inverse failure. **Fix:** invalidate and recompose looks immediately on disabled changes, including their opacity animation. Test both props-only transitions.

2. **BLOCKING — Explicit resets disappear during serialization.** [looks.rs:30](/tmp/rv-s8b2-r1/host/apple/src/looks.rs:30) uses a whole-style serializer that omits empty shadows and cleared values. Consequently, `:hover { box-shadow: none }` cannot remove a base shadow, and `opacity: unset` cannot replace a base opacity with 1. Other clears arrive only in `written`, which `looked` ignores. **Fix:** preserve explicitly named resets as concrete initial values or reset markers, and apply them during composition. Test focus resetting a hover value too.

3. **BLOCKING — Look colours using `currentcolor` become transparent.** [looks.rs:30](/tmp/rv-s8b2-r1/host/apple/src/looks.rs:30) serializes the look directly, bypassing the current-colour resolution performed for ordinary node styles. A hovered background or border therefore receives the string `"currentcolor"`; Swift’s colour decoder cannot resolve that string. **Fix:** resolve these look rows against the node’s presented text colour before serialization, and refresh them when that colour changes.

4. **BLOCKING — Hand-written press opacity composes incorrectly with hover opacity.** [PressPaint.swift:85](/tmp/rv-s8b2-r1/host/apple/Sources/ExactKit/PressPaint.swift:85) calculates the share’s depth from raw `ownOpacity`, while `lookOpacity` independently replaces that opacity. With base 1, hover 0.8 and press share 0.5, the two additive offsets produce **0.3**, rather than 0.4. **Fix:** calculate the share from the composed unpressed opacity, including hover/focus, while retaining the separate absolute-target calculation.

5. **SHOULD-FIX — Repeated look changes can jump during the press ease.** [PressPaint.swift:228](/tmp/rv-s8b2-r1/host/apple/Sources/ExactKit/PressPaint.swift:228) replaces the Core Animation segment when depth changes, but leaves `pressPaint.ease` describing the original segment. A second look update reconstructs `fromValue` from that stale curve rather than the opacity currently showing. Two updates at 40 ms and 50 ms can introduce an additional opacity jump of about 0.071. **Fix:** track the replacement segment’s timing and endpoints, or sample the current additive press contribution before re-aiming. Test intermediate values, not only `toValue`.

6. **SHOULD-FIX — An obsolete hover traversal can overwrite the current look path.** [PresenterMac.swift:932](/tmp/rv-s8b2-r1/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:932) unconditionally calls `hoverLooks(nodes)` in `defer`, even when the generation guards abort after a reentrant callback. If an ancestor’s enter callback replaces `[child, parent]` with `[parent]`, the outer defer subsequently reactivates the child’s look. **Fix:** gate look publication on the captured hover generation and retain attachment eligibility checks. Extend the existing reentrant-hover test with actual looks.

The focused Cargo test could not start because the sandbox denied target-directory creation. No files were changed.
