# Selecting a tab and pushing onto its stack in one update shows the tab's root

**Status:** Closed
**Resolution:** Not reproduced on iOS, macOS or the web; XCTests now cover a select-and-push for a navigation-controller tab and a held tab
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

## Not reproduced (2026-10-24)

On main at 87a46cf99b, one update that selects a tab and pushes onto its already-mounted
stack shows the pushed route on iOS 27 (iPhone 18 Pro simulator), animations on:

- a navigation-controller tab: the native fixture's Second, `open(nav, "/detail")` onto Home
  (`NavigationTabsIOSTests.testATabSelectedWithAPushOntoItsStackShowsThePush`), also driven
  with `--timing platform --chrome platform` by the button and by a navigate through the
  root with `follow` as `open` (a warm URL open);
- a held tab (no bar, nothing pushed): the held-tab fixture's Titled, `open(nav, "/detail")`
  onto Plain (`HeldStackIOSTests.testATabSelectedWithAPushOntoItsHeldStackShowsThePush`);
- macOS and the web, by agent drives of the same sequence.

What the Brooks port did differently (an app-owned tab container, a launch from the
background, a sheet over the tabs) is not known; reopen with that app's sequence.
