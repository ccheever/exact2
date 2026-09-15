# LLP 1039: Viewport facts — the width an app may branch on

**Type:** RFC
**Status:** Accepted by Charlie, 2026-09-14 ("maybe Contract should be given access to viewport-width?" — "sounds good, do the thing"; the implementer and order ruled with LLP 1038 §10 Q5). Implemented; the initial iOS cover viewport and Chrome software-keyboard follow-ups in §7 remain.
**Systems:** Runner (a second reserved source, `exactViewport`, answered by the runner; `set_viewport`; the viewport as a boot fact); Contract compiler (the bake's shape check, the TypeScript declaration and the receipt skip it as they skip `exactDelivery`; `aria-orientation`); Web host (`exact_boot` under a viewport, an `exact_resize` export, the `resize` listener); Apple and Linux hosts (their resize paths tell the runner); macOS presenter (the tablist projection leaves a vertical tablist alone); Agent API (nothing new — `layout` already reports the viewport)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Implementer:** Astra (`gpt-6-astra` through Codex), assigned by Charlie 2026-09-14, before LLP 1038's slice 1 in the same worktree; it stands alone and is about 200 lines
**Date:** 2026-09-14
**Related:** LLP 1038 D12 (the rail is a layout decision that needs this fact); LLP 1030 D7 (`exactDelivery`, the pattern this copies: `runner/src/delivery.rs`, `runner/src/runner/delivery.rs`); LLP 1005 §6 (settlement, one commit per re-answer); LLP 1007 §4 (the web ABI and the viewport meta); LLP 1008 §9 (the iOS layout viewport under the keyboard); LLP 1012 §1 (`layout` reports `viewport{w,h}`); LLP 1001 §1 (`env()` lengths); LLP 1017 §8 ("`match size` and a responsive grammar … each until an app needs it, then by fixture" — Interview is the app); LLP 1035.001 D10 (the tablist projection); LLP 1034 (the colour scheme stays its own fact). Research, never authority: exact1 LLP 0148 (content is form-factor-independent, presentation is not), 0047 (size classes from Taffy container width), 0010 Invariant 4 and 8

## Summary

A Contract app can read the layout viewport's width and height, in CSS
pixels, through one resource the runner answers itself:

```
shape Viewport
  width: number
  height: number
component Interview
  resource viewport = exactViewport() as shape Viewport
  derive wide = viewport.width >= 900
```

`when wide` is the media query. The host tells the runner the viewport at
boot, before the first settlement, so the first pixel is already the right
layout; when the viewport changes, the host tells the runner again and the
runner re-answers every resource that reads it in one commit — exactly how
`exactDelivery` works (LLP 1030 D7), with the same boot-fact channel, the
same field-name fill, the same bake rule. Nothing is added to the language:
no `match size`, no named size classes, no container queries. The app
names its own breakpoints. Two small things the rail also needs ride along
in D6: `aria-orientation`, and the macOS tablist projection recognizing that
a vertical tablist is not a segmented control.

## 1. Why now

Charlie wants Interview's bottom tabs to become a left rail on a big desktop
screen, as Twitter's do (LLP 1038 D12). Interview today chooses the tab
bar's shape by platform — `when data.authenticated and data.ios` for the
glass bar, `not data.ios` for the plain one
(`~/projects/interview/app.contract:713-738`) — which is the wrong axis: an
iPad in landscape and a narrow Mac window are decided by width, not by
operating system. Contract has no width signal. `env(safe-area-inset-*)`
and `env(keyboard-inset-height)` are lengths inside style values (LLP 1001
§1), not conditions, and there is no reading of the viewport anywhere an
expression can see. LLP 1017 §8 left "`match size` and a responsive
grammar … each until an app needs it, then by fixture". This is the app,
and the answer is a fact, not a grammar.

## 2. What the web does

CSS evaluates `@media (min-width: 900px)` against the layout viewport's
width in CSS pixels — `innerWidth`, scrollbar included. Under
`interactive-widget=resizes-content` Chrome shrinks the layout viewport to
the keyboard's top, so a height query changes when the keyboard shows; under
the default policy only the visual viewport shrinks and the query does not
change. Container queries (`@container`) exist but are fenced by containment,
because a box's size cannot depend on content the size affects.

exact2 already holds the first two rules on every host. The web page sets
the viewport meta from the root's props (LLP 1007 §4, `syncViewportFit`).
iOS frames the session to the safe area or the whole view under
`viewport-fit`, and under `resizes-content` to the keyboard's top
(`host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:110-135`), then calls
`exact_resize` when the size changed (`:154-157`); the Apple and Linux hosts
keep that size as the kernel's viewport (`host/apple/src/host.rs:493`,
`host/linux/src/host.rs:395`). The agent's `layout` reports it as
`viewport{w,h}` with exactly this meaning on every host (LLP 1012 §1). The
one place the number does not reach is the runner, and so the app.

