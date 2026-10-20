# Code review: host looks on the runner and the web (LLP 1121 stage 8 B1), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `2707b0ec8`, the runner's `SetLooks` and the looks on both web targets.
- **Method:** one brief (sha256 `4971e102a7bec1a64b4918e00803f15c884e153f23f78ec19a9a1527938e86b0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (blocking: disabled boxes take looks; should-fix: sentinel collisions with look literals; nit: the live-token test's second half).
- **Disposition:** in the round-1 commit. 1: a box's `disabled` (any value but "false") takes none, and the Chrome test disables as the hosts do. 2: look declarations join the sentinel scan. 3: the test now moves the hover's own token, then a token only the base reads, and checks the looks stay. The press.test.mjs failure it notes fails on clean main too.

---

The prior pass already has a candidate verdict. I'll confirm the cited lines against the commit and run the four look tests before writing it up.The pressed-paint browser test failed on text color. I'll check whether this commit's press glue caused that, or whether the assertion is just racing.**WRONG**

A disabled column still takes host looks. The nodes this feature allows are views, and the selector never matches the disabled attribute either web host writes. The three cargo look tests passed. `looks.test.mjs` passed. `press.test.mjs` failed once, on a path this commit does not touch (see below).

1. **BLOCKING** `host/web/src/host/looks.rs:84`. The overlay rules are `:not(:disabled,[disabled="true"])`, and the JS target emits that same string (`host/web-js/src/emit.rs:771` via `look_rules`). A disabled column still matches them.

   Both hosts write a boolean attribute with an empty value: `host/web/glue.js:460` does `setAttribute("disabled", "")`, `host/web-js/rt.js:705` does `toggleAttribute("disabled", true)`, and `host/web-js/src/rows.rs:859` pushes `""`. `[disabled="true"]` matches none of those. `:disabled` matches a `button`, `input`, `select`, or `textarea`, and a state part on a native control or a list row is refused (`contract/lower/src/nodes.rs:263`). A `column` is a `View` (`contract/lower/src/tags.rs:70`) and the web host renders it as a `div` (`host/web/src/element.rs:477`). That is the pressable in `contract/cli/tests/it/state_looks.rs` and in `host/web/tests/looks.test.mjs`. Hover, `:focus-visible`, and `[data-pressed]` still apply while it is disabled. A real `button` is covered only because of `:disabled`.

   The Chrome test encodes the same miss. `host/web/tests/looks.test.mjs:103` does `setAttribute('disabled', 'true')` on the div, which is the one value the selector matches and not the value either host writes.

   Fix: use `:not(:disabled,[disabled])`. In `looks.test.mjs`, disable the div the way the hosts do (`toggleAttribute('disabled', true)`) and assert hover and press stay on the base. `host/web/input-glue.js:330` still ignores pointerdown with the older `:disabled,[disabled='true']` check; hover and focus-visible never go through that check, so the look shows even when the press does not. The CSS change is what drops the overlay. The draft's stage 8 B1 entry spells the same `:not(:disabled,[disabled="true"])` selector; that spelling is what shipped, and it does not implement the "disabled node takes none" line next to it.

2. **SHOULD-FIX** `host/web-js/src/modes.rs:695`. `written()` builds the sentinel scan from `p.css` and `p.props` only. `project` then rewrites each look (`modes.rs:228`) with `substitute`, which replaces any sentinel form it finds with `var(--token)`. A look literal equal to the chosen sentinel is rewritten. For color token 0 at offset 0 that is `#e10000ff` and `rgba(225,0,0,1)`. The 256-offset search exists so an app's own colors are not stolen; look declarations never enter it. Press-scale numbers are copied onto the base style and are scanned.

   Fix: append every look declaration string inside `written()`.

3. **NIT** `contract/cli/tests/it/state_looks.rs:81`. The comment says a token no look reads sets no look. The fixture's `:hover` border is the token `ink`, and `assert!(set >= 1)` is the positive path again. It never checks the look value. The `accent.soft` block above is the assertion that actually watches the look change.

   Fix: set a theme token the looks do not read and assert the look vector is unchanged.

Composition holds on both targets: one rule per state, hover inside `@media (hover: hover)`, then `:focus-visible`, then `[data-pressed]`, equal specificity, source order last wins, and a row a look omits stays on the base. Wasm rules are `[data-view="id"]`; the JS target interns on the post-substitution declaration list as `[data-exact-looks="N"]` (exact match, so `1` and `10` do not collide). The runner re-sends on mount, mode, pin, live batch, and role override, and destroy clears the sheet entry. Wire decode and `validate` refuse a repeated or non-look row. A declared `:active` is what suppresses the 65% dim; LLP 1061 paint rules still match only nodes that carry `--exact-press-*` and `data-exact-painting`. Easing is `data-exact-easing` for the press look's properties, both ways, and it is omitted under Reduce Motion and on the agent clock. An app with no state parts gets no `SetLooks` and no `data-exact-look`.

`press.test.mjs:585` failed twice here: after a `color` transition during a press, the resting color was `rgb(255, 255, 255)` instead of `rgb(0, 0, 0)`. That fixture has no `--exact-press` and no `data-exact-look`, so `showPress` returns before the new easing state, and the 65% rule already skipped it via `--exact-press-opacity`. `paint()` is unchanged, so this is not a B1 regression.
