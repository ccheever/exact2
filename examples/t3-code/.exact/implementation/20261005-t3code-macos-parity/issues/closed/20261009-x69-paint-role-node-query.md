---
name: 20261009-x69-paint-role-node-query
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261009-blocked-theme-usage-highlight]
upstream_url: null
reproduced_on: null
---

# X69: No read lists the rendered nodes that a paint role reaches, so a theme editor cannot outline or count a colour's uses

Moved to main `issues/20261010-paint-role-node-list-read.md` (2026-10-10); tracked there. Filed on main by PR #386
(merged `e281d83e9`).

## Summary

T3 Code's theme editor outlines every element that a selected colour role paints and shows the count ("Accent color ·
4 uses"). To do that an app must find, at the moment of selection, every rendered node whose paint uses that role, and
each one's box. Contract's structural reads (`frame(id)`, `measure("id")`, `elementFromPoint(x, y)`) answer for one id
or one point; none lists the mounted nodes or tells which paint a node uses.

## Why it arose

The 2026-10-09 desktop audit compared the theme editor (finding S1-7, review
[20261009-desktop-audit](../../reviews/20261009-desktop-audit.md)).

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
outline, and the header stays "Select a color below". Where the clone hits it: the theme editor
(`settings-appearance-editor.ts`, the theme editor's contract files). A clone workaround would need a list of every
themed node in the whole app with a fixed id, kept in step with every view by hand; that is not a reasonable app change.

Related: X68 / [#321](https://github.com/ccheever/exact2/issues/321) (`elementsFromPoint(x, y)` for Inspect) is a point
read only and cannot find all of a role's nodes.

## Clone workaround

None. [blocked-theme-usage-highlight](../../tasks/20261009-blocked-theme-usage-highlight.md) declares the difference.

## Evidence and history

- Seen in the 2026-10-09 desktop audit; evidence (local, not committed): `target/t3-audit/evidence/settings-1/S1-7-ref.png`,
  `S1-7-clone.png`.
- Kept as a local draft (user decision, 2026-10-10) until main PR #386 filed it on main on 2026-10-10.
