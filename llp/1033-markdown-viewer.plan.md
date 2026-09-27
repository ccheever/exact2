# LLP 1033: Markdown viewer — a useful macOS application first

**Type:** Plan
**Status:** Draft
**Systems:** Text, Apple host, Web host, Contract, External apps
**Author:** Charlie Cheever / Codex
**Implementer:** Codex, starting 2026-09-05
**Date:** 2026-09-05
**Revised:** 2026-09-08
**Related:** LLP 1001 §6, LLP 1008 §3/§5, LLP 1010, LLP 1011, LLP 1030 D2 (the manifest is the one declaration), LLP 1031 D1/D6 (the embedder's value seam), LLP 1016 D1/D2 (the runner does no I/O)

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

## Decisions

Written 2026-09-08 with the landing below, on Charlie's ask: "one thing I
would like is for exact2 apps to be easily runnable from the command line on
macOS … as a test for this, I'd like to make two markdown viewing apps." The
two readers are the test; the three decisions are what the test forced.

### D1 — What an app opens is `file_handlers`, and the Apple bake derives the plist from it

The manifest gains the W3C Web App Manifest's own `file_handlers` key — an
`action`, a `name`, and an `accept` map of MIME type to extensions. The macOS
bake turns it into `CFBundleDocumentTypes` (`host/apple/build.mjs`), so an
Open panel, Finder's Open With, and `open -a` all offer exactly what one
declaration says. The hand-written `host.macos.documentTypes` that stood in
for it is deleted, not deprecated: nothing shipped against it.

One deviation, declared: the web has no MIME type for a directory, and the
LLP reader opens one. `inode/directory` — freedesktop's spelling, not IANA's
— maps to `public.folder`. A MIME type with no mapping fails the bake rather
than guessing `public.data` (LLP 0382).

Document types are declared `Viewer` and `LSHandlerRank: Alternate`, so
installing a reader never takes `.md` away from whatever already owns it.

### D2 — `exact` is the command line, and what it runs is a bundle

`scripts/exact.mjs`, on `PATH` through `package.json`'s `bin`:

- `exact run <app> [file …]` — build, then launch in the foreground with the
  app's log on this terminal and ^C ending it.
- `exact install <app>` — `~/Applications/<Name>.app`, registered with Launch
  Services, plus a shell shim named by the manifest's new `app.command`
  (`mdview`, `llpview`) in the first of `~/.local/bin`, `/usr/local/bin`,
  `~/bin` already on `PATH` (`EXACT_BIN_DIR` overrides). The shim runs
  `open -a`, so a second `mdview` hands its file to the copy already running
  instead of starting another.
- `exact uninstall <app>`, `exact list`.

Both verbs launch `<Name>.app/Contents/MacOS/ExactMac` — the executable
*inside* a bundle. A bare Mach-O, which is what `build.mjs --run` launched
and still launches, has no Info.plist, so it has no name, no document types,
no Dock tile, and Launch Services cannot find it. `--bundle` now assembles to
one stable path (`target/clients/<source-key>/<id>/macos/<Name>.app`) instead of a fresh
`mkdtemp` per build, because a path that changes every build is a path
nothing can be registered at.

The shim is a script, not a symlink into the bundle: a symlinked executable
takes the bundle identity of whatever the symlink sits beside, which is not
the app.

### D3 — Every way of opening a document ends at one node

A path reaches the app as the value of the single live node whose `testId`
is `open-file` — the seam an embedder already had (LLP 1031 D1), no new ABI,
no ninth agent operation. Four routes arrive there and are identical
afterwards: paths on the command line, `file:` URLs from Launch Services
(Finder, Open With, `open -a`, a second `mdview`), File ▸ Open… (⌘O), and a
followed link that names a file. ⌘O is the app's when it declares documents;
Develop ▸ Open Project… moves to ⇧⌘O behind it.

The host never reads the file. It hands over a path; the app's data source
reads and parses it on the host's worker through a continuation (LLP 1016
D1), so neither the read nor the parse is on the thread that lays out. An app
with no `open-file` node opens nothing and says so on stderr.

