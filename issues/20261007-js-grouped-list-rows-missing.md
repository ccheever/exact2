# JS target: a grouped list's authored rows are missing from the observed view

**Status:** Open
**Systems:** host/web-js (grouped list, `list appearance="auto"`), semantics/difftest (JS driver)
**Author:** Claude (Opus 5.5), from the semantics event-timing lane
**Date:** 2026-10-07

`difftest explore contract/corpus --js` diverges on `contract/corpus/grouped-touch.contract`: the runner, and the Lean semantics, observe the grouped list's authored rows (`toggle-row`, `toggle`, `info-row`, `info`, `open`) on every step; the JS target's observation omits them, and a tap aimed at `toggle-row` reaches no element (`NoHandler` on the runner, nothing on the JS target). Kept case: the mini's `~/exact2-verify/wt-semantics/target/difftest/failures/18659-js-45.*`.

It is not the event-timing cause (20261007-semantics-event-before-due-work, closed): it diverged before that change (the async lane's 931f9fb7 run lists it), Lean agrees with the runner, and the difference is in the view lines from boot on. Likely the JS target's grouped list keeps its authored rows out of the DOM the observation reads (LLP 1084 D5: iOS draws its own cells over the authored rows, which the web and the other hosts draw), or the recent "list rows skip" change (e91a0afa8, 2d7fb57e0) reaching the web build.

To do: compare the JS target's DOM for a grouped list with the runner's view; if the web build should draw the rows (it should: only iOS replaces them), restore them; otherwise teach the JS driver where they are.
