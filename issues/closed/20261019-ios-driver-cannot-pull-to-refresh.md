# The iOS driver cannot drive a pull to refresh

**Status:** Closed
**Resolution:** Measured with real touches (--touch drag): 200 points past the top refreshes, 180 does not. The driver's own drag now starts the refresh control at that pull (AgentPanIOS scrollDrag, reply refreshed=true); verified on Shelf's discover list and by NavigationCollapseIOSTests.testTheAgentsDragPullsToRefreshAsAFingerDoes.
**Systems:** scripts/agent.mjs, Apple host agent (AgentIOS.swift), iOS pull to refresh (`refresh` event, `UIRefreshControl`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from LLP 1054 A4 (the Bluesky port)
**Date:** 2026-10-19
**Related:** LLP 1054 A4 and X2; LLP 1035.003 (reproducible native gestures); LLP 1080.000 (real touches on iOS); QUEUE.md "iOS after wave 2's keyboard and bars" item (1) and "Pull to refresh beneath the Bluesky clone's overlaid header"

## What happens

The Bluesky port added pull to refresh on iOS (`refresh` event, `refreshing`
prop, `UIRefreshControl`), but could verify it only by building it and reading
the code. A pull, like any held phase, came back `unsupported` from the iOS
driver.

The driver has changed since, but not enough:

- A held contact across requests is still unsupported on iOS: a `tap … down`
  phase returns `"delivery": "unsupported", "reason": "no held contact across
  requests on iOS (LLP 1080.000 P3)"` (`AgentIOS.swift`).
- `tap <scroller> drag 0 <dy>` exists. But under the driver's own drag (no
  `--touch platform`), a scroller cannot be pulled past its top (QUEUE.md,
  2026-10-11). So the refresh control never triggers.
- `--touch platform` sends a real touch, which can pull past the top. No one
  has recorded driving a refresh that way. QUEUE.md lists "pull-to-refresh on
  a grouped list by a finger" as not tried, and the Bluesky clone's pull
  beneath its overlaid header still needs a manual check.

## What would fix it

First, find out whether `--touch platform` with a downward `drag` on a
scroller at its top fires `refresh`. Then do one of two things. If it does,
document it and add an XCTest or smoke step. If it does not, make the driver's
own drag let UIKit overscroll the top and run the refresh control's
`valueChanged`, as a finger does. The agent should then report the `refresh`
it fired.