A followed link is `openURL` in the session delegate: a local path is a
document for this app, `http`/`https`/`mailto` goes to the system, and
anything else is refused — a link comes out of a document this app did not
write, and handing an arbitrary scheme to Launch Services hands it whatever
is registered for that scheme.

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

## Two readers, in this repository, 2026-09-08

Charlie asked for exact2 apps to be easy to run from a terminal on macOS, and
for two Markdown readers as the test of it. Both are in-repo apps beside
Caltrain and Fieldnotes; the external `~/projects/exact-markdown` of
2026-09-05 is gone and nothing here depends on it.

- **`apps/markdown`** — the general reader. `mdview README.md`, `mdview .`,
  a Finder double-click, or a path typed in its field. A folder strip lists
  the Markdown beside the open file; a link to a file opens it here, a link
  to a page goes to the browser.
- **`apps/llp`** — the same document, specialised. `llpview llp/` opens the
  corpus at its lowest-numbered document. The index is in number order with
  sub-documents (`1030.000`) indented under their parents and each row's
  kind, `current`/`foundation` overlay, and status; search runs over every
  document's *text* and counts hits per document; the outline shows one
  section at a time; `LLP 1234` and `RFC 0491` in prose are links to the
  documents they name.

**What is shared, and what the language would not let us share.**
`apps/markdown/parse` (`markdown-parse`) is one crate: the parser, the block
and run model, and the conversion into plan values. `apps/llp/data` depends
on it. The `Blocks` and `Runs` components are **copied** between the two
`app.contract` files: `use … from` resolves only inside one app directory
(`validate_use_path` refuses a `..` segment, and `load_source` refuses a path
outside the entry file's parent), so two apps cannot share a `.contract` file
today. Widening that is a compiler change with a compatibility-id
consequence and is not taken here. If a third reader appears, it is the
trigger.

**Driven and inspected.** `exact run markdown rules/RULES.md` opened the file
in a titled 1000×760 window named Markdown; `open -a Markdown.app QUEUE.md`
into the running copy replaced the document rather than launching a second
process; `llpview ~/projects/exact2/llp` opened the corpus from a bare shell.
Through the agent driver: `type open-file <path>` renders README.md and a
real LLP with headings, prose, emphasis, code spans, links, lists, block
quotes, rules and tables; `type search continuation` narrows 44 documents to
5 with per-document hit counts; `tap outline-11` filters LLP 1033 to its four
Bounds-and-evidence blocks; `tap document wheel 0 1400` scrolls the document
pane to 1400 while the header stays at y=0. Both apps also build for the web
and render the welcome document in headless Chrome.

**Two layout facts this found**, both of which had the reader looking wrong
before they were understood, and both of which are the kernel behaving as
written rather than as CSS reads:

- `line-height` is an absolute length, not CSS's unitless ratio. `1.62`
  meant 1.62 points and every line drew on top of the last. The readers use
  point values (26 for 16 px prose). This is an undeclared deviation from
  "the web is the standard"; it is noted in `QUEUE.md`, not fixed here.
- ~~A style row does not cascade into the inline `text` nodes the kernel
  measures as runs, so a heading whose runs said nothing drew at 16 px. Each
  run carries its own size, weight, height and colour; the paragraph's type
  is a prop of the `Runs` component.~~ Landed 2026-09-09 (LLP 1035.000 slice
  1, LLP 1001 §6): the text rows inherit into runs on every host, so the
  paragraph carries `color` and `line-height` and `Runs` threads only the
  size and weight it computes per run (the mono run's 0.92 em and the
  bold-or-base weight — CSS `em` and `bolder` are what would remove those).

**Tests.** `markdown-parse` has twelve, one of which parses every document in
`llp/` and asserts blocks, titles, headings and tables come out of the real
corpus. `llp-data` has five over the same corpus: number order with
sub-documents nested, the `current/` overlay at exactly 15, search over
document text, dotted references resolving, and code spans and existing
links left alone. All five repository checks pass.

**What remains.** File ▸ Open… (⌘O) is wired and the menu bar is present,
but a modal `NSOpenPanel` is not drivable through the eight operations, so
it is not driven — only its absence of a key collision with Develop ▸ Open
Project… is. iOS file opening, the browser's `launchQueue`, syntax
highlighting, heading anchors, and file watching are unbuilt. There is no
scrolling to an anchor in v1, which is why the outline shows a section
rather than jumping to one.

## Three scrolling faults, 2026-09-08

Charlie, reading with them: the main pane scrolled sideways a little, the
sidebar did too, and scrolling past either end bounced the whole window —
chrome included — rather than the pane under the pointer.

The first two were one fault and it was the reader's, not the kernel's.
`box-sizing` defaults to `content-box`, as CSS says it should, so every
`width: 100%` element that was also given padding came out wider than its
parent by exactly that padding — a list item indented by its depth, an index
row indented by its nesting, a pane with a 1-point border. Each of those
widened its scroll container's content, and a scroll container whose content
is wider than itself scrolls sideways. Every such element is now
`box-sizing: border-box`, and the three panes are `overflow-x: hidden`, so a
long unbreakable filename is occluded at the pane's edge instead of turning
the pane into a sideways scroller. The panes are now exactly 300, 730 and 250
of a 1280-point window, and no node's right edge exceeds the viewport.

The third was the host's and is fixed for every app. AppKit's `.automatic`
scroll elasticity rubber-bands an axis the page cannot actually scroll, so a
pane that reached its end and chained the rest of the gesture up to the page
(`ChainingScrollView` forwards to `nextResponder`, LLP 1010's nested
chaining) dragged the whole window's content even though the document was
exactly the viewport. `PageScrollView.syncElasticity` now follows the
browser's rule — an axis is elastic only while it has somewhere to go — and
is called from `tile()` and from `fitDocument()`. Chaining itself is
unchanged: Caltrain's fixture still reports "the scroll node stops at 652,
then the page scrolls (root at -501)" on every run.

What this does not do is make the inner pane bounce at its own ends. Nothing
bounces now when the page fits. A pane with its own elastic overscroll needs
either `overscroll-behavior` as a kernel row (the web's property for exactly
this, absent from `kernel/tables/schema.json`) or AppKit's native bounce
inside `ChainingScrollView`, which today clamps by hand for the reason its
comment gives. Neither is taken here; the entry is in `QUEUE.md`.

Repeated `smoke.mjs macos` runs after the change show one constant failure —
the cover-viewport titlebar difference already queued from 2026-09-06 — plus
a different intermittent each run (deck card placement, a hover highlight, a
motion seek reading 50 where 75 was expected, the last of which this document
already records as unexplained). None involve scrolling.

## D4 — `overscroll-behavior`, so a pane bounces at its own ends, 2026-09-08

Charlie, after the chrome stopped bouncing: "the overscroll … doesn't happen
in the main pane or side and i sort of think it should." It should, and the
web already has the property that says so.

`overscroll-behavior` is now a kernel row per axis
(`overscroll_behavior_x`/`_y`, bits 81–82, enum `auto | contain | none`,
default `auto`) with CSS's meaning: **auto** hands a gesture this scroller
has run out of room for to the enclosing one (scroll chaining, what Caltrain
depends on and what the mask carried before this); **contain** keeps the
gesture here and shows the platform's overscroll affordance — the rubber
band; **none** keeps it here and shows nothing.

It cost four small edits and no new host plumbing, because the two seams it
crosses are already generic: `host/apple/src/style.rs`'s `style_json`
serializes any set row by name, so the value reaches Swift for free, and
`host/web/src/css.rs`'s fallback turns `overscroll_behavior_x` into
`overscroll-behavior-x` and hands the browser the property it already
implements. The macOS half is `ChainingScrollView`: a contained axis is given
to AppKit whole — `super.scrollWheel`, with elasticity `.allowed` for
`contain` and `.none` for `none` — because the elastic state machine wants a
gesture's phases and not its tail; a contained axis with nowhere to go
swallows the event, which is what `contain` forbids passing on. An `auto`
axis keeps the hand-clamped path exactly as it was.

Both readers declare `overscroll-behavior: contain` on their panes. The page
elasticity rule from earlier today stays and is now the second half of the
same story: the chrome does not bounce because the page has nowhere to go,
and the pane does because it does.

Driven: both panes still scroll (index to 400, document to 900) through the
new AppKit path, so a synthetic wheel is not lost by it; Caltrain, which sets
no `overscroll-behavior` and so is `auto`, still reports "the scroll node
stops at 652, then the page scrolls (root at -502)" on repeated runs, with
only the known cover-viewport failure. All five checks pass. The rubber band
itself is not driven — an elastic bounce needs a real gesture's phases, and
the eight operations send a wheel without them.

### D4a — the lift is a zero-delta event, 2026-09-08

The first cut of D4 stretched and stuck: Charlie's pane overscrolled "like
it's in the mud" and stayed there. The band was forming and never releasing.

`ChainingScrollView` opened with `if dx == 0 && dy == 0 { return }`, which
predates this work and was harmless while every axis was hand-clamped. It is
not harmless now. A trackpad gesture's **lift carries no delta** — a
zero-delta event whose `phase` is `.ended`, and momentum finishes the same
way — so AppKit's elastic state machine never learned the fingers were up and
held the stretch. A contained axis now forwards every phase-bearing event to
`super`, delta or not; an `auto` axis, which is driven here and has no use for
them, still drops them, so Caltrain's chaining is byte-for-byte unchanged.

Elasticity also moved out of `scrollWheel` and into `applyStyle`, written only
when it differs: it is a property of the node's style, and writing it while a
gesture is in flight disturbs the machine it exists to enable.

Ruled out by reading, not by guessing: nothing re-clamps the clip origin
during a contained gesture (`contentView.scroll(to:)` is only on the `auto`
path), and `clipScrolled` repaints and refreshes visible text without
touching the offset.

Still not driven. The eight operations send a wheel with no phases, which is
precisely the input class this bug lived in — the agent could scroll the pane
perfectly while a real trackpad stuck. Verifying an elastic release wants
synthesized phase transitions and a settled animation; that is a timing test,
and `rules/RULES.md` prefers no check to a flaky one. The feel is confirmed by
hand.

### D4b — three ways to see a host bug, 2026-09-08

Charlie, after D4a: "let's do all three of them." What the rubber-band bug
showed is that a host's *input routing* is invisible to every check this
repository has — the five run Rust, the agent's wheel carries no gesture
phase, and a screenshot draws a stretched band and a scrolled pane the same.
Three additions, in the order of what they cost:

**`layout` reports overscroll.** A scroller past its own ends now carries
`ox`/`oy` beside `sx`/`sy`, rendered as `overscroll {ox},{oy}`, on the page
as well as on each node; iOS reports the same from `contentOffset`. Not a
ninth operation — one more fact on a line `layout` already prints, and a
pure function of geometry, so it cannot flake. The sign says which end.

**`tap … wheel <dx> <dy> gesture`.** A trackpad's gesture, not a bare delta:
`.began`, `.changed`, and the zero-delta `.ended` that is a lift, all
delivered inside one request. `rules/DEFERRED.md` admits this explicitly —
"a new input is a form of `tap` or `type` (a wheel is a `tap`; so would a
drag be)" — so the eight operations are still eight. The comment that used
to sit here refused phases because they "would put the top-level scroll view
into a tracking loop that a synchronous call cannot feed"; a gesture
delivered *complete* is never left waiting, and the driver does not hang. A
host that cannot phase a wheel refuses the flag rather than quietly sending
a plain one.

