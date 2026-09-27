# LLP 1057: Gestures across platforms, and how complete exact2's should be

**Type:** Research
**Status:** Draft
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-26
**Systems:** Contract (handler attributes, `touch-action`), Plan (`EventKind`), Runner (event dispatch), Motion (holds, springs, velocity), Web host (`glue.js`, `input-glue.js`, `motion-glue.js`, `collection-glue.js`), Apple host (UIKit/AppKit recognizers), Linux host (evdev contact), Agent API (forms of `tap`)
**Related:** LLP 1002 D4/D5/D6 (the platform recognizes; the engine follows; the arena deleted), LLP 1003, LLP 1010 (scrolling), LLP 1012 (agent API), LLP 1035.001 (native interaction ownership), LLP 1035.003 (reproducible native gestures), LLP 1041 §8.5/§8.7/§8.13 (the four-interaction campaign), LLP 1043.000 D8 (`pan` commits state), LLP 1047 stage 3 (motion and drag linked by use), LLP 1050 (fill and fling), LLP 1054 X2 (pull to refresh), LLP 0559 F1 (the bug annuity; research), `rules/NOT-DOING.md` §Motion, §Agent API

## Summary

Charlie asked whether exact2's gestures are as complete as iOS's. **No, and
by design they are not trying to be.** exact2 recognizes about a third of what
UIKit offers, and its authors can arbitrate nothing. Arbitration is the
platform's, plus a few rules each host hard-codes.

What exists is good at what it covers:

- tap, double tap and context menu;
- hover enter and leave, and keys;
- one-finger `pan` deltas;
- one-direction `swiperight` with a caught, reversible, velocity-carrying
  spring;
- three purpose-built drag bindings (sheet height, photo transform, list
  reorder);
- pull to refresh on iOS;
- native scrolling everywhere, where scroll always wins.

What's missing compared with iOS:

- pinch and rotate;
- any multi-touch outside the GPU canvas;
- long press as an event;
- pan phases and velocity;
- authorable relationships (simultaneous, require-to-fail);
- authored edge pans;
- pointer position and `cursor`;
- stylus input;
- trackpad pinch, rotate and swipe;
- accessibility alternatives for path gestures.

The platforms split along one axis. Some decide ownership **up front and
declaratively**: the web's `touch-action`. Others **negotiate at runtime**:
UIKit's recognizer graph, Flutter's arena, Android's intercept, React
Native's responder system. The declarative model is the only one that works
the same on four hosts with no per-frame app code. It is also the web's, so
it is the one exact2 should keep.

**Recommendation:**

- Close the capability gap, not the architecture gap. Add a small closed set
  of recognizers as Contract events, each mapped to the host's native
  recognizer.
- Extend `touch-action` to the CSS values exact2 lacks.
- Write one fixed, host-implemented precedence table instead of authorable
  composition.
- Keep follow-and-release holds for presentation.
- Phase 1 is pinch on the existing transform binding. Its consumer is already
  admitted (NOT-DOING.md:64-71). The generic arena, authorable composition,
  per-frame callbacks and owned scroll physics all stay refused. §10.7 names the
  trigger that would reopen them.

## 1. What exact2 has

### 1.1 Contract: the handler attributes

`HANDLERS` has 37 names (contract/analyze/src/lib.rs:381-419). EventKind has
the same set in plan order (plan/tables/format.json:551-589). Lowering maps
each attribute to `AttrTarget::Handler` (contract/lower/src/tags.rs:246-268).
The input handlers, with the arguments each one adds to its action
(`handler_payload`, lib.rs:424-432; `handler_arity`, lib.rs:437-451):

| attribute | payload | meaning | web source |
|---|---|---|---|
| `press` | none | click / tap | `click` (host/web/glue.js:477-480) |
| `dblclick` | none | double click / double tap | `dblclick` (glue.js:492-498) |
| `contextmenu` | none | secondary click or long press | `contextmenu` (glue.js:492-498) |
| `hover` | `bool` | pointerenter/leave | glue.js:511-514 |
| `key` | `string` (`e.key`) | keydown while focused | glue.js:519-521 |
| `swiperight` | none | recognized right swipe, committed past 64 px | `motion.attachSwipe` (glue.js:486-487) |
| `pan` | `dx, dy` | coalesced recognized deltas, no phases, no velocity | `inputHandlers.pan` (glue.js:481-483) |
| `scroll` | `left, top` | scroll position | `scroll` (glue.js:484-485) |
| `refresh` | none (0 args) | pull to refresh | none on web |
| `heightrelease` | `height, velocity` | sheet handle released | `motion.attachHeightDrag` (glue.js:488-489) |
| `transformgeometry` | `bw, bh, pw, ph` | photo binding geometry | motion-glue.js:294-305 |
| `transformrelease` | `x, y, scale, vx, vy, vscale` | photo released | `motion.attachTransformDrag` (glue.js:490-491) |
| `reorderdrop` | `item, before` | list reorder dropped | `arrangeController` (motion-glue.js:648-836) |

The drag bindings are props, not handlers: `reorderFor`, `transformDragFor`
and `heightDragFor` (tags.rs:269-271). The one declarative arbitration row is
`touch-action` (tags.rs:447). Its enum is `auto | none | pan-x | pan-y |`
the four single directions, four two-value combinations, and `manipulation`
(kernel/tables/schema.json:986-1000). **`pinch-zoom` is absent**, and there
is no `cursor`, `pointer-events` or `user-select` row in the schema.

### 1.2 Runner

Hosts deliver `Event` values (runner/src/runner/event.rs:122-216):

- `Pan(f64, f64)` is "incremental recognized pan displacement … the action
  commits layout state, never a hold" (event.rs:172-175, LLP 1043.000 D8).
- `TransformRelease` carries `vscale`, but the doc comment says "primary pan
  supplies zero" (event.rs:201-213).
- Payloads are validated before any clock movement (event.rs:289-352,
  `invalid_payload` :294-332).

