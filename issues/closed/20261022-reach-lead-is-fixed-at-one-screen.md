# `reachstart` and `reachend` fire one screen ahead, which a fling outruns

**Status:** Closed
**Resolution:** `list reach-margin` sets the lead, a length or a share of the port on each side (LLP 1145 D4), and a fling toward an end leads its band as far as the collection window leads (LLP 1027.004 D3); a fling that outruns the loaded rows still stalls at their edge, which needs an estimated extent past them
**Systems:** the virtualized `list` (runner, every host)
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): an ask

## Symptom

A fast fling up a transcript reaches the first loaded row before `reachstart`'s page
of older history arrives, so the scroll stops at the edge and then jumps on. Excord
pages from its scroll handler instead (within 50 rows of either end, 100 more).

## Ask

A lead the author sets, as `IntersectionObserver`'s `rootMargin` does for the web
(`intersect-margin` is Exact's name for that already): e.g. `reach-margin="200%"`.
