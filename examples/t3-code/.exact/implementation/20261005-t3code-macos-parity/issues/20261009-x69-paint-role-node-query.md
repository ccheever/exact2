---
name: 20261009-x69-paint-role-node-query
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261009-blocked-theme-usage-highlight]
upstream_url: null
reproduced_on: null
---

# X69: No read lists the rendered nodes that a paint role reaches, so a theme editor cannot outline or count a colour's uses

**Status (2026-10-09):** local draft from the desktop audit. Not reproduced in a one-file app, not searched upstream
beyond `main`'s `issues/`, not published. `issue-open` reproduces it and checks it against #321 first.

## Summary

T3 Code's theme editor outlines every element that a selected colour role paints and shows the count ("Accent color ·
4 uses"). To do that an app must find, at the moment of selection, every rendered node whose paint uses that role, and
each one's box.

Contract has three structural reads, all action-only: `frame(id)`, `measure("id")` and `elementFromPoint(x, y)`
(`docs/contract-grammar.md:543-545`). None lists the mounted nodes, and none tells which paint a node uses. So an Exact app
cannot outline or count a role's uses.

## Why this issue arose

The 2026-10-09 desktop audit compared the theme editor (finding S1-7, review
[20261009-desktop-audit](../reviews/20261009-desktop-audit.md)).

The T3 Code behavior (`1e2ecbd975`, `apps/web/src/components/settings/`):
- Each role row's label is a toggle ("Show Accent usage"). Selecting it, or opening the row's colour picker, outlines
  every element painted with that role. The header reads e.g. "Accent color · 4 uses" (`ThemeEditorPanel.tsx:556, 1183`).
- `highlightThemeRoleUsage` (`themeInspector.ts:405-450`):
  - takes every element outside the editor (`themeInspectorCandidates`, `:346-352`, `document.body.querySelectorAll("*")`);
  - reads each one's computed paint (`getThemePaintSnapshot`, `:259-295`: background, border, outline, shadow, text,
    fill, stroke, caret colours from `getComputedStyle`);
  - sets the role's CSS variable to a probe colour, reads the paint again, and keeps every element whose paint changed;
  - draws a spotlight over the kept elements' boxes and returns their number.

What the clone does (`c603c22d6`): the role labels are plain text; pressing one or opening a colour picker draws no
outline, and the header stays "Select a color below". Evidence (local, not committed):
`target/t3-audit/evidence/settings-1/S1-7-ref.png`, `S1-7-clone.png`.

Where the clone hits it: the theme editor (`settings-appearance-editor.ts`, the theme editor's contract files). A clone
workaround would need a list of every themed node in the whole app with a fixed id, kept in step with every view by
hand; that is not a reasonable app change.

## Relation to existing issues

- X68 / [#321](https://github.com/ccheever/exact2/issues/321) (`issues/20261009-elements-from-point-read.md` on `main`):
  `elementsFromPoint(x, y)` for Inspect's click. Its report lists "Uses. The editor then shows 'N uses'" as part of
  Inspect's flow, but its request and its current scope ("Design the action-only elementsFromPoint structural read") are
  a point read only. A point read cannot find all of a role's nodes.
- #127 (X22, layout facts): not this; this is a one-time read in an action, not a reactive fact.
- #116 (pixel readback): not this; the reference reads computed style, not pixels.

## Why it must be resolved

Without it the theme editor cannot show where a colour is used, which is how a person checks a theme edit. Any Exact app
with a theme editor or a design inspector hits the same wall. The plan's goal is a complete clone, so the difference
stays declared until the issue is resolved or closed by decision.

## Proposed resolution (options for the owner)

1. An action-only read that lists the mounted nodes carrying a given attribute value, front to back, by id (the analogue
   of `document.querySelectorAll('[data-paint~="accent"]')`). The app names each themed paint's role in that attribute;
   `frame(id)` then gives each box. This keeps the lookup structural (no computed-style or pixel read).
2. Fold the need into #321's design, if Charlie prefers one structural-read design for Inspect and uses.
3. Close by decision: the usage outline becomes a permanent declared difference.

## Workaround in the clone

None. [blocked-theme-usage-highlight](../tasks/20261009-blocked-theme-usage-highlight.md) declares the difference.

## Status and next action

Draft. Next: `issue-open` reproduces the gap in a one-file app on the pinned `main` (show that no read lists nodes by an
attribute), checks for duplicates (#321 first), and prepares the report. Publication needs the user's approval.