`dispatch` finds the node's handler for the kind and evaluates curried
arguments in the instance scope. It appends the payload and runs the action
as one transaction (event.rs:675-826). The runner has no notion of
recognition, phase, pointer identity or arbitration. It receives recognized
facts only, which is the LLP 1002 D4 boundary.

### 1.3 Motion: holds, springs, velocity

LLP 1002 D4 is the model: "the platform recognizes; the engine follows."

- `begin_hold` / `update_hold` / `end_hold(Release { velocity })` take over
  one property's presentation. On release it springs back to the newest
  authored target with the release velocity (llp/1002:102-133;
  motion/src/engine/hold.rs:81, :192, :256, :279).
- `begin_transform_hold` pairs Translate and Scale atomically for the photo
  consumer. It "does not add physical pan/pinch recognition"
  (llp/1002:135-147; hold.rs:103).
- `VelocityTracker` is a recency-weighted least-squares estimator for hosts
  whose platform supplies no velocity (motion/src/velocity.rs:1-8).
- On the web, motion is linked only when the plan uses a spring, a swipe or
  a drag handler, or a drag prop (llp/1047:564-577). `motion-glue.js` is an
  after-paint piece, and input readiness waits for it "so no gesture lands
  before its handler" (llp/1047:799-812).

### 1.4 Hosts

**Web.** Recognition is JavaScript over Pointer Events on the main thread:

- `pan`: pointer capture, a 4 px slop, and one dispatch per animation frame
  (host/web/input-glue.js:51-72, slop :59).
- `swiperight`: 8 px slop, axis-dominance rejection, a 64 px rubber-band knee
  at 0.2 resistance, commit at ≥ 64, and click suppression
  (motion-glue.js:573-641).
- Transform drag handles the primary pointer only (motion-glue.js:402-499,
  `isPrimary` :434).
- Release velocity comes from endpoints over ≤ 80 ms / 8 samples
  (motion-glue.js:482, :578). That is a third estimator, not
  `VelocityTracker`.
- A node with a drag action gets `touch-action: none` forced onto it
  (glue.js:373, :388, :567, :581).
- Collections hold one interaction slot per pointer, with a transferable
  lease for reorder (collection-glue.js:255-316).
- Only the GPU canvas sees raw multi-pointer input with `pointerType`
  (gpu-glue.js:448).

**iOS (UIKit).** Real recognizers, with the arbitration written in delegate
code:

- `press` is `touchesBegan/Ended` on `NodeView`, not a tap recognizer. It
  relies on the scroll view's `canCancelContentTouches`, so "scroll always
  wins" (NodeViewIOS.swift:1371-1409).
- `contextmenu` is a `UILongPressGestureRecognizer` (default duration), not
  `UIContextMenuInteraction`. `dblclick` is a two-tap `UITapGestureRecognizer`
  (NodeViewIOS.swift:231-263).
- `hover` is `UIHoverGestureRecognizer`, which never delays or cancels
  touches (NodeViewIOS.swift:128-136, :438-448).
- `swiperight` is a one-touch `UIPanGestureRecognizer`, admitted by
  `gestureRecognizerShouldBegin`. The contact must start ≥ 20 pt from the
  edge, be horizontally dominant, and not be allowed by `touch-action`
  (NodeViewIOS.swift:163-196).
- `pan` is `ContactLayoutPan`, one touch, keeping the original contact so the
  slop isn't lost (LayoutPan.swift:68-110).
- Reorder is a pan plus a long press (ReorderIOS.swift:9-34). Height and
  transform are pans (HeightDragIOS.swift:8-20, TransformDragIOS.swift:17-26).
- `ScrollView.gestureRecognizerShouldBegin` intersects `touch-action` from
  the hit node up to the scroller, as CSS does (NodeViewIOS.swift:41-58).
  `allowsTouchPan` is the translation (:152-160).
- The system back swipe is UIKit's `interactivePopGestureRecognizer` (and
  `interactiveContentPopGestureRecognizer` on iOS 26). Its delegate refuses
  when a `swiperight` node is under the start point
  (NavigationIOS.swift:137-138, :347-371).
- Swipe actions project the row into a one-row `UITableView` so UIKit owns
  that gesture (SwipeActionsIOS.swift:1-12).
- `refresh` is `UIRefreshControl` (NodeViewIOS.swift:1209-1231). Web and
  AppKit have no equivalent (llp/1054:50, :188).
- Nothing uses `UIPinchGestureRecognizer`, `UIRotationGestureRecognizer`,
  `UIContextMenuInteraction`, `UIPointerInteraction`, drag-and-drop
  interactions or Pencil APIs (no match under `host/apple/Sources`).
- The only VoiceOver entry point is `accessibilityActivate`
  (NodeViewIOS.swift:1441). There are no `accessibilityCustomActions`.

**macOS (AppKit).** No `NSGestureRecognizer`s:

- `mouseDown` offers the contact to a fixed chain: layout pan, height,
  reorder, transform, swipe (NodeViewMac.swift:1360-1368). `mouseDragged`
  and `mouseUp` walk it in a slightly different order (:1403-1413,
  :1427-1431).
- `dblclick` is `clickCount == 2` (:1433-1443).
- `contextmenu` is `rightMouseUp` (:1415-1419). Control-click appears not to
  fire it. I didn't verify that.
- Swipe recognizes a dominant rightward mouse drag with a 4 pt slop
  (MouseSwipeMac.swift:1-30, :61).
- Scroll chaining overrides AppKit to match CSS `overscroll-behavior: auto`
  (NodeViewMac.swift:109-167; llp/1010:125-146). That is a precedent: exact2
  already owns an arbitration rule where the platform disagrees with the web,
  and implements it with the platform's mechanism.
- No magnify, rotate or swipe event handlers, and no pressure.

**Linux.** Raw evdev with one primary contact:

