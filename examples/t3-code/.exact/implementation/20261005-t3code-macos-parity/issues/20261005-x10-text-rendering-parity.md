---
name: 20261005-x10-text-rendering-parity
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-interface-font-size, 20261005-live-automations-and-clones, 20261005-provider-settings-upkeep, 20261005-shiki-residuals, 20261005-upstream-timeline-and-markdown, 20261005-usage-pooled-view]
upstream_url: https://github.com/ccheever/exact2/issues/128
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/266
---

# X10: Text renders differently from Chrome in five separate ways (ellipsis, code wrap, balance, placeholder, weight)

## Summary

T3 Code is drawn by Chrome, so every text surface follows Chrome's rules. Exact2's macOS host draws
text with five measured differences: it truncates at word boundaries, breaks code lines at other
positions, has no `text-wrap: balance`, draws no placeholder colour, and renders heavier strokes. Each is
small; together they shift line breaks and pixels on most screens. The clone has no workaround for any of
them. Each needs host support, listed one by one below. No plan ticket waits for X10, but every pixel
pair that moves because of it is a difference the user does not accept as final.

## Why this issue arose

Common to all five. Evidence for exact2: GAPS X10 (EXACT2-GAPS.md, earlier sessions, framework source at
exact2 `c1522fdac`, checked against `main` `d2cb661eb`) lists "truncation at word boundaries instead of
per character (`text-overflow: ellipsis`); code-line wrap break positions; no `text-wrap: balance`; no
placeholder colour; text looks heavier than Chrome." and asks for "Chrome-matching behavior for each row,
held by conformance cases." Bundled library: `capabilities` (revision `fd1c879072aa836bcd2002a1e41aa83c7607e836`)
lists only ordered font fallback and hyphenation for text; everything below is not covered: unknown.
The clone's pixel pairs against Chrome record the differences (AGENT-HANDOFF.md:84-86 and gap #2, line 195).
Paths below: reference `apps/web/src/` at `1e2ecbd975`; clone `examples/t3-code/` (mc-orch tree, 2026-10-05).

### 1. Ellipsis truncation per character
- Reference: single-line `truncate` (`text-overflow: ellipsis`) in 365 uses across 139 files, and
  `line-clamp` in 9 files. Chrome cuts at the last character that fits: "fixture compl…".
- exact2: cuts at a word boundary: "fixture…" (AGENT-HANDOFF.md:86, "word-boundary truncation (…, framework)").
  README.md:270 also lists it.
- Clone: every `line-clamp=` and `text-overflow` row; no workaround. A user sees shorter titles in sidebar
  rows, tab labels, tool rows and tooltips, and text that differs from the reference at a given width.
- **Requested support:** `text-overflow: ellipsis` (and `line-clamp` on the last line) cuts at the last grapheme
  that fits, as Chrome does, without regard for word boundaries.

### 2. Code-line wrap break positions
- Reference: a wrapped code block is `white-space: pre-wrap; overflow-wrap: anywhere`
  (`index.css:1954-1955`, `[data-wrap="true"]`); table cells use `overflow-wrap: anywhere` (`index.css:2021`).
  Chrome prefers break opportunities (after `/`, `-`, spaces) and breaks inside a word only when none fits.
