# On the web, a list stops 2 points short of its end after a touch drag

**Status:** Open
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
