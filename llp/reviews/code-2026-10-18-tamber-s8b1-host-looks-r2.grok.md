# Code review: `:active` as the press paint; looks on the runner and the web (LLP 1121 stage 8 B1, B1'), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `529bb8896..cc98d1d21` (B1, its round-1 fixes, B1' for D8 as amended).
- **Method:** one brief (sha256 `ae0b8a0389d166e8b1b6eae3eb30376b1e0f3050c4e2285fc918d7add0d95d60`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (blocking: the JS target's looks over the press paint; should-fix: the D7 docs paragraph, the press mix's frozen base under a live sheet edit; nit: the internal row in a style block and typo suggestions).
- **Disposition:** in the round-2 commit, with tests. 1: unscoped, asserted. 2: the paragraph says `:active` is the same paint, one spelling. 3: the glue re-reads what is beneath on a change to the head's sheets or the root while it paints (Chrome test edits the looks sheet mid-press). 4: refused in a `style` block (test), and never suggested.

---

The review is still open. I'll finish the test-module move check and the remaining correctness items, then give the verdict.The three test modules are pure moves. Next I’ll pin line numbers on the cascade, lowering, and digest checks that still decide the verdict.**WRONG.** Round 1’s fixes hold (disabled look selector, sentinel scan, `SetLooks` validation, boot clearing `#exact-looks`, wasm looks under the press paint, Space on an `:active`-only button, `unset` crossing `written`). The caps test-module moves are pure moves. Digests match the two pins (`FORMAT_DIGEST` `0xa73a_9b21_a5db_bd34`, `SCHEMA_DIGEST` `0x249a_8376_117a_eb08`); apps with no state parts change only those 16 header bytes. The JS target still paints `:hover` / `:focus-visible` over the press.

1. **BLOCKING** — `host/web-js/src/emit.rs:770`. Look rules are prefixed with `#exact-root`, then `look_rules` (`host/web/src/host/looks.rs:35-36`) adds `:hover` or `:focus-visible` and `:not(:disabled,[disabled]:not([disabled="false"]))`. That selector is `(1, 4, 0)` and `!important`. The shell press rules (`host/web/index.html:305-308`) are `(1, 2, 0)` and `!important`. `host/web-js/build.mjs:610` concatenates shell CSS and then app CSS in one `<style>`, so a tie also goes to the look. On the JS target, a hovered or focus-visible press shows the look’s `background-color` and `opacity` instead of the press mix. Wasm already omits the id (`host/web/src/host/looks.rs:51`), which is why the press wins there: `(0, 4, 0)` loses to an id. `modes_tests.rs:538` only searches for the substring `[data-exact-looks="1"]:hover`, which the prefixed selector still contains.

   Fix: emit `format!("[data-exact-looks=\"{}\"]", i + 1)` and assert the stylesheet has no `#exact-root [data-exact-looks`.

2. **SHOULD-FIX** — `docs/contract-for-agents.md:2386`. The pressed-look paragraph still says Contract has no `:active` and no pressed block. The state-part section this range updated (`docs/contract-for-agents.md:3096-3100`) says `:active` lowers to the press paint and that its opacity is the absolute pressed opacity. An agent that stops at the earlier paragraph will not write `:active`.

   Fix: rewrite the paragraph at 2375 so `:active` is that press paint (absolute opacity, one spelling with the D7 rows) and point at the state-part section.

3. **SHOULD-FIX** — `host/web/input-glue.js:241-243` and `:273-274`. The press mix freezes `--exact-press-base-opacity`, `--exact-press-base-background`, and `--exact-press-base-color` from `getComputedStyle` at paint time. Repaint runs only when that element’s `style` or `class` changes, or the color scheme flips. A live token rewrites the look in `#exact-looks` (`host/web/glue.js:615`) or, on the JS target, a `var()` in that rule, without touching the element. For the rest of the hold, opacity, background, and color keep easing from the values captured at press, while border and shadow (which the press rules do not override) follow the new look.

   Fix: after the looks sheet or the theme sheet is written, repaint every element in `paints` (a hook the looks/theme ops call, or a `childList` observer on those style elements).

4. **NIT** — `contract/lower/src/style_names.rs:286`, `contract/lower/src/lib.rs:345`, `contract/lower/src/tags.rs:985`. `-exact-press-opacity-to` is a real style name, so a `style` block may contain it (refusal happens only once a node expands it, `contract/lower/src/nodes.rs:288-296`) and `similar_attr` can offer that spelling for a one-edit typo. `contract vocab` already hides it.

   Fix: reject that name in the style-declaration loop with the same `lower-unknown-attr` as `nodes.rs`, and exclude it from `similar_attr`. Leave it in the table lowering uses after the fold.