- "no pointer acceleration, touchpad gestures, or hotplug"
  (host/linux/src/input.rs:1-4). Multitouch slots aren't tracked
  (input.rs:108-111).
- `pointer_down` picks one candidate by a fixed priority: arrange, pan,
  transform, height, swipe (presenter/contact.rs:236-241). A 4 px slop and
  axis dominance then begin the hold (contact.rs:259-340).
- Velocity comes from `VelocityTracker` (contact.rs:4, :458).
- Photo transform is "Primary pan + authored zoom controls; no pinch or
  wheel-zoom inference" (presenter/transform.rs:1).
- `hover` and `key` dispatch (presenter/events.rs:41, :64-101). There is no
  `contextmenu` or `dblclick` path.

**Swipe constants, three times.** The 64-point knee and 0.2 resistance
appear in JS (motion-glue.js:575-577), Swift (GestureHold.swift:18-36) and
Rust (presenter/swipe.rs:8-22).

### 1.5 The agent

A new input is a form of `tap` (NOT-DOING.md:295-302; llp/1012:314-317).
LLP 1035.003 added contact phases, `down / move / hold / up / cancel`,
with one contact per session. It explicitly leaves out "a second contact,
multitouch and recording" (llp/1035.003, D1). Every reply names its
delivery: `platform`, `recognized`, `activation` or `unsupported`
(llp/1012:224):

- web: real CDP input.
- macOS: real `NSEvent`s.
- iOS: `tap` is a hit-test plus activation, because UIKit has no public
  touch synthesis (AgentIOS.swift:1-12). `contextmenu`, `dblclick` and
  `hover` are injected as already recognized.
- iOS and Linux answer `unsupported` to phases.

### 1.6 What the rules say

- NOT-DOING.md:258-261 refuses "A gesture arena, claims, leases,
  compositions, or an interactive-navigation model. Recognition,
  hit-testing, and scroll-vs-pan arbitration are the platform's
  (`touch-action`, `UIGestureRecognizer`) … Scroll always wins."
- NOT-DOING.md:64-71 expanded the campaign to "continuously interactive
  Messages, photo zoom, virtualized reorder and a draggable sheet". It kept
  "a generic gesture arena" out and admitted "the bounded geometry/motion
  work those consumers need".
- NOT-DOING.md:275-276 refuses decay drivers: "A spring carries release
  velocity."
- NOT-DOING.md:291 says `runOnJS` and runtime graph admission never existed.
- LLP 1002 D5 deleted 11K lines of exact1's Flutter-style arena
  (llp/1002:55-60, :189-193). LLP 1035.001 §6 again refuses "an Exact
  gesture arena … per-frame application callbacks".

## 2. iOS: UIKit and SwiftUI

**UIGestureRecognizer state machine.** Every recognizer starts in
`.possible`:

- A **discrete** recognizer (tap, swipe) moves to `.recognized` (the same
  value as `.ended`) or to `.failed`.
- A **continuous** recognizer (pan, pinch, rotation, long press, hover)
  moves `.began → .changed* → .ended | .cancelled`, or `.failed` before it
  begins.
- The window hit-tests a touch. It then delivers the touch to every
  recognizer attached to the hit view and its ancestors *before* the view's
  own `touches*` methods.

**Exclusivity and relationships.** By default, a recognizer that begins
forces every other still-possible recognizer that it `canPrevent` into
`.failed`. The escape hatches:

- `gestureRecognizer(_:shouldRecognizeSimultaneouslyWith:)` lets both run.
  This is how pinch and pan, or pinch and rotate, combine on one photo.
- `require(toFail:)` makes one recognizer wait for another to fail. The
  classic case is a single tap waiting on a double tap, which delays the
  single tap by the double-tap interval.
- The delegate also has `shouldRequireFailureOf` /
  `shouldBeRequiredToFailBy` for cross-hierarchy wiring, `shouldReceive
  touch/press/event` for filtering, and `gestureRecognizerShouldBegin` as the
  last veto.
- Subclasses override `canPrevent` / `canBePrevented`. exact2 does this in
  `CollectionContact` and SwipeActions' `TouchWatch` so they observe without
  competing (CollectionIOS.swift:5-9, SwipeActionsIOS.swift:126-142).

**Touch delivery to the view.**

- `cancelsTouchesInView` (default true): on recognition the view gets
  `touchesCancelled`.
- `delaysTouchesBegan` (default false): withholds `touchesBegan` until the
  recognizer fails.
- `delaysTouchesEnded` (default true): withholds `touchesEnded` briefly so a
  multi-tap can still recognize.

**UIScrollView.**

- It owns a `panGestureRecognizer` and, when zoom is configured, a
  `pinchGestureRecognizer`.
- `delaysContentTouches` (default true) holds content touches for a short
  delay to see whether the touch is a scroll. I believe the delay is about
  150 ms but haven't checked it.
- `canCancelContentTouches` with `touchesShouldCancel(in:)` lets a pan steal
  from content. `UIControl` resists by default.
- Nested same-axis scroll views don't hand a pan outward at the inner edge.
  exact2's comment agrees (NodeViewIOS.swift:30-33).
- Fling landing is exposed through
  `scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`
  (llp/1050:135-137).

**System gestures.**

- `UIScreenEdgePanGestureRecognizer` is the authorable edge pan.
- `preferredScreenEdgesDeferringSystemGestures` on a view controller makes
  the home-indicator and Control Center edge swipes need a second swipe, so
  an app gesture at that edge wins the first.
- The navigation controller's interactive pop is a system recognizer that
  apps arbitrate through its delegate, as exact2 does.

**Beyond recognizers.** Interactions: `UIContextMenuInteraction` (the
preview and menu, including long press), `UIPointerInteraction` (iPad
pointer effects), `UIDragInteraction` / `UIDropInteraction`,
`UIPencilInteraction` (double tap and squeeze) and `UIEditMenuInteraction`.
`UITouch` carries:

- `type`: direct, pencil or indirect pointer;
- `force`, `altitudeAngle`, azimuth and `majorRadius`;
- coalesced and predicted touches for high-rate Pencil input.

`UIPanGestureRecognizer.allowedScrollTypesMask` admits trackpad scrolls
(iPadOS 13.4+).

**Accessibility.** VoiceOver replaces the gestures. Apps expose:

- `accessibilityActivate`;
- `accessibilityCustomActions` (the swipe-actions alternative);
- adjustable `accessibilityIncrement/Decrement`;
- `accessibilityScroll`, `accessibilityPerformEscape` and magic tap;
- `.allowsDirectInteraction` for drawing surfaces.

**SwiftUI.** `Gesture` values: `TapGesture`, `SpatialTapGesture`,
`LongPressGesture`, `DragGesture` (default `minimumDistance` 10, with
`velocity` since iOS 17), and `MagnifyGesture` / `RotateGesture` (iOS 17
names). Composition happens in the value:

- `.simultaneously(with:)`;
- `.sequenced(before:)`, for example long press then drag;
- `.exclusively(before:)`, where the first gets priority.

Attachment decides precedence against child gestures: `.gesture` (the child
wins), `.highPriorityGesture` (the parent wins) and `.simultaneousGesture`.
`GestureMask` limits this further. `@GestureState` is transient state that
resets automatically when the gesture ends or cancels, which is SwiftUI's
answer to "cancellation leaves no residue". iOS 18 unified the UIKit and
SwiftUI recognizer systems and added `UIGestureRecognizerRepresentable`. I'm
confident about the API. I'm less sure exactly how far the internal
unification goes.

## 3. Android

- **MotionEvent.**
  - Actions: `DOWN / MOVE / UP / CANCEL`, `POINTER_DOWN / POINTER_UP` for
    extra fingers, hover enter/move/exit, and `SCROLL` for wheels.
  - Each pointer has an index and a stable id.
  - Batched history via `getHistorical*`.
  - `getToolType`: finger, stylus, mouse or eraser. Pressure, size, tilt and
    orientation are axis values.
- **Dispatch is "the parent may steal".**
  - `ViewGroup.dispatchTouchEvent` asks `onInterceptTouchEvent` on every
    event. A parent that returns true mid-stream takes the rest, and the
    child gets `ACTION_CANCEL`.
  - A child that returns true from `onTouchEvent(DOWN)` owns the stream
    unless stolen. `requestDisallowInterceptTouchEvent` vetoes the parent.
  - The scroll-vs-child decision is written by hand in every scrolling
    container's intercept logic, against `ViewConfiguration.getScaledTouchSlop`
    (about 8 dp).
- **Detectors are helpers, not a framework.**
  - `GestureDetector`: down, show press, single tap up, single tap confirmed
    (which waits out double tap), double tap, long press, scroll, and fling
    with velocity.
  - `ScaleGestureDetector`: focus point and scale factor. There's no built-in
    rotation detector.
  - `VelocityTracker`.
  - The view feeds them events and decides what to do.
- **Nested scrolling.**
  - `NestedScrollingChild/Parent` (API 21, and AndroidX `*3`). The child
    offers each delta to its parents before (`dispatchNestedPreScroll`) and
    after (`dispatchNestedScroll`) consuming it, and the same for fling.
  - `CoordinatorLayout` behaviors are built on it.
  - This is the closest native analogue of CSS scroll chaining and
    `overscroll-behavior`.
- **System gestures.**
  - `View.setSystemGestureExclusionRects` (Android 10) reserves edge areas
    from the back gesture, with a capped height.
  - Predictive back (13+) is a system-owned, interruptible back gesture that
    apps observe through callbacks.
- **Jetpack Compose.**
  - `Modifier.pointerInput(key) { … }` runs a coroutine.
    `awaitPointerEventScope { awaitPointerEvent(pass) }` reads events in
    three passes: `Initial` (ancestor to descendant), `Main` (descendant to
    ancestor) and `Final`.
  - Arbitration is by **consumption**. A detector calls `change.consume()`,
    and others check `isConsumed` and back off.
  - Detectors: `detectTapGestures` (tap, double tap, long press, press),
    `detectDragGestures`, the axis variants, `detectTransformGestures` (pan,
    zoom and rotation around a centroid) and
    `awaitTouchSlopOrCancellation`.
  - Higher up: `draggable`, `scrollable`, `transformable`,
    `anchoredDraggable`, and `nestedScroll(NestedScrollConnection)` with
    pre/post scroll and fling.
  - Accessibility comes from `semantics { onClick; customActions; scrollBy }`.
  - Consumption is an arena without the name. Whoever consumes first in pass
    order wins.

## 4. The web

- **Pointer Events** (Level 2 is a Recommendation; Level 3 is in progress).
  - Events: `pointerdown/move/up/cancel`, `over/out/enter/leave` and
    `got/lostpointercapture`.
  - Properties: `pointerId`, `pointerType` (`mouse | pen | touch`),
    `isPrimary`, `pressure`, `tiltX/Y`, `twist`, `width/height`, and
    `altitudeAngle`/`azimuthAngle` in Level 3.
  - `getCoalescedEvents()`, and `getPredictedEvents()` in Level 3.
  - `setPointerCapture` routes one pointer to one element.
- **`touch-action` is the arbitration model.**
  - Values: `auto | none | pan-x | pan-y | pan-left | … | pinch-zoom |
    manipulation`.
  - At `pointerdown` the browser intersects `touch-action` from the target
    up to the nearest scroller and decides once which direct manipulations it
    will perform.
  - If it starts panning or zooming, the page gets `pointercancel` and no
    more moves for that pointer.
  - There's no mid-gesture negotiation, and the decision runs on the
    compositor thread without waiting for script. This is why browser
    scrolling stays smooth under a busy main thread.
- **Passive listeners.** Chrome made document-level `touchstart/touchmove`
  (Chrome 56) and `wheel` (Chrome 73) listeners passive by default, so they
  can't block scrolling (llp/1050:74-75). `preventDefault()` in a passive
  listener is ignored.
