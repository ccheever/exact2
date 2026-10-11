# Pitfalls when writing exact2 apps

**Adding an entry:** only a pitfall you hit and reproduced on current `main`, written
as symptom → cause → what to do, with where it was found. Delete the entry in the
change that fixes the footgun, or that makes the compiler, runtime or driver
diagnose it. **Candidate diagnostic** marks one that could become a check cheaply.

> **Lookup reference; start at [start-here.md](start-here.md).** Its last section names
> the pitfalls that cost agents the most. Search this list by symptom (`grep -n`) when
> something compiles and misbehaves; don't read it front to back.

This list holds what the [agent guide](contract-for-agents.md)'s rules don't make obvious.

## Layout

- **A root with `min-height="100%"` and `overflow-y="auto"` does not scroll itself.**
  `min-height` lets the box grow with its in-flow content (CSS), so it has nothing to
  scroll and the document scrolls instead: on the web a screenshot shows only the first
  screen, and on iOS a builder found the results stuck below the keyboard (a finger can
  be caught by the root's own scroll view, which has no range). Fix: `height="100%"`
  with `overflow-y="auto"`, or a `column height="100%"` holding a `scroll` with `flex=1
  min-height=0`. (Authoring bench, LLP 1087, r33 t1-tip and ios25 t1-tip, 2026-10-06.)

- **A heading's lines are a screen apart with `line-height=28`.** Cause: a bare
  number is CSS's unitless `line-height`, a multiple of the font size (at 22 px,
  28 × 22 = 616 px), unlike a bare number on a length row such as `font-size` or
  `width`, which means pixels. Fix: write `line-height="28px"`, or a ratio such as
  `line-height=1.3` (28.6 px at 22 px). (Authoring bench,
  LLP 1087: three Codex builders, caught only by a screenshot, 2026-10-05.)

