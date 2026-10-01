# A text field's 20 characters are wider on native than on the web

**Status:** Closed
**Resolution:** declared, not matched (Charlie: the web is the standard, not Chrome): a field's character is the width of 0 in its font on every host; LLP 1001's declared deviations
**Systems:** kernel, Apple host, Linux host
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** issues/closed/20260930-native-input-stretches.md, LLP 1069.001

A text field at `field-sizing: fixed` (the default) is 20 characters wide, as HTML's `<input>` is. The kernel measures that as the width of twenty `0`s in the field's font (`kernel/src/arena.rs` `text_runs_after`, `NodeType::TextInput`). Chrome uses the font's average character width instead. With the same font (`system-ui`, 16 px, macOS 27), Chrome's content box is **175 px** and the macOS host's is **195.31 px** (20 × 9.77, the width of `0`). 175 / 20 = 8.75 px matches SF Pro's OS/2 `xAvgCharWidth` (1120 of 2048 units at 16 px), which is what Blink's `LayoutTextControlSingleLine` multiplies by when the font has a valid one. A textarea (20 columns) differs the same way: Chrome 183 px, macOS 195.31 px.

Measured 2026-09-30 with a fixture of fields in block, flex, absolute and percentage containers, driven on web, macOS and Linux (`agent.mjs <host> --plan inputs.plan layout`). After the stretch fix, every case agrees in kind; only this width differs.

To match: the text measurer needs one more question, the font's average character width (CoreText: the OS/2 table through `CTFontCopyTable`; cosmic-text: the face's OS/2 table), with `0`'s width as the fallback when a font has none, as Blink does. The kernel would then multiply by the field's size (20) where it now measures the string.

## What Blink does, and why it isn't matched yet (2026-09-30, later)

Blink's rule, read from its source (`core/layout/layout_box.cc`
`TextFieldIntrinsicInlineSize` and `TextAreaIntrinsicInlineSize`;
`core/layout/forms/layout_text_control.cc` `GetAvgCharWidth`,
`HasValidAvgCharWidth`):

- `char` is the primary font's OS/2 average width, `max(avg, roundf(avg))`,
  unless the family is on Blink's list of fonts with a bad average
  (Helvetica, Times, Courier, Lucida Grande and about 40 more) or the
  average exceeds 1.7 × the width of `0`; then `char` is the width of `0`.
- An input is `ceil(size × char + (maxChar − char))`, `size` 20, with
  `maxChar` the font's `MaxCharWidth` when the average is valid, else 0.
- A textarea is `ceil(cols × char)` plus the scrollbar's thickness under
  `overflow: auto`.

Chrome's measured widths for the page's generic families, content box, px
(`--plan` fixture, headless Chrome on macOS 27):

| family | 13 | 16 | 20 | textarea 16 |
|---|---|---|---|---|
| system-ui | 146 | 175 | 214 | 183 |
| sans-serif | 145 | 166 | 208 | 175 |
| serif | 145 | 166 | 208 | 175 |
| monospace | 164 | 205 | 248 | 215 |

These do not follow from the macOS fonts' own tables under that rule.
CoreText gives SF (`.SFNS-Regular`) at 16 px an average of 9.273 and a `0`
of 9.766, so the rule gives at least 185 px against Chrome's 175. Helvetica,
Times, Menlo and Courier New miss too (script:
`CTFontCopyTable` for OS/2 and head, run 2026-09-30). So either headless
Chrome resolves these generics to other faces than CoreText's defaults, or
Skia's `fAvgCharWidth`/`fMaxCharWidth` differ from the raw tables.

Next step, before any code: measure in the page what face Chrome actually
paints for each generic (a text of twenty `0`s, and one `x`, per family and
size, beside the fields), then fit the rule to those. The fix itself needs
the average and maximum character widths from each host's measurer
(CoreText's OS/2 and head tables; cosmic-text's face), which on Apple is an
addition to the measure callback in `exact.h` (an `EXACT_ABI_VERSION` bump).
Stopped after three rounds (a formula from memory, the fonts' tables,
Blink's source), per AGENTS.md.

## Closure audit (2026-09-30)

Retained open. The average/maximum-width mismatch and the recorded three-round
stop still apply; no guessed formula or host ABI change was made during the
ticket cleanup. This needs the resolved Chrome face/metrics experiment named
above before an implementation can be justified.

## Decided (Charlie, 2026-09-30)

Not matched: "the web is the standard not chrome". The web leaves a
character's width to the browser, and browsers and platforms differ, so the
kernel keeps its rule (twenty `0`s, Blink's own fallback, for every font) and
LLP 1001's declared deviations say so. What made layouts differ, a field
stretching in a block on native, was fixed in 90d967b3.
