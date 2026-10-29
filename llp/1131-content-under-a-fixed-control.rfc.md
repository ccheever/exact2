# LLP 1131: Content under a fixed control

**Type:** RFC
**Status:** Accepted as built, 2026-10-19, after three design rounds (Astra; Grok in two, its third cancelled) and three code rounds (Astra; `~/audits/exact2-no-tells-2026-10-11/llp1131/`). Building it measured one thing the draft had wrong, now in D2: UIKit's safe area does not reach a scroll view drawn past its box, so a band is wholly the scroll view's own inset.
**Systems:** the iOS scrollers' bands (`host/apple/Sources/ExactKit/IOS/PageEdgesIOS.swift`), a route's scroller and covers (`NavigationBarIOS.swift` `collapse`, `reachBottom`, `reportCovers`), paint and hit order (`PaintOrder.swift`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:**
- LLP 1116.008 D3 (round 10's team-kanban: "Scrolled cards clip hard under the control with no material or divider"), which left the bar's material over fixed content to LLP 1115.000 D11; QUEUE.md's item for it.
- LLP 1130 (a page under the status bar), whose bands this extends from the safe area to a route's fixed boxes.
- Apple, [`UIScrollEdgeElementContainerInteraction`](https://developer.apple.com/documentation/uikit/uiscrolledgeelementcontainerinteraction) (iOS 26), [`UIScrollEdgeEffect`](https://developer.apple.com/documentation/uikit/uiscrolledgeeffect); SwiftUI's `safeAreaBar(edge:content:)` (iOS 26), which does the same for a view pinned over a scroll view's edge.

## Summary

On iOS 26 and later a screen whose control is pinned between the navigation bar and a list
(a segmented switcher, a filter row) or under it (a chat composer, a button row) lets the
list run under that control, inset by it, and blurs the content passing under it with the
scroll edge effect: SwiftUI does it for a `safeAreaBar`, UIKit for a container that carries
a `UIScrollEdgeElementContainerInteraction`. Exact lays the control and the list out one
after the other, so the list's content stops in a hard line at the control's edge and the
bar above shows no edge effect at all. This RFC draws such a list under its fixed
neighbours, as LLP 1130 draws a scroller under the status bar: layout does not change.

## 1. What happens now

A UIKit probe on the iOS 27 simulator (iPhone 18 Pro;
`~/audits/exact2-no-tells-2026-10-11/fixed-control-probe/`) with an inline title "Board", a
segmented control pinned under the bar and a table below it, scrolled 150 points:

- `clip` (the table starts below the control, as Exact lays it out): the rows stop in a hard
  line at the control's foot; the bar and the control stand on plain white.
- `under` (the table fills the view, its `contentInset.top` the control's band, the
  control's container carrying the interaction with `edge = .top`, the table the
  controller's content scroll view): the rows pass under the control and the bar, blurred
  by one soft edge effect shaped around the bar's title and the control.

Exact's team-kanban draws the first.

## 2. Decisions

Three review rounds (Astra, Grok; `~/audits/exact2-no-tells-2026-10-11/llp1131/`) shaped this
draft: the first found the trigger too broad, the insets owned twice and the paint order
overwritten; the second found the opposite edge dropped, the large-title screens excluded
after all, and the bars' membership, ties and bounds unsaid; the third, a layout change in
handing the covers over, the route's own choice of scroller, an edge open but not reached
and an unbounded band. A third probe mode, `never`, is
Exact's configuration (`contentInsetAdjustmentBehavior = .never`, the band as `contentInset`,
the controller's content scroll view, the interaction): the content under the control is
blurred (`never.png`). The gate measures it again through Exact's own hierarchy (a padded
route, a bottom edge).

### D1 — a control bar

A **control bar** is a box that is shown (`!isHidden`), in flow (`inFlow`), not transformed,
positioned `static` or `relative`, with no authored `background-color`, holding at least one
control, and nothing else but wrappers:

- **Controls**, taken whole without looking inside: a button, a link, a field (`input`,
  `textarea`), a `select`, a switch or checkbox, a tab, and a native projection of any of
  them (a `tablist` drawn as UIKit's segmented control is one control, whatever its hidden
  tabs hold).
- **Wrappers**: a box with no text, image, `svg`, canvas, video or scroller of its own and no
  authored `background-color`, whose children are controls or wrappers. Borders, padding,
  gaps and an authored `background-color` on a control are allowed.

A text, an image, an `svg`, a canvas, a video or a scroller outside a control makes the box
not a bar: a caption, a heading, a summary, an illustration. A role (`tablist`, `toolbar`,
`search`) does not override this. An authored `background-color` on the box or a wrapper
says what the bar looks like: it keeps today's layout and edge (CLAUDE.md, "What the author
writes wins").

### D2 — a scroller's bands

A vertical scroller S (an Exact `scroll`, a `list`, a grouped list) whose
`-exact-scroll-edge-effect` is not `none`, in a parent P that neither clips nor transforms,
spanning P's content box from side to side. P is a route node, a page root with no
container, or a box that alone fills one of those (no padding or border on that edge, no
in-flow sibling), as `bottomScroller` walks. The **container** C is that route node or page
root.

S's **top edge is banded** when P's shown in-flow children before S are control bars (one at
least), each ending above S's top (laid-out frames, as `bottomScroller` reads them; the host
is not told the flex direction); its **bottom edge is banded** likewise with those after S,
each starting below S's foot. S must take what its bars leave, as a hand-built screen's list does: it is a growing item
(`flex-grow` above 0, no `max-height`) of a column flex container, which takes every point of
the free space (CSS Flexbox §9.7), so its length is the room between them; the host's layout
says so to the presenter (`flexGrows`), which is not told layout's styles. A short scroller
in a form, sized by its author, is not one. A box between S and C has no padding or border
on that edge, and no host view between them clips. An edge with no
children between S and C's edge is **open**.
S takes this LLP when at least one edge is banded and every other edge is banded or open.

The same holds a level up (2026-10-26, bake-off r24's chat): a box between S and C whose
shown in-flow siblings on that side are control bars is banded there when it spans its
parent and takes what they leave (`flexGrows`, sent for every growing column item), as a
thread wrapped with its floating button sits over a composer; the bars of every level are
S's on that edge.

A route's top is judged apart from its foot (2026-10-27, bake-off r28's chat): a foot with
anything but bars between S and C (a composer its author filled) keeps its place and no
longer keeps the top from its band; and an open top under the route's cover (the inline
bar) is banded with no bars at all. A hand-built table runs under its navigation bar
whatever is pinned under it, and r28's thread, over a filled composer, stopped in a hard
line under the bar. A top with more than bars keeps both edges as before, and a page's
scroller keeps the rule above.

An edge is **open** only where S reaches its parent's content edge there (nothing, no
padding or border, between them, as `bottomScroller` requires today) and P reaches C's.
For a page, C must itself meet the page area's edge, with nothing between S and the
document that clips, transforms, scrolls or hides (LLP 1130's `pageBand` checks, kept).

When S takes this, each of its banded and open edges reaches C's edge. **Covers stay exactly
as they are today**, so nothing is laid out anew and the bars stay where layout put them:

- S's scroll view's frame grows to C's edge on each such edge (its **band**: the bars, and on
  an edge where the route keeps a cover, the cover; for a page, the page's margin on that
  edge, which is zero at the foot while a keyboard ends the area, LLP 1130 D2, and at the
  foot only in a document no taller than the area).
