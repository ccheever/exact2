# 2026-11-07 — native tab accessibility labels

## Rough

The icon-only compact tab contract exposed `aria-label` correctly, but Exact's
UIKit projection also used that label as `UITabBarItem.title`. The tab bar then
showed text even though the reference uses icon-only items. A direct test edit
could not hide a native button's face because its children are held by the
control host; the fixture now toggles the face through contract state instead.

## Lean in

Keeping the visible title and accessibility label separate lets the same
contract produce icon-only UIKit tabs and still name them for VoiceOver. The
native test fixture exercises the transition rather than relying on a screenshot.

## Needed

- accessibility: provided after the tab projection seam was split into visible title and spoken label.

## Checkpoints

- iOS XCTest: smooth (NavigationTabsIOSTests passed on iPhone 18 Pro Max simulator)
