# On the web, a list stops 2 points short of its end after a touch drag

**Status:** Closed
**Resolution:** Fixed in 16ed05d9c7: the glue holds a request's correction while the port coasts and lands it at rest; runners count only landed corrections
**Systems:** web host (the JS target's `list.js`), the virtualized `list`
**Severity:** P3
**Author:** Claude (Opus 5.5), from Excord's transcript lane (`apps/excord/llp/0001-excord.rfc.md` §3.9)
**Date:** 2026-10-22

## Symptom

After a touch-emulated drag on a virtualized list, a `scrollIntoView` to its last row
with a large `offset` (which the list clamps to its true end) reports done but leaves
the list 2 points short of its end. Without a drag, and on iOS, the same call lands at
the end exactly. Excord's test "a send while the list is still moving lands at the live
end" checks for the live tail rather than a zero gap because of it.

## Repro

`bun scripts/agent.mjs web --app excord`: drag the transcript up past one window, send
a message, `clock settle`, then `layout transcript`: the scroll extent minus the offset
is 2 pt more than the port.

## Diagnosis (2026-10-24)

Reproduced on main with Excord's test sequence (`tap transcript into …`, `type composer …`,
`tap transcript drag 0 500 over 60`, `tap send`, `clock settle`): `endGap` 1 or 2 in two of
three drives. Chrome's touch fling from the drag goes on after the send's `scrollIntoView`:
an instant `scrollTo` does not end it (nor does toggling `overflow` within the task), so
each frame the fling's step moves the port off the end and the glue places it back
(`collection-glue.js` `commit`, the authored correction), 29 px, then 26, 25 … 1. The request
counts two aligned reports and ends `done` while the fling still has a pixel or two to go,
and those land after it.

## Decision needed

How an authored jump meets the reader's momentum on the web. The Apple host does not
move a decelerating list (`CollectionIOS.swift` `correct`: UIKit owns the deceleration; the
request lands at the next report after it), so the jump waits for the fling. The web host
fights it. Options: (a) defer an authored correction while the port moves with no input
since the request, as Apple does, and land it at `scrollend`; (b) keep the request
`seeking` until the port rests (no move between two frames), re-placing it each frame;
(c) end the fling by making the port unscrollable for a frame (`overflow: hidden`), which
can repaint scrollbars and costs a layout.