- Its `contentInset` grows by the whole band, keeping the logical `scrollTop` (LLP 1130's
  `setPageBand`; a list following its end, `scrollFollowEnd`, stays at its end), whoever
  owns it, so nothing is handed over: a scroller UIKit already insets (today's collapse
  scroller under an inline title, or the bottom edge's scroller) keeps `.automatic` and its
  `HostCover.inset`, and UIKit goes on adding the safe area on the edge its box reaches. A
  view's safe area is its superview's, and the scroller's box ends at the bars, so UIKit adds
  nothing past it: the band, the cover included, is the host's on a banded edge whichever the
  owner (measured: a scroll view drawn past its box under the bar reports a safe area of 0).
  `reportCovers` reports no `HostCover.inset` for a band: it is a UI inset, not layout.
- `reachPageEdges` computes every scroller's band, routes included, and is its one writer.
  `scrollTopInset`, the port and `scrollFloor` count the whole band (LLP 1130 D3); the host's
  own-inset check (`ownedChanges`) counts it as the host's.
- S is the controller's content scroll view for a banded edge only where the route has not
  chosen another (`navigationScroll`, `contentScroller`): the top edge follows that choice,
  an unresolved name choosing none; the bottom edge is S's, as `reachBottom` chooses it by
  place today.

**A large title** keeps today's screen whole: a route whose bar shows a large title takes no
band at all, top or bottom, because its title collapses only with the scroller UIKit insets
under it and a large title over fixed content never collapses (LLP 1115.000 D11; LLP
1116.008 D3 puts a fixed control under an inline title). Shelf's Library and Discover (a
level-1 heading over a filter `tablist`) are unchanged; a route with an inline title and a
switcher (`pattern rest`, team-kanban's winning shape), a conversation over a composer (the
chat recipe, Messages) and a page over a composer take it.

