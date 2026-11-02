# A page of workouts aggregates every session in the window

**Status:** Closed
**Systems:** Android Health Connect (`packages/health/android/Health.kt`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** `healthWorkouts` asks for a page, at most 500 rows and 50 by default. The call reads every exercise session in the time window and, when distance or active energy is granted, aggregates each one, then slices to the page. The next page repeats the scan. A year of history with a few hundred sessions does not return until that work finishes.

**Cause.** `sessions` follows `pageToken` until it is null, with a page size of 1000, and maps every record through `workout`. `workout` issues one `aggregate` per session when either sum is granted. `workouts` applies `limit` and `after` only after that list comes back.

**Reproduction.** Call `healthWorkouts` with `start` a year ago, `end` now, the default limit, and distance granted, on an account with a few hundred sessions. The answer waits on one read per thousand sessions and one aggregate per session. Passing `after` from that answer does the same work again.

**Fix.** Walk the ascending pages only until the cursor is passed and `limit` rows are collected. Aggregate those rows only. Resume the next call from the last page token or the last start and id.

**Acceptance.** A window of 400 sessions and a limit of 50 issues aggregates for 50 sessions. The following page does not reread the first page's sessions.

**Resolution.** Read a requested workout page and its timestamp boundary before aggregating. Resume from the cursor timestamp, retain deterministic id ordering at tied timestamps, and reject stale cursors.

**Verification after the fix.** Compiled the real Health Connect package against SDK 1.1.0. A Kotlin harness of the paging function checked 400 sessions: each 50-row page performs 50 workout aggregates, the second starts at its cursor timestamp, and a stale cursor performs none. No device Health Connect store was queried.