## 3. Decisions

### D1 — One reserved source, `exactViewport`, filled by field name

`resource <name> = exactViewport() as shape <S>` where `S` names any of
`width` and `height`, both `number`, CSS pixels (points on Apple). The
record is filled by field name from the declared shape, as `exactDelivery`'s
is (`runner/src/runner/delivery.rs:1-12`): an app that wants only the width
declares only `width`. A field the runner does not know is refused at bake
(`bake-viewport-field`, beside `bake-delivery-field`,
`contract/cli/src/lib.rs:522-550`); arguments are refused as delivery's are.
No data crate is ever asked for it; the TypeScript declaration and the
receipt skip the name as they skip `exactDelivery`
(`contract/cli/src/typescript.rs:64`, `receipt.rs:359`).

Reads are ordinary expressions: `when viewport.width >= 900`, a `derive`,
or a `fn` for an app that wants named classes —
`fn sizeClass(w: number): string = w >= 1200 ? "expanded" : (w >= 600 ? "medium" : "compact")`
— which is the app's vocabulary, not the runner's. exact1's `compact |
medium | expanded` (0047) is research; CSS gives numbers, and so does this.

### D2 — The runner holds it; the hosts set it

`Runner::boot*` take the viewport beside `delivery`, and the runner answers
`exactViewport` from it before the first settlement. `Runner::set_viewport(width,
height)` replaces the fact and re-answers every resource reading it in one
commit through `recommit` (`runner/src/runner/settlement.rs:27-57`) — the
shape of `set_delivery` (`runner/src/runner/delivery.rs:57-72`) — and
returns `None` when the fact did not change or nothing reads it.

Hosts call it where they already handle a size:

- **Apple and Linux** already boot under a viewport (`exact_boot(rt, width,
  height)`, `host/apple/include/exact.h:119-121`) and have a resize path
  (`Host::resize`, `host/apple/src/host.rs:493`, `host/linux/src/host.rs:395`).
  Both pass the size on to the runner; the re-answer's commit rides the
  same batch as the relayout.
- **Web.** The runner does no layout there and has never heard the size.
  `exact_boot` and `exact_boot_plan` gain `width, height` like the Apple
  ABI's; `exact_resize(width, height, now_ms)` joins the six exports (LLP
  1007 §4); the glue passes `innerWidth`/`innerHeight` at boot and on every
  `resize` event. No debounce: an unchanged fact produces no commit, and a
  changed one re-evaluates the tree in the milliseconds LLP 1005 §8's
  scaling correction measured. A window drag that proves otherwise earns a
  coalescing rule, measured.

### D3 — The first pixel is laid out for the real size

The resource is device data, as delivery is: the runner marks it a reader
(`runner/src/runner.rs:405-412`), so it never takes the compiled value and
never a carried one — the boot fact wins, as `boot_with_delivery` documents.
Bake answers `LINT_VIEWPORT` (390 × 844, `contract/cli/src/lib.rs:68`), so
the compiled first frame is the phone's, which is what the lint already lays
out; a host boots with its own size before anything settles, so a desktop
window's first frame is the wide one. No relayout flash, no second first
frame, no app JS before first pixel — the boot rules `rules/RULES.md`
holds.

### D4 — The number means what CSS's means

`width` and `height` are the layout viewport's, in CSS pixels: the web's
`innerWidth`/`innerHeight` (scrollbar included), the frame the Apple and
Linux hosts lay out under. Under `interactive-widget="resizes-content"`,
`height` shrinks to the keyboard's top on the web and on iOS, which already
agree (§2); under the default policy it does not. `viewport-fit="cover"`
makes it the whole screen, with `env(safe-area-inset-*)` carrying the insets
as today. They are the same numbers the agent's `layout` reports as
`viewport{w,h}`, and `state.resources.<name>` shows the app's copy, so a
test can hold the two equal.

### D5 — The viewport only; no container queries

A node's own width is a layout result. A branch on it feeds layout back
into the tree it lays out — the loop CSS fences with containment and exact1
paid for in 0047's Taffy-derived classes. exact2 declares the deviation:
there is no `@container`, and a component adapts to the viewport or to a
prop its parent computed. A second consumer that cannot be written that way
brings the design and its fixture.

### D6 — What the rail needs beside the fact

- **`aria-orientation`** joins the attributes as `accessibilityOrientation`
  (`horizontal` | `vertical`), emitted as the ARIA attribute on the web, the
  way `aria-selected` is (`contract/lower/src/tags.rs:258-259`). A vertical
  tablist is ordinary ARIA.
- **The macOS D10 projection** (`SegmentsMac.swift:39`: owners by
  `accessibilityRole == "tablist"`, `button` children with `role="tab"`)
  applies only when `aria-orientation` is absent or `horizontal`. A vertical
  tablist stays the authored column on macOS and on iOS. Nothing else in
  LLP 1035.001 D10 changes: selection stays the authored props, activation
  the synchronous `press`.

## 4. Interview, as it would read

The navigation root stays `main` and the routes stay its direct children
(LLP 1038 D6); the rail is a sibling of the routes, absolutely positioned at
the left the way today's bar is positioned at the bottom, and each screen
pads its content by the rail's width when the viewport is wide.

```
shape Viewport
  width: number
