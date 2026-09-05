# LLP 1033: Markdown viewer — a useful macOS application first

**Type:** Plan
**Status:** Draft
**Systems:** Text, Apple host, Web host, Contract, External apps
**Author:** Charlie Cheever / Codex
**Implementer:** Codex, starting 2026-09-05
**Date:** 2026-09-05
**Revised:** 2026-09-05
**Related:** LLP 1001 §6, LLP 1008 §3/§5, LLP 1010, LLP 1011, LLP 1031 D1/D6

## Milestone

Charlie can launch **Exact Markdown.app**, choose a local Markdown file with
Open… or ⌘O (also Finder's Open With), and read it comfortably on macOS.
The window resizes, long documents scroll, links work, and text can be selected
and copied. Opening a second file replaces the document; cancellation keeps it;
an unreadable file produces a useful error without losing the current document.

The product targets **macOS, iOS, and the web**, with one parser and one Contract
rendering implementation. The first deliverable is the macOS app; iOS file
selection and platform-specific interaction validation follow it. This order
records Charlie's 2026-09-05 instruction, not a promise that web-only rendering
is a completed native milestone.

## Implementation

The external app lives at `~/projects/exact-markdown`, consumes Exact2 by path,
and uses the shared build/agent scripts through `EXACT_APP_DIR`. The macOS
adapter embeds `ExactKit`; it owns the window and OS file opening. Markdown is
parsed below the data seam into ordered blocks and styled runs, then rendered
by Contract through Exact's own text nodes. No HTML document in a webview.
The parser is shared Rust so native and wasm consume identical content data.

1. **Prove paragraphs.** Render wrapped normal/bold/italic/code/link runs from
   one paragraph. Make Apple paint the runs its kernel already measures.
   Add native selection using the same CoreText lines; exercise links and copy.
2. **Read a real document.** Headings, paragraphs, emphasis, links, images,
   ordered/unordered lists, block quotes, rules, and fenced code blocks. Give
   prose a comfortable reading width and generous line spacing; code preserves
   whitespace. Unsupported embedded HTML remains inert. Tables and syntax
   highlighting may follow the basic reader and are reported explicitly.
3. **Deliver macOS.** Open dialog, ⌘O, Finder Open With, filename in the window,
   cancellation/errors, a self-contained `.app`, and a representative real-file
   run including resize, scrolling, selection, copy, and a second open.
4. **Complete the other surfaces.** Run the same content on web and iOS; adapt
   their file pickers and selection/link gestures without replacing the parser
   or the Contract document renderer. Relative local images need an explicit
   containing-directory policy on each platform.

## Bounds and evidence

Viewing only: no editor, accounts, document library, blog/CMS, or publishing.
Opening a file is a local application operation, not a new agent operation.
File content is data, never Contract source; loading a document compiles no UI.
Native selection belongs to the host, never to plan state. CoreText remains the
Apple measurement and drawing engine (LLP 1008 §3).

Scope take: this reader is a new real consumer alongside Caltrain; speculative
native-module implementation (LLP 1024, still awaiting a consumer) is deferred
behind it. Its `current/` link leaves to keep the 15-document working set.
No new framework `fileinput` component is needed for the macOS embedder.

Verify changed behavior with parser/host tests and the existing eight-operation
agent driver, inspect screenshots, and run the repo's five checks. Record what
actually passed and any unfinished milestone requirements here when it lands.

## First implementation, 2026-09-05

The macOS app is built at `~/projects/exact-markdown/target/Exact Markdown.app`.
Driven and visually inspected: a real LLP opened as Markdown; Finder-style open
into the running app and a second open; Open… showing the native file panel;
a relative image; an inline link opening another local Markdown file; drag
selection across paragraphs; select-all/copy; long-document scrolling; and an
invalid UTF-8 file refusing without replacing the current document. The shared
renderer also builds and displays the welcome document in Chrome and on an
iPhone 17 simulator. The three parser/data-seam tests and Exact2's five checks pass.
The existing Caltrain macOS smoke suite also passes (41.3 seconds).

The app reads UTF-8 files up to 2 MB. iOS/web file-opening adapters, iOS selection
and link gestures, tables, syntax highlighting, heading anchors, and file watching
remain. The native panel's cancellation path and interactive desktop resize
still need a dedicated drive; the iOS run exercises the narrow layout. Keyboard
selection extension and bidi highlight geometry are follow-ups, not claimed here.
The complete cross-platform plan therefore stays Draft.

## Long documents, 2026-09-05

Charlie reported choppy scrolling and resizing with the 16,399-word
`0566-router-core-thread-home.decision.md`. Codex implemented run coalescing,
unmounted inline text identities, bounded cache eviction, width-independent
CoreText typesetter reuse, shared measurement/paint line breaks, cached paragraph
specs and selection order, and viewport-limited text painting. Selection and
copy still cover the complete document. No block-layout virtualization was added.

An optimized Swift harness over the actual app archive and ExactKit drove 60
updates per case, synchronously setting the AppKit scroll offset/window size,
displaying the window, and flushing Core Animation. These are update costs on
this Mac, not a display-link frame-rate trace. Before/after median milliseconds:

| Operation | Before | After |
| --- | ---: | ---: |
| Scroll | 48.3 | 7.6 |
| Scroll with all text selected | 58.9 | 7.6 |
| Resize above the maximum reading width | 68.4 | 13.2 |
| Resize through wrapping widths | 130.2 | 28.0 |
| Resize height only | 18.8 | 5.1 |

Logical nodes fall from 4,991 to 2,233; mounted native nodes fall to 676. Scroll
p95 is 10.4 ms; wrapping resize p95 is 32.6 ms. Full reflow still exceeds a 60 Hz
frame budget, so lazy block measurement with scroll anchoring remains a candidate;
these results do not claim sustained 60/120 fps during arbitrary resizing.

Driven checks: full-document copy includes the final block (106,361 characters),
drag selection, deep scroll and return with window-server screenshots, local link
opening a second document, and no runtime errors. The parser's four tests pass.
A standalone CoreText check exercises cache eviction, geometry/color separation,
Unicode, line clamping, and candidate-checkpoint restoration.
The iOS and web previews build and render. The Caltrain macOS smoke passes on
repeat (11.9 seconds); its first run reported one timer-transition step at width
50 instead of 75. No motion code changed, and the cause of that intermittent
failure is not established.
All five repository checks pass. A focused native ownership check also passes:
frame-only updates retain cached text, inline mutations invalidate the paragraph
and its enclosing canvas, selection indexes only mounted paragraphs, and teardown
releases the inline tree. The final macOS and iOS builds include that canvas fix.