**`ExactKitTests`.** A Swift test target — `node host/apple/build.mjs
--test`, which builds the archive `ExactKit` links and then runs `swift
test`; `EXACT_TESTS=1` narrows the package, because `swift test` otherwise
builds the two UIKit executables and they cannot build for macOS. Not one of
the five checks, which `rules/RULES.md` caps at five. `ChainingScrollView`'s
decision is now a named `Routing` computed apart from acting on it, and
`Agent.overscroll` takes numbers rather than a view (`NSClipView` clamps an
origin set through its own API while a rubber band puts one out of range, so
a test could not otherwise build the state). Fifteen assertions, 0.06
seconds, no window, no run loop, no clock. Checked by reintroducing the
D4a defect: `testAZeroDeltaLiftReachesAppKitOnAContainedAxis` fails with
"drop is not equal to appKit" and passes again when it is restored.

**And the three found a fourth bug, which is the point.** Handing a contained
axis to `super.scrollWheel` made its scrolling *asynchronous* — AppKit scrolls
over several frames — so an agent that wheels and then reads the offset races
the animation. It read 600 sometimes and 0 others; the "both panes still
scroll" checks earlier in this document were racing and got lucky. The fix is
also the correct platform behaviour: only a *phased* gesture goes to AppKit,
because elastic overscroll is a trackpad's and a mouse wheel does not
rubber-band on this platform. A phaseless wheel is clamped here, which is
synchronous, and `tap document wheel 0 600` now reads 600 three runs out of
three. A gesture's effect stays asynchronous by nature; `gesture` is for
driving the path a finger takes, not for reading an offset back in the same
breath.

