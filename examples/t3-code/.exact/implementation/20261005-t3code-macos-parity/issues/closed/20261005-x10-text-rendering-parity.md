---
name: 20261005-x10-text-rendering-parity
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-interface-font-size, 20261005-live-automations-and-clones, 20261005-provider-settings-upkeep, 20261005-shiki-residuals, 20261005-upstream-timeline-and-markdown, 20261005-usage-pooled-view]
upstream_url: https://github.com/ccheever/exact2/issues/128
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/266
---

# X10: Text renders differently from Chrome in five separate ways (ellipsis, code wrap, balance, placeholder, weight)

Moved to main `issues/20261009-balanced-text-placeholder-color.md` (2026-10-09); tracked there.

## Summary

T3 Code is drawn by Chrome, so every text surface follows Chrome's rules. Exact2's macOS host drew
text with five measured differences: it truncated at word boundaries, broke code lines at other
positions, had no `text-wrap: balance`, drew no placeholder colour, and rendered heavier strokes. Each is
small; together they shift line breaks and pixels on most screens. The clone has no workaround for any of
them.

## Why it arose

The clone's pixel pairs against Chrome recorded the differences (AGENT-HANDOFF.md:84-86 and gap #2, line 195).
Paths below: reference `apps/web/src/` at `1e2ecbd975`; clone `examples/t3-code/` (mc-orch tree, 2026-10-05).

1. **Ellipsis truncation per character.** Reference: single-line `truncate` (`text-overflow: ellipsis`) in 365 uses across 139 files, and
   `line-clamp` in 9 files; Chrome cuts at the last character that fits ("fixture compl…"). exact2 cut at a word boundary ("fixture…").
   Already matched Chrome on `4c893fef6`, so not filed.
2. **Code-line wrap break positions.** Reference: a wrapped code block is `white-space: pre-wrap; overflow-wrap: anywhere`
   (`index.css:1954-1955`); table cells use `overflow-wrap: anywhere` (`index.css:2021`). The clone uses the same rows
   (`timeline-work.contract:296-318`); exact2 broke them at other positions, changing the number of lines and every row below.
3. **`text-wrap: balance`.** Reference: tooltips (`components/ui/tooltip.tsx:112`), toasts (`ui/toast.tsx:731`), empty states
   (`ui/empty.tsx:22,115`) and popovers (`ui/popover.tsx`). In the clone tooltips and toasts wrap greedily, so a two-line message ends with
   a short last line where the reference shows two even lines.
4. **Placeholder colour.** Reference: `placeholder:text-placeholder` and similar, 38 uses in 24 files (`ui/input.tsx:25`; token
   `--color-placeholder`, `index.css:248`), themeable (`--app-theme-placeholder`, `index.css:1252`). In the clone placeholder text ignores the theme token
   (the macOS placeholder is the field's text colour at 0.30).
5. **Text weight (font smoothing).** Reference: Settings › Appearance › "Font smoothing" (macOS only; default on) sets `-webkit-font-smoothing: antialiased` on the root
   (`appearanceFonts.ts:126-132`; default `true`, `packages/contracts/src/settings.ts:398`; row `components/settings/SettingsPanels.tsx:1676-1700`).
   The clone stores and shows the setting (`settings-core.ts:53`, `settings-appearance.ts:198`) but nothing applies it; the pixel pairs score "heavier native text" (AGENT-HANDOFF.md:84).

## Clone workaround

None for any item. Declared differences: tooltips and toasts are not balanced and placeholders do not take the theme colour until main adds them;
font smoothing is a permanent declared difference (decision of 2026-10-08): the stored setting stays unapplied and text looks heavier than in Chrome.
Once main adds balance and an authored placeholder colour, the clone applies them (`::placeholder` colours from the theme token, `text-wrap` on tooltips, toasts and empty states) and re-shoots the affected pixel pairs.

## Evidence and history

- Filed as [#128](https://github.com/ccheever/exact2/issues/128) (2026-10-06; code wrap, placeholder colour, balance, smoothing). Closed by main #208 (`05c767ba8`), in the feature branch since main `463acda68`
  ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)): a macOS paragraph takes the shared walker's Chrome line-break opportunities (item 2).
- Adoption (r4): nothing to remove. The clone's `pre-wrap` + `overflow-wrap="anywhere"` rows and its Markdown paragraphs take #208's breaks with no clone change.
  Agent drive at 1280×840: every path fits the bubble or breaks at a space; at 840×620 Before (`887b2491b`) and After are pixel-identical and two long paths run past the bubble's right edge
  ([image](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r4/03-x10-wrap-840-before-after.png)); the clone's `UserMarkdown` paragraphs set no `overflow-wrap` (follow-up finding in the task record).
- The rest filed as [#266](https://github.com/ccheever/exact2/issues/266) ([Design], 2026-10-08), reproduced on main `0365ad1a4` before filing: `text-wrap="balance"`, `text-wrap-style`,
  `-webkit-font-smoothing` and `placeholder-color` are `lower-unknown-attr`; `::placeholder` is not a tag or attribute.
- #266 was narrowed on 2026-10-08 to balance and an authored placeholder colour; font smoothing deferred.