- **Click synthesis.**
  - `click` follows `pointerup` on the same element (or a common ancestor).
    Touch also gets compatibility mouse events.
  - The historical 300 ms double-tap-to-zoom delay is gone with
    `width=device-width` or `touch-action: manipulation`.
  - `dblclick` fires *after* two `click`s. The web never delays the first
    click to wait for the second. That's the opposite of UIKit's
    `require(toFail:)`.
  - `contextmenu` fires on right click, and on touch long press in Chromium
    on Android. I believe iOS Safari doesn't fire it on long press but
    haven't checked current versions.
- **No standard gesture events.**
  - Safari has `gesturestart/gesturechange/gestureend` with `scale` and
    `rotation`, on iOS for touch and on macOS for trackpad pinch/rotate. It's
    non-standard.
  - Chromium and Firefox report a trackpad pinch as `wheel` with `ctrlKey:
    true`.
  - A touch pinch or rotate is two pointers the page tracks itself.
  - There's no standard long press, swipe or fling. These are userland.
- **Scroll interplay.**
  - `scroll-snap-type/align/stop` picks the resting point.
  - `overscroll-behavior: auto | contain | none` controls scroll chaining and
    the browser's own overscroll affordances (pull to refresh, swipe
    navigation).
  - `scrollend` exists. There's no fling-target API (llp/1050:91).
- **Accessibility.**
  - WCAG 2.5.1 (Pointer Gestures): a multipoint or path-based gesture needs a
    single-pointer alternative.
  - WCAG 2.2's 2.5.7 (Dragging Movements): dragging needs a non-drag
    alternative.
  - 2.5.2 (Pointer Cancellation): activate on the up event.
  - ARIA has no custom-action equivalent to UIKit's.

## 5. Flutter

- **Gesture arena.** On pointer down, the hit test gives a path. Every
  interested `GestureRecognizer` on it adds itself to that pointer's arena.
  - A member can `accept`, and the first to do so wins while the others are
    rejected. A member can also `reject`, and the last one left wins.
  - At pointer up the arena is swept. If nobody has declared victory, the
    first member wins.
  - `DoubleTapGestureRecognizer` holds the arena open with `hold/release`.
  - `GestureArenaTeam` makes several recognizers compete as one, optionally
    with a captain.
  - `EagerGestureRecognizer` accepts immediately.
- **Recognizers:** `Tap`, `DoubleTap`, `LongPress`, the drags (`Horizontal`,
  `Vertical`, `Pan`), `Scale` (which includes rotation) and `ForcePress`, on
  top of `OneSequence`/`PrimaryPointer` bases.
  - `GestureDetector` and `RawGestureDetector` wire them.
  - `Listener` sees raw pointers.
  - I recall `kTouchSlop` = 18 logical px, `kLongPressTimeout` = 500 ms and
    `kDoubleTapTimeout` = 300 ms. I'm fairly but not fully sure of the slop.
- **Everything is in Dart.** That includes scroll physics
  (`BouncingScrollPhysics` vs `ClampingScrollPhysics`), and it's the tail
  LLP 0559 F1 calls a permanent bug annuity (llp/research/0559:97-115).
  exact1 ported this arena and exact2 deleted it (llp/1002:55-60).
- Accessibility comes from a parallel semantics tree with `SemanticsAction`s,
  custom ones included.

## 6. React Native

- **The old responder system.** Arbitration runs in JS:
  - `onStartShouldSetResponder(Capture)` and
    `onMoveShouldSetResponder(Capture)` pick the responder.
  - `onResponderGrant/Move/Release`, then `onResponderTerminationRequest` /
    `onResponderTerminate` for handoff.
  - `PanResponder` wraps it.
  - Every decision crosses to the JS thread. A native scroll view can take
    the touch away mid-gesture (termination), and anything that tracks a
    finger lags the busy JS thread.
- **React Native Gesture Handler.** Recognition moves native:
  - iOS wraps `UIGestureRecognizer`. Android has its own orchestrator,
    because Android has no recognizer framework. RNGH ends up with an
    arena-like arbiter on that platform, which is a telling data point.
  - v2 API: `Gesture.Tap/LongPress/Pan/Pinch/Rotation/Fling/Hover/Manual/Native()`.
  - Composition: `Gesture.Simultaneous`, `Gesture.Exclusive` and
    `Gesture.Race`.
  - Cross-component relations: `simultaneousWithExternalGesture`,
    `requireExternalGestureToFail` and `blocksExternalGesture`.
  - This is UIKit's relationship model, reified for JS.
- **Reanimated.** Callbacks are *worklets* run on the UI thread. They mutate
  shared values that `useAnimatedStyle` reads, with no bridge hop per frame.
  - `withSpring({ velocity })` and `withDecay` finish the release.
  - `runOnJS` hops back to JS.
  - This is exactly the "second value graph" and "`runOnJS`" that
    NOT-DOING.md:262-263 and :291 refuse.

## 7. Games and engines

- Engines **poll**. Each frame they read the active touches, mouse and pad
  state, and game code interprets them.
  - Unity's Input System: `EnhancedTouch`, `Touch.activeTouches`.
  - Godot: `InputEventScreenTouch` / `ScreenDrag`, plus
    `InputEventMagnifyGesture` / `PanGesture` from OS trackpad gestures.
- **Action mapping with triggers** is the useful idea. Unreal's Enhanced Input
  maps raw input to named actions through declarative triggers (tap, hold,
  pulse, chorded). Unity's Input System has similar interactions (tap, hold,
  slow tap, multi-tap). The recognizer is a *declared parameter*, not code.
  I'm not sure what Unreal's current built-in multi-touch gestures cover.
- UI layers in engines (Unity's EventSystem, with `IDragHandler` and a drag
  threshold) are simple bubbling models with no arena.