Caltrain's chaining fixture passes on repeat, all five checks pass.

### D5 — the appearance switcher is exact1's blog icons, 2026-09-08

Charlie: use the system/light/dark SVGs the exact1 blog used instead of two
text buttons. The artwork is `js/src/blog/lib/icons.ts`'s `monitor`, `sun`
and `moon` — Lucide strokes on `currentColor`.

They ship as **PNG**, not SVG, and the reason is worth writing down because
it is a real boundary of this repository rather than a shortcut. An Exact
`image` is a raster asset (LLP 1011: `image/png`), and the Apple host decodes
through `CGImageSourceCreateWithData` — ImageIO, which has never read SVG.
The web host already serves `.svg`, so an SVG would have worked there and
nowhere else; making it work on Apple means either a second decode path
through `NSImage` or a rasteriser, and neither is proportionate to three
icons. The exact1 mechanism that made these tintable — an `image` carrying
`svgSource` rendered as inline `<svg>` reading `currentColor` — does not
exist here, so the icons are one fixed grey and *which one is chosen* is said
with a pill and with `opacity`, not by tinting the artwork. They were
rasterised at 48 px (3× their 16-point box) with the headless Chrome the web
smoke already uses; the one-off script stayed in the scratchpad and the PNGs
are the assets, as `apps/caltrain/assets/caltrain.png` is.

