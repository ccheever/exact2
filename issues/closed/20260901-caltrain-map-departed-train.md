# Caltrain map follows a departed train

**Status:** Closed
**Resolution:** The Caltrain GPU map now selects the first departure at or after the current time.
**Systems:** Caltrain, GPU
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1009

The Caltrain GPU map selects `board.first()` and clamps negative remaining time
to zero (`apps/caltrain/gpu/src/lib.rs:114-120`). The app supplies the whole-day
board. At the seeded 11:10 clock, Mountain View's first northbound departure is
hours in the past, so the train dot is pinned at completed progress instead of
tracking the next upcoming train.

Select the first departure whose timestamp is at or after `now` (with a stated
between-trains/end-of-service rule), and test a realistic board containing
both past and future departures. The current GPU test supplies only one future
departure and cannot catch the production case.