- exact2 already follows this for games: the GPU canvas gets raw
  multi-pointer input with ids and kinds (gpu-glue.js:448,
  CanvasInputIOS.swift:14-15, :53).

## 8. Comparison

### 8.1 Arbitration models

| model | who | decided when | authorable | cross-host portable |
|---|---|---|---|---|
| declarative up-front | web `touch-action` | at down, once | CSS value | yes — a value, not code |
| recognizer graph + relationships | UIKit, AppKit, SwiftUI, RNGH | continuously, by state machines | delegate / composition | only by reimplementing the graph (RNGH on Android) |
| arena | Flutter; Compose (by consumption) | first accept / consume, sweep at up | recognizer code | yes, but you own every behavior (0559 F1) |
| intercept / steal | Android Views | any event; parent wins | per-container code | no |
| responder negotiation | RN (old) | JS callbacks per event | callbacks | yes, but on the JS thread |
| poll + game logic | engines | every frame | code, or action triggers | yes, you own everything |
| **exact2 today** | `touch-action` + per-host fixed rules | at down (web, UIKit delegate), fixed chains (AppKit, Linux) | `touch-action` only | the rules are unwritten and differ per host (§1.4) |

### 8.2 Where recognition runs, and latency

| | recognition | per-frame follow | release |
|---|---|---|---|
| UIKit / Android | native, main thread | native | native (or Core Animation) |
| browser scroll/zoom | compositor thread | compositor | compositor |
| web JS gestures | main-thread JS | main thread (rAF) | CSS/WAAPI if handed off |
| Flutter / Compose | in-process UI thread | same | same |
| RN responder | JS thread | JS → native per frame | JS |
| RNGH + Reanimated | native | UI-thread worklets | UI-thread spring |
| **exact2 native** | platform recognizers (iOS) or host code (AppKit, Linux), main thread | host writes the held presentation; no app code per frame | `exact-motion` spring with velocity |
| **exact2 web** | glue JS over Pointer Events | glue writes the held style; `pan` dispatches one action per frame (input-glue.js:52-62) | browser runs the lowered spring (llp/1041:1572) |

exact2's hold model is already the RNGH+Reanimated latency shape: follow
without app code, release into a velocity spring. It gets there without a
shared-value graph or worklets. The exception is `pan`, which runs a runner
transaction every frame, by design (LLP 1043.000 D8).

### 8.3 Accessibility

UIKit, Android and Flutter all let a gesture expose a named non-gesture
action. The web gives only DOM semantics plus WCAG's rule to provide an
alternative. exact2 has activation (NodeViewIOS.swift:1441,
NodeViewMac.swift:1355-1359) and `aria-keyshortcuts` (input-glue.js:25-49),
and apps put `aria-label` on handles (apps/exact-live/app.contract:695,
:719). Swipes and drags have no generated accessibility action. The photo
viewer's Fit/2× buttons (llp/1041:1547-1550) are the authored alternative
WCAG asks for.

### 8.4 Cross-platform parity

- Only the web's declarative layer is a *standard*. Pointer Events and
  `touch-action` exist and behave the same everywhere. Every higher
  recognizer (long press, swipe, pinch, rotate, fling) is platform-specific
  or userland.
- So "the web is the standard" (AGENTS.md; RULES.md Scope) can't mean "use
  the web's pinch event", because there isn't one. It can mean:
  - use the web's names where they exist (`contextmenu`, `dblclick`,
    `touch-action`, `cursor`);
  - use the web's ordering rules where they exist (`dblclick` after two
    `click`s, no delay; activation on up; scroll chaining);
  - declare each non-web recognizer as a deviation, as `swiperight` and
    `refresh` already are.

## 9. Assessment

### 9.1 Against iOS

| capability | iOS | exact2 | where |
|---|---|---|---|
| tap | `UITapGestureRecognizer` | `press`, all hosts | glue.js:477-480; NodeViewIOS.swift:1377-1409; NodeViewMac.swift:1360-1394 |
| double tap | taps=2, `require(toFail:)` | `dblclick` on web, iOS and macOS; not Linux | NodeViewIOS.swift:243-254; NodeViewMac.swift:1433-1443 |
| long press | `UILongPressGestureRecognizer` | only as `contextmenu` and reorder lift; no event | NodeViewIOS.swift:231-238; ReorderIOS.swift:15 |
| context menu | `UIContextMenuInteraction` | `contextmenu` + exact2's own preview (`contextTarget`) | NodeViewIOS.swift:257-260; tags.rs:329-330 |
| pan | phases, translation, velocity | `pan(dx, dy)` only: no began/ended/cancel, no velocity, one touch | event.rs:172-175; LayoutPan.swift:97 |
| swipe | four directions, discrete | `swiperight` only, but with a caught, reversible spring | motion-glue.js:573-641 |
| pinch | `UIPinchGestureRecognizer` | **none**; `vscale` plumbed, always 0 | event.rs:201-213; transform.rs:1 |
| rotate | `UIRotationGestureRecognizer` | **none**; transform binding requires no rotation | llp/1002:151-157 |
| multi-touch | per-touch identity | **canvas only** | CanvasInputIOS.swift:14-15 |
| simultaneous / require-to-fail | delegate, `require(toFail:)` | not authorable; hard-coded per host | NodeViewIOS.swift:177-196; NodeViewMac.swift:1363-1368; contact.rs:236-241 |
| velocity / fling | `velocity(in:)`; scroll deceleration | release velocity on the three drag bindings and swipe; scroll fling is native; no fling for app content (refused: NOT-DOING.md:275-276) | event.rs:182-213 |
| edge pan | `UIScreenEdgePanGestureRecognizer`, deferral | system back only; swipe yields the first 20 pt; no authored edge pan or deferral | NodeViewIOS.swift:192; NavigationIOS.swift:347-371 |
| hover / pointer | `UIHoverGestureRecognizer`, `UIPointerInteraction` | `hover` bool; no position, no `cursor`, no pointer effects | event.rs:146-148; schema.json (no `cursor`) |
| stylus | `UITouch.type`, force, tilt, Pencil interactions | none outside the canvas | gpu-glue.js:448 |
| trackpad | scroll types, pinch, rotate | macOS phased scroll only; Linux none | NodeViewMac.swift:153-167; input.rs:1-4 |
| pull to refresh | `UIRefreshControl` | `refresh` on iOS only | NodeViewIOS.swift:1209-1231; llp/1054:188 |
| drag and drop | drag/drop interactions | in-list `reorderdrop` only | motion-glue.js:648-836 |
| VoiceOver | custom actions, adjustable | activate only | NodeViewIOS.swift:1441 |
| agent drive | none (XCUITest, private) | `tap` forms; iOS phases `unsupported` | llp/1012:224 |

