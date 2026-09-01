# Caltrain board blanks after service ends

**Status:** Closed
**Resolution:** Caltrain boards now query with the current time, omit departed trains, and render the end-of-service message.
**Systems:** Caltrain
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1000

`apps/caltrain/app.contract:257-264` checks `isEmpty(board)` on an all-day board
and otherwise filters past departures row by row. After the final train, the
board is nonempty but every row is suppressed, so the UI is blank and the
authored “No more trains today” branch is unreachable at ordinary stations.

Derive or request the upcoming board and branch on that collection, or compute
an explicit `hasUpcoming` value. Add an app test that advances beyond the last
departure in both directions and asserts the end-of-service message.