Following the system is now a real third choice, and on every host it is the
*absence* of an override rather than a third appearance: `app.appearance =
nil` on macOS, `.unspecified` on iOS, and CSS's `color-scheme: light dark` on
the web — which is the property saying "the page supports both, the user's
preference decides". `light` and `dark` are unchanged.

What this does **not** do, and what the two text buttons did not do either:
change the readers' own palette. Contract has no scheme-aware colour — no
`prefers-color-scheme`, no colour that resolves per appearance; every colour
in these documents is a literal. So choosing Dark darkens the window's
chrome, its scrollers and its native controls, and leaves the page white.
A real dark palette needs a scheme-aware value in the kernel or a `scheme`
prop threaded through every component; neither is taken here.

Driven: the switcher renders the three icons with the chosen one in a pill,
`tap scheme-dark` moves the pill and logs `command setScheme("dark")`, and
the same works in the Markdown reader. All five checks pass, the fifteen
Swift host tests pass, and Caltrain's smoke is unchanged.

### D6 — a real dark theme, and what "use Facet" can mean here, 2026-09-08

Charlie: switching to dark only changed the title bar; make it a real dark
theme, and "try to use the facet system please."

**There is no Facet system here, and there cannot be the one exact1 had.**
Facet's theme is a *capability with signal semantics* — a token store in
`@exact/facet-core` shared with React, `$`-prefixed token references through
a registered resolver, scope subscriptions, fine-grained updates. Every layer
of that is refused by standing rules: no second value graph and no bindings
(`rules/DEFERRED.md` §Motion), no React Facet bindings, nothing running
JavaScript above the data seam, and Contract's `use` admits no packages —
`contract/corpus/rejects.txt` carries `use theme from "@exact/facet-contract"`
as a *refusal fixture*.