- exact2: breaks code lines at other positions (GAPS X10, handoff gap #2). The clone uses the same rows
  (`timeline-work.contract:296-318`, `white-space="pre-wrap" overflow-wrap="anywhere"`).
- Clone: no workaround. A user sees the same code wrapped at different columns, which changes the number
  of lines and every row below it.
- **Requested support:** `overflow-wrap: anywhere` under `white-space: pre-wrap` uses Chrome's break
  opportunities first and falls back to any character only when needed.

### 3. `text-wrap: balance`
- Reference: tooltips (`components/ui/tooltip.tsx:112`), toasts (`ui/toast.tsx:731`), empty states
  (`ui/empty.tsx:22,115`) and popovers (`ui/popover.tsx`), 5 uses in 4 files. Chrome balances line lengths of
  short blocks.
- exact2: no such row (GAPS X10). Clone: tooltips and toasts wrap greedily, so a two-line message ends with
  a short last line where the reference shows two even lines. No workaround.
- **Requested support:** `text-wrap: balance` (CSS Text Level 4). Chrome limits balancing to short blocks;
  match the limit of the Chromium inside the pinned Electron (to confirm).

### 4. Placeholder colour
- Reference: `placeholder:text-placeholder` and similar, 38 uses in 24 files (`ui/input.tsx:25`; token
  `--color-placeholder`, `index.css:248`), themeable (`--app-theme-placeholder`, `index.css:1252`).
- exact2: the placeholder colour is fixed at the text colour × 0.30 and there is no `::placeholder` row
  (GAPS X11, citing `Mac/TextAreaMac.swift:87-96`). README.md:270: "draws no placeholder colour".
- Clone: no workaround. A user sees placeholder text in a colour that ignores the theme token.
- **Requested support:** `::placeholder { color; opacity }` for `input` and `textarea`.

### 5. Text weight (font smoothing)
- Reference: Settings › Appearance › "Font smoothing" (macOS only; default on): "Use thinner grayscale text
  smoothing instead of the macOS default." On sets `-webkit-font-smoothing: antialiased` on the root; off
  keeps the "heavier" platform default (`appearanceFonts.ts:126-132`; default `true`,
  `packages/contracts/src/settings.ts:398`; row `components/settings/SettingsPanels.tsx:1676-1700`).
- exact2: draws with the macOS default; no row for `-webkit-font-smoothing` is known (to confirm).
- Clone: the setting is stored and shown (`settings-core.ts:53`, `settings-appearance.ts:198`) but nothing
  applies it. The pixel pairs score "heavier native text" (AGENT-HANDOFF.md:84; "pill crops 3.9 / 7.2" in
  gap #2). This is the likely cause of "text looks heavier than Chrome" in GAPS: the reference default is
  the thin setting (to confirm by a render with the property).
- **Requested support:** `-webkit-font-smoothing: antialiased | auto` (inherited) on the macOS host. It is
  the property Chrome and Safari honour on macOS; the standard `font-smooth` was never finalized.

## Why it must be resolved

The goal is a complete clone, and a declared visible difference is not an end state. These five differences
touch almost every screen: truncation in lists and titles, wrap in code and tables, balance in transient
messages, placeholders in every field, weight in all text. They also move line breaks, so a layout that is
pixel-exact in one place drifts below it. Every UI ticket's matrix gate says a moved cell is fixed or
declared "with an issue link"; this issue is that link for text. Cost of leaving it: permanent noise in the
pixel pairs (the settle-sweep sidebar cells score 5.08 / 3.21 / 6.36 / 4.59, attributed in part to heavier
native text, AGENT-HANDOFF.md:84), which hides real regressions.

## Requested support
One line per behavior (see sections 1–5): ellipsis at the last fitting grapheme; Chrome break opportunities
for `overflow-wrap: anywhere` in `pre-wrap`; `text-wrap: balance`; `::placeholder`; `-webkit-font-smoothing`.
All on the macOS host first; the web and iOS hosts are checked against the same conformance cases.
Alternative for each: none; the app cannot draw host text itself. (Item 5 can be partly applied by the app
only if the host exposes the property.)

## How to reproduce
To confirm on the pinned `main` at `issue-open`. One minimal app, five boxes, each compared with Chrome:
1. A 120 px box, `text-overflow: ellipsis`, text "fixture complete table". Chrome: "fixture compl…".
2. A 200 px `pre-wrap` + `overflow-wrap: anywhere` block with a long path. Compare break columns.
3. A 180 px tooltip-like box with `text-wrap: balance` and a 2-line sentence. Compare line widths.
4. `input` and `textarea` with `placeholder` and `::placeholder { color: red }`. Chrome: red.
5. The same paragraph with and without `-webkit-font-smoothing: antialiased`; measure stroke darkness.
Capture both hosts with the agent (`screenshot`) and Chrome through the repository's conformance run.

## Acceptance for the fix
For each behavior a conformance case that renders the box in Chrome and on the macOS host and compares them
(`conform.mjs --strict`, as `CLAUDE.md` describes) plus a macOS agent screenshot; the clone's pixel pairs
for the affected cells (sidebar rows, code blocks, tooltips, toasts, inputs) move toward the oracle by the
amount the cause explains. No case needs a person.

## App adoption after resolution
- Remove "word-boundary truncation", "code-line wrap" and similar notes from README.md and AGENT-HANDOFF.md.
- Apply the font-smoothing setting (`settings-appearance-look.ts:56`) to the root; add `::placeholder`
  colours to the inputs, using the theme token.
- Re-shoot the matrix at 1280×840 and 840×620, light and dark; every previously moved cell is now fixed or
  has a new cause. `issue-close` verifies the conformance cases and the re-shot cells.

**Adoption owner.** The tickets in `blocks` re-shoot their pixel pairs. The app-level work in
this section (apply the stored font-smoothing setting, add `::placeholder` colors) goes into
a follow-up ticket `20261005-adopt-x10-text-rendering` that `issue-close` creates when the fix
lands (planned plan revision).

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce each of the five, search for duplicates, prepare the report for the user's
approval; publication only after approval). If the maintainers prefer, file five reports; this document is the
shared context.

## Merged upstream; partly fixed (2026-10-07, adopt-main-fixes-r4)

Filed as [#128](https://github.com/ccheever/exact2/issues/128) (code wrap, placeholder colour, balance,
smoothing; ellipsis and `line-clamp` already matched Chrome on `4c893fef6` and were not filed). #128 was
closed by main #208 (`05c767ba8`), in the feature branch since main `463acda68`
([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)): a macOS paragraph takes the
shared walker's Chrome line-break opportunities, so a path no longer breaks after each `/`, and
`overflow-wrap: anywhere | break-word` breaks inside a word only when no opportunity fits (item 2).

Not on `463acda68`: items 3, 4 and 5. `kernel/tables/schema.json` has no `text-wrap`, no
`placeholder-color` or `::placeholder`, and no `-webkit-font-smoothing` row; a macOS placeholder is still
the field's text colour at 0.30. Linux paragraphs still break after `/` (main's QUEUE).

