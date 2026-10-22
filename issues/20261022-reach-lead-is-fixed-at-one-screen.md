# `reachstart` and `reachend` fire one screen ahead, which a fling outruns

**Status:** Open (an ask)
**Systems:** the virtualized `list` (runner, every host)
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

A fast fling up a transcript reaches the first loaded row before `reachstart`'s page
of older history arrives, so the scroll stops at the edge and then jumps on. Excord
pages from its scroll handler instead (within 50 rows of either end, 100 more).

## Ask

A lead the author sets, as `IntersectionObserver`'s `rootMargin` does for the web
(`intersect-margin` is Exact's name for that already): e.g. `reach-margin="200%"`.
