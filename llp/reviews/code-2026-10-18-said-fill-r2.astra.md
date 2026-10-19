# Code review: the JS target's fill marker for a pressable button with a bound fill (conform interaction-gallery), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f2c395099`.
- **Method:** one brief (sha256 `8b267801d688169017d38541ee3a7615ed487a7e1f3ce6f0d62926b102ef6dfe`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; four should-fix (changes while hovered; a stale word after the binding clears; mixed light-dark halves; a missing alpha).
- **Disposition:** all taken in the round-2 commit (motion-held writes read at the next pointer event).

Nothing is blocking. Reviewed the two explicit hashes; four SHOULD-FIX findings remain.

1. **SHOULD-FIX — Marker stops tracking changes while hovered.** [host/web-js/said.js:18](/tmp/rv-said-2/host/web-js/said.js:18)  
   Hover a transparent button, then click to give it an opaque fill without moving the pointer. No new `pointerover` occurs, so it keeps the gray wash. Motion-held writes, token/mode changes, and scheme changes have the same gap. Adoption after pointer entry can also miss initialization. **Fix:** synchronize after adoption and relevant fill/mode/motion changes, including already-hovered buttons.

2. **SHOULD-FIX — Removing the inline fill permanently preserves a stale word.** [host/web-js/said.js:11](/tmp/rv-said-2/host/web-js/said.js:11)  
   Hover an opaque bound fill, then clear its binding to blank/null. `css()` removes the inline property; subsequent pointer entries return here without removing `background-color`. The now-unfilled button keeps the ink wash even after leaving and returning. **Fix:** distinguish previously managed bound fills from untouched static fills, and restore the appropriate compiled/class-backed marker when the binding clears.

3. **SHOULD-FIX — Mixed-alpha `light-dark()` disagrees with Rust.** [host/web-js/said.js:13](/tmp/rv-said-2/host/web-js/said.js:13)  
   For bound `light-dark(transparent, red)` in light mode, this removes the word. [element.rs:1215](/tmp/rv-said-2/host/web/src/element.rs:1215) retains it because either half has nonzero alpha. Thus equivalent literal and bound fills select different washes immediately. Computed CSS exposes only the active half. **Fix:** preserve enough resolved color information to apply Rust’s two-half rule. [CSS Color 5](https://www.w3.org/TR/css-color-5/#light-dark)

4. **SHOULD-FIX — Missing alpha (`none`) is classified as opaque.** [host/web-js/said.js:8](/tmp/rv-said-2/host/web-js/said.js:8)  
   Modern computed colors can serialize as `color(srgb 1 0 0 / none)`. Neither regex matches, so `alpha()` returns `1`, although missing alpha renders as zero. **Fix:** recognize `/ none` as zero; distinguish an omitted alpha from an explicit missing component. [CSS Color 4](https://www.w3.org/TR/css-color-4/#missing)

For an event the listener receives, the synchronous mutation is early enough for the next paint under the [event-loop model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model). Neutral hover changes `background-image`, so it does not contaminate the measured background color. Touch and pen deliver `pointerover`, including before contact for non-hover devices. [Pointer Events](https://www.w3.org/TR/pointerevents3/#the-pointerover-event)

Keyboard focus needs no hover marker. No-document evaluation is safe; ordinary class-only static markers remain intact; `rgb()` and numeric slash-alpha handling work.

Validation used source inspection and in-memory Bun probes. No builds, browser paint tests, or file/index changes.