Adoption: the clone had no workaround for any item, so nothing is removed. Its `pre-wrap` +
`overflow-wrap="anywhere"` rows (code blocks, tool output, the user message fallback) and its Markdown
paragraphs take #208's breaks with no clone change. Agent drive at 1280×840 (lane message with three long
paths): every path fits the bubble or breaks at a space, and Before (`887b2491b`) and After draw the same
lines. At 840×620, where the bubble is narrower than a path, Before and After are pixel-identical too, and the
two long paths do not break at all: they run past the bubble's right edge and the window
([image](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/adopt-main-fixes-r4/03-x10-wrap-840-before-after.png)). #208 does not change that line. The clone's `UserMarkdown` paragraphs
(`markdown.contract`) set no `overflow-wrap`; whether T3 Code breaks these paths inside the word needs a
reference capture of the same message (follow-up finding, task record). The stored font-smoothing setting stays unapplied (item 5). The issue stays open.

## Rest filed upstream (2026-10-08)

Upstream (the rest): https://github.com/ccheever/exact2/issues/266 (#266, [Design] Text rows: `text-wrap: balance`, an authored placeholder colour, `-webkit-font-smoothing` (rest of #128)). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) before filing: `text-wrap="balance"`, `text-wrap-style`, `-webkit-font-smoothing` and `placeholder-color` are `lower-unknown-attr`; `::placeholder` is not a tag or attribute (the default placeholder colour differs per platform by LLP 1104). One "Decision needed" comment (LLP 1104:165, LLP 1077:127). Searched open and closed issues and PRs: no duplicate.

## Decided upstream (2026-10-08): narrowed; smoothing is a declared difference

[Charlie on #266](https://github.com/ccheever/exact2/issues/266#issuecomment-6055585630): "Choose balanced text and authored ::placeholder color; defer font smoothing. … Keep platform
defaults when absent."
- Waits for main fix of [#266](https://github.com/ccheever/exact2/issues/266): `text-wrap: balance` and an authored placeholder colour (the planned
  adoption follow-up).
- **Declared difference (permanent):** font smoothing is deferred. The stored font-smoothing setting stays
  unapplied, and text looks heavier than in Chrome (item 5).
