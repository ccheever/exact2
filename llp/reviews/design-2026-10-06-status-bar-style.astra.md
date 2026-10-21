# Design review: LLP 1105 r1, the status bar's style (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, read-only, a detached worktree at `318722c82`.
- **Method:** one brief (sha256 `e686ad8fa627de293685318eea2d180429a5be501c45449d8be895559c042fad`), blind to the other review. The author (Claude) is not a reviewer.
- **Disposition:** r2 (LLP 1105 §5).

---

NOT READY

1. **MATERIAL — D4 must update the controller that owns the status bar.** The root-only invalidation is insufficient once a presented controller captures appearance.

   | Situation | UIKit ownership in this host |
   |---|---|
   | Primary navigation stack or tabs | The standalone `Controller`: navigation and tab controllers are its children, and it does not forward `childForStatusBarStyle`. |
   | Sheet | Normally the presenter; setting `modalPresentationCapturesStatusBarAppearance = true` transfers responsibility to `ModalController`. |
   | Exact “fullscreen” or zoom route | Exact uses `.overFullScreen`, not `.fullScreen`. Set capture on the presented `ModalController`; the zoom changes its transition, not its identity. |

   Evidence: [root Controller](host/apple/Sources/ExactIOS/main.swift:128), [navigation containment](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:265), [tab containment](host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:139), [modal styles](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:23), and [zoom configuration](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:373).

   **Fix:** invalidate the active Exact appearance owner synchronously, including capturing modals. Capture is appropriate for the proposed sheet policy, but that policy differs from UIKit’s default. Root and modal-wrapper answers can cover settled navigation without overrides on every route. If introducing delegation, UIKit’s tab controller forwards automatically; navigation-controller forwarding must be explicit. Document the embedder’s ownership responsibilities. [Apple containment guidance](https://developer.apple.com/documentation/technotes/tn3105-customizing-uistatusbar-syle?changes=_3_3), [capture semantics](https://developer.apple.com/la/documentation/uikit/uiviewcontroller/modalpresentationcapturesstatusbarappearance).

2. **MATERIAL — D2’s depth ordering selects declarations behind overlays.** A deeply nested compact header beats a shallower, later fullscreen overlay. Equal-depth document order also loses to an earlier sibling with higher `z-index`. Neither is “topmost.”

   Exact already distinguishes paint order from tree order: [PaintOrder.swift:145](host/apple/Sources/ExactKit/PaintOrder.swift:145) orders by effective layer depth, and [NavigationIOS.swift:417](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:417) deliberately places navigation beneath later authored overlays. Moreover, `carrying()` sorts node IDs, not document positions ([PresenterIOS.swift:59](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:59)).

   **Fix:** retain priority between actual presentation boundaries, then use existing paint ordering between sibling branches. Descendant specificity should override an ancestor declaration, not every shallower declaration elsewhere. Classify lifted overlays by where they are presented, rather than solely by their authored route. Select the active tab and stack explicitly; native custom tab containers can retain multiple mounted branches ([NavigationTabsIOS.swift:114](host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:114)).

3. **MATERIAL — One moving global answer cannot preserve transition endpoints.** During a back swipe, both routes are mounted, but Contract navigation state changes only after a completed pop. The host already tracks the source and cancellation in [NavigationIOS.swift:690](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:690) and [NavigationIOS.swift:733](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:733).

   Dismissing a modal is worse: `closeTop()` removes its live presentation record before UIKit finishes dismissal ([ModalIOS.swift:431](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:431)). Its frozen content can remain onscreen after the corresponding nodes leave the presenter’s map. Re-resolving every controller to the underlying white screen’s dark text can therefore change the departing black lightbox too early. Waiting until completion produces the opposite problem during an interactive reveal.

   **Fix:** preserve source and destination resolutions through the transition. Use existing appearance/navigation callbacks and the transition coordinator to select and restore them on completion or cancellation. Test completed and cancelled back swipes, sheet drags and zoom dismissals, including state changes while the finger is down. “Settles when the transition ends” does not specify sufficient behavior.

4. **MATERIAL — “Shows” conflates authored visibility with native projection.** A native navigation header’s authored view is deliberately hidden while UIKit displays its replacement ([NavigationBarIOS.swift:455](host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:455)). D2 would discard a declaration on that header.

   Conversely, a context-preview row is lifted out of its hidden popover into a separate `PreviewController` ([ContextMenusIOS.swift:248](host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:248)). Native menu source trees remain hidden, while confirmations become `UIAlertController`s ([MenusIOS.swift:121](host/apple/Sources/ExactKit/IOS/MenusIOS.swift:121), [MenusIOS.swift:77](host/apple/Sources/ExactKit/IOS/MenusIOS.swift:77)). A window-membership scan consequently treats these presentations inconsistently.

   **Fix:** define eligible declaration carriers and distinguish authored hiding from projection ownership. The small version supports route/container and authored-overlay declarations, leaving native menus, previews and alerts to UIKit; a preview starts claiming as a route when committed. The keyboard is not an Exact presentation and should not enter this election. Explicitly define opacity/offscreen behavior instead of implying that `window != nil` proves visibility.

5. **MATERIAL — D3 needs separate meanings for explicit `auto` and no declaration.** UIKit’s `.default` does not mean “resolve using this arbitrary descendant’s scheme.” Exact applies subtree appearance to `NodeView`, independently of its controller ([ColorScheme.swift:13](host/apple/Sources/ExactKit/ColorScheme.swift:13)).

   **Fix:** resolve explicit `auto` from the winning node’s effective traits into `.lightContent` or `.darkContent`. Preserve the existing UIKit fallback when there is no declaration, including avoiding unconditional capture changes that would contradict “nothing changes from today.” Refresh on trait changes and mounting, not just batches and transition completion; the existing node trait callback is the natural hook ([NodeViewIOS.swift:534](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:534)).

   Also say that `auto` follows appearance, not measured background brightness. Without `viewport-fit="cover"`, the status-bar background remains the first root’s canvas; native navigation bars can supply their own material ([PresenterIOS.swift:1278](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1278), [ExactViewIOS.swift:211](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:211)). A dark subtree is not necessarily what paints underneath the bar.

6. **MATERIAL — The proposed test does not establish the same-frame requirement.** Reading `preferredStatusBarStyle` inside `apply` proves that a getter changed, not that UIKit displayed the new text with the header. UIKit documents an invalidation request and animation-block participation, not a displayed-frame guarantee. [Apple’s update contract](https://developer.apple.com/documentation/uikit/uiviewcontroller/setneedsstatusbarappearanceupdate%28%29?language=objc).

   **Fix:** resolve and invalidate after the outermost projection finishes, without another dispatch turn. Handle attachment and deferred navigation explicitly: layout can defer during `apply`, and navigation can defer during transitions ([ExactViewIOS.swift:192](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:192), [NavigationIOS.swift:349](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:349)). Keep same-frame display as an acceptance requirement and verify it with an actual screen capture; the normal agent screenshot captures the viewport hierarchy ([AgentIOS.swift:894](host/apple/Sources/ExactKit/IOS/AgentIOS.swift:894)).

   The animation block is appropriate for style changes. Define animation as belonging to the incoming winner, suppress ambient animation for `none`, and use transition timing during interactive navigation. `preferredStatusBarUpdateAnimation` controls hiding/showing, not this color change. Follow existing agent-freeze behavior and test the fade with native timing enabled. [Apple’s animation-property contract](https://developer.apple.com/documentation/uikit/uiviewcontroller/preferredstatusbarupdateanimation?changes=l_4_2).

7. **MINOR — Keep the two props, but make validation and the embedding callback precise.** A state-bound prop is the right form; `light` and `dark` meaning text color matches [Expo](https://docs.expo.dev/versions/latest/sdk/status-bar/). The hyphenated attributes lowering to string props fits the existing [viewport attributes](contract/lower/src/tags.rs:558) and [schema](kernel/tables/schema.json:370).

   “Other values are a compile error” overpromises for arbitrary runtime strings. The lowering path checks a string type and selected literals, not a finite string domain ([lib.rs:1405](contract/lower/src/lib.rs:1405), [values.rs:958](contract/lower/src/values.rs:958)).

   **Fix:** reject statically known invalid values and specify runtime invalid-value and clearing behavior. Have the embedding API expose both resolved style and animation, synchronously notify changes, and provide the current value when subscribed, as `onTitle` already does. Cut the suggestion that hiding would become another style value; it is a separate concern.

8. **MINOR — Keep D5’s no-ops, but distinguish resolution from observed application.** `layout.statusBar: light` can pass even when UIKit is still consulting another controller.

   **Fix:** identify the report as Exact’s resolved request and include its source node. UIKit integration tests should also inspect the scene’s status-bar manager; that observation still does not establish fade progress or pixel timing. Unsupported hosts should report unavailable or omit the applied observation. [UIKit’s status-bar observation](https://developer.apple.com/documentation/uikit/uistatusbarmanager/statusbarstyle?changes=_3).

   Explicitly include tvOS and Windows in the no-op policy and guard UIKit status-bar APIs for iOS. Keep `theme-color` separate. Reuse [ChromeIndex](host/apple/Sources/ExactKit/ChromeIndex.swift:1) so apps without declarations incur no whole-tree scan. Existing ordering, controller callbacks and trait hooks are sufficient; a new controller registry, visibility engine or animation subsystem would exceed this need.