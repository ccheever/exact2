# On the web, a list opened at its end reports one scroll away from the end

**Status:** Open
**Systems:** the virtualized `list` on the web (`scroll-start="end"`, `scrollFollowEnd`), its `scroll` event
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's wiring lane (`apps/excord/llp/0001-excord.rfc.md` §3.1)
**Date:** 2026-10-22

## Symptom

Excord's transcript opens with 200 rows at its end. On the web its `scroll` handler
hears three reports as the channel opens: the end (first visible row 9995, 0 px from
the end), then first visible row 9990 866 px from the end, then the end again (row
9994, 0 px). Nothing scrolled the list; the middle report comes while the rows' measured
heights replace `estimated-item-height` and the images decode in their declared
boxes. The iOS simulator reports only the end.

A handler that decides "the reader left the end" from the report (Excord's does,
§3.7–3.8) acts on the middle one: Excord pinned its window and paged two more history
requests on every open on the web, and its tests' windows differed between hosts.

## Repro

With `bun apps/excord/fixture/server.ts --world transcript --reset-on-identify` running,
record each report in Excord's `scrolled` action (`state probe: string[] = []` and
``probe = probe.concat([`${k}@${round(gap)}`])`` before `let moved`), build the web, and:

```
bun scripts/agent.mjs web --app apps/excord --epoch 2026-10-22T18:00:00Z \
  "clock +500 real" "tap channel-general" "clock settle" "state probe"
→ ["<row 9995>@0", "<row 9990>@866", "<row 9994>@0"]
```

## Expected

A list kept at its end by `scroll-start="end"` and `scrollFollowEnd` reports positions
at its end while its content settles, as UIKit's does, or reports none until it has.
