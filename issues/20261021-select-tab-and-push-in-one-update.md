# Selecting a tab and pushing onto its stack in one update shows the tab's root

**Status:** Open
**Systems:** Apple host navigation (NavigationHost tabs and stacks), runner router
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

A widget URL opened while the app is running (a warm open) targets a feature inside the Profile tab, which the user visited earlier. The tab is selected and the navigation state holds the pushed route, but the screen shows Profile's root. A cold open of the same URL shows the feature.

## Cause

Not isolated. When one update both selects a tab and pushes onto that tab's already-mounted stack, the native stack does not display the push. The port works around it by selecting the tab, then pushing 50 ms later.

## Fix

Apply a tab selection and a push to the selected tab's stack as one native transition, whether or not the stack was mounted before, and cover warm cross-tab URL opens in the iOS tests.
