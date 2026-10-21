# Code review: the JS target's fill marker for a pressable button with a bound fill (conform interaction-gallery), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `45a5cac26`.
- **Method:** one brief (sha256 `1ac24d00c8a202b35925aabb6227c7b5476027f2d485605d4c7d926ed537a13e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker (adopted pages skip the marking and keep the static attribute) and two should-fix (alpha; the gate).
- **Disposition:** taken in the round-1 rework.

**1. BLOCKER — Adopted pages skip `saidFill`, then the static `data-exact-said` wins.** `host/web-js/rt.js:839` and `host/web-js/rt.js:661`

`S()` records `rendered = Adopt` (`rt.js:817`). On the first run, `css()` returns while the adopted inline `background-color` already matches, and that return is above the `saidFill` call (`rt.js:843`). `adopt()` has already overwritten `data-exact-said` with the compile-time attribute (`rt.js:661`).

A pressable button with a bound fill does not get `background-color` from `props_of` (the value is not in the static patch). A written top or bottom edge does. `border-width=0` sets both, so the template attribute is `border-top border-bottom` (`element.rs:1229`). The server render, including the Rust document host, puts `background-color` on the attribute when the fill lands. Adoption replaces that with `border-top border-bottom`, and the early return never puts the word back. The binding is unchanged by hover, so the grey wash (`neutral.css:88`) stays for the session. The interaction-gallery buttons are this shape: `background-color=theme.card` or a conditional, and `border-width=0`. `Sm` and `Sr` do not pass `rendered`, so only the `S()` path has the hole.

Fix: call `saidFill` after `last[prop]` is stored and before the `rendered` return, so a skipped style write still updates the attribute.

**2. SHOULD-FIX — `clearFill` does not use resolved alpha, so some clear fills stay marked.** `host/web-js/said.js:3`

`element.rs:1214` marks a fill when either scheme’s resolved alpha is non-zero. `clearFill` only accepts `null`, `transparent`, and an `rgb`/`rgba` whose alpha is `0`.

These land as CSS text and stay marked, so a clear button gets the ink wash (`neutral.css:90`):

- A transparent theme token. `color_css` writes `#00000000`, or `light-dark(#00000000, #00000000)` when both sides are clear. `#rrggbbaa` and `#rgba` with a zero alpha are not clear. `light-dark(#00000000, #ffffffff)` and `light-dark(transparent, #fff)` correctly stay marked (one side paints).
- `"unset"`. The kernel treats that as clearing the row (`background_color` becomes absent, so the word is not written). `clearFill` does not.
- A varying token serialized as `var(--…)` whose resolved color is transparent. An opaque `var(--exact-…)` or `var(--x-…)` matches, because both sides add the word.
- `currentcolor`. The row stores `None` (`keyword_color`), so `element.rs` does not mark it. `saidFill` does.
- `color-mix(...)`. It is not a `ColorValue`. An opaque mix is marked on the JS target, where it paints. A mix that is actually clear (`color-mix(in srgb, red 0%, transparent)`) is marked too.

`null`, `transparent`, and zero-alpha `rgb`/`rgba` do drop the word, and a later change from an opaque value to one of those keeps `border-top` / `border-bottom`.

Fix: treat the same strings the kernel resolves as alpha 0 as clear, including `#00000000`, `#rgba` with a zero alpha, `light-dark()` of two clear sides, `unset`, and `currentcolor`. For `var()` and `color-mix`, resolve alpha (or leave `var()` marked only when the token’s color is non-transparent).

**3. SHOULD-FIX — The gate is a `press` handler, not a Pressable button.** `host/web-js/rt.js:843`

`element.rs:1212` writes the word for every Pressable `<button>` with a written non-transparent fill. `saidFill` runs only when `data-exact-on` contains `press`.

A Pressable `<button>` with a bound fill and no `press` handler (other handlers, or none) never gains the word, so its hover stays the grey wash. A native Control button (`appearance="auto"`, `data-button-style`, `press=`) does gain the word, which `props_of` does not write. That second case does not change the wash: `neutral.css:90` excludes `[data-button-style]`.

Fix: key off a `<button>` that is not a Control button, the same set `element.rs` uses. Non-button pressables (a pressable `<div>`, an `<a href>`) are already unmarked on both sides.

Checked and holding:

- Word order matches `element.rs`: `background-color`, then `border-top`, then `border-bottom`. `~=` does not care about order. Clearing the fill leaves the edge words.
- A bound fill that becomes `null` or `transparent` removes the word on the client path.
- Implied backgrounds are static literals with `written` false. They do not go through `css()`, and `props_of` does not mark them.
- Opaque `light-dark()` with one transparent side is marked on both sides.
- `build.mjs` ships `said.js` beside `rt.js`, and the server DOM’s `dataset` would run `saidFill` during the render. The adopted client is what drops the word.