component Interview
  resource viewport = exactViewport() as shape Viewport
  derive wide = viewport.width >= 900
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=followLink …
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` padding-left=(wide ? 88 : 0) …
          Screen(entry=e)
      when data.authenticated and wide
        column role="tablist" aria-orientation="vertical" aria-label="Main navigation" position="absolute" left=0 top=0 bottom=0 width=88 padding-top="env(safe-area-inset-top)" …
          button role="tab" press=selectTab("home") aria-selected=(nav.tab == "home") …
          button role="tab" press=selectTab("prompts") aria-selected=(nav.tab == "prompts") …
          …
      when data.authenticated and not wide
        row role="tablist" aria-label="Main navigation" position="absolute" left=12 right=12 bottom="calc(env(safe-area-inset-bottom) + 8px)" …
          …                                   // today's bar
```

The `data.ios` branches that choose the bar's shape go; the glass material
is a style, not a platform. An iPad in landscape gets the rail, as Twitter's
iPad app does; a Mac window narrowed under 900 gets the bar; a phone never
sees the rail. The router value is the same in every branch (LLP 1038 D12).

## 5. Verification

| case | where | observation |
|---|---|---|
| `contract/corpus/viewport.contract`: two branches by width; the reject `bake-viewport-field` | `contract/cli/tests`, `rejects.txt` | bake compiles the 390-wide frame; the reject by id |
| Boot at 1280 × 900 | runner test | the wide branch is in the first batch; no second layout |
| `set_viewport(390, 844)` then `set_viewport(390, 844)` | runner test | one commit with the narrow branch; then `None` |
| Interview at `[420, 900]` and `[1280, 900]` | `agent.mjs web`, `macos`, `linux` (`open({size})`, `EXACT_SIZE`) | `tree` has `tab-bar` or `rail` by `testId`; `layout.viewport` equals `state.resources.viewport` |
| A live window resize across 900 on macOS | by hand until the driver can resize a window | the bar becomes the rail and back; the stack's screens keep their scroll |
| Keyboard under `resizes-content` on iOS and Chrome | `smoke.mjs ios`, `web` | `viewport.height` shrinks by `env(keyboard-inset-height)` on both |
| A vertical tablist on macOS | `agent.mjs macos tree` | no `NSSegmentedControl`; the authored buttons are live |

