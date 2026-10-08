---
name: 20261005-x12-textarea-field-sizing
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-composer-fidelity]
upstream_url: https://github.com/ccheever/exact2/issues/130
reproduced_on: 4c893fef6
---

# X12: A textarea with `field-sizing: content` is sized from its plain string, not from what the native text view draws

**Status (reclassified 2026-10-08):** Bucket 6, deferred: #130 keeps hatch-driven layout feedback out; a permanent declared difference.

## Summary

The T3 Code composer grows with its content, measured as drawn: a file mention is a short pill, so a prompt
with several mentions stays a line or two tall. The clone's composer is an Exact `textarea` with
`field-sizing: content`; exact2 sizes it from the plain string, in which every mention is a long
`[label](t3-context://v1/…)` link, so at narrow widths the box can be a line taller than its pills need.
The clone has no workaround. Needed: size from the native view's laid-out content, or a way for a hook to
report the content height to layout.

## Why this issue arose

### The T3 Code behavior
- The composer is a rich-text editor (`apps/web/src/components/ComposerPromptEditorTiptap.tsx:780`, classes
  `composer-tiptap block max-h-52 min-h-19.5 overflow-y-auto`): its height is whatever its content lays out
  to, between a minimum and a maximum, then it scrolls. Mentions, skills and citations are inline nodes with
  `contentEditable={false}` (`:235,396`) that draw a short label.
- States: empty (minimum height), growing line by line while typing or pasting, at the maximum (inner
  scrolling), resting (collapsed to one line while the thread is scrolled), narrow window (more wraps).
  The browser does all of this by layout; no code measures it.

### What exact2 does today
- GAPS X12 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked against
  `main` `d2cb661eb`): "A textarea sizes to its plain value. A prompt whose chips render wider than the text
  can be one line taller or shorter than REF. Support needed: `field-sizing: content`, or a measured height
  hook." Note: the clone uses `field-sizing: content` already, so what is missing is sizing by the drawn
  content, not the property name.
- Bundled library: not covered: unknown.
- Observed in the clone tree (mc-orch, 2026-10-05): `composer.contract:88` declares
  `field-sizing=(resting ? "fixed" : "content") height=(resting ? 32 : "auto") min-height=(resting ? 32 : 70)
  max-height=(resting ? 32 : 200)`; the draft string holds
  `[label](t3-context://v1/<kind>/<id>)` for each mention (`composer-editor-menu.ts:244-247`); the native
  text view draws the pill (`modules/apple/T3ComposerStyler.swift`).

### Where the clone hits it
- The README lists it as a known in-app difference: "a textarea sizes to its plain value (a prompt whose chip
  links are long can be a line taller than its chips draw at narrow widths)" (README.md:274).
- The GAPS summary table names "measured height" as the workaround. Searching `composer.contract`,
  `r5-composer-measure.ts`, `r4-composer-overlay.ts` and `modules/apple/T3Composer*.swift` finds no such code:
  `r5-composer-measure.ts` measures toolbar labels, not the text height. So the workaround is: none.
- What a user sees: with several long file mentions at a narrow chat column, the composer shows an empty
  extra line under the pills, and the transcript's end reservation follows that taller box.

## Why it must be resolved

The goal is a complete clone, and a declared difference is not an end state. The composer is on screen all
the time, and the extra line moves the composer's top edge, the transcript's end reservation
(`r4-composer-overlay.ts`), so a one-line error shifts several cells in the pixel pairs. `20261005-composer-fidelity` adds an overflow menu, a ring and queue-edit chips around this box and
must verify them at several widths, so it carries the difference. Cost of keeping it: none in code; a
permanent mismatch whenever mentions are long.

## Requested support

The web way: `field-sizing: content` on a form control sizes it from its content as laid out (CSS UI Level
4); a rich editor sized by layout does the same. On the macOS host:
- **A (recommended).** For a textarea whose native view is replaced or decorated by an app hook, size
  `field-sizing: content` from the native view's used content height (the text container's laid-out height,
  including attachments and decorations) instead of from the string.
- **B.** A hook-to-layout channel: let a hook report an intrinsic content height (like `ResizeObserver` for
  content) that layout uses as the element's `auto` height. This is general; it is also what X22 (reactive
  layout facts) would need for other cases.
Trade-off: A is narrow and keeps the contract unchanged; B is reusable but adds a new API and reactivity rules.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Lane backend; open a draft in a project with files. Insert three file mentions with long paths
   (`@` menu), type a short word. Narrow the chat column (open the right panel). Screenshot and read
   `layout composer`.
2. Reference: open the same draft in the desktop oracle (`target/t3-ui-parity/electron-oracle.mjs` from
   `20261005-desktop-oracle-and-trace`) at the same column width. Expected: two lines. Actual in the clone:
   three or more (to confirm), with the pills on two lines and an empty line below.
3. Minimal app (preferred for the report): a `textarea` with `field-sizing: content` whose app hook adds
   inline attachments wider or narrower than the string; compare the used height with the text view's
   `usedRect`.

## Acceptance for the fix
- The minimal app: the textarea's height equals the native view's laid-out content height at three widths,
  while typing, pasting and deleting; max-height and min-height still clamp; no layout loop.
- The clone: for the reproduction in step 1 the composer line count equals the oracle's at 1280×840 and
  840×620; agent `layout composer` shows the height; AppKit `composer` binary gains a case with long
  mentions.
- The transcript reservation (`r4-composer-overlay.ts`) follows the new height without a jump.

## App adoption after resolution
- No clone workaround exists; remove the README.md:274 note and the GAPS summary "measured height" entry.
- Re-shoot the composer cells in `20261005-composer-fidelity` (overflow menu, queue edit, ultrathink ring) at
  both window sizes. `issue-close` verifies the line counts and the cells.

## Status and next action
Published 2026-10-06 as [#130](https://github.com/ccheever/exact2/issues/130) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Decided upstream (2026-10-08): declared difference

[Charlie on #130](https://github.com/ccheever/exact2/issues/130#issuecomment-6055588225): "Keep hatch-driven layout feedback deferred. … Keep the app's specialized editor in its module. … do
not add a generic reportHeight channel."
- **Declared difference (permanent):** the composer can be one line taller or shorter than the reference when its
  chips draw wider than their plain text (narrow widths). The clone has no workaround; EXACT2-GAPS's old
  "measured height" cell was wrong and now says so.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): explicitly deferred; no generic reportHeight.