- **A ported `clamp(inset, 15, 60)` never stops at 60.** Cause: React Native
  code's `clamp(value, min, max)` (lodash's order) is not CSS's `clamp(MIN, VAL,
  MAX)`, which is `max(MIN, min(VAL, MAX))`: `clamp(env(safe-area-inset-bottom),
  15px, 60px)` is the inset whenever it passes 15. Fix: `clamp(15px,
  env(safe-area-inset-bottom), 60px)`. (Bluesky clone's bottom bar, the kernel
  test that caught it, 2026-10-07.)

- **An image tile grows to its picture's size.** An album tile in a flex row became
  900×1200 pt. Cause: a flex item's automatic minimum is its content size (CSS), and
  an image's content size is its intrinsic size. Fix: give the image or its flex
  parent `min-height=0` (`min-width=0` across a row), or position it absolutely in
  a sized box. (Signal Clone DIARY, build 7.)
- **A sized `symbol:` image is stretched.** An ellipsis became three tall bars.
  Cause: `object-fit` defaults to `fill`, as for `<img>`. Fix: don't size a symbol;
  it follows its text (in a button, tab or bar item, its title's font; give it a
  `font-size` if it must differ; a native button refuses its symbol's `width` and
  `height`). A symbol you do size by its box needs `object-fit="contain"`. (Signal
  Clone, 2026-10-02.)
- **In dark mode the navigation title and status bar turn white on a light page.**
  Cause: the app paints a fixed light background (`background-color="#FAF7F2"`)
  while the system is dark, so UIKit's bars and the platform's colours go dark
  around it. Fix: leave the background unsaid (the platform's, LLP 1115), or, for a
  deliberately light-only design, write `color-scheme="light"` on the root, which
  holds the whole app in light as `overrideUserInterfaceStyle` does. (Shelf port,
  2026-10-09.)
- **On iOS the whole window scrolls when the keyboard opens, and the header goes
  with it.** Cause: without `interactive-widget`, the root is not resized for the
  keyboard. Fix: `interactive-widget="resizes-content"` on the root, or
  `overlays-content` with a `role="toolbar" toolbarPlacement="keyboard"` for a
  toolbar that rides the keyboard without relayout (LLP 1008 §9.1). (Signal Clone.)

- **The app is wider than the window, and its tests still pass.** A root with
  `width="100%"` and `padding=24` is 48 points too wide: sizes are `content-box`, as
  on the web, so padding adds to the width. Buttons at the right edge are cut off,
  and an `expect` never sees it. Fix: `box-sizing="border-box"` on any box sized in
  percent that also has padding or a border (`exact new`'s template does), and
  `min-width=0` on a `flex=1` input in a row. Look at a screenshot on each host.
  (Fresh-agent README trial, 2026-10-04.)

- **A list of buttons is ragged: each is only as wide as its text.** A `button`
  directly in a `scroll` or another block container shrinks to fit, so a row's
  `flex-grow` title never grows and its trailing columns sit right after it.
  Cause: as on the web, a button is `inline-block`; only a flex item, or one given
  a width, fills its line. Fix: `width="100%"` on the button, or put the rows in
  a `column`. (The LLP terminal reader's document tree, 2026-10-08.)

- **A raised `z-index` leaves a dragged card under the next column.** A card at
  `position="relative" z-index=10`, dragged over a neighbouring column, paints
  beneath it. Cause: `z-index` orders siblings, not a whole stacking context as in
  CSS (a declared deviation, [LLP 1001](../llp/1001-kernel-v1.spec.md) "`z-index`
  orders siblings"), and on the web a box that follows a positioned or stacked box
  becomes a paint group (`isolation: isolate`). A child cannot rise above its
  parent's later siblings. Fix: raise the ancestor that is a sibling of the others
  (the card's column, while it holds the dragged card), or draw the dragged card in
  an overlay at the board level. For cards between lists, a `reorderGroup` on the
  lists has the host draw the card in its top layer instead ([LLP
  1094](../llp/1094-dropping-across-lists.rfc.md)). (Authoring bench, LLP 1087,
  t4-kanban: two builders, 5 and 10 minutes, 2026-10-04.) A chess board's drag ghost
  is the same: a ghost inside the square it left stays under the squares after it;
  draw it as a later sibling of the squares, positioned in the board (chess diary #5).

- **The content of an overlay vanishes behind its own background.** Cause: a
  background box with `position="absolute"` (a dimmer, a gradient) paints
  over every later sibling that is not positioned, as CSS orders it. Before
  LLP 1083.000 the Apple hosts painted in tree order and hid this. Fix: give
  the content `position="relative"`, or give the background `z-index=-1`
  inside a parent that stacks. (Signal Clone's call screen, build 16.)

- **`flex=0` collapses a column that has a `width`.** A side column written
  `width=400 flex=0 min-width=0` in a 1,100 px row laid out 0 px wide, and the
  pane beside it covered its search field. Cause: `flex: 0` is CSS's `0 1 0%`:
  the basis is `0%`, not the width, and the item shrinks (`flex=<n>` is always
  `<n> 1 0%`; `contract vocab flex`). Fix: `flex="none"` (`0 0 auto`: the width
  is the size), or no `flex` and `flex-shrink=0`. (Stocks DIARY, about 15
  minutes; reproduced on the web and macOS, 2026-10-04.)

- **A `width="100%"` box with padding runs past its parent.** A full-width column
  with `padding=16` measured 1,232 px in a 1,200 px window; an inbox row's time
  painted off the right edge of a phone. Cause: a box is `box-sizing: content-box`,
  as in CSS without a reset, so padding and border add to `width` (and to
  `height="100%"`). Fix: `box-sizing="border-box"` on the padded box or its style;
  or drop `width`, since a box in a block or a `column` already fills the width.
  (Chat2 and Workout DIARY, Gallery's `height`; reproduced on the web, 2026-10-04.)
  **Candidate diagnostic:** the compiler could name `box-sizing` when a
  content-box node has `width="100%"` and horizontal padding.

## Lists and scrolling

- **A tap that changes one row of a long list takes ~80 ms on the web.** Cause: the
  mutation answers the whole list (10,000 rows), and on the JS target a Rust
  module's answer crosses into JS as a copy and every row is checked again. Fix:
  make the list's resource a window (LLP 1027.004: `feed(cursor)` answering at most
  200 rows, `reachstart`/`reachend` on the `list` moving the cursor) and give the
  mutation `refreshes feed`: 16–24 ms. A development build on the JS target says so
  in `logs` (`big answer: <resource or mutation> … carries N list rows`) once an
  answer's longest list passes 2,000 rows (`host/web-js/seam.js`). (Heavy-list
  bench against Dioxus, 2026-10-03.)
- **A bounded list hitches at its first window shift, or its first tap is slow.**
  Cause: a Rust data source that parses or builds its data on first use does it
  then; the first window is baked into the plan, so the first query is the first
  `reachend` (or tap), on the main thread mid-scroll (75 ms for a 7 MB JSON on the
  web). Fix: do that work in `DataSource::activate`, which runs after first pixel.
  (Heavy-list bench, 2026-10-04.)
- **A transcript or feed should open at its newest row.** Writing `scrollTop` to a
  huge number lands short on a virtualized list by the estimate error of the rows
  it has not built (84 pt with `estimated-item-height=52`). Fix:
  `scroll-start="end"` on the `list` (LLP 1010 §6.5), not a `scrollTop` write or a
  `scrollIntoView` after the first command. (Signal Clone, build 4.)
- **The app works hard at rest.** Cause: it commits state that shows on a timer
  (a clock's text written every 250 ms), so every tick changes a view and the host
  runs its whole post-apply pass (navigation, controls, menus, every scroller's
  position) four times a second; before `32805146` this also cut the reader's
  scrolling. A timer's or a scroll handler's commit whose writes show nowhere
  skips that pass; any other still runs it. Fix: write state
  only when it changes (a minute-resolution clock; poll fast only while something
  is in flight: `task poll when inFlight every(200, tick)`), and remove
  `scroll=` handlers left over from experiments: each runs an action per scroll
  frame. (Signal Clone, build 2.)
- **A gate reads state at commits.** A gated task (`task hide when toast != ""
  key=toastUntil`) is armed or dropped by the commit that changes its gate or
  key, never as the clock moves: so a gate cannot read `performanceNow()` (refused), and the
  `after`'s action runs at its deadline exactly, `performanceNow()` equal to it. An action
  that re-tests `performanceNow() > toastUntil` there does nothing and the toast stays up
  forever; clear it unconditionally. (LLP 1092 D8; ledger2 #1, chat F7.)
- **An "on return" task also runs at every launch.** Cause: a gated task arms
  when its gate becomes true and at boot when it is already true (LLP 1092 D8),
  and the page is `visible` at launch: `task returned when page.visibilityState
  == "visible"` with `after(1, onVisible)` runs `onVisible` once at launch and
  once after each return from the background. A gate follows a state; it is not
  an event. Fix: have the action skip its own first run (`if launched` … then
  `launched = true`); it then hears every return. To keep the run out of the
  launch as well, latch the hide (`task hid when page.visibilityState ==
  "hidden"` with `after(1, markHidden)` setting `wasHidden`) and gate the return
  on `wasHidden and page.visibilityState == "visible"`, clearing `wasHidden` in
  its action: that misses a return which lands before the hide's timer has fired
  (`prefer visibility-state hidden` then `visible` with no clock step between).
  (Web and iOS simulator, 2026-10-06.)

- **A custom row in a grouped list overflows its card on the right.** Cause:
  the sheet already gives each row its margin (16 pt, or 56 pt after an icon)
  and a 16-pt trailing padding. A custom row with `width="100%"` adds the
  margin on top and runs 16 pt past the card. Fix: leave custom row content
  at its natural width (`flex-grow=1` on the part that should stretch), not
  `width="100%"`. (Signal Clone, build 15.)

- **A data answer past 16 MiB fails only on the JS target.** A 64 MiB string from
  a Rust source loaded on the wasm web host and on Linux, and on the JS target
  the resource failed (`Rust module rejected the call`). Cause: the JS target's
  Rust data seam carries at most `MAX_HOST_WORK_BYTES` (16 MiB) a message; the
  runner's own data source has no such cap. Keep an answer under 16 MiB, or page
  it. (LLP 1090 conformance plan, `host/web-js/conformance/budget.contract`.)

- **A finger on a card's ellipsis title does not lift the card on the web.** A `text`
  inside a `reorderFor` grip with `overflow-x="hidden"` (which `text-overflow="ellipsis"`
  needs) is a scroll container, and `touch-action` is resolved from the touched element
  up to its nearest scroll container (Pointer Events), so the grip's `none` is never
  consulted: where the page can scroll the browser takes a touch that starts on the
  title, and nothing lifts; the journal says `reorder: the browser took the touch contact on a grip to scroll before it lifted` and names the scroll container (LLP 1102 §3.17). A mouse, or a finger on the grip's
  padding, works. Driven at phone size on the web: the card stays; without the overflow,
  or with `touch-action="none"` (or `pointer-events="none"`) on the title, it moves. Fix:
  put `touch-action="none"` on that text too. (Authoring bench, LLP 1087, r32 and r33
  t4-kanban, 2026-10-06.)

- **A second card drag right after a drop does nothing.** A drag that starts before
  the last one's session ends is refused (LLP 1094 D8): the drop is held until its move
  shows (a second at most; [the agent guide](contract-for-agents.md#views-layout-and-interaction),
  boards). A new drag ends the landing that follows at once (LLP 1102 §3.18), so only
  the hold refuses; the journal says `reorder: a drag refused: the last drop is held
  until its move shows`, and the agent's `drag to` reply carries it as `note` (LLP 1102
  §3.17). A board whose drop
  sends a mutation that `refreshes` its cards holds until storage answers, so a quick
  second drag is easy to lose (a person's, or a test's: two `drag to` steps in a row).
  Fix: in a test or drive put `clock settle` between drags. Showing the move in
  the drop's own commit (an `overlay` on the cards' source, or the board in state the
  action writes, saved through the mutation) only removes the wait for storage, which
  shortens what a person meets. (Authoring bench, LLP
  1087, r26 and r29 t4-kanban, 2026-10-05.)

## Native presentation and navigation (iOS)

- **Edge-swipe back does nothing.** Cause: the pop gesture presses the control named
  by the root's `navigationBack`, and a route that declares one that is disabled
  refuses it (the log says "back gesture refused" and why). A route with no such
  control always goes back (LLP 1115 D5). Fix: enable the control, or leave it out.
- **A pushed screen's content is cut off and never scrolls, or its large title never
  collapses.** Cause: a route is a box, not a scroller, and `navigationScroll` only
  names one; a title collapses only with the scroller right after the route's
  `header`. Fix: a `scroll id="feed" flex=1 min-height=0` right after the `header`
  and `navigationScroll="feed"` on the route (docs/contract-for-agents.md, "Routes and
  web documents"). The compiler refuses a name nothing in the route carries, or one
  on a box that never scrolls (`lower-route-scroll`). **Candidate diagnostics:** the
  compiler could refuse a named scroller that is not right after the `header` (an
  iOS-only rule today, so not refused); the hosts could journal a route whose
  content overflows with nothing to scroll it (QUEUE.md).
- **A sheet won't swipe down to dismiss.** It springs back (the log says "modal
  dismissal refused: …" and why). Cause: the sheet declares the root's `navigationBack`
  control and it is disabled, or the sheet has `closedby="none"`; a sheet with no such
  control always swipes down (LLP 1115 D5). (`ModalIOS.swift`, `refusesDismissal`.)
- **The app looks like an imitation of iOS.** Cause: controls built from boxes
  (a painted switch, buttons laid out as a tab bar or a title bar, rows drawn as a
  grouped list), and values copied from a screenshot: Apple's system colours as hex
  (`#8e8e93`, `#007aff`, `#f2f2f7`), headings as `font-size=34 font-weight=700`, a
  large title or a search bar painted in the content. Each is a tell: it misses dark
  mode, Increased Contrast and the next iOS. Fix: the native Contract forms
  ([the agent guide](contract-for-agents.md#views-layout-and-interaction), "Prefer
  native controls"): a `header` with a `role="heading" aria-level=1` (the large
  title) and an `input type="search"` (the bar's search field); a colour role
  (`-exact-secondary-label`, `AccentColor`, `-exact-grouped-background`) or nothing
  where you would write a hex. A hand-built lookalike of a system control is a bug.
  Match a reference's structure and controls, not its pixels; copy
  [`apps/shelf`](../apps/shelf/app.contract), the recipe, and run `bun
  scripts/no-tells.mjs <app>`, which lists every literal colour, font size and weight
  left. (Exact-new iOS app feedback, 2026-10-04; LLP 1115.)
- **The agent's screenshots and tree don't show the native bars.** Under
  `scripts/agent.mjs` the navigation bar, tab bar, `UIMenu`s and header search are
  not presented; the authored header, tablist and popover paint instead, by default.
  To see the bars, the tab bar and sheets as a person does, drive with `--chrome
  platform` and take `screenshot out.png window`; menus stay the agent's popovers
  there. (Signal Clone, builds 5 and 10; Splitter, rough 4 and 11.)
- **An overlay's backdrop stops at the navigation or tab bar.** Cause: content
  inside a route draws under the native bars. Fix: render full-screen overlays
  (menus, action sheets) as root children after the tab container, or as a
  `navigationPresentation="fullscreen"` route. (Signal Clone, build 10.)
- **A link in the app's own scheme never matches.** `signalclone://connect?x=1`
  reaches the root's `navigate` handler as the location `/connect?x=1`: the scheme
  is dropped and the host becomes the first path segment (`location_of`, LLP 1038
  D8). Fix: match on the path (`startsWith(location, "/connect?")`), not the URL.
  (Signal Clone, phone path.)
- **VoiceOver reads the screen behind a full-screen overlay.** A call screen
  or menu drawn as a root child above the bars hides the chat list from sight,
  not from VoiceOver: it still reached the rows and the native tab bar behind.
  Fix: `role="dialog" aria-modal=true` on the overlay's root (LLP 1080.003); on
  iOS its siblings, the tab container and its bars among them, are then skipped
  while it shows. (Signal Clone, build 13.) Not `role="alertdialog"` on content
  you lay out: see the next entry.
- **`role="alertdialog"` refuses a dialog laid out in a `column`.**
  `lower-alertdialog: … a `column` row is not text, an action or the cancel`.
  Cause: an `alertdialog` popover or `dialog` is a native confirmation (LLP
  1021, LLP 1115 D6: iOS's alert or sheet, macOS's menu), whose rows can only be text, buttons that
  close it, and one cancel. Fix: give a modal you lay out yourself `role="dialog"
  aria-modal=true`; keep `alertdialog` for a flat list of text and buttons.
  (x2apps onboarding's Delete account?, 2026-10-04.)
- **`tabIndex` on a module tag is refused.** `` `paint-surface` has no attribute
  `tabIndex`; `tabIndex` is spelled `tabindex` here ``. Cause: HTML's
  `tabindex` is now an attribute on every element and module tag's box (LLP
  1088 D7.3), with no DOM-property alias. Fix: write `tabindex` (x2apps paint,
  2026-10-04).
- **With `viewport-fit="cover"`, route content goes under the native bar.** Cause:
  the bar's cover is added to the route's padding, but a cover-fit root has no top
  safe area. Fix: put `env(safe-area-inset-top)` on the route column, not on each
  authored header. (Signal Clone, build 5.)

- **After a relaunch on iOS, a form opens with old values and ignores the fresh answer.**
  While the data module is not ready, a native host (and the wasm web target) can make
  the form's child from the resource's kept answer: a device-state reader's last small
  answer for the same arguments. A child's states start once, so the fresh answer does
  not reset them, and a write the resource did not hear about leaves that kept answer
  old. The default web JS target keeps none, so a web run never shows it. Fixes: [the
  agent guide](contract-for-agents.md#composition-and-lifetime), the form that edits a
  saved record (refresh the resource after each write, or key the child by a string or
  number from the answer). (Authoring bench, LLP 1087, ios19, ios22 and ios32
  t7-wizard, 2026-10-05/06.)
- **A grouped-list row you lay out loses its side insets, or its content runs off the
  card.** Cause: the sheet gives a custom row (a `row` that is not a title/value row)
  its padding, and `padding="12px 0"` replaces all four sides, so the row's content
  starts and ends at the card's edge; a `row`'s children are a flex row, so a `column`
  or a bar inside takes no width either. Fix: write `padding-top`/`padding-bottom` only,
  and put the content in a `column flex=1` (`apps/shelf`'s Progress and Yearly Goal
  rows), or make the row itself a `column`, which stacks its children (LLP 1116 D2).
  (Shelf recipe, 2026-10-10.)
- **A text field or `textarea` in a grouped-list row draws a rounded box inside the
  card.** Cause: a native field keeps its own border wherever it is (`UITextField`'s
  rounded rect); a settings-style form's fields have none. Fix: `appearance="none"` on
  a field that is a grouped-list row's content, as `apps/shelf`'s add sheet does.
  **Candidate fix:** the iOS host could drop the border of a field carried into a
  list cell. (Shelf recipe, 2026-10-10.)
- **A section `footer`'s colour is ignored.** A validation message written
  `color="-exact-system-red"` in a footer shows in the footer's grey on iOS: the list
  reads the footer's text, not its colour. Leave it grey (`apps/shelf` does) or put the
  message in a row. (Shelf recipe, 2026-10-10.)

## Actions

- **An `every(N, …)` task does not fire at mount.** Its first tick comes `N` ms
  after it starts, so state it fills is empty until then, after a test `reload`
  too: a "today" label or a comparison against it reads the stale or empty
  value. Fix: compute the value in a derive (from the `exactTime` source's
  `time.epochAtZero + performanceNow()`), or set it in the mount action, and let
  the task only refresh it. (Authoring bench, LLP 1087, t5-pomodoro, 2026-10-08.)

- **A token kept in an app data file.** Exact has a secret store, and it holds
  strings, not only keys: grant `secret.keep <name>` and use
  `store.set`/`store.get`/`store.forget` in an answer (the Keychain on Apple,
  `localStorage` on the web; a Linux launch forgets it at exit for now). The
  Signal clone kept its signal-cli bearer token in a plain config file because
  `secret.keep` read like the P-256 key store of LLP 1069.005. (2026-10-06.)

- **A superseded send's fetch rejects natively and completes on the web
  build.** A newer `send x = command(…)` replaces the pending one; natively
  (and in the web's wasm module realm) its `await fetch(…)` then rejects with
  a `FetchError` of kind `Aborted` though the request may have been sent,
  while on the web build (the JS target) the reply arrives and is dropped. Clear a busy flag or lock in a `finally`, and don't retry on
  `Aborted` (it would send twice); declare the mutation `queue` when every
  send's reply matters (LLP 1092). Until 2026-10-05 the continuation vanished
  natively, which held the Signal clone's sends forever (build 35).

- **`Date.now()` in a data module passes its Bun tests and fails on the
  device.** Since 2026-10-05 the build refuses a direct use by file and line
  (`Date.now()`, `new Date()`, `Math.random()`, timers), including literal
  bracket access such as `Date['now']()`. An alias or dynamic key still gets
  past the build and throws on first use on every host but Bun. Take the time
  from the call's arguments (the Contract's `wallTime.epochAtZero + performanceNow()`), as
  every source already receives it. (Signal clone build 34, 2026-10-05.)

- **A helper action does not see what its caller just assigned.** `sel = next`
  then `follow()`, with `follow` reading `sel`, would read the old `sel`: a call
  is its callee's statements in the caller's one commit, and every statement
  reads the state the action started with (LLP 1089 D2). The compiler refuses the
  read (`analyze-call-stale-read`), naming both lines. Fix: pass the value the
  helper should see, `follow(next)`, or a `let` bound before the assignment for
  the old one. (Spreadsheet F21 and Files F27 diaries, where a copied block was
  the workaround.)

- **A delete or save is lost when the page reloads right after it.** An action that
  `send`s a write and navigates away in the same commit passes every test, but a
  browser reload in the next ~100 ms comes back without the write. Cause: the write
  is the data module's, and it is done only when its mutation answers; the reload
  ends the page first. Fix: navigate in the mutation's `then`, which runs once the
  write has answered. (Authoring bench, LLP 1087, a2-contacts and t2-todo, 2026-10-05.)

- **Repeating with `pause` mutations.** A `then` cannot send its own mutation
  (`analyze-then-self-send`), and two mutations whose `then`s send each other
  are the same loop. To repeat while a condition holds, use a task with `when`
  and `every`. See "Repeating while a condition holds" in the guide.

## Sound

- **A scheduled sound plays after Stop.** A sequencer that schedules each step
  ahead (`playSounds(…)` with a future `at=`) keeps sounding the hits already
  scheduled for a beat after the Stop press. Cause: a voice is the runner's once
  its commit stands; a state change does not unschedule it. Fix: call
  `stopSounds()` (or `stopSounds(group=…)`) in the stop action: it stops what
  sounds and cancels what waits (`by cancelled`). (Drums adoption, LLP 1096 D13.)
- **The first sound on the web is silent.** A sound a page plays on load, or
  from a timer before anyone has pressed anything, is dropped and journaled
  `sound blocked: the page has had no user activation`. Cause: browsers start
  audio only after a user activation. Fix: start sound from a press (its own
  commit plays), or accept that a timer's sounds begin after the first tap or
  key. Native hosts have no such rule. (Trivia F5; LLP 1096 D7.)
- **A sequence's first hit is late when the first timer tick plays it.** A
  gated or new `every(25, tick)` first fires 25 ms after it starts, so a downbeat
  left to the tick is 25 ms late. Fix: schedule the first window from the press
  itself (`playSounds(hitsBetween(song, performanceNow(), performanceNow() + 100))` in the start
  action), then each tick the next (`[scheduledTo, performanceNow() + 100)`). (Drums; LLP
  1096 D3.)

## Media session

- **A remote pause leaves a bound `paused` false.** The lock screen's or a media
  key's pause pauses the element; an app that binds `paused` and does not mirror
  the element's `pause` event still holds `paused = false`, so its play button
  and its next commit disagree with the player. Fix: `pause=hostPaused
  playing=hostPlaying`, each setting `paused`. (LLP 1098 D3.)
- **The lock screen's skip is not your action's.** The lock screen shows
  `seekbackwardOffset`/`seekforwardOffset` (default 10), not the number in
  `seekforward=skip(30)`. Fix: keep the two equal, or take the record and seek
  by `d.seekOffset`. (LLP 1098 D2.)
- **On iOS a media session needs a playback session and background audio.**
  `build.mjs --ios` refuses a `metadata=` without `"audio_session": "playback"`
  and `"audio"` in `host.ios.backgroundModes`: the lock screen shows only a
  playback session's media, and audio stops at the lock without the mode. (LLP
  1098 D8.)

## Input

- **A keyboard shortcut does nothing.** Two causes, both as on the web.
  `aria-keyshortcuts` belongs to a node that must be on the screen: a button with
  `display="none"` (a "hidden" shortcut holder) is not, so its key never fires; give
  the key a visible control (a status line of keys, as the LLP reader's) or handle
  it in a `key=` handler. And Shift is part of the shortcut: `n` is the unshifted
  key, a capital `N` is `Shift+N`, so write `aria-keyshortcuts="Shift+N"` for it (in
  a terminal too, where a typed capital is reported as `Shift+N`).

- **A hold's `pointerup` never arrives.** Cause: the press started on a node that a
  state change replaced during the hold; the up is not delivered to a node that no
  longer exists (declared in LLP 1005). Fix: keep the pressed node across the state
  change (change its contents, not which branch renders it).
  **Candidate diagnostic:** the runtime could log a dropped up in development.
- **A `transformDragFor` handle does nothing.** No pinch or drag follows and
  nothing is logged. Cause: the binding resolves only for a strict shape and
  otherwise returns none silently (`kernel/src/transform.rs`; only the agent's
  transform-drag command reports "no photo binding"). The target, the handle's one
  ancestor with that `id`, needs
  `width="100%" height="100%" box-sizing="border-box"`, no padding, border,
  margin or offsets, and its direct parent needs `overflow="hidden"` on both axes
  and no padding or border; every other ancestor must be untransformed. (Signal
  Clone, build 11: the photo viewer lacked `overflow="hidden"` and
  `box-sizing`.) **Candidate diagnostic:** the compiler or a development log
  could name the failed condition.

- **A field normalized while the person types fights the typing.** A `task … when
  people != "${count}"` (or a timer, or an `input` action) that rewrites an
  emptied field to `"1"` lands between keystrokes: clearing the field and typing
  `3` reads `13`, for a person as for a test. Fix: keep the raw text while
  editing and normalize in `change` (Enter or blur), as the guide's "Editing a
  value: the field's contract" shows. (Authoring bench, LLP 1087, t1-tip,
  codex, 2026-10-07: per-person share 8.85 for 3 people, because the field read
  13.)
- **A field's `change` fires when a tap elsewhere blurs it.** A dialog that adds
  a word on `change` adds it the moment the person taps a duration or another
  control, before they meant to submit. Cause: `change` is the commit event, and
  a text field commits on Enter and on blur, as in HTML. Fix: submit on `submit`
  (Return, a form's button) and keep `change` for normalizing the draft.
  (The Bluesky clone's muted words dialog, 2026-10-08.)
- **A text field shows an edit its action refused or normalized.** A field bound with
  `value=text input=edit`, where `edit` ignores a blank value, shows the blank while
  `text` keeps the old value, and the next keystroke builds on what is shown; so does
  one whose action or source normalizes `-2` to the `0` it already held. Cause: on
  the web (both targets) a text field is re-set only when what its binding reads
  changes, so an unchanged binding does not overwrite the edit. Fix: bind the field to draft state that `edit` always writes, and on commit
  (`change`, Enter, `blur`) write the accepted value or reset the draft to it, which
  changes the bound value and redraws the field; the guide's "Editing a value: the field's contract" has the recipe. (Authoring bench, LLP 1087, t2-todo:
  two builders, about 10 minutes each, 2026-10-04; t1-tip, a normalized count,
  2026-10-05.)

- **A checkbox bound to a resource field does not tick until the save answers.** With
  `checked=form.terms change=editTerms`, where `editTerms` sends a mutation that
  `refreshes form`, a click shows the box unchecked again while the save's answer is
  out and checked only when the refreshed answer lands, so a slow store shows no tick,
  and a test that clicks and reads `checked` before the answer fails. Cause: every host (both web targets, iOS,
  macOS) re-sets the box to its binding, `form.terms`, which is still `false` until
  the answer. Fix: give the mutation `refreshes form` and export an `overlay` that lays
  the pending save over `form`'s answer (the agent guide's "Optimistic writes: the
  overlay"), so `form.terms` reads the new value from the click until the refreshed
  answer has it; or bind the box to state the
  action writes at once (`terms = value`, then `send`), seeded from the saved record as
  a form does. (Authoring bench, LLP 1087, codex17 t7-wizard, 2026-10-05; the overlay
  checked on the web with a save that never answers, 2026-10-09.)

- **A test `drag` is a touch unless `mouse` is set.** `tap "chart" drag 20 0`
  is a finger (`pointerType` `touch`) on the web, so a `pointerup` that treats
  a touch as the finger leaving clears the hover the next assertion still wants.
  Fix: on touch-up, end the drag and leave the hover, or write `mouse` for the
  left button. iOS refuses `mouse`; macOS and Linux drag with the mouse anyway
  ([authored tests](contract-grammar.md#authored-tests)). (Stocks diary: the chart
  readout unmounted, about 10 minutes, 2026-10-04.)

- **A `pan` hears nothing from a finger on the web.** A drag with
  `tap <id> drag dx dy` (or a real touch) moves nothing and logs nothing. Cause:
  without `touch-action="none"` on the pan's box the browser takes the touch for
  scrolling and the pointer events are cancelled. Fix: `touch-action="none"` on the
  dragged box (only that box, so the page still scrolls from elsewhere). (Authoring
  bench, LLP 1087, t4-kanban: about 15 minutes, 2026-10-04.) **Candidate
  diagnostic:** the compiler could warn on a `pan` without `touch-action`.

- **There is no `swipeleft` for swipe-to-delete.** A row built from `pan`,
  `panrelease` and `translate` reveals its Delete button, but by hand on every host.
  Cause: `swiperight` is the reply gesture (a message bubble), not a direction pair;
  the row whose leading or trailing actions a swipe reveals is a horizontal `scroll`
  with `scroll-snap-type="x mandatory"`, its content and action buttons as snap
  children, naming them with `swipeContent`, `swipeLeading` and `swipeTrailing` ids,
  which the web scrolls and iOS turns into UIKit's own swipe actions. Fix: copy
  `apps/shelf/app.contract`'s `BookRow` (`swipe-…`; `apps/messages` paints its own
  circles in hex and is no model for the rest). (Ledger2 DIARY, "Needed:
  swipe gesture", about 15 minutes, 2026-10-04.) iOS refuses a row whose content is
  not exactly the scroll's size, and `logs` says which rule failed (`swipeContent on
  #243 is refused: swipeContent "row" is 390x68, not the row's 390x68.5 border box`):
  a border on the scroll itself shrinks it, so put a hairline on the content (Splitter
  DIARY, rough 10).

- **A `swiperight` hears nothing from a finger on iOS.** A mouse drag fires it on
  the web and macOS, in a drive and in a test, while a real touch on an iPhone
  does nothing. Cause: with `touch-action` at `auto` a horizontal pan is the
  platform's, as in a browser, so the swipe never begins. Fix:
  `touch-action="pan-y"` on the swiped node, which leaves vertical scrolling to the
  page. Messages also gives the bubble `transition="translate -exact-spring(300, 30, 1)"`,
  which moves it with the finger; that does not arm the gesture. (Chat2 DIARY,
  which credited the transition, about 20 minutes; reproduced with `agent ios
  --touch platform`, 2026-10-04.) **Candidate diagnostic:** the compiler could
  warn on a `swiperight` without `touch-action`, as on a `pan`.

- **A dragged piece snaps back, lands on the wrong square, or a tap moves it.** A
  board has no drop target; build the drag from the pointer events (Chess DIARY,
  about 30 minutes; this recipe driven on the web and macOS, 2026-10-04):
  - Each square takes `pointerdown`, `pointermove`, `pointerup` and
    `touch-action="none"` (a finger otherwise scrolls and the events are cancelled).
  - The square that took the `pointerdown` holds the pointer: its `pointermove`s and
    its `pointerup` come wherever the pointer goes, with `offsetX`/`offsetY` measured
    from that square. The drop square is the held one's column plus
    `floor(e.offsetX / cell)`, and its row plus `floor(e.offsetY / cell)`.
  - Start a drag only past a threshold (`dx * dx + dy * dy > 64`), so a tap stays a tap.
  - Draw the ghost as a later sibling of the squares inside a `position="relative"`
    board (`position="absolute"`, `pointer-events="none"`): `z-index` orders siblings
    only, so a ghost inside a square cannot float over the next one.
  - Keep the piece in its square while it is dragged, dimmed with `opacity`.

  ```text
  action down(i: number, e: PointerEvent)
    from = i
    dragging = false
  action move(i: number, e: PointerEvent)
    if from >= 0
      dx = e.offsetX - half
      dy = e.offsetY - half
      dragging = dragging or dx * dx + dy * dy > 64
  action up(i: number, e: PointerEvent)
    if dragging
      drop(from, from + floor(e.offsetX / cell))
    from = -1
    dragging = false
  ```
- **A write left running after a source answers can be lost on iOS or macOS.** The
  web kept it; the native host did not, and a list was empty after a relaunch. Cause:
  the native data executor runs a source's promises while a request waits on them,
  one request at a time, so a `promise` started and not awaited (a fire-and-forget
  SQLite write) can stay unfinished, and an answer queues behind a request still in
  flight. Fix: await the write before answering, or carry it in a
  request of its own that the view sends (a `flush` source called with the change).
  (Authoring bench, LLP 1087, t2-todo on iOS: about 20 minutes, 2026-10-05.)
- **`showPicker("x")` names the input's `id`, not its `testId`.** With only
  `testId="x"` the press opens no picker, and a drive's `type @x photo.jpg`
  answers `no held device request … (held: none)`. Fix: give the
  `input type="file"` both, `id="x" testId="x"`. (Interview's profile photo,
  LLP 1108, 2026-10-08.)

## Driving and testing

- **An alert's action has not run by the next step on iOS.** `tap "remove-confirm"`
  then `clock data` finds nothing removed, though a drive shows the press. Cause:
  UIKit runs an alert button's handler after the alert has gone, a transition the
  tap does not wait out. Fix: `clock settle` after tapping an alert's button (and
  after the action that shows it, before tapping in it), as `apps/shelf`'s tests do.
  (Shelf recipe, 2026-10-10.)
- **`xcrun simctl ui <udid> appearance dark` does not darken an agent drive.** The
  driver sets the media preferences itself. Fix: `"prefer prefers-color-scheme dark"`
  as the drive's first operation. (Shelf recipe, 2026-10-10.)

- **Asserting a boot loading state against a live backend is a race.** The
  real reply lands on real time, and under load it can arrive before the first
  `expect`, so `expect tree has "loading"` passes once and fails once. Assert
  loading against a stand-in server that answers late, never by racing the real
  one; `fail fetch` (LLP 1103) tests the error state and its retry, not loading.
  (Authoring bench, LLP 1087, t3-recipes, Studio, 2026-10-08.)

- **On the web, a save still in flight when the tab closes is lost.** A
  `storage.fs` write is one IndexedDB transaction, and the browser drops a
  transaction still running when the page goes away (an immediate close after
  Enter lost it 4 of 4 times; native hosts finish it, LLP 1097 D10). In a test,
  let saves land (`clock data`, or wait for the saved state) before `reload`,
  `relaunch` or closing. In the app, show a saving state until the write
  resolves. (Authoring bench, LLP 1087, t5-pomodoro on the Android round,
  2026-10-08.)

- **A test fixture that patches `window.fetch` after boot changes nothing.** A
  data source's `fetch` is captured when the data module loads, so a stub
  installed later (a browser script's `page.evaluate`, a console patch) never
  sees its requests. Fix: to test a failure, `fail fetch "<url prefix>"` (the
  guide's testing section); a fixture that must stand in for the network
  installs its stub before the document loads (`addInitScript`) or serves a
  stand-in server. (Authoring bench, LLP 1087, t8-library, codex, 2026-10-08.)

- **`xcrun simctl io booted screenshot` can capture the wrong simulator.** With
  several simulators booted, `booted` names any one of them, not the one the
  app runs on. Fix: use the UDID the build prints (`… on iPhone 17 <UDID>`), or
  the agent's own `screenshot`, which targets the app's simulator. (Authoring
  bench, LLP 1087, t9-profile, 2026-10-07.)

- **`axe tap` misses on an exact2 app about one time in three.** Its default
  style (FBSimulator `tapAt`) sometimes lands as nothing: a mute button toggled
  1 of 3, a drawer button did not open, and the app looked frozen until
  relaunched. Fix: `axe tap … --tap-style physical` (a real touch down and up),
  which hit 6 of 6; or drive by `testId` with the agent. (Bluesky clone, b12,
  2026-10-08.)

- **Records an agent drive writes are dated 2026-01-01.** The driver's clock
  starts at a fixed epoch, so `wallTime.epochAtZero + performanceNow()` is that date, and a
  `createdAt` taken from it is too: Bluesky's AppView then sorted the clone's
  test posts out of the author feed, and they turned up only through search.
  Fix: `--epoch now` (the machine's clock, read once at launch; a test file's
  `epoch now`) on any drive or test that talks to a real service, `snapback4 dev`
  included: a backend on real time otherwise sees the app's dates months early
  ("Due in 403783 min"). (Bluesky clone, b12, 2026-10-07; app farm round 2.)

- **`screenshot` of a sheet is the sheet alone, at full width.** On iOS it
  captures the presented route (a fit-content repost sheet came out 402 × 270
  points), so its type looks twice its size when viewed as a screen. Fix:
  `screenshot out.png window` for the screen as a person sees it. (Bluesky clone,
  b12, 2026-10-07.)

- **A screenshot right after a state change shows a transition's start.** A
  `transition` (a background colour, an opacity) is held by the driver's clock,
  so the frame and the computed style still read the old value: the toggle
  looks unchanged. Fix: `clock settle` (or `clock +N` past the transition)
  before `screenshot`. (Authoring bench, LLP 1087, t5-pomodoro, 2026-10-07:
  about 5 minutes and a probe script to find.)

- **A test passes on the web and fails on iOS right after an input that saves.** An
  `expect` straight after `type` or `tap` reads what the input's mutation answered,
  but an input step only finishes the `then`s of answers already settled; it does not
  wait for outstanding storage ([authored tests](contract-grammar.md#authored-tests)).
  A fast web reply (the web input waits two frames) can make the `expect` pass while
  the native reply is still pending. Fix: put `clock data` after the input, before the
  `expect` that reads what its reply sets.
  (Authoring bench, LLP 1087, ios23 t5-pomodoro, 2026-10-05; iOS round 6.)

- **An `iframe` of `http://` from a named host loads under the agent and shows an
  App Transport Security error in the macOS app.** `http://localtest.me:5173/` or
  `http://example.com/` read "The resource could not be loaded because the App
  Transport Security policy requires the use of a secure connection" in the `.app`
  (an IP literal such as `http://127.0.0.1` loads). Cause: ATS reads the bundle's
  `Info.plist`, and `agent macos` runs the bare executable, which has none. Fix: set
  `host.macos.appTransportSecurity` (and `host.ios.…` for iOS) to
  `{ "allowsArbitraryLoadsInWebContent": true }` in `app.json`; it relaxes web views
  only. On iOS, the host's wrapper for a remote HTTP page also uses HTTP:
  an HTTPS wrapper would still block that page as mixed content after the ATS
  opt-in. The inner iframe keeps its sandbox and its authored dimensions.
  To drive what a user sees on macOS, build with `bun exact.mjs mac --bundle` and set
  `EXACT_MAC_BIN` to the `.app`'s `Contents/MacOS/ExactMac`; the driver then skips its
  stale-build check, so rebuild the bundle before each drive. An app's own page
  (`src="assets/…"`) is served at `http://exact.localhost`: its `http:`
  sub-resources on loopback load, and one on a named host follows the same ATS
  setting. A sub-resource that fails is logged (`exact: iframe assets/…: <url>
  did not load (<reason>)`) (#135).
  (Issue #106, 2026-10-06.)

- **A focus ring never shows in an `agent macos` screenshot.** `type save key Tab`
  moves the focus (`state` reads it in `focus.logical`), yet neither `screenshot`
  nor `screenshot … window` shows AppKit's ring. Cause: the agent's app is an
  accessory whose window is ordered front but never key, and AppKit draws a focus
  ring only in the key window; nor can a script activate a dev build here (macOS
  refuses `activate` from the background). Fix: read the focus from `state`, and
  for the ring itself make the presenter's window an `NSPanel` with
  `.nonactivatingPanel` in an XCTest (it becomes key without activating the app)
  and read its pixels with `CGWindowListCreateImage`. (Issue #179, 2026-10-07.)

- **A drive script kept in the app folder makes the build stale.** Editing
  `verify.mjs` beside `app.contract` made the driver refuse the next drive until
  `bun exact.mjs web-build`. Cause: a file in the app folder counts as a build input
  unless it is an output (a screenshot, a log) or git-ignored outside the input trees
  (`data/`, `web/`, `assets/` and the like count even when ignored); any `.json`
  counts, since the bake captures it. Fix: keep drive scripts, evidence, logs and
  runtime files in the app's `.exact/` (no build, dev-loop watcher or freshness
  check reads a dot directory at the app's root), or outside the app folder.
  (Authoring bench, LLP 1087: five builders, 2026-10-05; Depot's evidence JSON,
  2026-10-05.)

- **A latency test passes at once, or a reply never lands.** Cause: every drive
  holds the app's clock, in every browser and on every host: `clock +N` moves the
  app's time and nothing else, while a `fetch`, a storage call or a stream's next
  message arrives on real time. Fix: `clock settle` to land what is in flight;
  `clock +N real` to let N ms of wall time pass with the clock moving alongside
  (polling, a server push, a measured latency). `state` lists what is pending.
  (Depot on three backends, 2026-10-05.)

- **A date input reads `10/05/2026` beside a label the app wrote in UTC.** Cause:
  `input type="date"`/`"time"` show the browser's own locale format and mean a
  local wall time, as on the web; the app's label used another zone. Fix: label in
  the viewer's zone, or pass `--locale` and `--time-zone` on a drive (`locale` and
  `time-zone` lines in a test file) so both agree and the run stays reproducible.
  (Depot, 2026-10-05.)

- **An iOS screenshot right after a tap shows a segmented control on its old
  segment.** The tree says the new one is selected. Cause: UIKit animates the
  selection on real time, and `clock +N` does not move it. Fix: `clock +1000 real`
  before the screenshot (it moves the app's clock that second too, so a timer due in
  it fires). (Authoring bench, t1-tip on iOS, 2026-10-05.)

- **The same test passes on the web and fails on iOS at a date past `max`.** Cause:
  the runner refuses a date, time or datetime outside `min`/`max` (the agent says the
  date was refused) on iOS, macOS, Linux and the wasm web; the JS web target keeps the
  value as a browser's date input does. Fix: test values inside the range, or the
  bound itself. (Authoring bench, t7-wizard on iOS,
  2026-10-05.)

- **The agent taps the simulator by screen coordinates** (`axe tap -x -y`,
  `simctl`), and the drive breaks whenever layout moves. Cause: the controls have
  no `testId`, or the driver was not used on iOS. Fix: give every control a
  `testId` and drive with `bun exact.mjs agent ios tree "tap <testId>"
  "screenshot s.png"`; find targets with `tree`, or `tree --ax` for the
  accessibility tree. (Exact-new iOS app feedback, 2026-10-04.)

- **`build.mjs --ios … --phone <udid>` installed onto someone else's simulator.**
  Cause: a simulator build reads `--sim` (or `EXACT_SIM`); `--phone` is the
  device's, so it fell back to a booted iPhone, another drive's
  (2026-10-09). `agent ios` and `smoke.mjs ios` choose the same way. Now a
  choice among several booted iPhones is refused, naming the caller's way to
  choose; fix: always pass
  `--sim <udid>` to `build.mjs` and set `EXACT_SIM=<udid>` for the agent and
  the smokes.

- **`tap <row>` pressed a link card, Like or Repost inside the row.** Cause: a
  plain tap aimed at the row's middle, and a finger there presses the deepest
  control (the Bluesky clone liked and reposted real people's posts,
  2026-10-09). Now a tap that names a node presses that node: one with its own
  `press` is pressed beside the control its middle holds (the reply's `avoided`
  says so, `at` where it landed), or refused when no point of it reaches it; one
  without a `press` refuses rather than press a control inside it
  (`tap post-0 would press card-0 inside it; tap card-0, or tap post-0 at <x> <y>`).
  Fix: tap the control by its own `testId`, or `tap <row> at <x> <y>` (a point
  from its top left) for whatever a finger there reaches; name a point too when a
  refusal says no point reaches the row but a thin strip of it does (the search
  is a grid). Every host (LLP 1012 §1).

- **Every date in a screenshot is 1 January 2026** (31 December 2025 west of UTC).
  Cause: the agent's clock starts at `2026-01-01T00:00:00Z`, in UTC. Fix: `--epoch <ISO time> --time-zone <zone>` on
  `scripts/agent.mjs` for dates that read as intended and stay reproducible; in a test
  file, `epoch "…"` and `time-zone "…"` lines, so a run without the flags still means it.
  Against a live backend, `--epoch now` (`epoch now`) instead.
- **A simulator measurement shows a 100–200 ms stall the app never makes.** A
  plain launch's frames hold 16.7 ms, but a run driven with `axe` shows one
  stall with no batch applied about 0.4 s after `axe` first reads the screen
  (`describe-ui`, `swipe`, `tap` by label), and every push afterwards is 30+ ms
  slower. Cause: an accessibility client turns on UIKit's accessibility runtime
  in the app; Time Profiler puts the whole stall in
  `-[UIApplication _accessibilityInit]` (loading the accessibility bundles,
  starting the server), and from then on UIKit keeps accessibility state for
  every view it moves. A phone pays this only with an assistive technology on
  (VoiceOver, Voice Control, Switch Control). `xctrace` and `sample` attached to
  the app stall it too. Fix: time a plain launch; drive taps with `axe touch`
  (HID, no accessibility) or have the host press a row itself, and read frames
  from Save Trace or a temporary log, not from a run an accessibility client
  touched. (x2-perf, 2026-10-05.)
- **`axe` stops delivering taps.** Use `axe` only as a last resort, for native
  chrome only a normal launch presents (bars, `UIMenu`s); drive everything else
  with `agent ios` by `testId`. After `axe touch --down --up --delay` (a long
  press) or an `axe drag`, a following `axe tap` often reaches no window; it is
  intermittent, and a native bar button can miss the same way with no gesture
  before it. `axe touch --down --up` lands more often, not always. Fix: use the
  agent's own taps where it can; with `axe`, tap by touch, check every step with
  `axe describe-ui` before the next, and `xcrun simctl shutdown` / `boot` the
  simulator when taps stop landing. `axe` also cannot press tab bar items or
  `UIMenu` rows. (Signal Clone, builds 10 and 11; reproduced on `05d0c576e`.)
- **A `UISwitch` does not flip under `axe tap -x -y`.** Cause: a tap by
  coordinates resolves no element, so `--tap-style automatic` sends the
  simulator's own tap, a touch with no duration, and iOS 27's `UISwitch`
  ignores it: a bare UIKit app's switch does not flip either. A grouped
  list's toggle (LLP 1084) looks broken while its app logic is fine. Fix:
  `axe tap --id <testId>` (a switch element gets a physical touch),
  `axe tap -x <x> -y <y> --tap-style physical`, or
  `axe touch -x <x> -y <y> --down --up`; or the agent's `tap <testId>`.
  (Signal Clone Privacy, 2026-10-04.)

- **The software keyboard never shows on a simulator that drives have used.**
  A field takes focus (its caret blinks) but no keyboard rises, and
  `keyboardWillShow` never fires, so a keyboard-riding toolbar cannot be
  measured. Cause: after agent and `axe` drives the simulator was in
  hardware-keyboard mode, likely left by the HID input they inject; a
  headless simulator has no Simulator.app setting to show. Fix: reboot it
  (`xcrun simctl shutdown <udid>; xcrun simctl boot <udid>`), or toggle
  Connect Hardware Keyboard where Simulator.app is installed. (Signal Clone
  keyboard timing, 2026-10-05.)

- **A storage test fails with `storage is busy`, or storage is "unavailable in
  agent mode".** Cause: a drive has no storage unless it names a scratch store, and
  an open SQLite database locks its file, so a mutation and the refresh it triggers
  collide. Fix: `--storage <name>` on an `agent` drive (authored tests get a
  store of their own), and queue every `storage.sqlite.open` in `app.ts`
  ([the human guide](contract-for-humans.md#writing-the-data-module) shows one).
  (LLP 1086 reading-list example, 2026-10-04.)

- **An agent drive shows the app's defaults (a mock, an empty store) though
  the app's files are there.** Cause: without `--storage <name>` every
  `storage.fs` call in the data module throws "storage is unavailable in agent
  mode…", and a module that catches a missing config file falls back silently
  (the drive says so once on stderr, `note: a data source was refused storage`, on
  every carrier: beside the op on the web, at the drive's end on a native one).
  The installed app's own files are not the drive's: a named scratch store lives
  apart (on iOS under `Library/Caches/exact/<app id>/agent/<name>/data`). Fix:
  `--storage <name>`, and copy the files the drive needs (a config, a saved
  store) into that folder first; `logs` shows the module's `console.log`.
  (Signal clone, live transport, 2026-10-05.)

- **A save that fails in the background is lost to the person.** An answer that
  saves unawaited has replied before the write fails, so no answer reports it;
  `logs` has `storage failed: …`, but the person sees nothing. Fix: keep the
  error in the module (`.catch((e) => { saveError = e.message; })`) and say it
  in the next answer. (LLP 1097, x2apps drums.)

- **A read misses a save chained on a promise.** `saving = saving.then(() =>
  storage.fs.atomicWriteFile(…))` issues the write only when the one before it
  lands, so a read issued meanwhile runs first and sees the older file. Cause:
  the module's storage runs in the order it was issued. Fix: call storage in
  the answer (`storage.fs.atomicWriteFile(…).catch(note)`) and let the queue
  order it. (LLP 1097 §1.)

- **Two awaited writes of one answer had another's write between them.**
  Answers interleave at their awaits, as two async calls do on the web. Fix: put
  writes that must stay together in one `transaction` (SQLite) or one operation.
  (LLP 1097 D4.)

- **A database stays locked after the answer that opened it failed.** An
  answer that opens a database and fails, or that the runner let go, leaves its
  chain running in the background; if that chain throws before `db.close()`,
  the handle stays open and every later open finds the database busy. The host
  does not close it for you, since an app may keep or share a handle across
  answers. `logs` says `storage: app:/data/x.db is still open after a failure in
  background work that opened it`. Fix: close in a finally,
  `try { … } finally { await db.close(); }`. (LLP 1097 D7, Charlie,
  2026-10-07.)

- **A native button refuses an image's frame or a third text.** Its face is
  semantic: two texts (title and subtitle) and one symbol. The symbol's size
  is `font-size` on the image, not `width`, `height` or `object-fit`. Its own
  colour is `-exact-tint-color`; the title's is `color`. Use a custom `button`
  without `appearance="auto"` for aligned image frames or arbitrary children.
  `-exact-control-size` and `-exact-corner-style` are admitted only on an
  explicit native button, including through classes. (LLP 1069.011.001 D12–D14.)

## Working on exact2 itself

- **A bisect that shares another worktree's Cargo target directory builds
  stale code.** `CARGO_TARGET_DIR` pointed at one worktree while checking out
  older commits in another left generated enums (`PropId`, `Stdlib`) from the
  wrong commit, and the build failed for no reason in either tree until a
  full `cargo clean` (65 GiB). Give a bisect or a second worktree its own
  target directory. (2026-10-06.)

- **A platform feature looks missing, and you start building it.** Cause: the
  feature already exists under a name you did not search for. Haptics
  (`haptic()`, `-exact-press-haptic`) were proposed as a new gap after they had
  landed. Fix: before calling something missing, search
  `docs/contract-for-agents.md` and the LLP index (`ls llp/`, then `grep -ril
  <term> llp`). Name the LLP that lacks it when you report the gap. (Signal
  Clone, 2026-10-04.)

- **`build.mjs --test --ios` never returns after the tests pass.** Cause:
  `xcodebuild test` can sit for ten minutes or more after `Test Suite 'Selected
  tests' passed` and its `Executed N tests` line, with or without your change
  (seen on 2026-10-05 on an iPhone 17 Pro simulator, Xcode 27). Fix: run it in
  the background with its log in a file, wait for the `Executed N tests …
  seconds` line of the whole run, read the verdict from it, then kill the
  `xcodebuild test` PID whose `-derivedDataPath` is under your own checkout.
  `build.mjs` then reports `BUILD INTERRUPTED`, which is not a test failure.
- **Conformance fails on apps you didn't touch.** Cause: `host/web-js/conform.mjs`
  compares against wasm dists under `--wasm-root` (default `/tmp/e3-wasm`, shared by
  every checkout), and without `--build` it uses whatever another checkout or an
  older commit left there. Fix: name the apps and pass `--build` with a
  `--wasm-root` of your own (`conform.mjs <app> --build --strict --wasm-root
  /tmp/<yours>`). With no apps named, an empty root compares nothing and passes.
- **A web size or speed number is inflated.** Cause: a development web build
  carries the agent adapter, install pages and the source map. Fix: measure the
  release: `host/web-js/build.mjs <app> --plan <wasm bake>/app.plan --production`,
  as `scripts/deploy.mjs` builds it.
  The current `metrics.mjs` app.js gate is a different measurement: it calls
  `host/web/build.mjs <app>-web` without `--production`, and prints its three
  app.js lines only with `--long`. Reproduce that invocation when investigating
  a gate violation; a release number cannot be substituted for it.
- **Host code must never set a scroll offset during a pan or fling.** An absolute
  `contentOffset` write while `isTracking`/`isDecelerating` cuts the reader's
  motion: frame rate holds, the motion is wrong ("janky but not dropping frames").
  Defer follows to the end of the drag or deceleration, and apply anchoring and
  estimate corrections as relative adjustments in the same layout pass (Signal
  Clone evening of 2026-10-02; `32805146`, `c03685dc`).
- **A copied Core Animation tree renders blank.** Calling `CALayer(layer:)`
  directly gives an empty layer: a measured copy had zero bounds and no fill
  or sublayers. For a capture, copy values into fresh layers and recursively
  copy children and masks. Keep the live hierarchy intact. (LLP 1083.000, Apple A2.)
- **An sRGB capture test changes the pixel it reads.** AppKit's
  `NSBitmapImageRep.colorAt` returns calibrated RGB even when the bitmap is
  sRGB. Converting that `NSColor` to sRGB again turned measured bytes
  `[128, 0, 127, 255]` into about 58% red and 57% blue. Compare the bitmap's
  components or bytes in its declared color space. (Apple A2 capture test.)
- **A canvas capture still shows the previous order after ranks flush.**
  Cached shadow layers are plain `CALayer`s: changing their `zPosition`
  outside a disabled-actions transaction implicitly animates the depth.
  Flush ranks before capture and disable actions for that flush, including
  mirror writes. A same-batch texture upload then sees the new front sibling.
  (LLP 1083.000, Astra 6 regression.)
- **A sub-agent's half-written crate breaks every build in the worktree.**
  Cause: a crate listed in the root `Cargo.toml`'s `members` is resolved by
  every `cargo` command, so one that does not parse or compile yet stops
  builds that never touch it (about ten minutes of the harness's build, LLP
  1101.002 §0 P15). Fix: list it in `exclude` while it is written, which lets
  `cargo build --manifest-path <it>/Cargo.toml` build it alone, and move it
  to `members` once that passes.
- **"I opened it in a terminal" is not "it is running".** `open -na
  Ghostty.app --args -e …` can return success while the window reports "The
  terminal failed to initialize". Before telling a person the app is up,
  confirm its process (`pgrep -f <binary>`) and kill a failed window's
  instance before retrying (LLP 1101.002 §0 P16).

- **An external native app rebuilds on every unchanged direct Cargo invocation.**
  Cargo treats a missing optional `assets`/`deck` input as perpetually dirty;
  watching its parent recursively would also watch app-local build outputs.
  Use the host builder (`bun exact.mjs windows` for a generated Windows game).
  It recomputes `EXACT_ASSET_ROOTS` before each Cargo invocation; the bake watches
  that inventory for first creation and existing roots for content changes.
  Do not set this variable to a fixed hand-maintained list for direct Cargo.

- **macOS helpers belong in a native resource tree, not `assets/`.** Assets are
  baked update bytes with portable names and capture limits. To ship a helper
  and its package tree, keep them in `server/` beside `app.json` and declare
  `"host": { "macos": { "resources": [{ "from": "server", "to": "Resources/server" }] } }`.
  `bun exact.mjs mac --bundle` copies the tree to `Contents/Resources/server`.
  Executable modes, spaces, `@scope` names and relative links within the tree
  survive; files above 64 MiB are allowed. Source and asset roots cannot be
  used as native resource roots. Links must resolve inside the declared tree.
  The tree is excluded from TypeScript capture and web assets. Its files,
  modes and link targets are binary inputs, so changes require a new binary.
  Snapshot-based delivery captures this tree, including ignored dependencies;
  use `--dirty` when those files differ from the committed source. Mach-O helpers and libraries are signed in
  the bundle, which changes their signature bytes; other files stay identical.
  `exact release` signs these files with the release identity before sealing
  the outer bundle. This field is macOS-only. (Issue #103, 2026-10-07.)

- **A background, border or radius makes a text field your own box.** A text
  field is the platform's own by default. Any such row, including a shorthand,
  a class row or a row on just one conditional arm, makes it bare for its whole
  lifetime. Write `appearance="none"` explicitly when drawing the field yourself.
  With `appearance="auto"`, those rows are refused; remove them to keep the
  platform's background, border and corners. `background-clip` and
  `background-attachment` alone keep the field native. (LLP 1104 r8 D2.)

## Access hatches

- **A view a hatch adds on macOS hears no click while its window is not key.**
  The click reaches the view by hit test and nothing happens: AppKit spends a
  first click on activating the window unless the view says otherwise, and an
  ancestor's click recognizer holds a plain `mouseDown` back. Give the view
  its own `NSClickGestureRecognizer` (declared with `element.owns(recognizer:)`)
  and override `acceptsFirstMouse(for:)` to return true, as Exact's own
  controls do. Found building the fixture's badge seal (LLP 1075.003.000.001
  §13.10): `tap <node>/<part>` answered `landed: "part"` and the press was
  never counted.
- **An absolutely positioned element a web hatch appends is not where its
  node is.** `position: absolute` resolves against the nearest positioned
  ancestor, which is rarely the hatched node, and a hatch must not restyle the
  node to make it one. Put what you add in the node's own flow (a block with a
  margin). Found the same way: `tree` showed the part's size right and its
  place elsewhere, and the aimed click was refused as outside the node's box.
- **A page module that exports `window` loses the global of that name.**
  `export function window(w)` is the window hatch, and inside that file
  `window.innerWidth` is then the function's property. Reach the global as
  `globalThis` (the hatch's `w.window` is it too).
- **On Linux, Windows and Android `changed` also means the node's size
  changed.** A hatch that acts whenever `element` is called again (a click, an
  input) loops when its act changes the node's own size, a label's width, say.
  Keep the words the node was last told with and act only when they differ.
  Found building the fixture's Linux hatches (§13.13): the pressing hatch
  pressed itself forever once its label grew a digit.

- **A background, border or radius makes a default button your own box.**
  Rich children and rows the native button refuses do too, including on one
  conditional arm or through a class. `buttonStyle` then refuses with
  `lower-button-style` and says why. Remove the named reason to keep the
  platform's button, or write `appearance="none"` without `buttonStyle` for
  your own box. Explicit `appearance="auto"` admits a radius as a native
  content field but refuses backgrounds, borders and unsupported children.
  (LLP 1104 r10 D1, D2.)

- **A projected segmented control can widen after its tabs become native
  buttons.** The projection supplies the tablist's minimum height and fills
  its content box; the authored children still decide that box's width.
  Native button insets then contribute even though the segmented control
  draws the face. If the row was designed around bare text tabs, write
  `appearance="none"` on those tabs. Native Fixture's header needed this
  after buttons became native by default. (LLP 1059 D2a, LLP 1104 D9.)