### 9.2 What's actually weak

These matter more than the feature count:

1. **The arbitration rules exist but are unwritten.** They are:
   - the UIKit delegate predicates (NodeViewIOS.swift:177-196, :219-230;
     ScrollView :41-58);
   - AppKit's `mouseDown` order (NodeViewMac.swift:1363-1368), which differs
     from its `mouseDragged` order (:1407-1411);
   - Linux's candidate chain (contact.rs:236-241);
   - the web's independent listeners, which settle it by
     `stopPropagation` and capture (motion-glue.js:434-439).

   A node with both `pan` and `swiperight` could behave differently on each
   host. LLP 1002 D4 says the platform arbitrates. In practice, for anything
   above scroll, *exact2* does, four ways.
2. **Recognition policy is triplicated** (swipe constants in §1.4).
   **Velocity has three estimators**: UIKit's, the JS endpoint estimator
   (motion-glue.js:482), and `VelocityTracker` (velocity.rs; contact.rs:458).
3. **The web recognizes in JS.** That's unavoidable, because the web has no
   recognizers. It means the web oracle only holds for Pointer Events and
   `touch-action`. It doesn't hold for swipe feel, which is glue code held to
   the native hosts rather than the other way round.
4. **The agent can't drive a pinch anywhere, or any phase on iOS.** So a
   new multi-touch recognizer can't be verified by the loop RULES.md
   requires.

### 9.3 Constraints any design must respect

- **Native-first.** The platform recognizes. Scroll always wins
  (NOT-DOING.md:258-261; llp/1002:102-106).
- **The web is the oracle.** Use web names and semantics where they exist,
  and declare deviations otherwise (AGENTS.md "The web is the standard";
  RULES.md Scope).
- **The agent drives it.** Every new input is a form of `tap` or `type`,
  with a truthful `delivery` (NOT-DOING.md:295-302; llp/1035.003 D2/D3).
- **No JS before first pixel.** Gesture glue loads after paint, linked by use
  (RULES.md time budgets; llp/1047:564-577, :799-812).
- **No arena, claims, leases or compositions; no second value graph; no
  per-frame app callbacks** (NOT-DOING.md:258-263, :291; llp/1035.001 §6).
  The trigger that moves an item off: one line naming what it unblocks, and
  something taken off the doing-list in the same PR
  (NOT-DOING.md:348-351).
- **Consumers.** Photo zoom, the sheet, reorder and Messages are admitted
  (NOT-DOING.md:64-71). Anything else needs its own.

## 10. Recommendation

The best, most complete design gets **iOS's capability set with the web's
arbitration model**. It uses three pieces, none of them a runtime arena:

1. a closed set of recognizers as Contract events, each lowered to the host's
   native recognizer, or to glue on the web;
2. `touch-action`, completed to the CSS vocabulary, as the only authored
   arbitration;
3. one written precedence table that every host implements with its own
   mechanism: UIKit delegates and `require(toFail:)`, AppKit dispatch order,
   web capture, the Linux candidate chain.

Presentation stays follow-and-release. The authorable composition that
UIKit, SwiftUI and RNGH expose is exactly what NOT-DOING refuses, and it's
the part that doesn't port.

### 10.1 Contract

- **Keep:** `press`, `dblclick`, `contextmenu`, `hover`, `key`, `scroll`,
  `refresh`, `swiperight`, `pan` and the three drag bindings.
- **Complete `touch-action`** with `pinch-zoom` and the multi-value forms CSS
  allows (schema.json:986-1000). This is how an author says "this photo takes
  pinch, the page keeps vertical scroll".
- **Pinch on the transform binding.** `transformDragFor` admits a two-finger
  pinch on hosts that have one. `transformrelease` already carries `scale`
  and `vscale` (event.rs:201-213). No new handler is needed.
- **Add `cursor`** as a style row. It's CSS, and it maps to `NSCursor`, to
  `UIPointerStyle` on iPad and to the web property.
- **Later, each behind a consumer:**
  - `longpress`, a declared deviation because the web has none;
  - `panrelease(vx, vy)`, completing `pan` with the velocity the other
    releases already carry;
  - free `pinch(scale, cx, cy)` / `rotate(radians)` events for canvas-like
    content that isn't a transform binding.
- **Never in Contract:** recognizer relationships, custom recognizers,
  phases as handlers, pointer ids, or per-frame callbacks.

### 10.2 The precedence table (spec, not runtime)

Write it once, in an amendment to LLP 1002 D4 or in the host specs. Each
host implements it with native means. It's static and derived from the tree
and the rows, so it's a rule, not a composition. Proposed:

1. The system's own gestures (back swipe, home edge) keep their edges. An
   authored swipe yields the first 20 pt, as iOS already does
   (NodeViewIOS.swift:192).
2. Scroll wins on every axis `touch-action` leaves to it (the web rule;
   NodeViewIOS.swift:41-58 already follows it).
3. The innermost node with a matching handler wins, and ancestors don't
   also fire (DOM bubbling, as `press` does today; glue.js:477-480).
