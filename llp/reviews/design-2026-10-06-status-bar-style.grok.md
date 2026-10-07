# Design review: LLP 1105 r1, the status bar's style (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, a detached worktree at `318722c82`.
- **Method:** one brief (sha256 `e686ad8fa627de293685318eea2d180429a5be501c45449d8be895559c042fad`), blind to the other review. The author (Claude) is not a reviewer.
- **Disposition:** r2 (LLP 1105 §5).

---

READY WITH CHANGES

The steady-state mechanism is right and small: one style, stored before `setNeedsStatusBarAppearanceUpdate`, returned by the only two controllers UIKit actually asks. The resolution rule and the transition timing are not implementable as written, and the value names collide with `color-scheme`.

1. **MATERIAL.** UIKit asks the window root, then a presented controller only when that presentation covers the screen or captures the bar. It does not ask the navigation, tab, or route controllers this host installs.

`Controller` is the window root (`host/apple/Sources/ExactIOS/main.swift:237`) and does not implement `childForStatusBarStyle` or `preferredStatusBarStyle` (`:128`). The default of `childForStatusBarStyle` is nil, so children are invisible to the bar. The primary stack is a child of that root (`NavigationIOS.swift:267-281`); a tab bar is the same kind of child (`NavigationTabsIOS.swift:139-145`). A sheet or full-screen route is `present`ed from that root, or from the previous `ModalController` (`NavigationIOS.swift:478`, `ModalIOS.swift:215`, `:396`). `ModalController` is `.overFullScreen` for `fullscreen` and `.pageSheet` otherwise (`ModalIOS.swift:21-23`). A zoom is still `.overFullScreen`; `.zoom` is only `preferredTransition` (`:341-387`).

So the walk is:

- No presentation, or a page sheet that does not capture: `Controller`.
- `.overFullScreen` (full-screen and zoom): `ModalController`, from the moment `present` begins, including the frames where its view is held at `alpha` 0 (`:386`).
- A page sheet: `Controller` until `modalPresentationCapturesStatusBarAppearance` is set, then `ModalController`.
- The embedded `UINavigationController` and `UITabBarController` are reached only if some parent returns them from `childForStatusBarStyle`. Their own defaults (top controller, selected tab) never run today. `RouteController` (`NavigationIOS.swift:9`) would then answer `.default` and drop the resolution.

Returning one stored style from `Controller` and `ModalController` makes those walks agree. `modalPresentationCapturesStatusBarAppearance` does not. Once both return the same style, the flag does not change the pixels; it only changes which of the two is asked. A medium detent and an iPad page sheet leave the status bar over the presenting screen, and a large iPhone detent draws the bar over the sheet, but both are `.pageSheet`. The flag cannot express that.

The keyboard is a different window. A context-menu preview is handed to the interaction (`ContextMenusIOS.swift:248-265`), not `present`ed onto the root. An action sheet is presented from the nearest view controller (`MenusIOS.swift:547-594`) with `adaptivePresentationStyle` `.none` (`:83`) and does not take the bar. None of these should be given a style.

**Fix:** Override `preferredStatusBarStyle` on `Controller` and `ModalController` only. Do not override `childForStatusBarStyle`. Call `setNeedsStatusBarAppearanceUpdate` on the window's `rootViewController` after the stored style is updated. Leave `modalPresentationCapturesStatusBarAppearance` unset. Assert the controller the walk actually reaches: the root with no modal, and `ModalController` while an `.overFullScreen` route is up. The test in §4, which only reads the root, passes while a lightbox is showing `.default`.

2. **MATERIAL.** A state flip inside the committed screen can share the batch's frame. A transition cannot, and D4 names both timings.

