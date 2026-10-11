---
name: 20261009-blocked-theme-usage-highlight
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Blocked: the theme editor's colour usage highlight and "N uses"

## Outcome

In the reference's theme editor, selecting a colour role outlines every element that the role paints, and the header
counts them ("Accent color · 4 uses"). The clone cannot find those elements: no Contract read lists the rendered nodes or
their paint. This record holds the finding until a framework read exists. It does not build anything.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S1-7 | Each row label is a toggle ("Show Accent usage"). Selecting it, or opening a row's colour picker, outlines every element painted with that role, and the header reads "Accent color · 4 uses" / "Background · 27 uses". | Row labels are plain text. Opening a colour picker or pressing a label shows no outline; the header stays "Select a color below". | Settings › Appearance › Create theme › press the "Accent" label (or the Background swatch). | `target/t3-audit/evidence/settings-1/S1-7-ref.png`, `S1-7-clone.png` |

## Why it cannot be built

- **How the reference does it.** `highlightThemeRoleUsage` (`apps/web/src/components/settings/themeInspector.ts:405-450`)
  takes every element in the document (`themeInspectorCandidates`, `:346-352`: `document.body.querySelectorAll("*")`
  outside the editor), reads each one's computed paint (`getThemePaintSnapshot`, `:259-295`: `getComputedStyle`
  colours, borders, outline, shadows, fill, stroke), swaps the role's CSS variable for a probe colour, reads the paint
  again, and outlines every element whose paint changed. The count is the number of outlined elements
  (`ThemeEditorPanel.tsx:556, 1183`).
- **What Exact has.** Contract's structural reads are `frame(id)`, `measure("id")` and `elementFromPoint(x, y)`, all
  action-only (`docs/contract-grammar.md:543-545`). None lists the mounted nodes, and none reads a node's paint. So the
  clone cannot find "every node painted by role R", and so cannot outline them or count them.
- **The binding rules.** The T3 branch builds no framework fix; a T3 task blocked on a framework gap waits for the fix on
  `main` and an adoption round (user, 2026-10-08, `plan.md:56-60`, "Framework vs T3 work"). For the same paint lookup
  behind Inspect, the user decided U18 (`plan.md:103, 414`): file a framework issue, then wait.
- **Not covered by an existing issue.** X68 ([#321](https://github.com/ccheever/exact2/issues/321), main file
  `issues/20261009-elements-from-point-read.md`) asks only for `elementsFromPoint(x, y)`, a point read for Inspect's click.
  Its current scope (Charlie's transfer, 2026-10-09): "Design the action-only elementsFromPoint structural read". It does
  not list nodes by paint. The closed [settings-scoped-controls-and-theme-editor](closed/20261005-settings-scoped-controls-and-theme-editor.md)
  excluded Inspect only (line 31). So this is a new framework gap: local draft
  [X69](../issues/closed/20261009-x69-paint-role-node-query.md).

## What was compared

Create theme in both apps; press each role label and open the Accent and Background swatches. Everything else in the
editor matched (rows, roles, Advanced groups, minimize, close), except Inspect (S1-15, waits for X68) and the colour
picker's placement (S1-10, waits for X17).

## Current clone state

The theme editor's role labels are plain text; no outline; the header says "Select a color below".

## Declared difference

No usage outline and no "N uses" count in the theme editor, until X69 is resolved on `main` and adopted.

## What would unblock it

X69 resolved on `main` (a read that lists the nodes a paint role reaches, with their frames, or an equivalent Charlie
chooses), then an adoption round. The clone then needs each themed paint to name its role (X68's report already suggests
ids such as `paint-surface-card`). If Charlie closes X69 as not planned, this becomes a permanent declared difference in
`EXACT2-GAPS.md`.

## Next action

Wait for X69's `issue-open` (reproduce, deduplicate against #321, report with the user's approval). No PR until then.
