# Scroll: restore a top-level virtualized list, scroll-padding and scroll-margin on any scroller, native smooth element jumps and scrollend (rest of #138)

**Status:** Open
**Systems:** Contract, runner collections, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/277

## Current scope

Amend LLP 1010/1070 for standard padding/margins, native smooth element jumps and scrollend. Use selected app-owned first-visible key/offset restore, not original automatic unmount caching. Check reduced motion and real-time settlement.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #138, split out because #138 closed when plain-scroll anchoring landed (#210). Three scroll capabilities remain, and two of them are decided against by accepted LLP text, so they need a design ruling first:

- **(a) Restoring a top-level virtualized list.** A list that is unmounted and mounted again (switching between documents or conversations in one pane) opens at row 0. Nested lists keep their anchor (the first visible key and the offset into it) under `scroll-restoration="auto"`; a top-level list does not.
- **(b) `scroll-padding` on any scroller, and `scroll-margin`.** Since the 2026-10-07 addendum to LLP 1010, `scroll-padding-*` is taken on a `list virtualized=true` only; on a plain `scroll` it is refused. `scroll-margin-*` is not carried anywhere.
- **(c) Smooth element jumps and `scrollend`.** On native hosts the element form `scrollIntoView(id, behavior="smooth")` lands at once (the list form animates). There is no `scrollend` event, so an app cannot tell when a jump has settled.

### Current and expected behavior