Scroll-driven compact state and the header move in one batch. `setNeedsStatusBarAppearanceUpdate` in that same turn, after the style is stored, is applied on the next commit. That is the same frame the eight-frame scroll-edge lag is not. `fade` is the 0.3s cross-fade and is outside that claim. The call has to be synchronous. `coversChanged` already defers work with `DispatchQueue.main.async` (`NavigationIOS.swift:466-473`); copying that is one frame late. The animation block wraps only `setNeedsStatusBarAppearanceUpdate`, or the batch's frames animate for 0.3s too.

Transitions do not work that way. An interactive pop does not change Contract state until `didShow` invokes Back (`NavigationIOS.swift:690-716`, `:733-784`). A sheet drag does not until `presentationControllerDidDismiss` (`ModalIOS.swift:493-507`). During the gesture the outgoing route is still the committed one, while the incoming screen occupies the status-bar area: a back swipe onto a dark banner, a zoom returning to the profile, a sheet sliding off a large detent. The bar keeps the outgoing style and then snaps. D4 also says to re-read after every batch. The batch that starts a push runs in `sync` before the animation (`NavigationIOS.swift:363-370`), while `sync` itself bails out for the rest of the transition (`:349`). Both routes are in the window for that interval, so "topmost, then document order" flips the bar at the start of the push. "Settles when the transition ends" describes the other timing. Those cannot both be the rule.

**Fix:** The bar follows the committed route. A push, a completed pop, a tab change, or a presentation whose Contract state has landed updates in the batch that committed it (a push therefore changes at the start of its animation). While `changing`, `interactiveTransition`, or `modals.inTransition` is set, keep the last committed style and apply the new one on the `didShow` / dismiss batch. Say that a finger-driven dismiss snaps when the gesture commits. Tracking the finger would mean a different style per route plus the transition coordinator, which this design is right to leave out.

3. **MATERIAL.** D2's "highest presentation, then deepest, then document order" does not match what is under the bar, and the presenter already has the facts that do.

`placeOwner` inserts the navigation view among the root's children so a later sibling paints over the stack (`NavigationIOS.swift:417-436`). Paint rank puts that container at ½: under a positive `z-index`, and under a later positioned sibling (`PaintOrder.swift:171-180`). A header authored beside the route outlet, over the top of the screen, is shallower than a declaration on the banner inside the route. Depth picks the banner. The header is what the bar sits on.

A route under a settled push really has left the window. A route under a sheet has not: `.overFullScreen` and `.pageSheet` both keep the presenting views mounted (`ModalIOS.swift:335-338`). "Not in the window" excludes the push and, if the test is `window != nil`, usually an unselected tab (`NavigationTabsIOS.swift:214-224`). It does not exclude the screen behind a sheet. If the sheet declares nothing, the covered screen's declaration still wins, including over a light sheet that covers the bar. If the sheet declares a style, it wins at a medium detent and on iPad, where the bar is still over the screen behind.

Popovers and menus are `isHidden` on device while UIKit shows its own menu (`MenusIOS.swift:140-145`). They should not count. A row lifted into a preview is in a window for the duration (`ContextMenusIOS.swift:256-264`) and must not count either. `visibility: hidden` is not `isHidden` (`NodeViewIOS.swift:1208`); it still occupies a window.

The presenter can compute this without a hit-test. `modals.layers` is the presentation stack. `navigationKey`, the selected tab, and `window` say which route is committed. `style["color_scheme"]` is already on the view (`ColorScheme.swift:13-21`). `zPosition` is the paint order.

**Fix:** The scope is the topmost presentation that covers the status bar: `.overFullScreen` and zoom always, a page sheet only while its selected detent is `large`. Anything else leaves the scope on the committed route of the selected tab. Inside that scope, the winner is the showing declaration with the greatest paint rank. Showing means in the window, not `display: none`, not `visibility: hidden`, not a context-menu lift, and not an unselected tab or a route this scope has covered. No declaration in scope resolves as `auto` for that scope. Re-read when the selected detent changes, not only on a batch.

4. **MATERIAL.** `light` and `dark` mean the text color, and `color-scheme` already uses those words for the opposite thing.

