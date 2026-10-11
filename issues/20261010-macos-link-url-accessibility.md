# macOS: a link's URL is not in the accessibility tree, and an inline link run's element is invalid outside the app

**Status:** Open
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x78-macos-link-url-accessibility.md; LLP 1080.002 (`tree --ax`); LLP 1115; issues/20261010-macos-inline-run-contextmenu.md; issues/20261010-macos-a-heading-inside-a-button.md

## Summary

On the web a link is in the accessibility tree with its URL. For the Exact web build, Chrome has `link "Example
link" url=https://example.com/link-node` for a `link href` node, and `link "site" url=…` for an inline run with an
`href` (CDP `Accessibility.getFullAXTree`). On macOS, Chrome exposes both as `AXLink` with `AXURL`. Hand-built Mac
apps do the same: a read-only `NSTextView`'s link run is an `AXLink` (`AXTextLink`) child with `AXURL`, a SwiftUI
`Link` is an `AXLink` with `AXURL`, and a `WKWebView` has an `AXLink` with `AXURL` for an `<a href>` and for an
inline one in a `<p>`, both in the web area's `AXLinkUIElements`.

The macOS host, read from another process through `AXUIElement` (the API VoiceOver uses), has two faults:

- **A `link` node has no `AXURL`.** It is an `AXLink` with its name and `AXPress`, but `AXURL` answers
  `kAXErrorAttributeUnsupported` (-25205). `updateRoleAccessibility`
  (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:368-384`) gives it the link role through
  `setAccessibilityToggle(… else: .link)` and never sets `accessibilityURL` from its `href`.
- **An inline link run cannot be read at all.** The paragraph is an `AXStaticText` "Visit the site today" with one
  child. That child answers `kAXErrorInvalidUIElement` (-25202) for every attribute, including its attribute names,
  each time the children are asked for. A hit test over "site" returns the same invalid element.
  `textAccessibilityChildren()` (`TextInteraction.swift:87-102`) builds new `InlineAccessibility` elements
  (`:106-132`) on every `accessibilityChildren()` call, and nothing keeps them. Those elements do carry the role
  `.link` and the run's `accessibilityURL`. A hand-built oracle shows that this is enough to break them: an
  `NSView` whose `accessibilityChildren()` returns a link `NSAccessibilityElement` made fresh on each call gives
  the same -25202. The same element kept by the view reads as `AXLink` with `AXURL`.

Inside the app the run is fine, so no check here sees it. The agent's `tree --ax` (LLP 1080.002) reads the tree in
the app's own process while it holds the returned array. It shows `link "site"` under the paragraph and reports no
finding. A native module in the same process can read the run's `accessibilityURL()` (the T3 clone's Copy Link
does).

What this means for VoiceOver, judged from the AX API only (VoiceOver was not turned on, because that changes the
machine's settings): VoiceOver would read the paragraph as plain text. It could not move into the run, name it as
a link, or press it, and an app that collects links by role would not find it. A `link` node would be read and
pressed as a link, but anything that reads `AXURL` gets nothing.

## Why this arose

T3 Code's `EXACT2-GAPS.md` entry "Text context menu" S3 asked whether a native module can read a node's `href`.
Main PR #410 checked S3 and did not file it as a module gap. During that check, the macOS tree read from another
process showed the `link` node as an `AXLink` with no `AXURL`, and the inline run's element answering -25202 for
every attribute (`fw-issues-20261010g/s3-record.txt`). In the clone, a Markdown table cell draws its links as
inline runs in one text node (`TableCell` → `ChatRuns`), so on macOS those links are invalid elements outside the
app. The clone's `link` nodes (provider docs, licenses, check details) are links with no URL. The clone has no
workaround and keeps the difference.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `474999b9fe95610fdde7b99314cec8b82723de1e`. Its
`app.ts` exports an empty `sources`. `contract build` compiles it with no warning.

```text
component X78app
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      link href="https://example.com/link-node" testId="l"
        text "Example link" font-size=24
      text testId="p" font-size=24
        text "Visit the "
        text "site" href="https://example.com/inline-run" testId="run"
        text " today"
```

Run `bun exact.mjs mac` and launch the built binary directly. Then read its window from another process with
`AXUIElementCopyAttributeValue` (the record's `axprobe` and `axdetail`; the reading process has the accessibility
grant). For the web, read Chrome's tree with each node's `url` over CDP (`x78-cdp-ax.mjs`, a fresh temporary
profile). For comparison, `bun exact.mjs agent <web|macos> "tree --ax"` reads the tree in-process.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| `link href` node, read from another process | macOS 26.6.2 (25G83), Apple Silicon | main `474999b9f` | `AXLink` "Example link" with `AXPress`. `AXURL`: -25205 | `AXLink` with `AXURL` https://example.com/link-node, as Chrome and WebKit give it | image (right), record M1, M2 |
| Inline run "site" with `href`, read from another process | macOS | main `474999b9f` | The paragraph `AXStaticText` has 1 child. Every attribute of that child is -25202, on all 3 asks. A hit test over "site" returns it too | `AXLink` "site" with `AXURL` https://example.com/inline-run | image (right), record M1, M2 |
| The same tree, read in-process (`tree --ax`) | macOS | main `474999b9f` | `link "Example link"`; `text "Visit the site today"` › `link "site"`. No finding | — | record M3 |
| The same app, web | Exact web (JS target) in Chrome 155.0.8059.39 | main `474999b9f` | `link "Example link" url=…/link-node`; `link "site" url=…/inline-run` | (the reference) | image (left), record W1, W2 |
| Hand-written `<a href>` and `<p>Visit the <a href>site</a> today</p>` | Chrome 155 via CDP, and headed Chrome read through `AXUIElement` | — | `link` with `url` (CDP); `AXLink` with `AXURL` (AX) | — | record |
| Hand-built: `NSTextView` with a `.link` run, SwiftUI `Link`, `WKWebView`, an `NSView` with an `NSAccessibilityElement` link child that is made fresh per call or kept | macOS 26.6.2, `swiftc` | — | `AXLink`/`AXTextLink` + `AXURL`; `AXLink` + `AXURL`; `AXLink` + `AXURL` (both, in `AXLinkUIElements`); fresh child -25202, kept child `AXLink` + `AXURL` | — | image (middle), record (oracle source beside it) |

![X78: a link's URL in the accessibility tree, the Exact web in Chrome, a hand-built Mac app, and the macOS host](https://raw.githubusercontent.com/ccheever/exact2/fa666be7faeaf4560af70be1d83601bfe7b98ec4/fw-issues-20261010h/x78-link-url-ax-web-oracle-macos.png)

Record (contract, commands, every tree, the oracles):
https://raw.githubusercontent.com/ccheever/exact2/fa666be7faeaf4560af70be1d83601bfe7b98ec4/fw-issues-20261010h/x78-record.txt
(the probe and oracle sources are `x78-axprobe.swift.txt`, `x78-axdetail.swift.txt`, `x78-cdp-ax.mjs.txt` and
`x78-appkit-webkit-oracle.swift.txt` at the same commit).

Not run: VoiceOver itself, #327's head, iOS (its `InlineAccessibility` is also made fresh per call), Linux,
`AXLinkUIElements` (no Exact element is a web area), and a run inside a link that wraps across lines. Also seen,
not filed: a hit test at the left edge of the `link` node returns an `AXStaticText` "Example link" that is not
among the link's `AXChildren`. A `text` node that carries its own `href` outside a paragraph is not a link on
either host (Chrome: `generic`; macOS: `AXStaticText`), so it is left out here.

## Constraints

- The web is the parity oracle (`CLAUDE.md`), and Chrome, WebKit, `NSTextView` and SwiftUI agree. LLP 1115: what
  the author leaves unsaid is the platform's, and the platform's link carries its URL.
- An inline run's element stays valid as long as its run is shown. It is the same element from one
  `accessibilityChildren()` call to the next, and a hit test returns that element. It is replaced when the
  paragraph's runs change.
- The run keeps its role, name (its text, or its `accessibilityLabel`), `AXLanguage`, frame and `AXPress`
  (`activateInline`). A run with only `press=` stays `AXStaticText`.
- A `link` node's `AXURL` is its `href`, as a run's already is. Chrome gives a link's resolved URL; what an
  in-app path (`/note/3`) should give natively was not checked.
- In-process readers (the agent's `tree --ax`, a native module) read the same values as today.
- Main PR #327 (head `14493d253`) changes how hover reaches a run (`TextInteraction.swift`'s `hoverInline`) and
  routes `NodeViewMac.swift`'s `mouseEntered`/`mouseMoved`/`mouseExited` through `presenter.trackPointer`. It
  changes neither `textAccessibilityChildren`, `InlineAccessibility`, `accessibilityChildren` nor
  `updateRoleAccessibility`, and none of its added or removed lines mention accessibility (read from its diff, not
  run).

## Acceptance criteria

- In the repro on macOS, read from another process: the `link` node is an `AXLink` with `AXURL`
  https://example.com/link-node. The paragraph's child is an `AXLink` "site" with `AXURL`
  https://example.com/inline-run, it answers on every ask, and a hit test over "site" returns it.
- `tree --ax` on macOS shows the same tree as today, and the run's press and link activation work as today.
- A macOS XCTest asks a paragraph with an inline link for its accessibility children twice. It checks that the
  child is the same element both times and that it carries the URL. It also checks a `link` node's
  `accessibilityURL()`.
