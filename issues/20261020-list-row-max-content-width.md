# A truncating row in list appearance=none lays out at its text's max-content width

**Status:** Open
**Systems:** kernel (layout), lists
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

A button row in `list appearance="none"` holding a single-line truncating text inside a
`flex=1 min-width=0` column: the row lays out at the text's max-content width. On a 440 pt
screen `layout` showed the column 628 pt wide, so titles ran off the screen instead of ending in an
ellipsis. Rows of wrapping text were laid out correctly.

Workaround: `width="100%" box-sizing="border-box"` on the row, with the side inset moved to the
list's padding. Cost: about 15 minutes.

## Reproduction

```
list appearance="none" flex=1
  button display="flex" margin-left=10
    column flex=1 min-width=0
      text "<200 characters>" white-space="nowrap" overflow-x="hidden" text-overflow="ellipsis"
```

`agent ios "layout <column>"`: the column is wider than the screen.

## Expected: settle against the web first

The web is the layout oracle, and it may agree with Exact here: a `<button>` is sized to fit its
content even as `display: flex`, and a `nowrap` text's min-content width is its whole length, so
the same tree in Chrome may also be 628 pt wide. Run the tree through the conformance oracle.
If Chrome stretches the row, this is a kernel bug. If Chrome agrees, the fix is in the docs: a
list row that truncates needs `width="100%"` (an entry in `docs/agent-pitfalls.md`), and the row
pattern in the docs should carry it.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.
