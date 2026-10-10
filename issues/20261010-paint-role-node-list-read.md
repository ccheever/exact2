# No read lists the rendered nodes a paint role reaches, so a theme editor cannot count or outline a colour's uses

**Status:** Open
**Systems:** Contract, runner geometry, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261009-x69-paint-role-node-query.md; issues/20261009-elements-from-point-read.md (#321)

## Summary

An app cannot ask "which mounted nodes carry this mark?". Contract has three structural reads, all
action-only: `frame(id)`, `measure("id")` and `elementFromPoint(x, y)` (`docs/contract-grammar.md:546-548`).
Each answers for one id or one point. None lists nodes.

An app can already tag a node with its own word: `data-paint="accent"` (LLP 1075.003 §9.1). The hosts
keep it: the web writes it as a real attribute, and the macOS agent tree shows `dataset: {"paint":"accent"}`
on each node. But no read finds the nodes that carry it. So a theme editor cannot count a colour's uses or outline them.

The web's answer is `document.querySelectorAll('[data-paint~="accent"]')`, then
`getBoundingClientRect()` on each.

## Why this arose

T3 Code's theme editor (`1e2ecbd975`, `apps/web/src/components/settings/`):

- Each colour role's label is a toggle ("Show Accent usage"). Selecting it, or opening the role's colour
  picker, outlines every element that role paints. The header reads e.g. "Accent color · 4 uses"
  (`ThemeEditorPanel.tsx:556, 1183`).
- `highlightThemeRoleUsage` (`themeInspector.ts:405-450`) walks every element outside the editor
  (`:346-352`), reads its computed paint (`:259-295`), and keeps those whose paint changes when the role's
  CSS variable changes. It draws a spotlight over their boxes and returns their number.

A Contract app writes every paint itself, so it can name the role in a `data-*` word instead of probing
computed style. What it lacks is the list.

In the clone (task `blocked-theme-usage-highlight`) the role labels are plain text, no outline is drawn,
and the header stays "Select a color below". The only workaround is a hand-kept list of every themed
node's id in the whole app, updated with every view. That is not a reasonable app change.

**Not #321.** `issues/20261009-elements-from-point-read.md` asks for `elementsFromPoint(x, y)`: every node
at one point, for Inspect's click. Its report says the "N uses" count and the spotlight are "not
requested here". A point read cannot find nodes that are elsewhere on screen. The two could share one
design (see Decision needed).

## Reproduction

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` exports an empty `sources`; `app.json` has
`"data": ["paint"]`.

```text
component X69app
  state rows = [0, 1]
  state uses = "-"
  action addRow
    rows = concat(rows, [length(rows)])
  action showUses
    // T3 Code counts and outlines every element a colour role paints ("Accent · N uses").
    // Contract's structural reads are frame(id), measure("id") and elementFromPoint(x, y).
    uses = "no read lists the nodes with data-paint=accent"
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 display="flex" flex-direction="column" gap=8 background-color="#ffffff" color="#111111"
      row gap=8
        button press=addRow testId="add"
          text "Add a row"
        button press=showUses testId="uses"
          text "Show Accent uses"
      text `Accent: ${uses}` testId="out"
      button id="save" data-paint="accent" background-color="#2563eb" color="#ffffff" testId="save"
        text "Save"
      each i in rows key=i
        text `row ${i}` id=`row-${i}` data-paint=(i % 2 == 0 ? "accent" : "muted") color=(i % 2 == 0 ? "#2563eb" : "#71717a")
```

| Scenario | Commands | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|---|
| The marks are there | `bun exact.mjs mac`; `bun exact.mjs agent macos --json --size 420x260 "tap add" "tap add" tree` | macOS 26.6.2 (25G83), Apple Silicon | main `abc1eadff`; #327 head `14493d253` the same | `save`, `row-0`, `row-2` carry `{"paint":"accent"}`; `row-1`, `row-3` carry `{"paint":"muted"}` | (the same) | record |
| A list read | `showUses` set to `` `${length(querySelectorAll("[data-paint~=accent]"))} uses` ``; `bun exact.mjs contract build` | any (compiler) | main `abc1eadff`; #327 the same | `` [type-unknown-function] `querySelectorAll` is not in the stdlib roster `` | a list read that answers `["save", "row-0", "row-2"]`, so the count is 3, and `frame()` of each gives the outline | record |
| #321's read | the same with `elementsFromPoint(0, 0)` | any | same | `` [type-unknown-function] `elementsFromPoint` … did you mean `elementFromPoint`? `` (not built; it would answer one point only) | — | record |

Record (contract, app.json, commands, trees, compiler output):
https://raw.githubusercontent.com/ccheever/exact2/d4109d63e545054c04bda13fea11338245556553/fw-issues-20261010/x69-record.txt

## Constraints

- The structural reads are action-only and one-shot (`docs/contract-grammar.md:546-548`; `frame` LLP
  1051.000 D1, `elementFromPoint` LLP 1094 D10 at `llp/1094-dropping-across-lists.rfc.md:366-385`). A
  list read would be the same kind: refused outside an action (`type-geometry-outside-action`), never
  reactive.
- Reactive geometry stays deferred (#127's scope, `issues/20261009-intersection-visibility-event-design.md`).
  This asks for no reactive fact.
- `data-*` words are declared in `app.json` and lowered to one `dataset` row
  (`llp/1075.003-native-platform-control-merged.plan.md:880-887`). A read over them adds no new attribute.
- No computed-style or pixel read is asked for (#116 keeps pixel readback deferred).
- #327 changes nothing under `contract/`, so its head refuses the same.

## Acceptance criteria

- In the repro, `showUses` can set `uses` from a read that returns the ids of the mounted nodes whose
  `data-paint` holds `accent`: `["save", "row-0", "row-2"]` after two "Add a row" presses, in one
  documented order (tree order, as `querySelectorAll`).
- A node that is not mounted, `display: none` or under an `inert` subtree follows one documented rule.
- `frame(id)` of each returned id gives its box, so the app can draw the outline.
- The same answer on macOS and the web; refused outside an action.

## Decision needed

Which shape, if any:

1. A list read over `data-*` words: `querySelectorAll` with a bounded selector subset
   (`[data-<word>~="value"]`), or a narrower `elementsByData("paint", "accent") -> list<string>`.
2. Fold it into #321's design: one structural-read design that covers both a point (`elementsFromPoint`)
   and a mark.
3. Close by decision: the theme editor's usage outline stays a declared difference in the clone.
