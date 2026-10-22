# Scroll events carry no drag start, end or velocity, and a control has no hit slop

**Status:** Open
**Systems:** Contract scroll events, Apple host scroll delegate, hit testing
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' collapsing header hides on scroll and comes back on an upward flick. From `scroll` (x and y) and `scrollend` (no payload) the port mistook a bounce at the bottom for a flick. It attached its own target to the `UIScrollView`'s pan recognizer from a route hook to learn when a drag began and how fast it ended. Small icon buttons copied at the source's size are harder to hit than the source's, which pads them with `hitSlop`.

## Fix

Report drag start and drag end, with velocity, as typed scroll events, matching what `UIScrollViewDelegate` gives (`scrollViewWillBeginDragging`, `scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`). Let a control grow its hit area beyond its box without changing layout, as a property the web host maps to a padded hit region.
