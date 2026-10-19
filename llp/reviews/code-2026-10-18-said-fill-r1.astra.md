# Code review: the JS target's fill marker for a pressable button with a bound fill (conform interaction-gallery), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `45a5cac26`.
- **Method:** one brief (sha256 `1ac24d00c8a202b35925aabb6227c7b5476027f2d485605d4c7d926ed537a13e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; five should-fix (a regex alpha; token/mode changes; adoption losing the marker; motion-held writes; a press handler is not the Pressable test).
- **Disposition:** the css()-time marking replaced in the round-1 rework by a reading of the page's resolved fill at hover.

Nothing blocking. Five **SHOULD-FIX** findings:

1. **Incorrect transparency classification — [said.js:3](/tmp/rv-said-1/host/web-js/said.js:3).** Opaque `rgb(255,0,0)` matches the regex: its blue component is mistaken for alpha. Conversely, `#0000`, `rgba(255,0,0,.0)`, `unset`, and `light-dark(transparent, transparent)` retain the filled-hover marker. These produce the wrong wash. **Fix:** classify accepted colors and clears semantically, checking both light/dark halves. One transparent half alone must still count as filled.

2. **Token transparency changes leave the marker wrong — [said.js:6](/tmp/rv-said-1/host/web-js/said.js:6).** A bound token emitted as `var(--x-card)` always counts as filled, even when its current mode resolves to transparent. Changing modes leaves that declaration string unchanged, so `css()` cannot refresh the marker. Varying synthetic `color-mix` tokens have the same problem. **Fix:** carry resolved fill presence per token/mode and refresh it when the token changes.

3. **Adoption loses the marker on bordered buttons — [rt.js:843](/tmp/rv-said-1/host/web-js/rt.js:843).** Either renderer can supply `background-color border-top border-bottom`. During adoption, static attributes replace this with `border-top border-bottom`; the matching-inline-style return at line 841 then skips `saidFill()`. The cached value prevents repair until the fill changes. **Fix:** reconcile the marker before the adoption return, while still skipping redundant style writes.

4. **Held motion updates bypass the fix — [rt.js:881](/tmp/rv-said-1/host/web-js/rt.js:881).** On a motion-held button, `Sm()` delegates to `Hooks.style` without calling `css()`. Clearing a previously filled button changes its CSS but leaves `background-color` in `data-exact-said`, including after release. **Fix:** update fill metadata on both the ordinary and motion-hook paths.

5. **A press listener is not the Rust eligibility rule — [rt.js:843](/tmp/rv-said-1/host/web-js/rt.js:843).** A custom button using only `pointerdown`/`pointerup`, with a bound opaque fill, remains unmarked despite qualifying in `element.rs`. Conversely, the guard admits Control buttons and carries no `written` information. **Fix:** select eligible bindings using `NodeType::Pressable` and `binding.written` during emission.

Ordinary null/empty/transparent clearing and border-word ordering work. Non-button pressables remain untouched; `data-button-style` controls are excluded by the hover selector.

Verified through source review and in-memory Bun probes. No builds or file changes.