**A virtualized list's band** is at most its port on each edge (D5); a longer one is not
taken.

### D3 — paint and touches

In `rankChildren`, written through `setPaintPosition`: a bar whose rank among P's children is
S's (both `z-index: auto`, the usual case, where the later sibling would paint over it) is
placed just above S, and below every sibling whose rank is above S's. If any bar on an edge
ranks below S by an authored `z-index`, that edge is not banded (it is then neither banded
nor open, and S takes none of this). Hit order follows (`hitOrder`). A touch on a bar is the
bar's, its empty area included; a box under `pointer-events: none` passes it on as today; a
touch in a band outside every bar reaches S (the bar's own region is the bar's).

### D4 — the edge effect

Each control bar gets a `UIScrollEdgeElementContainerInteraction` whose `scrollView` is S's
current scroll view and whose `edge` is its band's; it is retargeted when S's scroll view is
replaced (a grouped list mounting, a backend change), and removed when the band goes. On iOS
before 26 none of this applies.

### D5 — a virtualized list

The Apple host realizes a virtualized list's rows a port behind and ahead of what shows
(`lead`, with no lean on this host), so rows under a band of up to a port are drawn once
filled, and D2 takes no longer band; its port stays its box, so its offset corrections are
unchanged. Rows past the port are optional: after a jump, or while the list fills, a band
can show unfilled list for a moment, as the list's own edge does.

### D6 — what is left out

The web, Android, the Mac and tvOS. A large-title route (D2). A scroller deeper than D2
walks. A keyboard's accessory bar (QUEUE.md's item).

## 3. The gate

- A route with an inline title and a segmented switcher over a `list`, the chat recipe (a
  `list` over a composer, following its end, the keyboard up and down), and a page with no
  container over a composer, on the iOS 27 simulator, before and after, beside the UIKit
  probe's `under`, scrolled: rows under the bar and the control, blurred; at rest every row
  where it was (`agent layout`), `scrollTop` 0; Shelf's Library unchanged.
- XCTests over a fixture: S's frame and insets with a top band, a bottom band and both; a
  caption, a heading and a painted row are not bars; a large title takes no band; an authored `z-index` above the bar takes none; `scrollTop` kept as a
  bar appears and goes; a tap on a bar reaching it, a drag from its empty area not scrolling
  S; a field in the composer and a field in S revealed above the keyboard; the interaction
  retargeted when a grouped list mounts; pull to refresh's spinner where it was, and a band changing while it spins.
- Messages Stress, the iOS smoke, the UIKit XCTests, the tvOS compile and the six checks.
