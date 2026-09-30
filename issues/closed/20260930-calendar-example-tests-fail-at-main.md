# The calendar example's 12 XCTest-free Rust tests fail at origin/main

**Status:** Closed
**Resolution:** Updated stale tests to drive the sticker calendar buttons, single-date todos, current seed event spans and scroll-relative landing geometry; cargo test -p calendar-apple passes all 32 tests (12 failed before).
**Systems:** examples/ios/calendar
**Severity:** P2
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** examples/ios/calendar/apple/tests/calendar.rs, examples/ios/calendar/apple/tests/calendar/features.rs

`cargo test -p calendar-apple` fails 12 of 32 tests at `origin/main` (checked at 9fdf7e564-era main with and without LLP 1074: the same 12 fail either way, so it is not LLP 1074). Shapes seen: `NoHandler { view: 804, event: "input" }` (`calendar.rs:199`), `find_by_test_id("end-date")` empty (`features.rs:417`), and count assertions off by one (`calendar.rs:753`, 11 vs 12). The tests are in the workspace, so every `cargo test --workspace` on a Mac shows them red; the async lane's verify names them on every landing.
