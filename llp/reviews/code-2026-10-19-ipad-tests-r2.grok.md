# Code review: the iPad-only Swift tests and the PressFocus sheet flake, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8e5ae24e67`.
- **Method:** one brief (sha256 `1f0abe24bfef606f8face63f0c88507c8319b7c4d898c7cb4b553b57e34b3faf`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. The round-1 blocker is fixed, and the two product choices the commit leaves in place match the code.

## Findings

**nit** — The pushed-route test never reads the floating bar over Chat. `host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:353`

Not requiring the bar to leave on iPad is the right call: `hidesBottomBarWhenPushed` does not hide that bar, and whether Exact should call `setTabBarHidden` there is the QUEUE item. Home, Detail, and the return trip do go through `barShows`. Over Chat, when `barInsets` is false, there is no assertion at all, so a bar that disappears over Chat and comes back still passes, and so does one that never moves. The comment says the test records what each bar does today.

Concrete fix: in the `else`, assert `barShows(tabs)` over Chat (the bar stays). Keep the hide-and-inset wait inside `if insets`.

## Round 1

The touch give-back is in the right place. `NavigationDelegateProxy` snapshots `isUserInteractionEnabled`, runs `layoutIfNeeded`, and sets the bar back to true only on a true-to-false edge, before `host` and the app `didShow`:

```349:354:host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift
            let bar = nav.navigationBar, touches = bar.isUserInteractionEnabled
            nav.view.layoutIfNeeded()
            if touches, !bar.isUserInteractionEnabled { bar.isUserInteractionEnabled = true }
        }
        host?.navigationController(nav, didShow: controller, animated: animated)
        app?.navigationController?(nav, didShow: controller, animated: animated)
```

An app or host disable in `didShow` stays off. A bar that was already off before this layout (willShow, an interactive pop) is left alone. There is no later turn and no `transitionCoordinator` check, so a `pendingSync` push started inside `didShow` can no longer drop the restore. Enabling during the item animation is the tradeoff the commit names; the Chat test still hit-tests Back and the drawn title, not only the flag.

Inspect’s floating-tab claims are narrowed the way both reviews asked. A section-0 cell maps to that tab; anything else in the list, or a view that contains the list and not the selected tab, is the bar message. A hatch or other sibling on `tabs.view` fails `list.isDescendant(of: hit)` and falls through to the normal walk. `place(of:)` returns that cell’s window rect on each call, so reuse and scroll update the press box and the outline. The tab test checks the rect.

`syncScrollsToTop` turns the floating list off, and the inspect test counts every `scrollsToTop` scroller again (exactly one before, none while inspecting, the same one after) plus an explicit `scrollsToTop == false` on the list.

PressFocus’s presented-sheet test takes the clock before the tap, checks that the textarea is absent and the handoff is live, then seeks 1.5s so the one-second answer fires. The dismissal test is unchanged and still sound. Face symbols, `isTabBarHidden`, and the 40-row fixture are unchanged from the versions round 1 accepted. The new comment’s margin (40 rows against a 1,376-point 13-inch height) matches that earlier nit; row height is about 44 points, so the block is taller than that screen.

`AgentRevealIOS.bars(over:)` still appends only `tabBar`, then drops it because `tabBar.window` is nil, so reveal does not treat the floating pill as a cover. That was already true before these commits; Grok left it, and this commit does not claim it. The list’s own bounds are the pill (content taps beside it still resolve), so appending `floatingTabList` there would match the comment in that file. Not a regression of this diff.

## `floatingTabList` and `scrollsToTop`

The lookup is fail-closed: `tabBar` out of the window, the selected tab’s view not walked, first section’s item count equal to `viewControllers.count`, every later section empty. That matches the iPadOS 27 probe (tabs in section 0, an empty second section) and is what the passing iPad tap, outline, and status-bar tests identify.

`scrollsToTop = false` does not turn off dragging. A sidebar or an expanded tab list that matches this shape still pans; it only stops competing for the status-bar tap, which is the one-scroller rule in `ScrollsToTopIOS.swift`. A list that does not match (a non-empty extra section) is not touched, so its scrolling is unchanged and the status-bar gesture can see two scrollers until something else clears the flag. Selected-tab content is not a candidate: the walk does not enter that view, and a known route scroller that was cleared is set back when it is the chosen target.