4. On one node, one contact: drag binding (reorder > transform > height) >
   `pan` > `swiperight` > long press / `contextmenu` > `press`. Linux's chain
   (contact.rs:236-241) is nearly this already. AppKit's `mouseDown` and
   `mouseDragged` orders should both become it.
5. **Double click follows the web.** `press` fires on each click and
   `dblclick` after the second. There's no `require(toFail:)` delay. I didn't
   verify whether iOS currently cancels the second `press` when the two-tap
   recognizer fires (`cancelsTouchesInView`, NodeViewIOS.swift:245-249).
6. **Exactly one built-in simultaneity:** pinch with pan on a transform
   binding, and rotate when it comes. It's host-owned
   (`shouldRecognizeSimultaneouslyWith` on iOS; two pointers in glue on the
   web). Authors can't request simultaneity.

### 10.3 Runner

- Owns dispatch, payload validation and ordering, as now (event.rs:675-826).
  It learns no phases or pointer ids.
- States the units once: logical px, px/s, scale units/s.
- Keeps the one-action-per-frame coalescing contract for `pan`
  (LLP 1043.000 D8).
- Holds and springs stay in `exact-motion`. `VelocityTracker` becomes the one
  estimator for every host whose platform gives none: the web through the
  motion exports, and Linux. The JS estimator at motion-glue.js:482 and :578
  goes.
- Recognition thresholds that exact2 defines itself (the swipe knee and
  resistance, the slops) move to one Rust table the hosts read, instead of
  three copies. Where the platform has its own slop (UIKit's pan), the
  platform's wins.

### 10.4 Hosts

| host | owns |
|---|---|
| iOS | `UIPinchGestureRecognizer` (+ rotation later) on transform bindings, simultaneous with the binding's pan; the precedence table as delegate predicates and `require(toFail:)`; `UIPointerInteraction` for `cursor`; accessibility custom actions for `swiperight` and `contextmenu`; later `UIContextMenuInteraction` in place of the raw long press |
| macOS | `magnify(with:)` / `NSMagnificationGestureRecognizer` on transform bindings; Control-click as `contextmenu`; `NSCursor`; the table as one dispatch order; accessibility custom actions |
| web | two-pointer pinch in `motion-glue.js`, `wheel` + `ctrlKey` for Chromium/Firefox trackpads, `gesture*` for Safari; `touch-action` passed straight through; `cursor` as CSS; all of it after paint and linked by use |
| Linux | declared as not having pinch (no multitouch slots, input.rs:108-111); authored zoom controls stay the path; right button as `contextmenu` and `dblclick` by click count when a consumer asks |

### 10.5 Agent

- `tap <target> pinch <scale> [over <ms>]`, and later `rotate`, is a form of
  `tap`, not a ninth operation.
- Delivery by carrier:
  - web: `platform`, using CDP `Input.dispatchTouchEvent`, which accepts
    several touch points;
  - iOS and macOS: `recognized`, by injecting the recognized scale into the
    hold. I don't know of a public API that synthesizes magnify events.
  - Linux: `unsupported`.
- A `longpress` form is `tap … hold` from LLP 1035.003 D1 where phases
  exist, and `recognized` elsewhere.

### 10.6 Phases

- **Phase 0: no new surface.**
  - Write the precedence table.
  - Align AppKit's orders.
  - Add `pinch-zoom` to `TouchAction`.
  - Use one velocity estimator and one threshold table.

  This takes apparatus away, so it needs no NOT-DOING trade.
- **Phase 1: pinch on transform bindings.**
  - Hosts: iOS, macOS and web. Linux is declared as not having it.
  - Add the agent's `pinch` form.
  - The consumer is admitted: photo zoom (NOT-DOING.md:64-71; llp/1041:517).
    LLP 1041 §8.13 records the missing piece as "physical pinch"
    (llp/1041:1588).
  - This is "bounded geometry/motion work those consumers need", so it
    probably needs no trade. Charlie should confirm that.
- **Phase 2, each with a named consumer:**
  - `cursor`;
  - accessibility custom actions (WCAG 2.5.1/2.5.7 parity);
  - `panrelease` velocity;
  - `longpress`;
  - web and AppKit `refresh` (llp/1054 X2), where the web answer is probably
    none, since browsers own document overscroll.
- **Phase 3, only on demand:** free `pinch`/`rotate` events, other swipe
  directions, stylus on nodes (until then the canvas is the answer),
  trackpad swipe navigation on macOS, and keyboard reorder.

### 10.7 What stays refused

- A runtime gesture arena, claims or leases (NOT-DOING.md:258-261).
- Authorable composition: simultaneous, exclusive, sequenced,
  require-to-fail, `highPriorityGesture` equivalents.
- Per-frame app callbacks, worklets and shared values (NOT-DOING.md:262-263,
  :291).
- Owned scroll physics or fling shaping.
- Decay drivers (NOT-DOING.md:275-276).
- Custom recognizers written in Contract.
- A generic drag-and-drop system.

**Trigger for reopening the arena refusal.** A v1 consumer has two
independently authored recognizers, on one node or on nested nodes, whose
conflict none of the following can express:

- `touch-action`;
- the precedence table;
- the one built-in simultaneity.

An example is a map with pan, pinch, rotate, long-press-to-drop and
double-tap-zoom inside a horizontally paging container. Even then, the first
answer should be a per-host native relationship compiled from a static
declaration, not a runtime arena.

## 11. Uncertain

- The `delaysContentTouches` delay (about 150 ms).
- Android's default long-press timeout (400 or 500 ms by version).
- Flutter's `kTouchSlop` value.
- Whether iOS Safari fires `contextmenu` on long press.
- How far iOS 18's UIKit/SwiftUI recognizer unification goes.
- What Unreal's built-in touch gestures cover.
- Whether iOS's two-tap recognizer currently suppresses the second `press`
  (§10.2 item 5).
- Whether Control-click reaches `contextmenu` on macOS (§1.4).
- Whether a public API can synthesize trackpad magnify events for the agent
  (§10.5).
