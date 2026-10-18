# LLP 1132: Shown and interactive

**Type:** RFC
**Status:** Draft (2026-10-18). Reviews and the naming discussion are in `llp/reviews/1132-shown-and-interactive.*.md`. The loading ledger in D5 is implemented on branch `observe/semantics`. The remaining behaviour and the changes to that ledger are proposed. File and line citations refer to that branch. Earlier drafts and their reviews called this document LLP 1126, a number `main` has since given to system surfaces.
**Systems:** the kernel schema and Contract lowering (one new attribute, `markinteractive`); the runner (the set of marks); each host's timing marks (`host/apple/Sources/ExactKit/Launch.swift` and `NavigationMarks.swift`, `host/linux/src/launch_marks.rs` and `navigation_marks.rs`, `host/web-js/marks.js`) and the journal they write; `modules/observe`, as one consumer.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-17
**Implementer:** Claude (Opus 5.5), on `observe/semantics`, once this RFC is approved. Until then it specifies nothing.
**Related:** LLP 1115 (names are the web's, behaviour the platform's), LLP 1113 (capture settle: whether a picture can be taken), LLP 1038 (the router), LLP 1048.003 (placeholders), `modules/observe/README.md` (Expo Observe's names and rules).

## Summary

Exact measures **visits** and **sections**:

- A **visit** is a screen while its route is in place, a dialog while it is open, or the app, from the start of its session or from a return from the background. A screen's visit continues beneath a sheet, a fullscreen route or a dialog. The **launch** is the first app visit of the session that the process launched with.
- A **section** is any other element inside a visit that the app marks, such as a comments panel. Exact measures its interactive time separately.

A visit's **shown** time is its first presented frame.

An app marks its key UI with `markinteractive=(…)`. On a visit's own element, the attribute marks that visit. On any other element, it marks a section.

A marked visit or section can also have an **interactive** time. The visit must still last, the runtime must be ready, and the expression must be true; D2 defines the exact moment. Interactive measures the app's own work, so what the user can see, a transition's animation and the user's input do not delay it. A cover over a visit, such as a sheet or a dialog, neither pauses nor ends it. Without a mark, there is no interactive time.

Exact also knows when the launch has nothing left loading. It calls this **settled**. It keeps settled as a diagnostic and does not journal it.

Exact updates the set of marks as commits change it. It evaluates marks only when something changes, so no read walks the tree. The hosts journal times in Exact's own words. Expo Observe maps those events to its own names in its own module.

## 0. Why

No platform has one API for "interactive". The precedents rely on declarations by the app:

- **The web** has no such API. Lighthouse's Time to Interactive was a lab heuristic, removed in Lighthouse 10 ([2023-02-09](https://developer.chrome.com/blog/lighthouse-10-0)). `document.readyState === "interactive"` means only that the document was parsed. User Timing suggests the mark name `mark_interactive`, "as marked by the developer" ([User Timing](https://w3c.github.io/user-timing/)).
- **Android** reports Time To Full Display only when the app calls `reportFullyDrawn()`: "If you don't call `reportFullyDrawn`, no TTFD value is reported" ([launch time](https://developer.android.com/topic/performance/vitals/launch-time)).
- **Apple** extends the launch measurement until the app's launch tasks finish: "the later of the first rendered frame and the completion of all tasks" ([`ExtendedLaunchMetric`](https://developer.apple.com/documentation/metrickit/extendedlaunchmetric)). It covers only the launch. No UIKit or SwiftUI API marks a view as interactive.
- **Expo Observe** records TTI when the app calls `markInteractive()`. It uses the first call per launch. For a route, it measures from the navigation to that call on the destination ([metrics](https://docs.expo.dev/eas/observe/reference/metrics/)).

These precedents record that a task completed. Exact's mark is a live condition, evaluated while its visit lasts (D2). In each case, the app defines interactive. An app that says nothing gets no interactive time. As the owner put it: "TTI is when an app's key UI elements become interactive. In a way it is application-defined."

## D1 — Visits and sections

### Visits

A visit is one of three kinds.

**Screen.** A screen visit belongs to a route: an element with `navigationKey` that is a direct child of the navigation root or of a `role="tabpanel"` (`contract/lower/src/routes.rs:180-184`). A **sheet route** is either a sheet (`navigationPresentation="modal"`) or a fullscreen route (`navigationPresentation="fullscreen"`). It is a route too, so it has screen visits.

A stack contains the selected tab's routes, or the root's routes when it has no tabs. It extends up to the route that the root selects with its own `navigationKey` (`runner/src/head.rs:210-250`). In an app with a router, these routes are its entries. In an app without a router, they are the root's routes in document order (`host/web/navigation.js:307`).

A stack divides into **presentations**. It starts with the base presentation. Each sheet route starts a new presentation. That presentation contains the sheet route and the routes that follow it, up to the next sheet route (`NavigationRules.swift:39-41`).

A screen visit lasts while its route is **in place**. Several routes can be in place at once:

- the top route of each presentation in the stack, including when a later sheet or fullscreen route covers it;
- in a split view, the first route of a stack beside a pane (`runner/src/head.rs:109-144`; the web keeps it active, `host/web/navigation.js:313-319`);
- the destination of a transition that UIKit makes itself, such as a back swipe, from when UIKit begins it (`willShow`, `NavigationIOS.swift:946`). The router changes only when the transition completes (`NavigationIOS.swift:1079`). When the transition ends, the source route leaves its place if the transition completed, and the destination leaves its place if the user cancelled it.

A **cover** is a later sheet route or a dialog over a route. A host can stop drawing a covered route, as the web and Linux do beneath a fullscreen route (`host/web/navigation.js:313-317`, `host/linux/src/navigation.rs:53-66`). The route stays in place and keeps its visit, and a cover never delays its times (D2).

A screen visit **begins** when its route comes into place, when a `replace` gives its entry a new URL, or when the app returns from the background (Start, below). It leaves events only once it is shown (End, below). It ends when its route leaves its place:

- a push puts a route that is not a sheet route above it in its own presentation, unless it stays in place as a split view's first route beside a pane. A pushed sheet route starts a new presentation, so the route beneath it stays in place;
- its tab is deselected;
- a split view collapses over it;
- its route is removed, for example by a back;
- a `replace` changes its entry's URL (`route/src/router.rs:180-186`).

A `replace`'s new URL begins a new visit. That visit reads the mark on the route element then in place. If the template keeps the element mounted under the entry's key, the new visit reads the same element's mark. If the template renders a different element for the new route, it mounts that element with its own mark.

A route can come into place or leave it through the router, through state or through layout. An app without a router can use `state screen` with `navigationKey=screen` (`RouteSheetMacTests.swift:24-32`); the runner then reports no router change (`runner/src/runner/router.rs:667-671`). Either way, the route's place decides the visit. A router change adds labels, such as the route pattern and URL.

**Dialog.** A dialog visit begins when the host accepts the opening of a `dialog`, whether `showModal(id)`, `commandfor=id command="show-modal"` or its `open` state opened it (`kernel/tables/schema.json:614`). The host can hold an accepted opening back, as UIKit's host does until a dismissing alert has gone (`MenusIOS.swift:453, 895`). It ends when the dialog closes. Its cause follows the commit that opened it, as for a screen. A host that refuses a dialog has no dialog visit. Linux's DRM and headless displays refuse `showModal` (`host/linux/src/presenter/commands.rs:99-103`).

**App.** A **session** is one running Exact app. A Mac app has one session per window. A session's first app visit begins with the session and is shown at its first frame. In a later Mac window, that visit and the window's initial routes start when the window was asked for (`ExactMac/main.swift:180`), with the cause `program`; a route that replaces one of them is timed from its own cause. Another app visit begins each time the app returns from the background. The app visit's element is the root.

The first app visit of the session that the process launched with is the launch (D4).

### Cause

A visit's cause is the input that brought it about, such as a tap, a key or a back gesture. For a screen, this is the input whose own action made the commit that put its route in place, whether that commit changed the router or only state (D6). Without such an input, the cause is `program` (a commit), `launch`, `foreground` or `layout` (a split view expanding). A commit that a timer's action or a response's handler makes, such as a redirect after a response, has the cause `program`, even when an input came just before it.

### Start

A visit's **start** is its timing origin: the moment its cause happened. It can come before the visit begins. Exact measures the visit's shown and interactive times from it:

- for an input, the input's own timestamp, and for a transition that UIKit makes itself, the moment UIKit begins it;
- for `program`, the commit that put the route in place or opened the dialog;
- for `foreground`, the foreground notification;
- for `layout`, the moment the host applied the new layout;
- for `launch`, process start.

A visit caused by an input therefore starts before its commit: the gap is the time its handler works before committing. The hosts already time navigations this way (`NavigationMarks.swift:243`, `host/linux/src/navigation_marks.rs:246`).

The web has no process start, so a launch there starts at page activation. For a page that was not prerendered, this is the navigation timing origin. For a prerendered page, it is the activation (`marks.js:260-261`).

When the app returns from the background, new visits begin with the cause `foreground`: the app, every screen in place, and every open dialog. This includes a screen that came into place while the app was in the background. Their times exclude the time in the background.

### Shown

A visit is shown at its first presented frame:

- **On iOS**, this is the first frame after UIKit begins to show the route's view controller (`routeAppeared`, `NavigationMarks.swift:270-280`), or begins to present the dialog.
- **On macOS**, this is the frame that carries the route. For a route or dialog presented with `beginSheet`, it is the first frame after the host begins presenting it. The host calls `beginSheet` (`host/apple/Sources/ExactKit/Mac/SheetsMac.swift:228`). That call returns only when the entrance animation ends (`SheetsMac.swift:32-33`), so the host observes the first frame during the animation. No time waits for that return.
- **On Linux**, this is the page flip that shows it.
- **On the web**, this is the rendering opportunity after the commit (`requestAnimationFrame`, then a task). This estimates presentation; it is not a reported paint.

A cover never delays a shown time. A covered screen is shown at the first frame the host presents after its visit begins (Visits), whether or not that frame draws it. No earlier frame counts, even when the visit's start, from which its times are measured, is earlier, at its input. This covers a screen that comes into place beneath a cover, as at a deep link into a fullscreen route, and a covered screen's visit after a return from the background. A screen or dialog whose presentation the host holds back, behind another presentation's animation or the first draw, is shown the same way, so no transition delays a shown time either. UIKit's host presents one sheet route at a time and draws the initial content before a cover (`NavigationIOS.swift:479-481`).

Apple's frame times are estimates too: a frame counts as presented at the first display refresh after its commit (`Launch.swift:257-259`). Linux reads the page flip's own time. The same experience can therefore differ by about a frame between hosts.

On the web, the first app visit is shown at the page's first paint. Two observers race: Element Timing on the boot tree's first text, and First Contentful Paint. The first to report wins. Its time is clamped to page activation.

As a fallback, the host captures the next rendering opportunity. It uses that timestamp if neither observer has reported within 200 ms of it (`marks.js:231-257`).

A served page can paint before the script's commit (`marks.js:225-229`). The screen on top at that paint is shown at the same paint. A redirect after it is therefore a navigation. Screen history must use that paint, rather than the later rendering opportunity it currently uses (`marks.js:179`).

### End

A visit ends:

- for a screen, when its route leaves its place (Visits, above);
- for a dialog, when it closes;
- for any visit, when the app goes to the background, the runner is poisoned, the data module's activation fails, its session ends, or the process is told to end.

Poisoning and failed activation also cancel every marked visit that begins after them: such a visit is shown, then cancelled at its shown time (D2).

Closing a window ends only that window's session (`ExactSession.destroy`, `Session.swift:1475`). Linux has no background signal. It notes the termination signal (`host/linux/src/launch_marks.rs` `told_to_end`).

A visit that ends before its first presented frame leaves no times and no `cancelled` event, whatever ended it. Its sections leave no events either. The host holds a section's events until its visit is shown, then publishes them. If the visit ends first, the host discards them.

A route that replaces another before the other's first frame is timed from its own cause.

A cover over a visit neither pauses nor ends it, however many are stacked. The screen beneath keeps its own interactive time. That time can come while the cover is visible, even though the host makes the screen inert or hides it (`ModalIOS.swift:245-248`). An interactive time says when the app made the visit ready, not when the user could reach it, so no cover delays it.

### Sections

A section is an element with `markinteractive` that is not a visit's own element. Any element can be a section; the HTML `section` tag is not required.

Its **owner** is the nearest enclosing route or dialog. Outside every route and dialog, its owner is the app. Exact measures a section within its owner's visits.

Inside a tab panel of a navigation root, a `role="tabpanel"` that the root's `tablist` names (`host/web/navigation.js:13`), Exact measures a section only while every such enclosing panel is selected. Other authored panels have no selection of Exact's; an app that hides them writes that state into its expression (D3). An unselected panel stays mounted, hidden and inert (`host/web/navigation.js:297`). Deselecting a panel ends its sections' measurements; one still pending is cancelled with the reason `left`, and one already interactive keeps its time. Selecting the panel again creates new measurements, timed from the selection's cause.

Exact measures each mounted section separately. A section has one measurement for each period during which it is mounted, its owner's visit lasts, and all its enclosing tab panels are selected. Each use of a component has separate measurements. So does each element that shares an `id`. For now, the compiler refuses a section in a list row, which would repeat once per item (D3).

A measurement starts at the latest of these moments:

- the visit's start;
- the start of what mounted the section: the timestamp of the input whose own action made the mounting commit, or the commit's time when no input's action made it;
- inside tab panels, the start of the selection that showed its enclosing panels.

Its cause is the cause of that latest moment. A section mounted by a tap is therefore timed from the tap. This includes time the tap's handler spends working before mounting the section.

A measurement ends, and the next one begins, as follows:

- The end of its visit ends it. If it is still pending, it is cancelled with the visit's reason.
- An unmount ends it. If it is still pending, it is cancelled with the reason `unmounted`. A later mount creates a new measurement.
- The owner's next visit measures the section again if it is still mounted.

### Identity

Every visit and every measurement has an identity: the session and a sequence number unique within that session. A section's measurement also names its visit's identity.

Author-facing names can repeat. A route pattern covers several visits. An entry and its URL can recur across visits. Several elements can share an `id`. These names therefore serve as labels, not identities:

- a screen's route pattern, entry, URL and parameters;
- a dialog's `id`;
- a section's `id`.

## D2 — The two times

**Shown** applies only to visits (D1).

**Interactive** applies to a marked visit or section. It is the first moment when all the following conditions hold, at or after both the visit's shown time and the measurement's own start:

1. **The visit lasts.** For a section, its measurement must also last (D1).
2. **The runtime is ready.**
   - The data module's activation has completed. On Apple, activation runs after the first frame is drawn (`Session.swift:1251`). The web keeps baked answers while it waits (`host/web-js/rt.js:529`).
   - The input handling needed by the marked element and its mounted descendants is installed, including descendants inside nested sections. Elements outside that subtree do not count. Native hosts install input handling at boot. The web loads two parts later, on demand, when an element that needs one is created:
     - the deferred input module, used by pressables that are not native controls, keyboard shortcuts (`aria-keyshortcuts`, including on a native button), pans, and the arrow keys of a `radiogroup`, `tablist` or `menu` (`host/web-js/rt.js:616, 688, 914, 1050, 1146`; `host/web-js/src/emit.rs:986-995`; `host/web/input-glue.js:71, 155`);
     - the motion executor, used by swipes, drags and reorder handles (`rt.js:1031`).

     The emitter flags each element that needs a part, and its mount counts it in every enclosing mark (D6). A screen of plain native controls therefore waits for neither, regardless of what another tab contains. Marking a section inside a screen never changes what the screen waits for. A root mark waits for input handling anywhere the app has mounted. On a served page, the runtime adopts a long list's later rows in slices, a press, a key, an edit or focus entering such a row adopts it first (`rt.js:1340`), and the list adopts its pending rows before it reconciles (`rt.js:1219, 1245`); a row's needs count from its adoption, so a mark does not wait for rows still waiting to be adopted. Press appearance does not delay a mark: the input module also draws press feedback and button styles, but they change how a press looks, not whether it works.
3. **The expression is true.**

Interactive measures the app's work to make the visit ready, even while the host does not yet let the user interact. These therefore never delay it:

- the visit's own transition: a pushed screen, a sheet or a dialog can become interactive during its animation;
- a back swipe that the user holds, for the screen it reveals;
- a scene that iOS has not yet made active;
- a cover over the visit (D1).

The three conditions can change independently. Each of the following triggers an evaluation:

- a new value;
- a visit beginning;
- a tab selection showing a panel;
- activation completing;
- input handling being installed;
- an element that needs a deferred part mounting or unmounting beneath the mark.

Once the conditions hold, the interactive time depends on whether a change to the visit's content, such as a new value shown or a mounted element, is still waiting to be presented:

- If one is, use the presentation of the first frame that the host builds after that change, whether or not that frame draws the visit. This holds even when the condition that came to hold last changed no content, as when activation completes after the content changed. A cover can stop the host drawing the visit; the time is still the frame at which the change would have appeared.
- If none is, use the moment the conditions came to hold, or the visit's shown time if that is later. A change that only lets the user see content already there, such as a tab selection that shows a panel, is not a content change, since what the user can see does not delay interactive.

Even with a content change waiting, use the moment the conditions came to hold if the host builds no frame for that change, because it draws nothing that changed. The host decides when the change is made. A frame built before the change never counts, such as one that Linux had already submitted (`host/linux/src/display.rs:423`), and neither does a frame that later input causes.

Every condition must still hold at the chosen moment. An earlier true value that became false does not count, and neither does input readiness lost when an element mounts that needs a part not yet installed.

A measurement has at most one interactive time. If it ends before becoming interactive, the host journals `cancelled` with one of these reasons:

- `left`, when a screen's route leaves its place or a section's tab panel is deselected;
- `closed`, when a dialog closes;
- `unmounted`, when a section unmounts;
- `background`;
- `ended`, when the session ends or the process is told to end;
- `poisoned`;
- `activation_failed`;
- `input_failed`.

Poisoning and failed activation are terminal for every visit and section. A failed load of the web's input module or motion executor is terminal only for visits and sections that need it, including one that comes to need it when such an element mounts beneath its mark. They are cancelled with the reason `input_failed`. The web retains a failed input load and does not retry it (`host/web-js/rt.js:1140-1150`).

A marked visit or section that begins after a failure terminal for it can never become interactive:

- A visit that begins after the failure is shown, then cancelled at its shown time.
- A section is cancelled when its measurement is created. Its start can be earlier.

A measurement that does not need the failed part continues. A cancelled measurement receives no later interactive time.

## D3 — The mark: `markinteractive=(…)`

```contract
component App
  state heroReady = false
  resource feed = loadFeed() as shape Feed else empty()
  resource comments = loadComments() as shape Comments else empty()
  action heroShown
    heroReady = true
  action heroMissing(message: string)
    heroReady = true
  …
  view
    …
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` markinteractive=((feed.loaded and heroReady) or failed(feed))
          if feed.loaded
            image src=feed.hero load=heroShown error=heroMissing
          section id="comments" markinteractive=(comments.loaded or failed(comments))
            …
      dialog id="compose" markinteractive=(feed.loaded)
        …
```

`markinteractive` declares the app's readiness condition for recording a visit's or section's interactive time. Exact also waits for the runtime (D2).

The attribute observes readiness. It never enables, disables or hides anything. An element takes it at most once, bound or literal.

### Compiler checks

The compiler applies these checks:

- It refuses an expression whose type is not boolean (`lower-attr-type`, `contract/lower/src/values.rs:1223-1250`). If an expression of unknown type yields another type at run time, the runner clears the prop (`runner/src/instance.rs:1048-1053`). A cleared prop counts as false.
- It refuses `markinteractive` inside a `popover`. On iOS and macOS, many popovers become native menus that Exact never paints (`MenusIOS.swift:1-8`, `MenusMac.swift:1-8`).
- It refuses a section with a `for` row between it and its owner, including through a component used by the row. The refusal counts only rows below the owner. A router's screens are themselves rows of `for e in stack(nav)` (`docs/recipes/navigation-stack.md:56`, `apps/shelf/app.contract:198`). A marked screen and its sections therefore remain legal. A section in each row of a list on that screen is refused.
- It allows repeated uses of a component that contains sections. Each use is written in the source, so the author can see how many copies there are. A `for` row's count depends on its data.

The list-row refusal prevents journal overflow. Each row would have its own measurement. A list of 5,000 marked rows would journal 5,000 events. That would fill the journal's ring of 4,096 (`LaunchParts.swift:39`, `host/linux/src/journal.rs:16`, `marks.js:42`) before the services load. It would push out the launch's own events.

The error names the alternative:

> This `markinteractive` is inside a `for` row of its screen. Each row would be measured separately and journal its own event. A long list would overflow the journal. To measure the list once, put `markinteractive` on the element that contains it.

Starting with an error leaves room to make it a warning later without breaking an app. Changing a warning to an error could break one.

### Components and scope

A component use accepts only its declared props. `Card(title="x", markinteractive=true)` is therefore refused with `type-unknown-prop`. A component marks an element in its own view. When its user should decide readiness, the component can take a prop for that expression.

An expression reads what is in scope where it is written. Resources belong to the root, and "a child takes their results as props" (`contract/cli/src/vocab/concepts.md:655`). A mark in a child component can read that child's own state.

Whether the mark measures a visit or a section depends on where its element lands after component expansion. The same rules apply as for any other mark.

### Readiness expressions

Each mark stands alone. A visit does not wait for its sections' expressions or interactive times. Its input readiness still counts every mounted descendant, including those inside sections (D2). To include a section in the visit's key UI, the visit's expression must include that section's readiness condition.

The author also specifies readiness for conditional UI and failure:

- A resource shows its readiness through a `loaded: bool` field that its source always answers `true`, so `else empty()` keeps it `false` until the first answer. `failed(r)` is true once a request failed with no answer (`concepts.md:52-54`).
- For an image that is key UI, its `load=` and `error=` actions set state, as `heroReady` does above. The screen is interactive when both the feed and the hero image are ready, or when the feed has failed. The image mounts only after the feed loads. This prevents an error for the placeholder's empty source from setting `heroReady` early. These events fire on the web (`host/web-js/rt.js:893-906`), macOS and iOS (`docs/contract-for-agents.md:1351-1352`), but not yet on Linux.
- A screen that shows an image only sometimes writes `markinteractive=(not showHero or heroReady)`.
- A native module's view that is key UI follows the same pattern. A native module accepts a `load=` event. Its action can set state that the expression reads. Exact therefore does not track when the host attaches the view.
- If the app hides or disables key UI, the expression must include that state. Exact reads the expression; it does not inspect authored `inert`, `hidden` or `display: none`.

### Common cases

- **Several marks.** Each marked visit and each mounted section has its own times. A single mark on a container measures a group once.
- **No mark.** There is no interactive time, as on Android and in Observe.
- **Key UI removed while something loads.** Unmounting a section ends its measurement and cancels it if it is still pending.
- **Key UI ready while something else loads.** A mark does not wait for resources its expression does not read. A screen can be interactive while its images load and a feed below the fold still fetches.
- **Cost.** A mark is an ordinary binding. The runner evaluates it again only when something it reads changes (`runner/src/instance.rs:995`). It sends a value only when that value changes (`instance.rs:1015`). D6 covers the hosts.

## D4 — The launch

The launch is the first app visit of the session that the process launched with. It begins with that session.

If the app goes to the background before its first frame, that first visit remains the launch. It ends unshown and leaves no visit or section events (D1); the launch's raw marks are still journaled. The visit that begins on the return is not the launch.

In a Mac app with several windows, later windows have their own sessions. Those sessions are not the launch.

- **Shown** is the app's first frame.
- **Interactive** is the root's interactive time, when the root carries `markinteractive`. The background, the end of its session or process, poisoning, failed activation, and a failed input load needed by the root cancel it. Nothing modal pauses it.
- **Settled** (D5) is the launch's diagnostic.

An unmarked root gives the launch no interactive time of its own. Exact does not substitute a screen's time. A consumer decides whether a screen should represent the launch. Observe's module takes the launch screen's interactive time (D7).

The launch screen follows D1's shown rules, including those for covered and held-back screens. On iOS and tvOS that means the first frame after UIKit begins to show it (`routeAppeared`) unless it is covered or held back; the launch path does not wait for that today (`NavigationMarks.swift:207-222`).

**Services.** The app's launch modules defer loading their services so that service work does not delay what they measure. With an unmarked root, though, settled can come before the launch screen becomes interactive, and service work can then fall before Observe's startup `tti` (D7). They load at the launch's first applicable terminal outcome:

- its interactive time, when the root is marked;
- its settled time, when the root is unmarked;
- in both cases, the background, poisoning, failed activation, a failed input load needed by the root, the end of the launch's session or process, or 30 s after the launch's anchor.

At an orderly end of a native process, the services load and flush before it exits, as Apple does today (`Launch.swift:447`). A web page cannot load a service while closing because the loader imports it asynchronously (`marks.js:335`). If the page is hidden before its services load, it keeps its journal for the next page's services (`marks.js:329`). A service already loaded flushes through its own exit path.

Until the services load, a native journal keeps only its latest 4,096 events, and a web page hidden in that time keeps only its latest 512 (`marks.js:332`). A launch that journals more loses its earliest events, its own among them. A process killed before the services load loses its journal.

Services load at the terminal outcome even when that outcome journals nothing, such as a visit ending before its first frame. Loading services does not end measurement. Later times are still journaled.

The service-loading moment has no event of its own because no consumer needs one. Apple and the web already load services on any outcome (`Launch.swift:366`, `marks.js:68`).

The hosts use these anchors and loading paths:

- **Apple:** the anchor is didFinishLaunching. The 30 s bound already exists (`Launch.swift:122-130`).
- **The web:** the anchor is page activation (`marks.activation`). Add the 30 s bound. The current trigger is only the end of startup (`marks.js:66-75`).
- **Linux:** services keep starting on their own thread when the display opens (`host/linux/src/display.rs:351-353`). This keeps their work off the display path.

The hosts journal the launch's raw marks: process start and the anchor. On Apple, they also journal the constructor, the app delegate's init and didFinishLaunching. They journal whether prewarming or launching into the background suppressed the launch. Consumers own formulas over these marks.

## D5 — Settled: the launch's diagnostic

Settled is the first moment after the launch is shown when nothing in the whole app is still loading. If the last work changes the screen, settled is the presentation of the frame that shows that change. If nothing changed, it is the moment the work cleared (`Launch.swift:336-348`, `launch_marks.rs:323-351`).

Settled determines when an unmarked app's services load (D4). The timing log (`EXACT_TIMING_LOG`, D7) prints it. It is never reported as interactive.

Use the existing loading ledger below for settled. Stop journaling its time as the startup report's `timeToInteractive` (`Launch.swift:571`, `marks.js:79`). Remove the per-screen settled times that the branch computes for navigations.

Loading means:

- **In the runner (`Runner::outstanding`):** a request or stream for a resource whose value is still its placeholder, until its answer or first message arrives, whether or not any open view reads it; and a resource waiting for its source.
- **In the host:** the data module's activation. On Apple, this also includes a collection fill, a published batch not yet applied, owed canvas work including canvas image decodes, and on iOS SVG images (`ExactSession.hostOutstanding`, `Session.swift:426-435`). On the web, it also includes other loading in flight, such as Markdown and GPU work (`marks.js` `outstanding`).

Settled does not count this work:

- **Timers:** they are the app's logic.
- **Images outside a canvas:** the web has one exception. A tinted image from another origin, with `load=` or `error=`, starts a CORS probe. The probe counts as loading until it answers (`host/web-js/rt.js:888-892`).
- **`aria-busy`:** it means that a region "is being modified" ([ARIA](https://w3c.github.io/aria/#aria-busy)). Counting it would let a refreshing list's spinner delay settled on a usable screen.
- **Mutations, the data module's storage operations, refreshes behind a shown answer, and device holds.**

Settled covers the whole app because the runner does not know which views read a resource. A stream that never sends prevents settled. On Apple and the web, an unmarked app's services then load at the 30 s bound. Linux starts its services when the display opens (D4). On Linux, settled also requires the tree to be unchanged since the frame was built (`launch_marks.rs` `Ledger::settles`).

## D6 — Evaluated on change; no read walks the tree

A whole-tree read costs more as the tree grows. The removed `aria-busy` read walked the tree on every turn while startup waited. On the development Mac, a measurement in the `host-dev` profile (2026-10-16) found that one `Runner::outstanding()` call took 14 µs at 401 nodes, 61 µs at 4,001 nodes and 400 µs at 20,001 nodes.

Marks need no tree walk:

- **The runner keeps the set of marks.** After each accepted commit, it updates the set from the commit's receipt (`CommitReceipt`, `kernel/src/txn.rs:52-91`):
  - It adds a created node whose plan site binds `markinteractive`, whatever its first value. It must use the plan because an invalid first value clears a prop that was never set. That leaves no trace in the receipt (`txn.rs:734-737`). Literals are bindings too (`runner/src/instance.rs:951-1056`).
  - It updates a touched node's value. A cleared prop counts as false.
  - It removes a destroyed node and its descendants.
  - A refused commit leaves the set unchanged (`runner/src/runner/commit.rs:112`).
- **The Rust hosts**—Linux and Apple's Rust layer—read the runner's changes in-process.
- **Each change carries its commit's time and whether the input's own action made the commit.** Router changes already carry whether the input's own action made them (`RouterChange::from_event`, `runner/src/runner/router.rs:35, 676`); carrying each commit's time through batching is new. An overdue timer can run inside an input's dispatch before the input's action (`runner/src/runner/advance.rs:38`). A section mounted by that timer is timed from the timer's commit. It is not timed from the input. Linux already makes this distinction for navigations (`host/linux/src/navigation_marks.rs:57`).
- **Apple's Swift** receives each commit's mounts, values and removals in its batch, with those timing facts. This includes elements without a native view, such as SVG children (`host/apple/src/paragraph.rs:231`). It receives changes, never the whole set.
- **The web JS target** has no runner and keeps its own set:
  - The emitter registers each element carrying `markinteractive` when it creates the element. It registers either the constant value or the effect that updates the value. Literals skip the dynamic path (`host/web-js/src/emit.rs:1203`).
  - It finds the element's route or dialog after the commit attaches the element. The runtime builds each conditionally mounted block in a detached fragment before inserting it (`host/web-js/rt.js:1172-1175`).
  - The scope's cleanup removes the registration.

Costs:

- **A mount** finds its owner by following parent links: O(depth). On the web, an element that needs a deferred part also counts itself in each mark on that path, and its unmount uncounts it. Either change evaluates the pending marks whose count changed.
- **A deferred part arriving** evaluates the marks that counted an element needing it: O(those marks).
- **A value change** costs O(1).
- **A visit beginning or ending** evaluates or cancels the marks mounted in that visit: O(those marks). The compiler refuses sections with a `for` row below their owner (D3). The number of marks in a visit is therefore bounded by the marks written in the source and the uses of their components.
- **A tab selection** evaluates the sections under the panels it shows and hides: O(those sections). Each section records its enclosing panel when it mounts.
- **Nothing else runs.** A mark that never becomes true costs nothing until one of these changes, or one of the readiness changes listed in D2, happens.

To verify these costs, the implementation measures them once with a temporary test. It measures a marked container over 401 and over 20,001 unmarked rows, many uses of a marked component, and a dialog that closes over the list.

The implementing commit records the numbers. It adds no permanent benchmark, which would be new apparatus (`rules/RULES.md`, "Agents").

## D7 — Exact's journal, and Observe as one consumer

The hosts journal these events in Exact's words:

- **`router`**, a new event for each router change. It carries its commit's time, the top entry, URL, route pattern, parameters, removed entries and cause, and the identity of the visit on top.
- **`shown`, `interactive` and `cancelled`** for visits (`app`, `screen`, `dialog`) and sections. Only visits have `shown`. Each event carries its identity, labels, cause, start and time. A visit's `shown` says whether it is marked, and a screen's `shown` says whether its navigation root selects it at that frame. A section's events name its visit's identity.
- **The launch's raw marks.**

Every event gives its times in milliseconds on its session's monotonic clock, and also its wall time, for dating. Durations are differences on one session's clock. Today the journal's times are seconds on Apple and Linux (`launch_marks.rs:3`) and milliseconds on the web (`marks.js:38`). The clocks differ in one respect: Apple's (`CACurrentMediaTime`) stops while the device sleeps, Linux's (`CLOCK_BOOTTIME`) counts a suspend, and the web's (`performance.now()`) follows its browser.

Timing runs in these places:

- **Apple startup timing** runs for every launch that is not suppressed, except under the agent. This exception applies whenever `EXACT_AGENT=1`, regardless of the agent's clock (`ExactEnv.agentMode`, `Session.swift:61`, `Launch.swift:121`).
- **Apple navigation timing** runs under the agent too. Today it runs from each batch's router op (`Session.swift:1063`). Under this RFC, screen visit timing follows each route's place (D1), including a change by state or layout without a router op.
- **Linux** times only the display path: "a headless run reports none" (`launch_marks.rs:1`).
- **The web** times an app only when a launch module has a web service (`host/web-js/build.mjs:340-341`).

To check a mark's value, an author uses the agent's `tree`. To check its times, the author runs outside the agent with a launch module. The timing log and the agent's `timing` state also show, for each measurement, the condition that came to hold last or the conditions still unmet. That is for debugging on the author's machine; the journal does not carry it.

Observe's names and policies move from the framework to `modules/observe`. The framework keeps the facts those policies read. These move:

- **Navigation events in Observe's shape:** `navigation` events named `cold_ttr`, `warm_ttr` and `tti` (`host/web-js/marks.js`, `host/linux/src/navigation_marks.rs:334,354`, `host/apple/Sources/ExactKit/NavigationMarks.swift:338,386`).
- **Screen history:** cold or warm by screen history (`ScreenHistory` on every host, `NavigationMarks.swift:407-418`), and one TTI per screen, "(Observe records one per screen)" (`NavigationMarks.swift:1-6`).
- **Supersession:** a newer navigation cancels a navigation measurement still waiting (`NavigationMarks.swift:225-226`, `modules/observe/README.md:41`).
- **The startup report and its labels:**
  - Its metric names. On Apple these are `<type>LaunchTime`, `timeToFirstRender` and `timeToInteractive`, "in Observe's metric names" (`Launch.swift:486, 562-571`). On the web they are `timeToFirstRender` and `timeToInteractive` (`marks.js:89-90`). On Linux they are `coldLaunchTime`, `timeToFirstRender` and `timeToInteractive` (`launch_marks.rs:494-502`). Linux also uses `launchType: "cold"` and the Apple mark names `didFinishLaunching` and `boot` (`launch_marks.rs:422-424, 439`).
  - Its `tti` outcome field (`launch_marks.rs:442`, `marks.js:92`).
  - `launchScreen`, Observe's `isAppLaunch` rule (`Launch.swift:542-543`, `marks.js:93-94`, `launch_marks.rs:233-234`).
  - The route at TTI, "as Observe names the screen whose marker fired" (`Launch.swift:540`, `launch_marks.rs:449`).
- **Launch formulas:** the launch-time formula (`Launch.swift:568`), and the cold or warm heuristic with its `dev.exact.observe` defaults suite (`Launch.swift:460-462`). Linux's heuristic is already in the module (`modules/observe/linux/launch.rs`).
- **Frame statistics:** the slow and frozen thresholds and their `expo.frameRate.*` names (`FrameStats`, `Launch.swift:584-607`). They move with their collector into the module's launch part (`modules/observe/apple/launch/ObserveLaunch.swift`). That part runs at launch and sees frames from didFinishLaunching onward.

The framework keeps its own timing diagnostics and renames them from Observe to timing:

- `EXACT_OBSERVE_LOG` (`Launch.swift:373`, `NavigationMarks.swift:41`, `host/linux/src/display.rs:350`, `host/linux/src/journal.rs:163`) becomes `EXACT_TIMING_LOG`;
- the `observe:` lines (`ExactIOS/main.swift:162`, `ExactMac/main.swift:738`, `host/linux/src/display.rs:675`, `navigation_marks.rs:368`) become `timing:` lines;
- the web's "exact: observe marks:" prefix (`marks.js:45, 101`) becomes "exact: timing marks:";
- the agent's `observe` section of `state` on Apple and `exact.observe()` on the web become `timing`.

`modules/observe` maps the events. Its README states these rules:

- **Startup metrics come only from the launch session:**
  - `expo.app_startup.ttr` comes from the first app visit's `shown`.
  - If the launch's `shown` says the root is marked, `tti` waits for and comes from the root's interactive time. Otherwise, it comes from the launch screen's `interactive` (below), before the app goes to the background. A screen that a later navigation brings into place does not count: TTI measures when the app is ready, not what its user does. Observe's own rule, the first `markInteractive()` per launch, would count it.
  - Launch times come from the raw marks.
- **A `tti` longer than 30 s is not sent**, for startup or a navigation, matching the services' 30 s bound. Exact still journals the interactive time.
- **Navigation metrics come only from navigations:** screen visits that a router change brings into place, with cause `input` or `program`.
  - A transition that UIKit makes itself brings its destination into place before its router change arrives (D1). The module holds that visit's events and sends them only when a `router` event names that visit. A cancelled transition has no router change, so the events of its destination's visit are never sent, whether or not that visit became interactive.
  - A router change that leaves in place a visit that began before this navigation, such as dismissing a sheet route or a pop back to a split view's list, reveals content already there and loads nothing, so it gives no navigation metric. Today's code and Observe's Expo Router integration send `warm_ttr` for it (`NavigationMarks.swift:205`, `init.ts:74`).
  - `expo.navigation.cold_ttr` and `warm_ttr` come from each such visit's `shown`. The module's own history decides between them. A navigation is warm when its screen was shown before in the session, and cold otherwise. In that history, a screen is an entry at a URL (`NavigationMarks.swift:407-418`). A `replace` to a new URL therefore creates a new screen. A screen counts as shown once any `shown` event names its entry and URL, including a `foreground` event. A `router` removal forgets the entry, so a later push of the same URL as a new entry is a new screen, and cold.
  - `tti` comes from that visit's `interactive`, once per screen: a return to a screen that was already interactive sends none. This is the rule of expo-observe's Expo Router integration, which Exact's router corresponds to: re-focusing a screen "must not produce a second metric" (expo-observe 57.0.9, `src/integrations/expo-router/useObserveForRouter.ts`). Observe's documentation and its React Navigation integration record one per navigation instead. Observe owns its navigation measurement. A newer navigation ends its measurement of the earlier one, as its README states. A screen that a sheet route covers before it becomes interactive therefore sends no navigation `tti`. Its framework visit continues and journals `interactive`.
  - The launch screen is the screen that the navigation root selects at the launch's first frame, as its `shown` event says, with or without a router, or a redirect's destination if it replaces that screen before that frame. If that frame selects no screen, or the launch ends unshown, there is no launch screen, and an unmarked root gives no startup `tti`. A pane beside it or a route beneath a sheet route is not the launch screen. Its navigation metrics come from its own visit, start and shown time, although its cause is `launch`.
  - A visit that begins without a navigation, with cause `foreground` or `layout`, gives no navigation metric. The branch already treats a return from the background this way (`NavigationMarks.swift:132-141`).
- **Dialogs and sections** have no Observe metric. The module sends nothing for them until Observe has names.

Observe sends `tti` only from marks. An app that marks nothing sends no `tti`.

## What changes, where

| Where | Change |
|---|---|
| `kernel/tables/schema.json`, `contract/lower/src/tags.rs` | the `markInteractive` bool prop and its `markinteractive` attribute; the refusals inside a `popover` and of a section in a `for` row below its owner (D3) |
| `runner` | the set of marks, updated from each commit's receipt (D6) |
| `host/web-js` | the emitter's registration, resolved at attachment; in `marks.js`: visits, sections, interactive times, each mark's count of mounted descendants that need a deferred part with the loaders' installed and failed signals, the services' 30 s bound from page activation, the activation-failure signal and Exact's events |
| `host/apple/Sources/ExactKit`, `host/apple/src` | the batch's mark changes; screen visits that follow each route's place, not only router ops; dialog and app visits and sections beside `NavigationMarks.swift`; on iOS and tvOS, the launch route shown by D1's rules, gated on `routeAppeared` unless covered or held back, a presentation held back for another's animation or the first draw shown at the first frame after its visit begins, and a UIKit transition's destination in place from `willShow`; Exact's events |
| `host/linux/src` | the same visit and section timing and Exact's events, from the runner's changes |
| `modules/observe` | the mapping, every policy listed in D7 including the frame collector, the startup `tti` from the launch screen, the 30 s bound on `tti`, and the README |
| tests | below |

## Tests

Tests check event times as well as which events exist. Every host runs the timing-rule tests.

A host runs a presentation's tests only if it supports that presentation. Linux has no split view and refuses dialogs on its DRM and headless displays. It therefore checks the refusal and absence of a dialog visit instead of running the dialog and split-view tests.

Visits:

- A marked screen becomes interactive while a resource it does not read still loads.
- A marked screen with a literal `true` waits for activation.
- On iOS, a launch screen marked `true` becomes interactive when its first frame and activation are done, without waiting for the scene to become foreground-active.
- A marked screen beneath a sheet or dialog becomes interactive while the sheet or dialog is visible. The cover does not end its visit. Under two stacked sheets or beneath a fullscreen route, a screen keeps its visit even when the host hides it.
- A change that makes a covered screen's expression true and alters its content gives it the same time as an uncovered visit: the frame the host schedules for the change. A covered change takes the first frame the host builds after it, never one already submitted, and a later tap's frame does not count.
- Content changes and makes a screen's expression true while the input module still loads; the module then arrives without changing content, before the frame that presents the change. The interactive time is that frame, not the module's arrival. If the module arrives after that frame, the interactive time is its arrival.
- A dialog that its `open` state opens has a visit, timed from the commit that opened it, or from that commit's input.
- Dismissing a sheet route gives Observe no navigation metric for the screen beneath, which kept its visit. A back to a screen whose visit had ended gives `warm_ttr`.
- In a split view, the list beside a pane has its own visit and interactive time. Collapsing the split view ends that visit.
- Pushing away from a marked screen before its expression becomes true cancels its measurement with the reason `left`.
- In an app that navigates by state without a router, A → B → A ends A's visit and cancels its pending sections. Each visit gets a fresh identity.
- A `replace` that keeps the entry and changes the URL ends the visit and begins a new one. Check this both before and after the first frame.
- A redirect before the first frame leaves only the destination, timed from the redirect's commit. Observe classifies it as the launch screen.
- An app without a router launches with a fullscreen route over its base route. Only the fullscreen route's `shown` says its root selects it, so Observe takes it as the launch screen, whichever of the two is shown first.
- A marked alert, sheet or pushed screen whose expression is already true becomes interactive at its first frame, during its own animation. On macOS, an animated sheet is interactive before `beginSheet` returns. A back swipe that the user holds does not delay the screen it reveals.
- A shown screen's mark becomes true with a visible change, and a fullscreen route covers it before the next frame. Its interactive time is that frame, as if it were uncovered.
- After a return from the background, a screen still hidden by a fullscreen route is shown at the first frame after the return. It becomes interactive without waiting for the cover to close.
- A launch or deep link into a fullscreen route shows the route beneath it at the first frame, although no frame draws it. A marked `true` route beneath becomes interactive then.
- During a held back swipe, the screen it reveals has a visit and can become interactive. A cancelled swipe cancels that visit's pending measurement with `left`, and the screen the user stayed on keeps its visit. Observe sends a completed swipe's `warm_ttr` from that visit, and its `tti` if that screen was not interactive before, and nothing for a cancelled one, including when the revealed screen became interactive before the cancel.
- A covered screen's visit after a return from the background, and after a `replace` that keeps its element, is shown at the first frame after that visit began, not at an earlier visit's frame.
- A tap's handler works for 100 ms and then puts a route in place beneath a fullscreen route. A frame presented during the handler's work does not show the route; the first frame after the commit does.
- On iOS, one commit presents two sheet routes. The upper one is shown, and can become interactive, at the first frame after its visit begins, although UIKit presents it only after the lower one's animation.
- On iOS, an app closes a marked dialog and reopens it at once. The reopened dialog's visit begins when the host accepts the opening, and it is shown at the first frame after that, although UIKit presents it only after the old alert has gone.
- A section beside the routes, in a panel that a tab selection shows, with an expression already true, is interactive at the selection, without waiting for the frame that shows the panel. A section inside the selected tab's screen waits only for that screen's shown time.
- A section mounts and unmounts, then its visit ends before the visit's first frame. The section leaves no events.
- A visit starts at the input's timestamp when its handler works before committing the navigation, through the router or through state.
- A dialog refused by the host has no visit.
- After a return from the background, an open dialog's pending mark is measured in its new visit. Observe gets no navigation metric.
- A screen that came into place while the app was in the background is timed from the return.

Sections:

- A section pending when the user pushes another screen and comes back is cancelled. It is measured again in the new visit.
- The compiler refuses a section with a `for` row between it and its owner, including through a component used by the row. The error names the containing element as the place to measure the list. A marked screen produced by `for e in stack(nav)` and a section inside it are accepted. A component used twice gives two measurements.
- A section that unmounts and mounts again in one visit gets a new measurement.
- A section mounted by a tap is timed from the tap, including when its handler works before mounting it. A section mounted by an overdue timer inside the tap's dispatch is timed from the timer's commit.
- A section outside every route, opened after startup, belongs to the app visit.
- A section beside the routes in an unselected tab panel has an expression that is already true. It becomes interactive when its tab is selected, without a binding change or mount. Its time runs from the selection. Deselecting the panel while the section is pending cancels it with `left`.
- A mark with an invalid first value is still registered and counts as false.

Failures and the launch:

- A visit poisoned before its first frame leaves nothing. A marked visit that begins after poisoning is shown and cancelled at once.
- After an input load fails, a native control navigates to a marked screen with a custom pressable. The screen is shown and cancelled at once. A section that needs the failed module and mounts later is cancelled at its mount.
- The root's mark sets the launch's interactive time. A navigation does not cancel it.
- With an unmarked root and an unmarked launch screen, a marked screen that a navigation brings into place gives Observe no startup `tti`. So does a first screen that a later commit mounts after a launch frame that selected none. A `tti` longer than 30 s is not sent, and Exact journals the interactive time.
- An app sent to the background before its first frame keeps that unshown visit as its launch. The visit on the return is not the launch.
- An unmarked app that goes to the background or fails during startup loads its services at once. A native app that quits, or whose launch window closes, loads and flushes them before it ends. A web page hidden before its services load keeps its journal. The next page's services send it.
- Services load 30 s after the launch's anchor if the root's expression never becomes true. They load at the root's interactive time even when a stream keeps the launch unsettled.

The web:

- A served page painted before the script's commit is shown at that paint. A redirect after it is a navigation.
- A marked screen with a custom pressable, a native button with `aria-keyshortcuts`, or a `tablist` waits for a delayed input module. A failed load cancels it. The same applies to a section that only pans. A marked screen of plain native controls does not wait for these parts, even when an unselected tab contains a custom pressable.
- A section with a custom pressable makes its screen wait for the input module too. A custom pressable in another route, or in a conditional block that is not mounted, does not.
- A screen that waits only for the input module stops waiting when the last element needing it unmounts, and becomes interactive at the frame that presents that removal. A custom pressable mounted after the module failed to load cancels a pending screen with `input_failed`.
- A prerendered page's launch is timed from activation.
- A marked element mounted in a conditional block is assigned to its route when it attaches. This includes a block that the runtime builds in a detached fragment.

Also run the D6 cost measurement and record it in the implementing commit.

## Not in this RFC

- **Whether image readiness works on Linux.** Linux fires neither `load` nor `error` for an `image`. Only Apple and the web map `Event::Load` (`host/apple/src/abi.rs:1010`, `host/web-js/rt.js:893-906`). That parity lands separately. Until then, a mark that waits for an image never becomes true on Linux.
- **Whether settled can wait only for a screen's resources.** This RFC does not track which views read a resource. That information would let settled wait only for what a screen reads (D5).
- **Whether a section must be in the viewport.** A section scrolled out of view can still be interactive. The web's Container Timing proposal measures painted area ([Container Timing](https://w3c.github.io/container-timing/), an origin trial in Chromium 148–153). Nothing native does.
- **Whether interactive time has a deadline.** Sentry ends its time to full display after 25 or 30 s. Exact journals a late time and lets each consumer impose its own deadline. Observe's module sends no `tti` longer than 30 s (D7).
- **When Android and Windows get timing marks.** Their hosts (`host/android`, `host/windows`) have no timing marks yet. When Android gets them, it calls `Activity.reportFullyDrawn()` at the launch's interactive time. Android's own Time To Full Display will then be Exact's.

## Decisions

The owner decided these rules on 2026-10-17:

- **Explicit only.** There is no interactive time, and no Observe `tti`, without a mark (D3, D7).
- **No pause.** A cover over a screen, such as a sheet or a dialog, neither pauses nor ends its visit. Its interactive time can come while the cover is visible (D1, D2).
- **Split views.** Each pane in place is a screen visit (D1).
- **Sections.** A marked element that is not a visit's own element is a section. Each copy is measured separately, with `id`s as labels (D1, D3). For now, the compiler refuses a section in a list row: a section with a `for` row between it and its owner. The refusal can later become a warning (D3).

The owner decided these on 2026-10-18:

- **Readiness, not access.** Interactive measures the app's own work. What the user can see, a cover, a transition's animation, a scene the host has not yet made active and the user's input do not delay it. A navigation can still be timed from the input that brought it about (D1).
- **Input readiness counts the mark's mounted subtree**, nested sections included (D2).
- **Observe's startup `tti`** for an unmarked root comes from the launch screen only, its navigation `tti` once per screen, and no `tti` past 30 s (D7).
- **What held a time** shows in the timing log and the agent's `timing` state, not in the journal (D7).
- **Revealing is not loading.** A router change that only reveals a screen that kept its visit, such as dismissing a sheet route, gives no navigation metric (D7).

The attribute is named `markinteractive`. "Mark" says that it measures rather than controls; `interactive=false` would read as "disable input". The naming discussion is in `llp/reviews/1132-shown-and-interactive.naming.sol.md`.
