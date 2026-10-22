# A `time` element in a virtualized list's row draws no text on iOS

**Status:** Open
**Systems:** Apple host (iOS), the virtualized `list`
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

`list virtualized=true` → `for m in rows key=m.id` → `time datetime=m.at format="date-long"`:
on the iOS simulator the row's Text is empty; the web draws "October 22, 2026", and
the same `time` outside a list formats on iOS. Excord's Discord timestamps
(`<t:unix:style>`) draw blank in the transcript, and its date dividers use
`formatDate` instead.

## Repro

The Excord commit `93b72555ad` (branch `worktree-agent-ae0ab8a6e2cffecaf`) before its
`formatDate` change, or a three-line contract as above; `bun scripts/agent.mjs ios tree`
shows the `Text` with no text.
