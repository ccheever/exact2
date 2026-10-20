# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `04e114daa`.
- **Method:** one brief (sha256 `b18f03b820a9f9992028b45f2130127ce06ec70a97c42d8965e19cfbc55d73af`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken by a different fix. The page no longer guesses what is in view. The outline the server answers a selection made on this page with scrolls nothing, because the copy clicked is under the pointer (`inspecting.clicked`, set when the page reports its selection). Any other outline, the tool's Outline included, scrolls its first copy in as D4 says. That removes the viewport, clip, banner and empty-box cases. The test counts the page's `scrollIntoView` calls: none for the selection's outline (the old line made one), one for the tool's Outline.

---

The click case holds for a copy whose border box meets the viewport. "In view" is only that test, so a nested scroller, an empty box, and the inspect banner still decide wrong, and the new assertion does not fail on the bug it names.

**Medium** — `host/web/dev.js:632`

`seen` is true when the border box meets the layout viewport (`innerWidth` / `innerHeight`). A page `scroll`, a feed, or a virtualized list clips to its own port (`[data-scroll="true"] { overflow: auto }` in `host/web/index.html`). A mounted row just above that port still has a border box in the viewport whenever the port sits below a header or beside other chrome: the box covers the header band, the row's pixels are clipped, and the dashed outline is `position: fixed` on the body so it paints in that band. `found.some(seen)` is then true and `scrollIntoView` is skipped. The same happens on a horizontal scroller: an item slid fully past the port's left edge can still cross `x = 0`, so an Outline of that site leaves it clipped. A partial intersection of the viewport is treated as in view, which matches this commit for the window itself. `scrollIntoView({ block: "nearest" })` already uses the default `inline: "nearest"`, so a box that misses the window horizontally is scrolled once the predicate agrees.

Fix: count a copy as in view only when its box meets the viewport and every ancestor whose overflow is not `visible`, the walk `reveal` already does in `host/web/input-glue.js:592`. Use that same box for the decision.

**Medium** — `host/web/dev.js:632`

A zero-area box inside the viewport counts. A `0×0` rect at `(10,10)`, or a `height: 0` strip with any width and `top > 0`, passes `bottom > 0 && right > 0 && top < innerHeight && left < innerWidth`. One collapsed copy of a repeated node then suppresses scrolling a copy that has size and is off screen. A folded text is the other way: `display: contents` makes no box (`host/web/src/element.rs:154`), so `getBoundingClientRect` is the empty rect at the origin and `seen` is false while the glyphs are on screen. `found[0].scrollIntoView` then runs. On a contents element that call returns without scrolling (the host's `reveal` scrolls the parent instead, `input-glue.js:598`). If the first site-order copy is a different instance that does have a box and sits off screen, that one is scrolled and the on-screen label moves. A `display: none` first copy is the same no-op, so a later laid-out copy stays off screen. This path is also the tool's Outline button: `outlineSites` does not know which copy was clicked.

Fix: ignore copies with no client rect or with zero width or height. Measure a folded text with `viewBox`, and scroll that text's parent. Call `scrollIntoView` on the first copy that has a box.

**Low** — `host/web/dev.js:559`

The inspect banner is `position: fixed` at the top center (`z-index: 2147483647`) and the selection outlines sit under it (`2147483645`, `dev.js:637`). `seen` still uses the full viewport, so a copy whose box lies entirely under the banner counts as in view. Outline leaves it there, and the dashes stay hidden behind the banner.

Fix: treat a copy as out of view when the banner's box covers it, and inset `scrollIntoView` by that strip.

**Medium** — `scripts/tamber-server.test.mjs:477`

The new check reads `scrollY` around the "Swap pins" selection on the theme fixture. That page scrolls inside `scroll testId="page"` (`apps/theme-fixture/app.contract:15`), which is `overflow: auto` inside a `height: 100%` root (`host/web/index.html:19`, `:85`, `:300`), so the window's `scrollY` stays `0` whether or not the feed moves. "Swap pins" is a single copy already on screen, and `scrollIntoView({ block: "nearest" })` on a fully visible element leaves every scroller where it is. The assertion passes on the pre-fix line as well. The later tool `outline` compares mark rectangles to view rectangles and never reads a scroll offset.

Fix: give one site two mounted copies, put the sites-order first copy outside a nested scroller (header above the port, or a horizontal port) and the other inside it, and assert that scroller's `scrollTop` and `scrollLeft` stay put on a click. Then move every copy out of the port while leaving one border box over the viewport and assert that a tool `outline` scrolls the port. Cover a zero-area copy and a folded text the same way.

A selection whose border box meets the viewport no longer jumps the feed; the predicate still misses nested ports, empty boxes, and the banner, and the test does not prove otherwise.
