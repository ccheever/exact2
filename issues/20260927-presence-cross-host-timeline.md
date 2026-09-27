# Hold presence motion to one recorded timeline across hosts

**Status:** Open
**Systems:** `scripts/smoke.mjs` or the motion parity corpus, web/Apple/Linux
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063 (the 2026-09-27 ruling: no browser oracle, parity by a recorded timeline)

With the web running `layout-transition` and `exit-animation` by FLIP, Chrome can't serve as the oracle. Charlie ruled that parity is one recorded timeline compared across hosts.

**Do:** drive the same fixture on each host with the agent clock (`clock +N` steps through a move, a resize-during-move, a retarget, and an exit), record each moving node's presented box and opacity per step (`layout`/`state`), and compare hosts within a tolerance. Extend the existing motion parity cases (`host/web/src/parity.rs`) rather than adding a new tool.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