`color-scheme="dark"` is a dark surface (`llp/1034-scheme-aware-colour.rfc.md:355-359`). `status-bar-style="dark"` would be dark text, for a light surface. An agent writing the dark banner in the ask puts `dark` on it and gets dark text on the banner. Attribute spellings are permanent: a renamed one is refused, with no alias (`contract/lower/src/tags.rs:12-13`). There is no CSS property to inherit. The vocabulary that does exist in this repo is `color-scheme`.

**Fix:** Use `light-content` and `dark-content` for the text color (React Native's words), plus `auto`. Map `light-content` to `UIStatusBarStyle.lightContent` and `dark-content` to `.darkContent`. In the same sentence as the attribute, say they name the text, and that a dark banner takes `light-content`.

5. **MINOR.** `auto`, the animation, rejected values, the agent, and the web are one step short of what the code will do.

D3 says `auto` and "no declaration at all" both take the scheme where the winner sits, and also that no declaration anywhere leaves today unchanged. Those are different. Today the root returns `.default` (`main.swift:128`). A forced scheme exists only on the node view (`ColorScheme.swift:13-21`), so `.default` on the controller follows the window, not a `color-scheme` subtree. Returning a snapshotted `.lightContent` for unforced `auto` also goes stale when the system appearance changes with no batch.

`status-bar-animation` is a property of the change. Two visible nodes can disagree. The schema cannot say which applies.

"Other values are a compile error" is true for literals only if `check_prop_value` grows an arm, as `focusGuide` has (`contract/lower/src/values.rs:884-886`). The ask's own binding, `status-bar-style=(compact ? "dark" : "light")`, is not a literal. A bound `"blue"` lowers.

`layout` on iOS has no chrome field (`AgentIOS.swift:258`). macOS, Linux, and the web have their own `layout`. `statusBar: light` has to be the effective text style, so a drive can see `auto` under a dark subtree. Reporting the token `auto` cannot.

The web host writes every prop it does not know as `data-<name>` (`host/web/src/element.rs:961-967`), and the JS target then requires a DOM name (`host/web-js/src/style.rs:299-334`) unless the emitter skips it, as it skips `focusGuide` (`host/web-js/src/emit.rs:1147-1149`). "Ignored" still needs those two skips. tvOS compiles this presenter (`ModalIOS.swift:19-21`) and has no status bar; the overrides are harmless there.

**Fix:** No candidate in scope returns `.default` and does not consult a covered route. `auto` on a node whose computed `color_scheme` is `dark` is `.lightContent`, `light` is `.darkContent`, and an unspecified scheme is `.default` so the system still tracks. The animation is the new winner's, and `none` when the winner omits it. Literals outside the set are `lower-attr-value`; any other runtime string logs and counts as unset. iOS `layout` gains `statusBar` as `light-content`, `dark-content`, or `default`. The other hosts omit the key. The web and JS targets `continue` past both props.

6. **MINOR.** The extra surface is the presentation flag and a second re-read, not the two props.

The prop is the right form: the header and the lightbox are state the app already has, and a command would have to be replayed across navigation and the agent's clock. `fade` is the optional half of the ask; UIKit's third animation, slide, can stay out. Hiding the bar, `theme-color`, and Android are correctly left (LLP 1069 §10 is the line that names them, at `llp/1069-expo-sdk-survey.research.md:399-400`, and §9 of LLP 1008 is the root background behind the bar, not a text style). The sample host is a separate root with two panes (`ExactHostIOS/main.swift:97-115`) and also does not forward `childForStatusBarStyle`. One callback per `ExactView`, matching `onCanvasColor` (`ExactViewIOS.swift:38-58`), cannot name which pane owns the single bar. That host stays on the embedder path.

**Fix:** Keep the two props, the two overrides, and one re-read of the committed scope. Drop the captures flag and the second re-read at transition end. Add the detent-change re-read from finding 3, since a drag between `medium` and `large` is not a batch. Leave `ExactHostIOS` unwired.

