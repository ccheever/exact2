# `border-style`: `dotted` and `dashed`

**Status:** Open
**Systems:** kernel schema, Contract, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/287

## Current scope

Implement approved dotted/dashed borders from schema.json, compared with Chrome for mixed sides, widths and rounded corners. Preserve layout widths; other styles are not admitted.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

Dotted and dashed borders are common in desktop UI:

- a dotted underline marks an abbreviation or a "+N more" toggle;
- a dashed outline marks a drop zone or an empty slot.

CSS's `border-style` takes `dotted` and `dashed` (and `double`, `groove`, `ridge`, `outset`). Contract's border rows accept `none | hidden | solid | inset` only. LLP 1001 says "Other line styles are refused until a consumer needs their painting" (`llp/1001-kernel-v1.spec.md:100-103`). This issue is that consumer request for `dotted` and `dashed`.

### Current and expected behavior

- **Current:** `border-bottom-style="dotted"` fails `contract build` with `lower-attr-value`: "`border-bottom-style=\"dotted\"` is not a valid `border-bottom-style`: expected one of \"none\", \"hidden\", \"solid\", \"inset\"". `dashed` is refused the same way. `contract vocab border-style` lists `enum none|hidden|solid|inset`.
- **Expected:** `dotted` and `dashed` on `border-style` and `border-<side>-style`, painted as Chrome paints them (dot and dash length from the border width, corners as Chrome joins them) on every host, with Chrome as the oracle.

### Reproduction and evidence

In an app made by `bun scripts/exact.mjs new <dir>`:

```text
component Dotted
  view
    main testId="root"
      text "+3" border-bottom-width=1 border-bottom-style="dotted" border-bottom-color="#888888" testId="more"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| `dotted` | `bun exact.mjs contract build dotted.contract --json` | compiler (any host) | main `0365ad1a4` | `lower-attr-value` (expected one of none, hidden, solid, inset) | `[]`, and a 1 px dotted underline as in Chrome | compiler JSON |
| `dashed` | the same file with `dashed` | compiler | same | `lower-attr-value` | `[]` | compiler JSON |
| Vocabulary | `bun exact.mjs contract vocab border-style` | any | same | `enum none\|hidden\|solid\|inset, default none` | includes `dotted`, `dashed` | CLI output |

`kernel/tables/schema.json`'s border-style rows are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- `border-style`, `border-<side>-style` and the shorthand accept `dotted` and `dashed`. A bound value is checked against the same set.
- Native hosts paint them as Chrome does for 1, 2 and 4 px widths, on straight and rounded sides. The web emits CSS.
- `border_widths()` and layout treat them as visible styles (they keep their widths), as `solid` does.

### Constraints and related work

- Workaround: a `solid` border in the same colour, or an SVG `line` with `stroke-dasharray` beside the text. The SVG keeps the look, but the line is laid out outside the text box.
- `double`, `groove`, `ridge` and `outset` are not requested here.
- Related: LLP 1001 §Border semantics; LLP 1077 (CSS visual properties native hosts draw).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:32Z

**Decision: Admit dotted and dashed borders for the named consumer.**

Keep open with the bounded scope below.

LLP 1001 explicitly waits for a consumer; this request supplies one. Keep the scope to these two CSS styles.

Use schema.json as the authority and Chrome fixtures for side widths, mixed sides and rounded corners. Preserve border widths in layout; do not add the other unrequested border styles.
