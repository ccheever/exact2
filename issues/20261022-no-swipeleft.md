# There is `swiperight` but no `swipeleft`

**Status:** Open (an ask)
**Systems:** gestures (runner, every host)
**Severity:** P3
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

Discord's iOS app replies to a message by swiping it left. Exact has `swiperight`;
Excord builds the left swipe from `pan` and `panrelease`, which runs one action per
drag frame where the platform's gesture would run one at its end.