What survives is the idea, in the language's own terms, and it needed nothing
new. Contract already has `provide`/`inject` — which exact1's own theme file
calls "Contract's replacement for React context" — and it has a data seam. So
the tokens are **data**: `markdown-parse::theme` answers one `shape Theme` for
an appearance, each reader declares `resource theme = theme(scheme,
systemScheme)`, `provide`s it once at its root, and every drawing component
`inject`s it. No runtime, no signals, no lookup: the compiler fills each
`inject` from the enclosing `provide` (LLP 1017 P4a) and a missing provider is
a compile error. Twenty-one tokens; forty-odd literal hex values gone from the
two views. It is also how the two readers share a palette when they cannot
share a `.contract` file — colours are data, and data crosses.

**Following the system needed a fact no host reported.** `env()` carries
lengths only — insets and the keyboard — and nothing told the app what the OS
was set to, which is why "system" could only ever have been light. The host
now reports it through the seam documents already use (D3): a `view` can
carry a `change` handler, so a zero-sized node with `testId="appearance"`
receives `"dark"` or `"light"` at launch and again whenever it changes. The
reading is `AppleInterfaceStyle` in the global domain, *not*
`NSApp.effectiveAppearance`: an app that has set itself dark reports dark, so
following the system would have followed the app.

`setScheme` still does what it did — the window's chrome, its scrollers, its
native controls — and now the page follows it. The two are separate on
purpose: the appearance is the host's, the palette is the app's.

The icons are two drawings rather than one tint: a PNG has no `currentColor`,
so `theme.icons` names the suffix (`""` or `"-dark"`) and the same Lucide
artwork is rasterised in each palette's ink.

Driven: `tap scheme-dark` resolves `theme.page` to `#17181b` and redraws both
readers — chrome, index, code spans, accent, pills, field, and the switcher's
own artwork; `tap scheme-light` returns them. Text with no declared colour
draws black, which a dark page makes invisible, so the three bare labels and
the text fields now name `theme.ink` explicitly. All five checks pass, the
fifteen Swift host tests pass, and Caltrain's smoke is unchanged.

**Not done, and worth naming.** Only macOS reports the system appearance. iOS
would read `UITraitCollection` and the web `prefers-color-scheme`, each into
the same node; until then "system" on those surfaces draws light while the
host's own chrome follows the OS. And CSS's real answer to this — a
`light-dark()` colour the kernel resolves per scheme — would remove the
`systemScheme` slot and the reporting node altogether. It is the better
long-term shape and it is a kernel change, not taken here.

### D6a — superseded by LLP 1034, 2026-09-08

D6's `systemScheme` slot and `appearance` reporting node were built because
no host could tell an app what the system was set to, and every colour needed
to know. LLP 1034 landed the same day and removed the need: a colour row now
holds a `light-dark()` pair and the host resolves it, so **no colour depends
on the appearance** and the twenty colour tokens are one constant record.

What survives, and why: `iconSet`. The switcher's artwork is a PNG, which has
no `currentColor` to follow, so the drawing is chosen rather than tinted —
and choosing needs the appearance. It is now derived in Contract
(`(scheme == "system" ? systemScheme : scheme) == "dark"`) instead of taken
from the data source. `ExactDocuments.reportAppearance` and its watcher stay
for that one consumer. A painted `tint_color` would remove it; LLP 1011 §6
puts that outside v1.

D6's account of Facet is unchanged and still the reason the tokens are data.

## D7 — reading a corpus, 2026-09-08

Charlie, using the LLP reader on this repository. Six changes, all of them
the app's except the last.

**No weight on the chosen row.** The index marked the open document with
`font-weight: 700`, and bold text is wider than the same text unbolded — so
selection reflowed the row and the list shifted under the pointer. Selection
is a fill and a colour now (`theme.pill`, `theme.pillInk`), which cost no
geometry. The header's controls got the same treatment: `Chip` is one
component, on or off by fill, never by weight.

**The wordmark is gone.** "LLP" in the top left said what the window already
says, and the space is the Files toggle's now.

**A Files toggle beside Outline.** The index hides the way the outline does;
both are `Chip`s and both keep the panes' `overscroll-behavior: contain`.

