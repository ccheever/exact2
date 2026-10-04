# Code review: declarative nav basics, LLP 1075.003 §9.10 (151c5c0a5..b7d195a0d), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, one fresh session with `--cwd` a detached worktree at `b7d195a0d`. `--always-approve` because plan mode cancels shell reads; the brief instructs read-only, and the worktree was clean afterwards.
- **Method:** the shared brief (sha256 `31fe0e9fdce293c1eb9e7f952e0440d1ea32f3ea49688a7ef5a8f1f63a03d8f9`); round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** fixed in `4cd44c10f` except #3, argued:
  1. *Latched `scrollCollapsed` after a barless push.* Fixed as astra #2: sampled only while the bar is as the route wants it, an inline title's slack always 0, and cleared with `scrollOrigin`.
  2. *Only the selected route flagged.* Fixed: routes pushed in the same batch as the route the root names (a cold launch's `[Home, Chat, Photo]`) take the tablist's visibility too; routes already flagged keep theirs.
  3. *Avatar size.* Argued as astra #5 (layout rows do not reach the host); §9.10 now says 36 points, and the fixture's avatar is `border-radius="50%"`.
  4. *Tests absent.* Fixed as astra #6.
  5. *Fixed 44-point height.* Fixed: the fitted height, at least 44.

---

I'll review this commit read-only against LLP 1075.003 §9.10 and the binding rules, starting with the spec and the diff.The review stays read-only. Next I’ll read §9.10 and the commit diff so findings are grounded in the spec and the code.The spec is in place. I’ll read the new navigation code and the kernel cover path next, then check them against the tests.The scroll-range bug is real: hiding the bar for a barless route latches a minimum inset that later shortens the scroller you return to. The other gaps are the tab-bar flag on a multi-route stack, the avatar’s fixed size, and tests the plan says are here but are not.

1. **Must-fix** — A barless push latches `scrollCollapsed`, so the scroller you return to cannot reach its end. `host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:640`

`collapse` now puts an inline `navigationScroll` under the bar (`:503`), and `scrollTop` still uses Stage 3’s slack: `scrollOrigin` is the largest top inset seen, `scrollCollapsed` is the smallest (`NodeViewIOS.swift:930`). The new cover guard freezes edge covers while the bar is not as that route wants it, but the inset sampler just above it still runs for every controller in the stack. Push Chat (inline, scroller directly under the header) onto a stack that has a bar, then push Photo. `willShow` hides the bar, Chat’s safe area shrinks while Chat is still in the transition, and `scrollCollapsed` latches that smaller inset. It never moves back up. After the pop, slack is about one navigation-bar height, and an authored scroll-to-end — including a transcript that follows the end — stops short of the real end. The same latch hits a Stage 3 large title: the hidden-bar inset is smaller than the collapsed-title inset, so the end mapping from Stage 3 is wrong after any barless push.

Reset `scrollCollapsed` wherever `collapse` clears `scrollOrigin`. Sample both insets only when that route’s bar is in the state it wants and no transition is running. For an inline title, keep `scrollCollapsed == scrollOrigin` so slack stays 0.

2. **Should-fix** — Only the selected pushed route gets `hidesBottomBarWhenPushed`. `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:188`

`followTablist` writes the flag on `routes.dropFirst().first` matching the root’s `navigationKey`. UIKit reads the flag at push time and, on pop, shows the tab bar again for a revealed controller whose flag is still false. A cold launch or one batch that installs `[Home, Chat, Photo]` while the tablist is `display: none` marks only Photo. Popping Photo reveals Chat with the tab bar over the screen, which is the case this rule is for. One-at-a-time pushes work only because each route was the selected one on an earlier turn.

Before `setViewControllers`, set the flag on every non-root route whose stored `tablistHidden` differs. `prepareRoutes` still runs the route hook after `followTablist`, so a hook’s value on that first turn is what UIKit reads.

3. **Should-fix** — The title avatar is always the 36-point bar-item face, not the box’s width. `host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:124`

§9.10 says the avatar is §9.6’s drawn face at the box’s authored width, clamped to 20–40 points. `BadgeFace.size` is 36, and corners are computed in a 36×36 rect (`:153`). The spec’s own example is 40×40 with radius 20; the fixture’s Chat avatar is 32×32 with radius 16, which is a circle. Drawn at 36 with an unscaled radius of 16, `BorderPaint.reduced` does not turn it into a circle (32 < 36), so the bar shows a larger rounded square. `HeaderTitleView` then uses that image’s intrinsic size (`NavigationTitleIOS.swift:112`).

Draw into `clamp(authored width, 20, 40)` and size the image view to that side.

4. **Should-fix** — The XCTests §9.10 says landed are not in the commit. `llp/1075.003-native-platform-control-merged.plan.md:1259`

`NavigationBasicsIOSTests` is not in `host/apple/tests` or `host/apple/Tests`. The kernel cases in `kernel/tests/it/cover.rs` are present and match `header_inset` for the two authorings they name (header pads, or the route pads; a top cover of 0 adds nothing). Nothing in the tree locks the drawn title and its press, a hook’s title view left in place, Chat hiding the tab bar, Photo’s `willShow` / cancel, or `scrollTop` 80 — and a test that only assigns 80 would still miss finding 1, which shows up at the end of the scroll range after a Photo round-trip.

5. **Nit** — The drawn title’s height is fixed at 44 while its type tracks Dynamic Type. `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:105`

`intrinsicContentSize` returns the stack’s fitted width and a hardcoded height of 44. Headline plus footnote at the default size fits; with larger text the labels set `adjustsFontForContentSizeCategory` and draw outside that 44-point box, and the bar clips them. Use the fitted height (the bar already grows for its own subtitle on iOS 26).

Verdict: LAND WITH FIXES