## 6. What this deliberately does not add

- Container queries (D5). A second consumer that cannot adapt to the
  viewport or a prop.
- Named size classes, `match size`, a responsive grammar. A `fn` names them
  when an app wants names.
- Other media features — `orientation` is `width > height`; `hover` and
  `pointer` wait for an app that branches on them; `prefers-color-scheme`
  is LLP 1034's and stays there.
- A resize event handler. `when` is the handler; nothing runs on resize but
  settlement.
- A resize operation for the agent. `open` takes a size (`scripts/agent.mjs:119`,
  `:382`); the eight operations stay eight.
- Device pixel ratio. Images are already loaded one-for-one (LLP 1011) and
  nothing else needs it.

## 7. Landing

One slice, one implementer: `Viewport`, `SOURCE`, `FIELDS` and
`set_viewport` in the runner beside delivery's; the boot parameter on the
three hosts; the bake's field check and `LINT_VIEWPORT` answer; the
TypeScript and receipt skips; the web's two ABI changes and the `resize`
listener; `aria-orientation`; the `SegmentsMac` guard; the fixture and the
reject; Interview's rail. It keeps the five checks and the 1,500-line cap.
No question is open: the shape follows `exactDelivery`, which LLP 1030 D7
already ruled.

**Implemented — 2026-09-14 (Astra).** `68c8b86` lands the runner fact,
boot and resize paths on all three hosts, bake refusal and executor skips,
ARIA orientation and the two native projection guards, fixtures and the
amendments below; `fd551c0` keeps the web test on its existing batch
assertions. Interview's `rail` worktree lands the authored rail in
`2880119`. Apple boot rollback tests (`8041f72`) keep their old-host checks and expect
the runner's earlier `InvalidViewport` refusal. The two-size drives report
420 × 900 and 1280 × 900 on web,
macOS and Linux, with equal layout and resource dimensions; a live Mac
420 → 1000 → 420 resize keeps the content's scroll at 100. The iOS keyboard
reports 402 × 874 → 402 × 539 with a 335-point inset, then restores 874.
Evidence and the final check results are under `/tmp/lane-router/1039/`
(`report.md`, `*-420.json`, `*-1280.json`, `macos-scroll-resize.json`,
`macos-vertical-native.json`, `ios-keyboard.json`, and the check logs).
Desktop Chrome has no software keyboard; its shrinking-keyboard check is
still owed on a device with one. Known on iOS: under `viewport-fit="cover"`
the session boots at the safe-area frame, learns `cover` from the first batch,
and resizes before first draw, so a resource reading `viewport.height` in its
arguments may be asked twice at boot; having the host read the root's
`viewportFit` from the plan before boot is the owed remedy, not yet implemented.
This note does not change the Status.

## 8. What this amends, at acceptance

- **LLP 1005**: a second reserved source; the viewport as a boot fact.
- **LLP 1006**: `aria-orientation`; `bake-viewport-field`.
- **LLP 1007 §4**: `exact_boot`/`exact_boot_plan` under a viewport,
  `exact_resize`, the `resize` listener.
- **LLP 1008 §9**, **LLP 1015**: the resize paths tell the runner.
- **LLP 1035.001 D10**: the projection is for a horizontal tablist.
- **LLP 1017 §8**: the responsive-grammar line is answered by a fact, not a
  grammar; `match size` stays out.
- **LLP 1038 D12**: the rail's fact is this document.
