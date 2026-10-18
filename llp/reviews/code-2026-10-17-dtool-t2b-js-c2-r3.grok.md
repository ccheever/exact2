# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `8a74a0cf8`.
- **Method:** one brief (sha256 `0004b89048446fee514d5a5ae37c37d7940f63e5f352a94cb0b167be58116b84`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 high, 1 medium).
- **Disposition:** taken in part (the last round).
  - The relative flag is the kernel's own parse (`relative::parse`): `em`, `rem` and a hairline in any case or spelling, and nothing else, so `ch`, `lh` and calc text keep the table's value. Tested: the `em` padding is flagged and the plain font size is not.
  - A control start is recorded even where an expression binds the row, so a cleared expression row falls back to the control's initial value.
  - The table carries its plan's digest (`contract::plan_digest`) and each token's build-time slices, for step 3's checks.
  - Limits, for §10: a text style's size (a heading's too) is its ramp at the 16 px root, so a pinned root moves it in `node` and not here. The CSS map is `css_property`, which misses a few rows the emitter writes elsewhere (the press scale, the control size, legacy line clamps, timelines). Tokens an expression names are recovered only through custom properties, so family, type and inline tokens aren't.
  - Not taken: an exact release baseline in the test (the release rt.js was checked equal to main's by hand at each round).

---

I'll review this commit as a read-only pass: the inspect table, the dev-only row keys, and whether the round 1 and 2 fixes hold. First I'll read the review instructions and the commit itself.The commit is step 2 of the JS inspect table. I'll read the prior review rounds and the full diff next, then check each claimed fix against the runner's `node` behavior.Round 2 marked the `em` and hairline cases as read from the element. I'll compare that flag with how `node` resolves lengths, and with what the static tree actually keeps.The canonical `em`/`rem`/`hairline` flag and the release bytes hold. A platform text style still does not, and `u` is not the kernel's parser.

## High

**A text style's size is the 16 px ramp, and the page cannot recover `node`'s — `host/web-js/src/inspect.rs:189`**

`u` is set only for `em`, `rem`, hairline, and a few other spellings. A literal platform text style is none of those, so the row keeps the pixels the static tree resolved at root 16, and step 3 will treat that number as the value.

`font-size="-exact-title1"`, and a level-1 heading (the compiler writes that same literal, `contract/lower/src/headings.rs:87`), is 27 in the table. `text_style_size` is 28 at root 17 and 29 at root 18 (`kernel/src/style/relative.rs:584`). The stylesheet writes `(27/16)rem`, so the element is 28.6875 and 30.375 at those roots. The table, the element, and `node` are three different numbers. Body happens to be the root (`text_style_size(body, root) == root`); title, headline, and caption are not.

An `em` under that size is marked `u`, so the page reads the element and reports the rem approximation (30.375 at root 18) while `node` reports the ramp (29). The §10 limits do not say this. They only say an expression-chosen text style is not a style source.

Fix: recognize the literal with `relative::parse` (it already accepts `-exact-title-1`, the WebKit alias, and any case). Store the style id and the ramp (`TEXT_STYLE_BODIES` and each style's sizes) and let the page run `text_style_size` at the live root. Do not read the element's font-size for these rows.

## Medium

**`u` is a case-sensitive scan, so the round-2 fix misses spellings the kernel accepts and fires on lengths it has already turned into pixels — `host/web-js/src/inspect.rs:189`**

`relative::parse` takes `1e1em` as 10 em and `1EM` / `-EXACT-HAIRLINE` by ASCII case. The scan does not. With `column font-size={n}` (`n` is 20) and `padding-left="1e1em"`, the kernel and the element's `10em` are 200. The table stores 160 and `u` is false, so the page reports 160. `-EXACT-HAIRLINE` at a 2× display is 0.5 in `node` and 1 in the table.

The same scan sets `u` for `ch` and `lh`. Those are fixed cells (8 px and 16 px), and inside `calc` they are already pixels. `width="calc(10ch + 10%)"` is a calc `node` prints as text. `u` tells the page to replace it with the used width.

Fix: call `relative::parse` and set `u` only for `Em`, `Rem`, and `Hairline`. Leave `ch`, `lh`, and calc text on `v`.

## What holds

Literal `none` stays out of `r` (`vm::is_literal`). A one-sided class ternary stays dynamic, and the page treats it as own only while the element has it. A control start is recorded in `i` when an ancestor's mask has the row, including when the value equals the global initial. `b` matches a `mode` or `color-scheme` binding, and `implied` is the runner's eight rows. Tuple numbering matches `modes_of` (first axis most significant). Class rows are ordinary bindings, so they do not bypass this.

The registry does not leak or mis-find. `onEnd` drops the entry with the scope, `live` only skips a disconnected anchor, and effect re-runs do not register again. Nested `each` regions get distinct ids. Bracketed, adopted, lazy, and moved rows share the runtime's `start` and `end`. Region 0 is registered. A virtualized list stays `[region, null]` and keeps `data-listitemkey`. Cost while alive is one `Set` entry, one `raw` field, and one `onEnd`.

`project` is the old kernel build moved into `static_tree` and called once. The folding loop is unchanged. `table()` runs after `emit`, only under `dev_reload`. The region id is appended only then.

Release bytes: `withoutDev` of the current `rt.js` equals `HEAD~1:host/web-js/rt.js` (117314 bytes). `build.mjs` strips `rt.js` whenever `devReload` is false, and copies `dev-inspect.json` only for `--dev` after deleting any old one. The five named limits match the code, except the text-style size gap above is not among them.

The Rust tests compare the theme fixture, a `for`/`if` contract, and `font-size=20 padding-left="1em"` plus a button under `text-transform="uppercase"`, at the boot tuple and root 16. They never read `u`, never move the root, and never use a text style, `1e1em`, or a hairline. `inspect.test.mjs` checks that the stripped file is a subsequence of the source.

**Verdict:** Request changes — canonical `em`/`rem`/`hairline` and the release bytes hold, but a text style (including a heading's size) is still the 16 px ramp, and `u` misses the kernel's other spellings of those units.