**Newest first, as a toggle rather than a default.** The end of a corpus is
where the new documents are and where a reader most often starts, but number
order is the corpus's own, so it stays the default. Reversal is of the whole
list, which keeps a sub-document under its parent: the nesting is the point
of the order, not its direction.

**The declared working sets are filters.** `current` and `foundation` are
`llp/`'s two symlink overlays (`rules/RULES.md` §Budgets), already read by the
index for its badges; they are now also a way to see only what they link.
Pressing the one that is on clears it. `current` shows exactly 15, which is
the cap the rules state — the reader is now a way to check that.

**The window says which project is open.** "LLP — exact2", from the folder
containing what was opened; "Markdown — exact2" for a file in that
repository. The folder rather than the file because a reader shows the
filename in its own chrome, and because the project is what you pick out of a
window list. It is the host's, derived from the path Launch Services or the
command line handed over (`ExactDocuments.windowTitle`), because the app has
no hook that fires when a resource settles — `task` is `mount` only — and a
title pushed from an action would be one open stale. Four tests in
`ExactKitTests` state the rule; one of them caught that an empty path
resolves against the process's working directory, which would have titled the
window after wherever the app was launched from.

Driven: 45 documents in this corpus, `current` filters to 15, `newest` puts
1034 first with 1030.002/001/000 still under 1030, and the index pane hides
and returns. All five checks pass, nineteen Swift host tests pass, and
Caltrain's smoke is unchanged.

**Two corrections the same day, both Charlie's.** Hiding Files hid
*everything*: wrapping the index in `when showIndex` had swallowed the
document and outline panes with it, an indentation error the compiler could
not see because the result was still a valid tree. And the filters were in
the header, three panes away from the list they change; they are at the top
of that list now, outside its scroll so they stay put while it moves — a
filter you have to scroll back up to reach is a filter you stop using. Only
`Outline` stayed in the header, beside the pane it controls.

## D8 — `exact release`, the build a teammate can open, 2026-09-08

`exact install` makes an app for this Mac. A teammate's Mac refuses it: the
zip arrives quarantined, and an app that is not notarised is "cannot be
opened because the developer cannot be verified". Confirmed rather than
assumed — setting the quarantine attribute Safari would set and running
`spctl --assess` returns **rejected**.

`exact release <app>` (also `exact install <app> --release`) is the path that
clears it, and each of its three parts is required by the next:

1. **A Developer ID Application certificate.** Not "Apple Development",
   which signs what you run on your own machines; Apple will not notarise a
   build signed with one. This Mac has only the latter, so the verb refuses
   before building and says where to get the other.
2. **The hardened runtime and a secure timestamp** — notarisation's
   requirements, not preferences; a build without them is rejected at
   submission. Signed innermost first, because a bundle is sealed over its
   contents and a library re-signed after its container invalidates it.
3. **Notarisation and stapling.** `notarytool submit --wait`, then `stapler`
   into both the `.app` and the `.dmg`, so they open on a machine that is
   offline. The zip is rebuilt from the stapled app, since a zip carries no
   ticket of its own. It ends by asking `spctl` what a teammate's Mac will
   decide, and refuses to hand over artifacts that would be rejected there.

Credentials are never arguments: `notarytool` keeps them in the keychain and
this passes the profile's name, because an app-specific password on a command
line is in the shell history and the process table.

**What is verified, and what is not.** The refusal paths both fire with the
command to fix them — no certificate, and no keychain profile (the second
needed correcting: notarytool reports that one on *stderr*, and says "No
Keychain password item found", not what the first draft matched on). Forcing
the identity to the development certificate carries the rest through:
signing inside out, `codesign --verify --deep --strict` reporting "valid on
disk" and "satisfies its Designated Requirement", `flags=0x10000(runtime)`
and a timestamp on both the bundle and the dylib, and the packaged app still
launching and `dlopen`ing its WebKit arm — the hardened runtime's library
validation admits it because it carries the same team's signature. The
submission, the staple, and the Gatekeeper acceptance are **not** verified,
and cannot be from here: they need a Developer ID this Mac does not have.