Current, on main `0365ad1a4` (the compiler's `collection.rs`, `IntoView.swift`, LLP 1010 and LLP 1070 are unchanged on `e200397ec`):

- (a) A list scrolled to `scrollTop` 1500 (top row `l-18`, 61 px into it), toggled off and on, opens at `scrollTop` 0 with `l-0` at the top, on the web and macOS, with `scroll-restoration="auto"` set. LLP 1070 says: "A top-level list has nothing that retires it and ignores the row."
- (b) `scroll-padding-top=24` on a `scroll`: `lower-scroll-padding` "`scroll-padding-top` insets where a virtualized list's `scrollIntoView` aligns a row; native hosts read it nowhere else, so on `scroll` the web alone would follow it". `scroll-margin-top=8` on a `text`: `lower-unknown-attr`.
- (c) `host/apple/Sources/ExactKit/IntoView.swift:6`: "`behavior=\"smooth\"` lands at once here"; `docs/contract-grammar.md` (host commands): "Native hosts land `smooth` at once on the element form". `scrollend=` on a `scroll`: `lower-unknown-attr` "`scroll` has no attribute `scrollend`".

Expected (Chrome, and CSS where it applies):

- (a) No web element-level equivalent exists (the page's `history.scrollRestoration` is the nearest). Proposal: `scroll-restoration="auto"` on a top-level `list virtualized=true` keeps its anchor by the list's `id` (or an app-given key) across unmount, as nested lists already do; or a `ScrollEvent` field with the first visible key and offset, plus a restore through `scrollIntoView(list, key, …)` with an offset.
- (b) CSS `scroll-padding-*` on any scroll container and `scroll-margin-*` on its descendants, honored by `scrollIntoView` (CSS Scroll Snap 1 §4.1, as Chrome does) on every host.
- (c) `scrollIntoView(id, behavior="smooth")` animates on macOS and iOS as it does in Chrome, and lands at once under reduced motion; DOM's `scrollend` fires once when the scroll settles, on every host.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` answers `rows(n)` with `{ id: String(i), h: 40 + ((i * 37) % 80) }` for `i < n`:

```contract
shape Row
  id: string
  h: number

component ScrollRestore
  resource rows = rows(80) as shape list<Row>
  state shown = true
  state ly = 0
  action listMoved(x: number, top: number)
    ly = top
  action toggle
    shown = not shown
  action jump
    scrollIntoView("p40", behavior="smooth")
  view
    column testId="root" height="100%" display="flex" flex-direction="column"
      row
        button press=toggle testId="toggle"
          text "toggle"
        button press=jump testId="jump"
          text "jump"
      row flex=1 min-height=0
        when shown
          list virtualized=true scroll-restoration="auto" estimated-item-height=40 height="100%" flex=1 scroll=listMoved testId="rows"
            each r in rows key=r.id
              text r.id height=r.h border-bottom="1px solid #cccccc" testId=`l-${r.id}`
        scroll flex=1 height="100%" testId="plain"
          each r in rows key=r.id
            text `p ${r.id}` id=`p${r.id}` height=30 border-bottom="1px solid #cccccc" testId=`p-${r.id}`
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| (a) remount | `bun exact.mjs agent <host> --size 600x600 "clock settle" "tap rows wheel 0 1500" "clock settle" layout "tap toggle" "tap toggle" "clock settle" layout` | web (Chrome 154); macOS 26.6.2 after `bun exact.mjs mac` | `0365ad1a4` | before: `[rows] … scroll 0,1500`, `l-18` at y −43 (61 px into it); after: `scroll 0,0`, `l-0` at the top; both hosts | `l-18` at 61 px into it after the remount (proposal) | agent `layout` lines |
| (b) scroll-padding on `scroll` | `scroll testId="root" height=200 scroll-padding-top=24` with one `text`; `bun exact.mjs contract build <file> --json` | compiler | `0365ad1a4` | `lower-scroll-padding` (quoted above) | compiles; `scrollIntoView` aligns inside the padding on every host | compiler output |
| (b) scroll-margin | `text "row" scroll-margin-top=8` inside a bounded `scroll`; `contract build --json` | compiler | `0365ad1a4` | `lower-unknown-attr`: "`text` has no attribute `scroll-margin-top`" | compiles; honored by `scrollIntoView` | compiler output |
| (c) scrollend | `scroll … scrollend=ended`; `contract build --json` | compiler | `0365ad1a4` | `lower-unknown-attr`: "`scroll` has no attribute `scrollend`" | compiles; fires once per settled scroll | compiler output |
| (c) smooth element jump | tap `jump` on a real launch (`bun exact.mjs mac --run`); the agent's frozen clock cannot show motion on any host | macOS | `0365ad1a4` | lands at once (source and guide, quoted above); not observed with a real launch in this run | animates over several frames, as Chrome does | source reference |

### Acceptance criteria

- (a) In the app above, on the web, macOS and iOS, the toggle pair leaves `l-18` 61 ±1 px into the port's top, with `scroll-restoration="auto"`; `manual` keeps today's behavior.
- (b) A conformance case against Chrome: `scroll-padding-top=24` on a plain `scroll` and `scroll-margin-top=8` on a row, `scrollIntoView(block="start")` and `"nearest"` land the row at Chrome's offset on every host.
- (c) On a real launch, a smooth element jump takes more than one frame on macOS and iOS and lands at once under reduced motion; a `scrollend` action runs once after a jump, a wheel scroll and a fling, matching Chrome's count in a conformance case.

### Constraints and related work

- Blocking text: `llp/1070-nested-and-horizontal-lists.rfc.md:261` (accepted: "A top-level list has nothing that retires it and ignores the row"); `llp/1010-scrolling-v1.spec.md:1141-1156` and `contract/lower/src/collection.rs:53` (`scroll-padding*` refused anywhere but a virtualized list); `llp/1070.000-scroll-into-view.rfc.md:84,93` (`scroll-margin` deferred to a consumer).
- Related: #138 (closed by #210, plain-scroll anchoring only); #127 (layout facts; distinct).
- Workarounds today: native code that remembers a list's top row and offset and re-applies it; app-computed row offsets instead of scroll margins; re-placing a target repeatedly with no settle signal.
- Not tested: iOS and Linux; a real-time smooth jump on macOS (the agent's clock is frozen, so both hosts land at once under it).

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:18:53Z

## Decision needed

**Blocking text** (main `0365ad1a4`, unchanged on `e200397ec`):
- (a) `llp/1070-nested-and-horizontal-lists.rfc.md:261` (Accepted): `scroll-restoration` is "Default on, for a virtualized list nested in a virtualized list's row … A top-level list has nothing that retires it and ignores the row."
- (b) `llp/1010-scrolling-v1.spec.md:1141-1156` (the `scroll-padding` addendum, 2026-10-07) and `contract/lower/src/collection.rs:53`: anywhere but a virtualized list, `scroll-padding*` is refused (`lower-scroll-padding`), because the element form of `scrollIntoView` on a plain scroller is the host's own and reads no padding. `llp/1070.000-scroll-into-view.rfc.md:84,93`: `scroll-margin` "deferred to a consumer".
- (c) `host/apple/Sources/ExactKit/IntoView.swift:6` and `docs/contract-grammar.md` (host commands): "Native hosts land `smooth` at once on the element form". `scrollend` is not in the event table.

**Options.**
- **A. All three, by CSS's and the DOM's names.** (a) A top-level `list virtualized=true` with `scroll-restoration="auto"` keeps its anchor (first visible key and offset) by its `id` across an unmount, in the runner's bounded store that nested lists already use. (b) `scroll-padding-*` on any scroller and `scroll-margin-*` on any element, read by the hosts' element-form `scrollIntoView` (which then computes the target offset itself instead of calling the platform's). (c) Element-form smooth jumps through the platform's animated scroll (`NSClipView` animator, `setContentOffset(_:animated:)`), reduced motion landing at once, and DOM's `scrollend` on every host.
- **B. Only what has no web workaround.** (a) and `scrollend`; leave padding, margin and smooth element jumps as declared deviations.
- **C. (a) as an app-driven restore.** A `ScrollEvent` field with the first visible key and offset, and an `offset=` on the list form of `scrollIntoView`, so the app stores and restores the anchor itself; no runner store.

**Recommendation.** A for (b) and (c), since CSS and the DOM already define them and the web host follows them today; C for (a), since a top-level list's identity across unmounts is the app's to know, and the list form of `scrollIntoView` already lands by key.

**Cost (estimate, not measured).** (a) via C: one `ScrollEvent` field and one `scrollIntoView` option, runner and three hosts. (b): replacing the native element-form `scrollIntoView` alignment with a computed one on macOS, iOS and Linux, plus a conformance case. (c): one animated path per native host and a new event in the schema and runner.

### ccheever — 2026-10-08T08:07:45Z

**Decision: Choose standard scroll padding/margins, native smooth jumps and scrollend; use app-owned top-list restoration.**

Keep open with the bounded scope below.

Padding/margins and settlement have web semantics. A remounted list has no browser automatic-restoration equivalent, and only the app knows its document/conversation identity.

Amend LLP 1010/1070 scopes. Expose a first-visible key/offset and a matching explicit restore; avoid a hidden unbounded top-level cache. Verify reduced motion and real-time smooth scrolling, not only a frozen agent clock.
