# What Exact v1 Does Not Do

This is the most valuable file in the repo. It is longer than the doing-list on purpose.

## The bar that makes this list derivable

v1 is done when **one real application** — not a demo — runs from a single Contract
source on web, macOS, iOS, and Linux, hitting the time budgets in `RULES.md`.

**Decided (Charlie, 2026-08-30):** the Caltrain app is the one that defines v1 — it
already exists in Contract, it has real lists, navigation, search, text, and theming, and
"does it still work" is answerable in seconds — **and Weird Castle's wordmark is in v1
beside it**, which puts one bundled brand face in scope and nothing else about that app.
Everything not required by those two does not exist. What the wordmark unblocks: a
declared font, LLP 1019. Fonts were never on this list, so nothing comes off for them.

**Expanded (Charlie, 2026-09-05):** the Markdown viewer (LLP 1033) is a third
consumer: macOS file opening and comfortable reading first, shared with iOS and
web. Take: native-module implementation (LLP 1024, awaiting a consumer) moves
behind the reader. Blog/CMS and publishing remain out.

**Expanded (Charlie, 2026-09-07: "make a real app that uses them"):** Fieldnotes
is the storage consumer: notes in SQLite, backups in app-scoped files, and a
multiline editor on web and Apple. Take: further general-purpose API expansion
waits behind proving these shipped bindings in the app; Snapback2 stays deferred.

**Expanded (Charlie, 2026-10-08, option A for protocol-handler URL templates):**
`app.json` `protocol_handlers` carries a complete custom-scheme URL through
the existing navigation root as an encoded handler location (LLP 1038 D8).
The OAuth/sign-in callback and return-link consumer of issue #268 can read its
scheme and fragment. A no-router URL event remains deferred; Charlie's ruling
keeps the navigation root requirement. Take: no second delivery mechanism is
added.

**Expanded (Charlie, 2026-10-26, #1003):** an iOS app can ask to be the default
browser. `host.ios.webBrowser: true` emits Apple's managed
`com.apple.developer.web-browser` entitlement, registers the `http` and `https`
`protocol_handlers` the platform requires, and fails the build, naming the
profile, when the signing profile does not grant the entitlement. Apple
approves each app. Nathan Schroeder implements it, with T3 Code's browser as
the consumer. Take: none offered; waived by Charlie. Still out: http and https
handlers for any app that does not opt in.

**Expanded (Charlie, 2026-10-26, #1002; LLP 1133.028 §10):** Apple builds write
a privacy manifest (`PrivacyInfo.xcprivacy`) into each bundle that links a
required-reason API (the app, a widget extension, `ExactCore.framework` and
the other Exact frameworks). It is generated from an audited table of the
APIs Exact itself calls; a call with no approved reason is removed rather
than given one. Nathan Schroeder implements it (#1004). Take: none offered;
waived by Charlie. Still out: an app's own declarations, until an app with
native code of its own asks.

**Expanded (Charlie, 2026-10-26, Excord):** a virtualized `list` takes
`reach-margin`, a nonnegative `px` or `%` distance along the scroll axis, one
viewport by default, saying how far ahead of an end `reachstart` and
`reachend` fire. With it, the default edge band follows the collection's own
lead, as LLP 1027.004 D3 has it, so a fling that the window preloads for also
fires the edge event. Take: none offered; waived by Charlie.

**Expanded (Charlie, 2026-10-11, events ruling):** `keycapture=` admits DOM
capture-phase key dispatch for the coding-agent desktop app's early shortcuts;
`focusin`/`focusout` admit bubbling subtree focus observation for the clipped
block and row actions reported in #283. The ruling names no take. Held
modifiers continue to use `keyup` plus page focus; no new
keyboard fact or Tab stop is admitted.

**Taken off (Charlie, 2026-09-23: "Take off Messages as the Snapback4
consumer. We don't need that on the list"):** the trade for web rendering
(LLP 1048, below). Messages is no longer the Snapback4 consumer admitted
2026-09-12; Interview, built on Snapback4, is. Snapback2 stays deferred.

**Expanded (Charlie, 2026-09-14):** language parity for existing app operations
(LLP 1027.001): shared standard text/URL helpers, portable storage requests,
reusable mixed composition, and Fieldnotes backup in either language. Unblocks
moving app logic without changing durable data or permissions. Take: remove
app-private text helpers and Update Lab's private composer; generalized native
views and multi-module scheduling stay behind this consumer.

**Expanded (Charlie, 2026-09-14: "let's build this now"):** optional worker
placement for data modules (LLP 1027.002) — one module instance, one executor
owner, values cross and the runner commits; `main` stays the default and the
manifest opts a module in per platform. Unblocks app computation off the UI
thread with the same inputs, results, grants and durable data, ahead of a
measured hitch, for the story and because the limits will be hit. Take: Rust
on a web Worker waits behind a consumer that needs it (placement is
language-neutral on native, TypeScript-only on the web); a second worker, job
pools and load balancing stay out; the mixed-resource first-frame bake is
built with it rather than queued. The measurement bar in LLP 1027.002 §2 says
when a worker is claimed to help.

**Expanded (Charlie, 2026-09-16):** graceful-overload design and two bounded,
synthetic stress consumers (LLP 1041): Messages interaction under load and
async completion storms. Unblocks measuring queue pressure, UI settlement
bursts, and large-history costs on existing hosts. Take: broad new benchmark
apps and a generic job scheduler stay behind these two; no sixth blocking
check, additional worker pool, or parallel layout is admitted by these examples.

**Expanded (Charlie, 2026-09-16, Astra xhigh campaign):** add gigantic Markdown
and continuous native resize to LLP 1041's stress consumers; evolve bounded
native scheduling and viewport collections against these workloads. Take: more
showcase apps and speculative general job pools wait behind measured improvements
to these three. Existing ordered-effect and capability boundaries still apply.

**Expanded (Charlie, 2026-09-16: "Work through all four"):** continuously
interactive Messages, photo zoom, virtualized reorder and a draggable sheet with
a nested collection (LLP 1041 §8.5). Unblocks proving gesture takeover, collection
lifetime and background scheduling together on the existing hosts. Take: other
showcase apps and decorative effects wait behind these four and the original
three workloads. Shared-element presentation and the bounded geometry/motion
work those consumers need are admitted; a generic gesture arena, second
application-state graph and speculative parallel layout remain out.

**Expanded (Charlie, 2026-09-28: exact2's web support, not "Exact 3"):** the
web build's JS target (LLP 1071): the same Contract compiled ahead of time to one
ES module over a ~20 KB runtime, the DOM as the tree, used by default for an app
it compiles; the Rust render host stays the primary pre-render and the JS render
the option (his ruling of the same day). Unblocks a web first interaction in
about a tenth of the wasm target's bytes. What the JS target refuses fails an
app's build (a game builds wasm); left open: state carried across the
dev loop's reload and the rest of the agent (node detail, iframe outlines,
accessibility props, gesture and key deliveries; the smoke's fixtures build `--wasm`);
the JS render path's documents (canonical, og, robots, status, sitemap).
Take: none named yet (LLP 1071 §8 asks). A Rust source's Canvas 2D surfaces and
declared fonts closed on 2026-09-29 (Charlie's ask, relayed; LLP 1071 §7), as did
virtualized lists, `reachstart`/`reachend`, a dynamic `line-height`, and an app
outside the repo (Bluesky builds on the JS target); so did dynamic composite rows
(`clip-path`, SVG paint and dashes, `filter`, gradients, `animation`, the timeline
rows, an eased `transition`) and full SVG (Sparkline and SVG Gallery build); `performanceNow()`
readers, the reserved sources `exactPage`, `exactDelivery` and `exactSurface`,
localized strings, `openAuthSession` and `scrollIntoView` (LLP 1071 §7, "The runtime
gaps"); and `pan`, `panrelease`, `swiperight` and spring transitions over the motion
engine (Spark, Messages and Messages Stress build); and `select`, over the web
host's own Markdown editor, and the DOM's `cancel` (Markdown Stress builds); and
text around shapes over the web host's exclusions executor (Reflow and Text Flow
build); and the height and transform drags (`heightrelease`, `transformgeometry`,
`transformrelease`); and exit animations and layout transitions (presence-glue.js); and the build's gaps: the GPU module, Canvas 2D drawn by a TypeScript
source, a TypeScript and a Rust source in one app, native modules, delivery (the web
root publishes the JS build of the bake's plan; the web needs no update client) and
the dev loop's rebuild (~0.1 s an edit) — LLP 1071 §7, "Build and toolchain gaps";
and the reorder drag (`reorderdrop`, reorder on a virtualized list) and `frame`/
`measure` (Interaction Gallery and Exact Live build); and
files and storage: a file input and `showPicker`, `saveFile`, the document
pickers, `share`, and app storage for TypeScript and Rust sources (Fieldnotes and
Markdown build) — LLP 1071 §7, "Files and storage";
and press feedback, and every dynamic style row (shadows, font stacks, line clamp,
numeric variants, backdrop blur, press scale, a pressed node's animation and
transition, an SVG `transform`, markers and `url(#…)` references); and native
clients on the dev URL (the JS loop forwards them to the resident loop's producers,
which build the bake, not the wasm), the router sweep on the JS target,
delivery's bake without `app.wasm`, and animated images on the agent's clock — LLP
1071 §7, "Retiring the wasm target on the web". Games stay on the wasm target
(Charlie, 2026-09-29).

**Expanded (Charlie, 2026-09-17: one impressive demo on iPhone, macOS and web):**
Exact Live combines crew chat, photos, a runbook and background jobs in one
authored workspace, using existing hosts and data seams. Unblocks showing the
interaction and performance work together in a coherent application. Take:
additional standalone showcase apps wait behind this composition; existing full
stress workloads and controls remain intact. No new UI framework, general job
scheduler or cross-device synchronization service is implied.

**Expanded (Charlie, 2026-10-22: "Let's build a Discord app for iOS, MacOS, web,
Windows, Android"):** Excord (`apps/excord/llp/0001-excord.rfc.md`), a client
for the real Discord whose messages list and composer are its bar, built to find
Exact's gaps. Unblocks a real service's chat on every host as the consumer for
the transcript work. Take: waived by Charlie's ask. Voice, video, uploads beyond
images, and push notifications stay out (its §1); no Discord protocol enters
Exact itself.

**Expanded (Charlie, 2026-09-20: "make an app that shows off clones of" the
Pretext demos):** Reflow (`apps/reflow`) is a seventh showcase — balls, the
illustrated dragon, masonry, a drag-to-reflow magazine, proportional ASCII
art, an occlusion-culled wall and an obstacle-aware spread — to show exact2
has Pretext's capabilities: text around shapes from the kernel (LLP 1043.000)
and heights-before-layout from a data crate that measures with advances read
out of the app's font files at build time and breaks lines with the hosts'
own `exact-textflow` walker. Unblocks a public comparison on every host with
no new kernel row and no app-side measurement seam. Take: no Pretext library,
no runtime font parsing, and auto-height flow (LLP 1043.000 §8) stays owed
(Charlie, 2026-10-11: the font-picker consumer may supply installed/generic
`font-family` names as runtime strings under LLP 1019 D2; this does not admit
runtime font parsing, downloaded files or `loadFont`)
(built 2026-09-26 as §8's Stage 2, a bounded re-layout restricted to block
contexts; **accepted (Charlie, 2026-09-27)** on condition that `metrics.mjs`
shows a still layout costs about nothing and names the worst case for a page
with several shapes);
the dragon is a polygon, not a `shape-outside: <image>`.
Linux skips Thai word breaking (Charlie, 2026-09-28): with no segmenter
there, flowed Thai, Lao, Khmer and Myanmar text breaks only at spaces; no
ICU4X dictionaries. Web and Apple take their platforms' words (LLP 1043.000 §8).

**Expanded (orchestrator for Charlie, 2026-10-05, LLP 1093 §8: "admission via
your offered trade"):** CSS multi-column (`columns`, `column-count`,
`column-width`, `column-gap: normal`, `column-fill`, solid `column-rule`) and
the break rules that only mean something inside it (`widows`, `orphans`,
`break-before`/`-after`/`-inside` without page or region values). Native hosts
fragment through the kernel's cut, and the web is the browser's own multicol,
held to Chrome. The reader app's diary is the consumer: pagination and a
two-page spread from one flow. Unblocks book pagination without a polled
height or a duplicated chapter. Take: LLP 1043.000 stage 5
(`shape-outside: <image>`, the dancer's traced silhouette) moves behind
LLP 1093 stage 2. Still out: `column-span`, regions, `@page` (page size,
margins and page-margin boxes), `box-decoration-break: clone`, and slicing
decorated, fixed-height, flex-row and grid boxes (LLP 1093 §5); printing an
element is LLP 1133.024.

**Expanded (Charlie, 2026-10-26, "Accept all (Recommended)", waived; LLP
1133.024 §19 question 6):** printing one element through the platform's
print UI (`print` from `@exact/print`): a print layout of the element at
the page's width and the kernel's cut over pages of the platform's paper,
with `break-*`'s `page` and `avoid-page` values, which do nothing on
screen. Consumer: Fieldnotes' Print note. Unblocks printing and Save as
PDF of a view made for paper without app Swift or Kotlin. Take: none
named; the ruling is its own waiver. Still out (LLP 1133.024 §18):
`@page`, a PDF export without the print UI, slicing a monolithic box
across pages, a multi-column box continued across pages, and repeated
fixed boxes.

**Expanded (Charlie, 2026-09-21: "design and build a markdown editor that works
across all Exact platforms"):** WYSIWYG Markdown editing and one-node Markdown
reading (LLP 1045), Interview first. Unblocks rich posts that stay plain text
in storage and scan cheaply. Take (Charlie, 2026-09-21): the reader's open
follow-ups other than code-block highlighting (heading anchors, file
watching, the compact folder layout) wait behind it; no rich value type,
collaboration or HTML paste import. **Lifted (Charlie, 2026-10-15, LLP
1069.010.000 stage 1):** file watching comes off that wait, the editor being
built: a `doc:` watch re-asks the answer that reads the path, with no new
take. **Expanded (Daehyeon Mun, 2026-10-23; trade; LLP 1045.000):** inline
atoms in the editor and the reader: a span of the source (a link with a
scheme the app names, a relative link or a prefix token the app's faces name)
drawn as one pill with the app's label, icon and title, kept whole by the
crate's editing rules and pressed as `atompress`; the value stays the
source. Unblocks T3 Code's composer chips and transcript citations without a
References row or app editing rules. Take: the Apple hosts' adoption of the
crate's other hidden syntax (`QUEUE.md`, "Markdown editor") waits behind
atoms. Still out: pill padding and radius, a second control inside a pill,
atoms in the Linux, Windows and Android editors (they show the source), and
inline math (a child RFC's). **Expanded (Daehyeon Mun, 2026-10-29, waived;
LLP 1045 D11):** the app gives each source line of the editor its base
direction, `paragraphDirs` in HTML's `dir` words, drawn on the web and Apple.
Unblocks T3 Code's composer, whose original gives each block the direction of
most of its words. Take: waived (Daehyeon Mun, 2026-10-29): one attribute on
the lines the editor already draws, no doing-list item in this area. Still
out: the Linux, Windows and Android editors (they show the source) and the
reader. **Refined (Charlie, 2026-09-21:
"move forward with your plan"):** Native editing stays on TextKit 2;
dependable source editing precedes further marker hiding, with Interview as
the first consumer. **Refined again (Charlie, 2026-09-22: "let's remove
codemirror"):** the web editor is dependency-free `contentEditable`, and the
editing rules are one crate for every host (`markdown/editor/`), shipped on
the web as its own wasm fetched only when a Markdown textarea mounts: an app
without an editor pays nothing for it. CodeMirror is not admitted.
`fileinput` comes off in one form (LLP 1045 D10): an image/video picker for
captioned figures — no camera, no generic file input. **Widened (Charlie,
2026-09-27, LLP 1069.010):** a picker for the types an app declares in
`file_handlers`, plus export and import copies to and from `app:/`. Unblocks
the Markdown reader's remaining surfaces and Fieldnotes' backups. Take: none
named (Charlie: "idk what to trade it for, maybe relax the limit slightly").
**Waived (Charlie, 2026-10-11, coding-agent file attachments):** `input type="file"`
with no `accept` (or `accept="*/*"`) may pick any file, with no trade.
**Extended (Daehyeon Mun, 2026-10-20; no trade):** for the same consumer,
T3 Code's composer, whose drop, paste and Attach files are one path, `drop`
takes files and folders of any type, and a paste carries its files
(`ClipboardEvent.files`, pasted image data as a PNG), each a `doc:` handle
(LLP 1069.010, "Files dropped or pasted").
Specific `accept` types remain bounded as above. `capture` stays refused
(the camera is the `@exact/camera` package, §Components, 2026-10-23). Linux editing is v2
(Charlie, 2026-09-21): web, iOS and macOS first; Linux reads.

**Expanded (Charlie, 2026-09-21):** ship the full game engine from Black's
`lane/game` as an optional add-on, with a full-screen Weird Castle demo.
`game/` is a separate Cargo workspace; games opt in through their GPU artifact,
loaded after first paint. No core crate depends on the engine, renderer, physics
or audio. The core gains only canvas input, state publication, inspection and
save/restore seams. Take: further standalone showcase work waits behind this
consumer; no default engine linkage, core feature matrix or extra blocking gate.
See LLP 1046 §5 and LLP 1046.003.

**Clarified (Charlie, 2026-09-23, LLP 1046.006.000 §7):** game-owned render
hooks and reusable shader-packaging support in shared build tools are within
that optional add-on's scope. The same take holds; core apps gain no engine
dependency. Weird Waters may prove the hooks privately alongside independent
public fixtures on web, macOS and iOS.
**Expanded (Charlie, 2026-10-23, #905):** a game surface's published record
carries a host-owned readiness fact: true once the game's renderer has drawn a
complete frame for the current generation, false again after a restore or a
lost device. The app binds its own pause and loading cover to it; `PENDING`
still permits ticks and the runtime adds no barrier. The Minecraft consumer,
whose player took damage before its terrain drew, is the consumer. Take: none
offered; waived by Charlie. It follows Nathan Schroeder's #967.

**Expanded (Charlie, 2026-09-23: "ok let's do pre-rendering etc."; "let's use
Interview as our website"):** web rendering across the curve (LLP 1048), with
Interview as its consumer: build-time and per-request pages from a native Rust
renderer, adoption in place of hydration, route payloads, cache policies,
inferred partial prerendering and streaming. Unblocks websites that crawlers,
link previews and readers without JavaScript can read, and that paint before
the runtime loads. Take: Messages as the Snapback4 consumer (above). Still
refused: progressive forms (server actions without the runtime) and
server-driven UI; no Node on the render path.

**Expanded (Charlie, 2026-09-23: "let's build it in exact2 and see how we
stack up"):** RealWorld/Conduit (`apps/realworld`), the spec'd Medium clone,
against its hosted API, measured for browser load against a React 19 build of
the same spec. Unblocks comparing exact2 with React on an app we didn't design.
Take: no measurement harness, script or check in this repo; the React build and
the numbers live outside it. Other benchmark and showcase apps wait behind this
one. js-framework-benchmark stays out (Charlie, 2026-09-23: "let's skip it"):
its harness needs a table and Bootstrap class names the web host doesn't emit.

**Expanded (Charlie, 2026-09-27: "Admit Bluesky as a consumer"):** the Bluesky
client (`ccheever/bluesky-exact2`, outside the repo, consuming exact2 by path) is
a consumer. It unblocks what its port asks for (LLP 1054, 1054.000): arguments
on list edges (1054.000.006, landed with this entry), view-side formatting
(1054.000.003) and bounded feeds (1058 P4), each still held to its own review.
Take (proposed with the admission; Charlie may name another): new in-repo
showcase apps wait behind Bluesky's asks. **Clarified (Charlie, 2026-09-27,
LLP 1069.006):** its OAuth sign-in, replacing app passwords, is one of those
asks: the system-browser auth session and the P-256 keys it needs (LLP
1069.005). No trade beyond the admission's.

**Expanded (Charlie Cheever, 2026-09-26: approved implementing LLP 1024):**
native modules — a hyphenated tag is a `NativeView` backed by the app's one
module artifact — with a photo-editor example as the consumer. Unblocks a
platform widget without a host change or a new node type. Take: LLP 1024 §7's
(§Components loses no line; `terminal` and friends stay out as built-in tags).

**Expanded (Charlie Cheever, 2026-10-01: asked for LLP 1075.003 implemented,
its recommended rulings):** native platform control on Apple: hatches on an
app's module that receive Exact's own UIKit objects, `data-*` attributes
declared in `app.json`, a public route-content seam, and Exact's own navigation
bar and retained tab container built on it. James's app is the consumer, and
`apps/native-fixture` the one in the repo. Unblocks reaching UIKit's navigation
and tab containers without patching Exact. Take: new per-property UIKit rows
and any SDK-wide binding generator wait behind it. One exception to "No public
API stability": this surface (LLP 1075.003 §3.9) is named, and a break is one
commit that changes the fixture and says what to change. Still refused: a
deprecation window, native property names in Contract, and a JavaScript UI
tier.
**Expanded (Charlie, 2026-10-03: tab badges, "I think", kept minimal):** a
tab's child that is a filled box holding one text is its tab bar item's badge
(LLP 1075.003 §9.9), the box the web paints. The Signal Clone's unread-chats
count on Chats is the consumer. Unblocks a native tab badge with no module.
Take: none offered; waived by Charlie's approval. No badge attribute and no
badge on a bare tablist's bar.
**Expanded (Charlie, 2026-10-03: "Declarative nav basics"):** a heading's
group as a richer title (avatar, subtitle, press), the tab bar hidden for a
route pushed while the authored tablist is, no bar for a route with no
header, and a named scroller under an inline title's bar (LLP 1075.003
§9.10). The Signal Clone's conversation header and photo viewer are the
consumer. Unblocks them with no module. Take: none offered; waived by
Charlie's approval. No new attribute, no title menu, no bar added by a route.
**Clarified (Charlie, 2026-10-12, ruling 11 for #393):** explicit
`appearance="none"` on a header or root tablist keeps Pocket Budget's authored
bar; a header button with it remains authored too (LLP 1075.003 §9.10).
This uses the existing CSS row, admits no new attribute or apparatus, and
implements the approved feature ruling rather than inferring from styling.
**Expanded (Charlie, 2026-10-15: "keep making improvements to exact2 until
porting euxy … is higher fidelity than the others"):** the chrome euxy's
original customised. A route's `navigationGrabber` (`visible`, `hidden`,
`auto`) says a sheet's grabber; an alertdialog with one `input` is the
platform's text prompt (UIAlertController, NSAlert); a header button whose
face says more than a symbol or a title (a status dot, an authored font) is
drawn as written in its bar item; swipe actions whose controls all say
`appearance="none"` keep their paint. Take: none offered; waived by that
instruction. Still out: an arbitrary subtree as a bar item, a prompt with
several fields.
**Expanded (James, 2026-10-17: "Exact should allow the developer to
configure this on platforms that support it"):** a sheet route's
`navigationInitialDetent` names the `navigationDetent` word it opens at
(UIKit's `selectedDetentIdentifier`). Lexy's Last Parked sheet, which opens
tall over a half-height stop, is the consumer. Take: none offered; waived by
James, whom Charlie gave the call. Still out: reading or binding the detent
a person drags the sheet to.
**Expanded (Charlie, 2026-10-23, #854; part of LLP 1123 D7):** a presented
route's `navigationSizing` is `page` (the default) or `form`, which iPadOS
shows as UIKit's centred `.formSheet`; a host whose sheets have one size
ignores it. T3 Code's Add environment sheet (#216) is the consumer, and Nathan
Schroeder implements it. Take: none offered; waived by Charlie. Still out:
D7's `automatic` and `fitted`, which stay in stage 6.
**Expanded (Charlie, 2026-10-03: "A native grouped-list role mapped to
UICollectionView list layout"):** `list appearance="auto"` is a grouped list
(LLP 1084): sections with a header and footer, rows read by shape, UIKit's
own list on iOS and a measured sheet elsewhere; `listStyle` picks
`inset-grouped`, `grouped` or `plain`. The Signal Clone's settings screens are
the consumer. Unblocks native cell highlight, separators, dynamic type and
VoiceOver rows in a settings screen without a module. Take: none offered;
waived by Charlie's approval. No sidebar styles, no virtualized grouped list.
**Expanded (LLP 1116.001 D1, 2026-10-11, decided under LLP 1116's standing
instruction):** a grouped row that names swipe actions takes its list cell's
own swipe on iOS (grocery-list's swipe-to-delete did nothing in round 3).
Take: none; the one-row table it replaces is not made for such a row.
**Expanded (Charlie, 2026-10-06, via the lead: "Recs seem good. Approve and
push"):** hatches on every platform and what the agent sees of them (LLP
1075.003.000.001). The changes:
- app and window scopes;
- a frame clock and timers on the session clock;
- `input(text)` on an authored field;
- Linux and Windows hatches: an overlay the hatch replaces whole, and input it
  only observes;
- development-only diagnostics (`log`, `count`, `measure`, spans, `publish`)
  and Exact's own timing of each call;
- owned regions and parts in `tree`;
- a crash breadcrumb;
- per-platform hatch words checked by the delivery classifier.

`perf hatches` is a form of `perf`. Tapping a part is a form of `tap`, by real
platform input only. Neither is a new operation. The consumers are the Signal
Clone and `apps/native-fixture`. The ruling admits three pieces of
apparatus: `exact.mjs hatch <word>`, `EXACT_HATCHES=off` and the template's
`AGENTS.md` hatches section. Take: none offered; waived by Charlie's approval.
Still refused: claiming input on a painting host (a gesture arena), a tap that
runs hatch code by name, a hatch that writes Contract state or dispatches, a
node-scoped frame ticket, diagnostics collected in production, and JavaScript
above the data seam on native. Android's hatches are named but not built
(Android itself is admitted, 2026-10-07, §Surfaces).
**Renamed (Charlie, 2026-10-06: "ok access hatches it is"):** hooks are
access hatches, and the code word is `hatch` (`hatch="word"`, `app.json`
`hatches`; LLP 1075.003.000.001 §Rulings). The entries above use the new
word; nothing is admitted or taken by the rename.

**Expanded (Charlie, 2026-10-12 menubar lane ruling, confirming 2026-10-08):**
"menu bar: -exact-menu="app|file|edit|view|go|window|help" on button; root
-exact-menu-remove="…" fixed ids; enabled follows disabled; agent taps button —
OK for now". The coding-agent desktop app (#141) is the consumer (LLP 1021
§5.3). Take: none named; Charlie's approval admits this bounded extension.
Built 2026-10-12: placement and root removal of the fixed standard-item ids
in LLP 1021 §5.3. Develop remains in development builds.
**Expanded (2026-10-21; built 2026-10-22):** the menu bar's own items for the
T3 Code clone (LLP 1021 §5.3.1): app-menu and Help items enabled outside the
key window and under a sheet or modal; `-exact-menu="about"` and `"none"`;
removal ids for Go ▸ Back, Develop (its chords stay), Edit's template items,
Help's book item and View's zoom; View ▸ Actual Size, Zoom In and Zoom Out
over the root font size. Take: none; still Mac-only hints other hosts ignore,
with no dock menu, status item or Windows menu bar.
**Expanded (Charlie, 2026-10-23, Windows at full speed):** Windows presents
the declared menu bar as a painted Fluent menu row (LLP 1021 §5.3.2). The
dock menu and status item stay out.
**Expanded (Charlie, 2026-10-26, "Accept all (Recommended)", waived; LLP
1133.025 §15 question 8):** a status item and a Dock menu as presentations
of a `button` (`-exact-menu="status"`, `"dock"`), the window's `focus()`,
an accessory Mac app and a launch with no window, `keepRunning` and one
instance on Windows; a login item, global shortcuts and attention as
`@exact/login-item`, `@exact/global-shortcuts` and `@exact/dock`, on macOS
and Windows, absent elsewhere. Consumers: the T3 Code clone's SnapShot
shortcut, Excord on Windows and macOS, Agents' New Conversation. Take:
waived: no doing-list item is in this area, and the move retires the
clone's Carbon registration. Still refused: helper daemons, jump lists
(quick-actions'), Linux cells until a desktop host.
**Expanded (Daehyeon Mun, 2026-10-28, waived; LLP 1021 §5.3.3; built
2026-10-28):** File ▸ Close Window's chord, `-exact-menu-keys="close-window=…"`
on the root, for the T3 Code clone, whose original leaves Close Window without
⌘W. It amends the 2026-10-12 menubar lane ruling (the root's fixed removal
ids) with a second root list, of one fixed id. Take: waived (Daehyeon Mun,
2026-10-28): one fixed id for one consumer, a Mac-only hint other hosts ignore.

## Surfaces

**Expanded (Charlie, 2026-09-13):** replace app Rust below the data seam with
a separately linked native or interpreted Wasm module, in development and
production (LLP 1029.000). Unblocks business-logic updates without a host rebuild.
Take: multi-module scheduling and generalized native-view module loading stay
behind this single data-module consumer; Windows and Android hosts remain below.
**Refined (Charlie, 2026-09-14):** optional Wasm-first/native promotion for
explicitly stateless modules removes OS loading from the update latency path.
Generic executor-private state migration stays out; this is one committed
generation with an executor change, not a second app reload or patch protocol.

**Expanded (Charlie, 2026-10-03, waiver: "So maybe we just say we support all
Apple platforms in-tree, and out-of-tree is for like Amazon's new TV operating
system thing"; "yeah, this sounds good, let's pull it in"):** the Apple
platforms UIKit covers are in-tree surfaces: tvOS (Doug Lowder's port,
`host/apple/build.mjs --tvos`; the Siri Remote's focus, Select and Menu) and
visionOS when someone ports it. Each is a variant of the iOS presenter, not a
new one: shared Swift admits it beside `os(iOS)`, Rust cfgs beside
`target_os = "ios"`, and its own code compiles into its own binary alone.
Unblocks Caltrain on an Apple TV. watchOS, which has no UIKit, stays out (reversed
2026-10-17, below). A
platform with its own UI stack (Amazon's Vega OS) is a host outside this
repository, on the plan, runner and agent seams; its RFC is owed. The take is
waived.

**Expanded (Charlie, 2026-10-17, LLP 1126 §7: "reopen yes"; "admit watchos
now. let's try to hit full apple platform support for launch"):** system
surfaces and watchOS are in-tree. An app's widgets, Live Activities and
complications are entries of the one app, drawn on Apple by a SwiftUI
renderer whose snapshot driver WidgetKit archives (LLP 1126). watchOS is an
Apple platform like the others; it has no UIKit presenter, so its app is
drawn by that renderer's live driver. Unblocks an app's content outside its
window (the Home Screen, the Lock Screen, StandBy, CarPlay, HomeView, the
Mac) and Exact apps on the watch. Templates (CarPlay apps, controls) await
their own RFC. The take is waived.

**Expanded (Charlie, 2026-10-26, "Accept all (Recommended)", on LLP 1133.023
§15 question 8):** the app's content and actions reached from outside its
window: quick actions by the manifest's `shortcuts` and `setAppShortcuts`
(`@exact/quick-actions`), the system's search by the Content Index API's
names (`@exact/search-index`), and LLP 1111's declared tools offered to the
system's callers through a face in the manifest (App Intents, AppFunctions,
WebMCP), each arriving as a URL at the navigation root or as a tool call of
origin `tool`. Consumer: Shelf's Add Book, Continue Reading, books in
Spotlight and Log Pages. Unblocks reaching an app's screens and writes
without opening it. Take: none named; the ruling is its own waiver. Still
out, each until a consumer asks (LLP 1133.023 §13): App Intents in widgets,
controls and Live Activities; interactive snippets and
`requestConfirmation`'s snippet content; assistant schemas; `IndexedEntity`
and Spotlight's semantic index; `NSUserActivity` and Handoff; long-running
intents; an App Intents extension; a structured `returns` through a face;
entities that are not content-index entries; Android's App Actions built-in
intents; the web's Launch Handler; Windows' and Linux's system search and
Windows App Actions; a content index larger than 5,000 entries; and remote
icons.

**Admitted (Charlie, 2026-10-06: "this is pretty great. commit and push this
and document that TUI is now an officially supported platform for Exact2";
LLP 1101, 1101.000, 1101.001):** the terminal is an officially supported,
in-tree surface. `host/terminal` (`exact-terminal`) runs a terminal entry —
its own root `.contract`, compiled under the terminal profile (lengths in
`ch`/`lh`, the schema's `terminal` admission) — inline (settled transcript
entries printed once into the terminal's own scrollback, the live tail
redrawn; selection, copy and tmux stay the terminal's) or full screen.
Unblocks apps authored for the terminal: the coding harness
(`apps/harness`, real models through OpenRouter, Anthropic, OpenAI and
Ollama) and the todo fixture (`apps/todo`). A terminal entry is a separate
application root on a separate medium, not a platform override (LLP 1101
D9, §Authoring models). No take was named; the ruling stands as its own
waiver until Charlie names one.

**Expanded (Charlie, 2026-10-08: "rec good", to admitting it under the
terminal surface with a waiver; LLP 1101.003):** a terminal entry runs once
from a shell. With `--print`, or when stdout is a pipe, it settles (LLP 1101.003
D2, the command-line column of LLP 1113 D5), writes its rows, and exits. The
argument list is its URL, the active `head`'s `status` is its exit status,
`--json` writes its settled `state`, and one termination record decides how it
ends. Unblocks scripting the coding harness (`harness --print --prompt "…"`),
with `apps/todo` as the fixture. Implementer: a Claude (Opus 5.5) lane. The take
is waived. Still out: stdin and file operands, `exact install` for terminal
commands, writes on the location (`todo add milk`), and printing a GUI app
(LLP 1101.003 D9; a GUI app's static form is LLP 1112).

- **Windows.** A working Direct2D host exists in the old repo. It is real work, and it
  doubles the native matrix. **Admitted (Charlie, 2026-10-04):** "make an exact2
  implementation for the Windows platform, including the game engine" for standalone
  Skirmish (LLP 1015.000). Take: single-player first; multiplayer and general Windows
  app expansion wait behind the working game. The engine remains optional.
  **Expanded (Charlie, 2026-10-04):** "something simple and app-like that uses a
  lot of windows native stuff" admits Windows Desk, a bounded text workspace
  using Contract and an app-local Win32 control presenter. General Windows
  control parity remains behind these two consumers; no additional showcase
  or framework is admitted by this example.
  **Admitted (Charlie, 2026-10-23, waiver):** "We are no longer deferring
  Windows. We are going full speed ahead on it right now." Windows is a host
  for general apps, built to LLP 1115.004: Fluent painted by Exact's presenter,
  with what Windows owns taken from Windows. First consumers: `apps/shelf` and
  `apps/excord`, from S1. Floor: Windows 11 build 22621 on x64. Implementer:
  Claude (Opus 5.5), S0 from 2026-10-23, on the NucBox. It admits a painted
  Fluent menu bar for an app that declares one (LLP 1021 §5.3.2), and Exact's
  delivery on Windows (LLP 1115.004 D15).
- **Android.** Same, until 2026-10-07. **Admitted (Charlie, 2026-10-07,
  waiver):** "admit android. we are working on android now -- it's becoming
  real. amend that." Unblocks Android as a supported host instead of work
  below the line:
  - **Its host work:** the Canvas host (LLP 1076 §3.3) and the GPU painter.
    Most of it is already merged into main from `android/explore`; the
    branch keeps landing.
  - **Parley's Android arm** (`text/parley-android`, LLP 1085.000).
  - **A place in the sweep.**

  No take was named; the ruling stands as its own waiver until Charlie names
  one.

  **Expanded (Charlie, 2026-10-23, LLP 1115.001.003 §5):** Android TV is
  admitted as a variant of the Android phone host, as tvOS is of the iOS
  presenter: the same APK, Canvas reader and presenter, which tell a TV by
  its UI mode at run time; no second artifact or build flag. Unblocks Caltrain
  on an Android TV, driven by the remote's D-pad, Select and Back. No take was
  named; the ruling stands as its own waiver until Charlie names one.

Every surface multiplies the sweep, the presenter count, and the number of ways one
change can break.

## Authoring models

- **React tier.** The door stays open — meaning we design nothing that forecloses it.
  We do not build it. No React Facet bindings, no RecipeIR generation, no dual-framework
  parity gate. One authoring model, one set of bugs. Logic below the data seam is
  TypeScript by default or Rust (LLP 1027; Charlie, 2026-09-03); nothing runs
  JavaScript above it — not in Contract, not in the tree, not before first pixel.
- **Rust Native roots.** No Rust-owned UI roots, no deployment-manifest registrations,
  no app-ABI generator.
- **Platform-suffixed route overrides** (`.native.tsx`, `.mac.tsx`, ...). One route, one
  file. If a platform needs different behavior, that is a branch inside the component or
  a bug in the presenter. A terminal entry (LLP 1101) is a separate application root on
  a separate medium, not a platform override; GUI surfaces still have one source.

## Features carried over as "no"

**Admitted (Charlie, 2026-10-12, textrows ruling):** bounded
`text-wrap: balance` and a `::placeholder` part containing only `color` for
the tooltips, toasts, empty states and themed fields of
`issues/closed/20261009-balanced-text-placeholder-color.md`. This
amends LLP 1104's placeholder refusal. `-webkit-font-smoothing` stays deferred.

**Admitted (Daehyeon Mun, 2026-10-28, waived; LLP 1019 D12):**
`-webkit-font-smoothing: auto | antialiased`, an inherited paint row. This
amends the 2026-10-12 textrows ruling above, which kept it deferred. Consumer:
the T3 Code clone, whose web original sets `antialiased` on its root; drawn
without it, its labels read a weight heavier than the original's. `none` and
`subpixel-antialiased` stay refused. Take: waived (Daehyeon Mun, 2026-10-28):
a paint-only row, no doing-list item in this area.

**Runtime**

**Expanded (Charlie, 2026-10-13, issue-program ruling 10):** runner-owned
WebSocket sending: the stream mapper receives a socket handle and a mutation
sends ordered, never-coalesced protocol frames (LLP 1016.000 D7). T3 Code's
subscribe/Ack/Ping protocol and queued Supabase/Convex consumers justify the
expansion. No take named; the approved feature ruling admits it. Ambient
WebSocket globals remain refused. TypeScript on the web JS target and the
native hosts has it; D7 lists the host deviations and what remains. D7's
amendments let the mapper answer its own frames (2026-10-16) and an answer
in flight send its own requests (2026-10-18).

**Expanded (Daehyeon Mun, 2026-10-23; LLP 1133.005):** a local listener,
`exactListen("unix:<path>", options)`: a data capability in `fetch`'s shape
whose connections reach the stream mapper with LLP 1016.000 D7's sends, under
a `net.listen` grant, on macOS (Linux built with a fixture, accepted with a
Linux consumer); a loopback callback `http://127.0.0.1:<port>/<path>` with a
fixed port for `openAuthSession` on macOS (RFC 8252 §7.3), through the default
browser; and a bare `focus()` for the window, best effort. Unblocks T3 Code's
`t3 app <dir>` handoff and its Codex sign-in, and the 1,038 lines of the
clone's Swift that do them. Take: waived (Daehyeon Mun, 2026-10-23): the
listener is a transport the runner's streams own, as `fetch` is, and no
doing-list item is in this area. Still out: listening beyond loopback, binary
framing and raw byte streams, Rust replies, peer credentials and abstract
sockets, the ephemeral-port callback, an HTTP server for content, and `tcp:`
and `http:` addresses, which are written down only (LLP 1133.005 §11, D5).


**Expanded (Charlie, 2026-10-08, issue #283; focus lane, 2026-10-12):**
DOM `focusin`/`focusout` with target and related target information bubble to
an ancestor without adding a Tab stop. The clipped content and row action
wrappers in `issues/20261009-bubbling-focus-events.md` are the consumers.
Moves between descendants and removal also dispatch. The approved ruling
names no take; `:focus-within` remains out.

- GPU / WebGPU substrate. The old repo carries ~139,000 lines of Rust adapter code
  across two files, neither marked generated. The door stays open the way React's
  does — we design nothing that closes it: an owned-pixel surface is a leaf node
  with a kernel-owned box (like `NativeView`), GPU content never influences layout,
  the host owns the frame, and animatable properties are extensible (CSS
  `@property`). **Ruled (Charlie, 2026-10-22):** it compiles nothing on the
  boot path (LLP 1009 D4, D5): a GPU module compiles its shaders at a canvas's
  first use, after the first pixel, and Canvas 2D's module on Apple ships
  them compiled (LLP 1056 §8.5). The exception is the Linux host's GPU
  painter (LLP 1015 §7), which compiles its shaders on the first launch on
  a machine and reads them from a pipeline cache after. That first launch
  is accepted over the cold-start budget: on RADV, device creation and
  shaders alone cost 170 ms (93 + 77 ms; LLP 1015 §6).
- ~~Server generation in every form: SSR, streaming, static export, progressive
  forms, hydration, route payloads, response caching.~~ Moved 2026-09-23 (Charlie,
  LLP 1048): static and per-request pages, streaming, adoption in place of
  hydration, route payloads and cache policies are on the doing-list (the
  entry above). Still refused: progressive forms and server-driven UI.
- Aquifer data tier, durable worker tier, capability capsules, durable capability grants.
  **Moved, one form (Charlie, 2026-10-15, waiver; LLP 1069.010.000 stage 2):**
  a document the person chose, the object itself, kept in the platform's
  bounded recents the person can clear from outside the app, under the
  `doc.keep` grant, on macOS, Linux in a desktop session and the web (not
  iOS). Unblocks document apps reopening their documents after a relaunch.
  Every other durable grant stays refused, and so does keeping what a route
  or the app reached rather than what the person chose.
- A hardened-runtime exception on the app's own executable (Charlie, 2026-10-15):
  `runtime.*` grants name helper executables only (LLP 1069.008 D9), until the
  main executable has a consumer. The likely first, an in-process wasmtime loader
  (LLP 1028), would need only `allow-jit` or `allow-unsigned-executable-memory`,
  never `disable-library-validation`.
- **Admitted (Daehyeon Mun, 2026-10-23, waived; LLP 1133.007):** desktop
  processes (`spawn`, `execFile`, supervision, the login environment) and
  `home:` and `bundle:` paths on macOS and Linux (Windows at stage 3,
  unclaimed until it is driven), absent elsewhere; a password field's held
  secret on every host with the field.
  Consumer: the T3 Code clone's local server and SSH environments. Take:
  waived (Daehyeon Mun, 2026-10-23): nothing refuses processes by name, no
  doing-list item is in this area, and the change retires app Swift. Still
  refused: a pseudo-terminal, shell command lines, daemons that outlive
  the app, and processes on iOS, Android and the web.
- ~~Snapback / update economy.~~ Moved 2026-09-03 (Charlie: "you can pull out snapback
  for now"), in LLP 1026 D11's minimal form only: an installed app fetches a signed
  bundle of static files from a baked-in origin, after first pixel, into an on-disk
  store, for its next launch (LLP 1030 for every layer; 1030.000 for the verb). Unblocks
  deploying a change to installed apps from a bake and an upload. Take: `EXACT_DEV_PLAN`'s
  file poll and the two dev-only URL loaders collapse into that one store with two
  policies (1026 D12) — fewer paths after than before. What stays refused, as
  the *service* half of the old economy: per-user targeting, cohorts by identity, an
  update console, server-side anything, push delivery.
- Hot revision surfaces, staged reload. (HBC compilation came off 2026-09-03 for the
  *bake only* — LLP 1027 D5: `hermesc` runs at build beside the Contract compiler;
  nothing compiles at runtime, and the lean VM a host links cannot. A dev reload *carries
  state* — slots by name where their types still fit, settled resources where
  their arguments still match, the clock — and is otherwise a restart: no patch
  format, no generations, no identity matching. LLP 1007 §6; Charlie, 2026-08-28.)
  **Admitted (Charlie, 2026-10-14; LLP 1121 §9 Q3), development builds only:** Tamber's
  live table, which replaces design token values in a running development session without
  a reload (LLP 1121 D13). It patches values only: no code, no slots, no tree. A scrub
  session previews values for the length of a gesture. Release builds change their
  design only by an update. Charlie's waiver; no take is named.
- Islands and inline islands.
- Cross-runtime shared data in production. **Measured exception (Charlie,
  2026-09-18, LLP 1027.003 §9):** all four isolated transfer/storage directions
  were tried. The small conversion and app fixes are selected for delivery;
  immutable views, typed buffers and a mutable shared heap remain research.
  **Priority (Charlie, 2026-09-18, LLP 1027.004):** bounded Messages answers come
  next. Unblocks keeping producer, transfer and settlement work proportional to
  the requested window. Take: further shared-representation tuning and generic
  arrival-time structural reconciliation move behind this consumer. No new
  benchmark app, scheduler, core feature matrix or second application-state graph.
  Production ABI changes, arbitrary object unification and a default shared heap
  remain unselected.
- Notifications were never on the doing list (LLP 1069 §1 ranked them first of
  the "next"). **Expanded (waived by the orchestrator under Charlie's 2026-10-04
  delegation, "make decisions without me"):** local notifications by the web's
  Notification API names — `showNotification` now or at a time, `closeNotification`
  by tag — under the existing `device.notifications` grant. Consumers: the Habits
  app's reminders (x2apps habits F13) and Dash's price alerts, both faked as
  in-app banners. Unblocks a reminder that reaches the person outside the window.
  Take: none offered. Still refused: push delivery from a server, actions and
  replies on a notification, badges, repeating schedules, and a readable
  permission fact.
  **Expanded (Charlie, 2026-10-11, notification click ruling; LLP 1114.001 D1):**
  a plain click on a local notification may open its `navigate=` location
  through the existing router (a warm `navigate` event, a cold launch location).
  Consumer: the T3 Code clone opens the finished thread from its reminder.
  This admits the body click only; action buttons and replies, badges, push,
  repeats, and permission facts remain refused. Take: none named; Charlie's
  scoped ruling admits this expansion.
  **Expanded (Charlie, 2026-10-17, "approve", on LLP 1126 §7 question 5):**
  ActivityKit update tokens for an app's own Live Activities. The app reads an
  activity's push token and gives it to its own backend, which sends to APNs;
  exact2 sends nothing and relays nothing (LLP 1030.000 D6), and no app code
  runs when an update arrives (the system replaces the activity's state and
  the extension redraws it). Consumer: Interview's AI answer being written,
  which only its server can advance. Unblocks a Live Activity that stays true
  while its app is suspended. Still refused: push-to-start, remote
  notifications, and everything the click ruling left refused. Take: none
  named; the ruling stands as its own waiver.
  **Expanded (Charlie, 2026-10-18, "recs work, go for it", on LLP 1133.002
  §12 question 6):** a readable permission fact for geolocation,
  `geolocationStatus()` (`granted`, `denied`, `prompt`, and its accuracy),
  the first module of the standard library (LLP 1133). Consumer: Weatherlight's
  "Use my location", which shows its button, its position or a way to Settings
  by that fact. Take: waived. Notifications' own permission fact, and
  everything else this entry refuses, stay refused until notifications' RFC.
  **Expanded (Daehyeon Mun, 2026-10-23, waived; LLP 1133.004 stage 1):** the
  notifications module completed: `silent` and `data`, a click delivered as
  `notificationclick` to the app's root, posting from data code, the app badge
  by the Badging API's names, and notifications' permission fact and request,
  on the web, macOS and iOS. Consumer: the T3 Code clone's thread and
  review-guide notifications. Unblocks a notification click without a router
  and the clone's notifications without app Swift. Take: waived: no doing-list
  item is a fair trade; the move is the roster's own wave-1 item (LLP 1133.000
  §5), and geolocation's matching move was waived (2026-10-18, above). Still
  refused: push delivery, repeating schedules, and actions and replies until
  stage 2's ruling.

**Components** — roughly 15 built-in tags, not 40; roughly 12 Facet components, not 47.

**Expanded (Charlie, 2026-10-12, issue #275 rangeedit ruling):**
“setRangeText: (b) fire one input (insertReplacementText) as recorded deviation;
selectMode preserve; no undo step — for now”. Consumer: the issue's editor
replacing `@name` with a reference. LLP 1069.001 records the web, AppKit, UIKit and Linux implementations
and remaining host limits. Take: none named; this feature lane is explicitly approved.

**Restated (Charlie, 2026-09-27, LLP 1069.001):** the tag count guards against
invented components, not HTML's own elements. A built-in tag is an HTML
element with HTML's meaning; `select` and `option` are admitted on that
reading. Nothing that isn't HTML is added by it.

- No `lottie`, `rive`, `fileinput`, `pager`. Video admitted 2026-09-18
  (Charlie: design video and build a keyboard-resizing player; LLP 1042). Unblocks
  the native/web video consumer; router/viewport-fact integration (1038/1039)
  follows this player. (`webview` came off
  2026-08-30 as LLP 1020's `iframe` — it unblocks Weird Castle's entire content
  model, the client that exists to surface Castle web decks; the take: LLP 1013
  view transitions moved behind the deck lane. LLP 1020 §6. What stays no from
  exact1's webview: `top` topology, navigation policy, the controller ops, `allow`,
  author-facing `srcdoc` — each with its return trigger in LLP 1020 §5.)
- **Expanded (Daehyeon Mun, 2026-10-19; trade):** `postMessage(text, iframeId)` sends
  ordered strings to one mounted iframe: D2r's replies and host-initiated updates (LLP
  1020 §11). Unblocks T3 Code's HTML and MCP App documents: live theme, shortcut and
  `host-context-changed` updates. Take: QUEUE "Shared elements, stage 2" (LLP 1013.000
  §3) moves to Later. Structured clone, broadcast, a permission broker and Linux
  delivery stay no.
- **Expanded (Daehyeon Mun, 2026-10-23; trade; LLP 1020.002):** a browser shell on
  the `iframe`: `partition` makes a guest a top-level page with its own storage (LLP
  1020's `top` topology, for a partitioned frame only); its new windows, external
  schemes, dialogs, files, downloads and fullscreen get the platform's answers,
  and its permission requests are denied (navigation policy, with no veto);
  `navigate`, `traverseHistory`, `reload(id)`, `stopLoading`,
  `captureScreenshot` and (2026-10-31) `openDevTools` of an `iframe` join
  `HOST_COMMANDS` (`openDevTools` added because the T3 Code oracle's Browser
  tab opens its page's developer tools, and nothing else in this expansion
  reaches them. Nothing comes off the list for it; Daehyeon Mun's ruling of
  2026-10-31, LLP 1020.002 D11, stands in place of the waiver), with the Navigation
  API's events (the controller ops); `clearSiteData`, `zoom`, and
  WebDriver BiDi automation under the `browser.automation` grant in production
  builds. macOS and iOS first; the web a detectable subset; Android and Windows
  later; Linux absent. Unblocks T3 Code's Browser panel without app-owned
  `WKWebView` Swift. Take: QUEUE "Web view rows: memory against SwiftUI" (LLP 1020
  §10) moves to Later. Still no: a veto on the guest's own navigation, `allow` and
  a permission broker, author-facing `srcdoc`, a bridge in a partitioned page's
  world, a picture of any element but an `iframe`, cookie import, and LLP
  1020.002 §13's other uses.
- **Expanded (LLP 1096; admitted by the orchestrator under Charlie's
  2026-10-04 delegation, "make decisions without me"):** short sounds an
  action can schedule: a `sound` declaration (a WAV in `assets/`, at most 10
  s), `playSound(src, at=, gain=, group=)`, `playSounds(hits)` and
  `stopSounds(group=)`. The runner keeps the voice table; the web plays
  through Web Audio, Apple through one mixer in an arm loaded on demand;
  Linux and Windows keep the record; `app.json`'s `audio_session` picks the
  Apple session. Consumers: drums (R1–R3), snake (F1), trivia (F5).
  Unblocks a retriggered, sample-accurate hit without two media elements per
  voice or a 2 ms poll. Take (offered in LLP 1096 §9 Q1, accepted): the
  `audio` element stops being the sound-effect path — R1's retrigger change
  to the media glue is not built, and the guide's "Ding" recipe and LLP 1042
  §8's sound-effect paragraph are deleted. Round-1 review A held that neither
  is doing-list work; to the extent it is not, the admission stands on the
  orchestrator's waiver under the same delegation. Still out: synthesis,
  loops, fades and gain automation, effects, a handle to one voice, a stop
  at a future time, compressed formats, sound produced by a data module,
  the microphone as a sound source (recording it to a file is LLP
  1133.009's, §Components), and output on Linux and Windows (LLP 1096 §7;
  rate and pan came in with LLP 1125 D10). No tag is added.
- **Expanded (LLP 1125; Charlie, 2026-10-16: "once happy with it, go ahead
  and implement"):** a sequence (`sequence NAME when COND`, `play(tracks,
  bpm=, from=, countIn=, clock=, clockOut=, ended=)`) that the runner expands
  a second ahead into the voice table, a MIDI queue and timed haptics, with
  `exactTransport()`; MIDI by Web MIDI's names (`exactMIDI()`,
  `requestMIDIAccess()`, `sendMIDI(output, data, at=)`, `midimessage=` with
  `midiinputs=`, clock out and in); `playbackRate=` and `pan=` on a hit;
  `wakelock="screen"`; `share(files=)` with `app:/` paths and, on Apple and
  Android, an element's picture (`"#id"`). Consumers: euxy's port (its
  scheduler, sampler, CoreMIDI and share card), a metronome and drum
  machine. Unblocks timed sound and MIDI with no app commit per step and no
  app-owned audio thread. Take: none offered; waived by that instruction.
  Still out: on-demand decoding, web pictures of an element, QR in exact2,
  SysEx, Bluetooth MIDI pairing, MIDI 2.0, Continue and Song Position out,
  clock thru (sending clock while following one), Ableton Link, swing, Linux, Windows and Android output, synthesis (LLP 1125
  §8).
- **Expanded (LLP 1098; admitted by the orchestrator under Charlie's
  2026-10-04 delegation, "make decisions without me"):** the media session,
  by the Media Session API's names, on HTML's media element:
  `metadata=MediaMetadata(title=, artist=, album=, artwork=)` on an `audio`
  or `video` claims it; `seekbackward`, `seekforward`, `seekto`,
  `previoustrack`, `nexttrack` and `stop` are that element's events, with
  `MediaSessionActionDetails`; the platform's play and pause act on the
  element; position and playback state are the player's. The web publishes
  through `navigator.mediaSession`, Apple through `MPNowPlayingInfoCenter`
  and `MPRemoteCommandCenter` in the video arm; Linux and Windows keep the
  record. On iOS it needs `audio_session: "playback"` and the `audio`
  background mode. Consumers: podcast (F13, Top 5 #1), jukebox (media keys).
  Unblocks media keys, Now Playing, the lock screen and Control Center, a
  headset's buttons and the browser's media hub. Take (offered in LLP 1098
  §9 Q1, accepted): LLP 1042 §5's "Complete-player extension design
  (unimplemented)", a spec with no implementer or date, is deleted, and
  `QUEUE.md`'s "Video (LLP 1042)" lines lose "complete the designed
  track/controller and app audio-session ownership APIs"; to the extent that
  is not doing-list work, the admission stands on the orchestrator's waiver
  under the same delegation. Still out, each until a consumer asks: the
  other actions (`skipad`, slides, calls, picture in picture), Apple's
  scanning, rate, rating and like commands, an app-set `playbackState` or
  position, more than one artwork size, `app:/` artwork, resuming the
  player after an interruption and route policy, MPRIS on Linux and
  Windows's transport controls; and from LLP 1042 §5's deleted design,
  `source` and `track` children, a media controller (play, load, seek,
  fullscreen and PiP requests with results), DRM, and downloads and caches
  (LLP 1098 §7). No tag is added.
- **Expanded (Daehyeon Mun, 2026-10-23; LLP 1133.003):** video decode by
  WebCodecs' names in data code (`VideoDecoder`, `EncodedVideoChunk`,
  imported from `@exact/webcodecs`), its frames written to a track that a
  `video` shows (`srcObject=`) and a GPU surface samples, on the web,
  macOS and iOS; Android, Windows, tvOS and Linux `later`, the terminal
  absent. With it, LLP 1016.000 gains a byte stream (`fetch(…,
  {exactStream})`), built first as the module's throughput gate.
  Consumer: the T3 Code clone's device streams. Unblocks live H.264 from
  a server without an app-owned decoder. Take: `QUEUE.md`'s "Re-measure
  VideoArm's position read with many autoplaying videos" moves to Later.
  Still out, each until a consumer asks (LLP 1133.003 §13): controls,
  full screen and picture in picture for a track on Apple; presentation
  by timestamp; `VideoEncoder`, `AudioDecoder` and `ImageDecoder`; frames
  in data code; HDR output; `ExternalTexture` as a tier 2 word; a third
  party's decoder; tracks from other producers; WebRTC; Rust data code's
  binding.
- ~~No camera anything.~~ Moved 2026-10-02 (Charlie: "I don't know why camera is so
  aggressively deferred. Can we change that and record the change? It's time to work
  on it"; take waived, none offered): a camera is an app native module in LLP
  1067.000's shape — one object per session holding a preview view and its calls
  (start, stop, capture, flip), gated by the `device.camera` grant (LLP 1069.008), with
  an agent substitute before the OS (LLP 1069.007). Unblocks taking a photo in an
  app, on the web, iOS and macOS first. Not a built-in tag. Video recording and
  barcode or document scanning stay out until a consumer asks.
  **Expanded (Charlie, 2026-10-23, "Accept the take", on LLP 1133.008 §14
  question 7):** the camera is the `@exact/camera` package, in place of the
  app native module above, over `@exact/media-capture` (Media Capture's
  `getUserMedia`, tracks, devices and permission facts, shared with
  recording): a viewfinder (`exact-camera srcObject=`) and a photo by Image
  Capture's `takePhoto`, on the web, iOS, macOS and Android, Android in a
  fixture app; with the seam a package's view and an Android implementation's
  Maven libraries need (LLP 1133.008 D8). Consumer: the photo editor, which
  takes a photo and then edits it. Unblocks a photo in an app without app
  Swift. Take: the photo editor's Android port moves to later. Still out,
  each until a consumer asks (LLP 1133.008 §12): video recording, a camera
  track in `video srcObject=` and frames to code, two cameras at once on
  iOS, document scanning (narrowed from "barcode and document scanning" by
  LLP 1133.027's record below), constraint dictionaries, zoom on the
  web, `input capture` and the system camera UI, and the Windows, Linux,
  tvOS, visionOS and watchOS cells.
- **Expanded (Charlie, 2026-10-23, "Expand, no take", on LLP 1133.009 §15
  question 6):** recording the microphone to a file and reading its level,
  by Media Capture's and MediaStream Recording's names (`getUserMedia(audio=)`
  from `@exact/media-capture`; `startMediaRecorder`, `audioLevel` and the
  rest from `@exact/recording`), on the web, iOS, macOS and Android, a
  capture being a claim on the host's one audio session (LLP 1096 D8).
  Consumer: the recorder, whose own module the package replaces. Unblocks a
  recording without app Swift that leaves the app's own sounds playing.
  Take: none; waived by that ruling. Still out, each until a consumer asks
  (LLP 1133.009 §13): the microphone as a sound source, monitoring and
  effects (the LLP 1096 entry's), data while recording, starting a capture
  in the background, Android's background recording, constraint
  dictionaries, two inputs at once, `getDisplayMedia`, and the Windows,
  Linux, tvOS, visionOS and watchOS cells.
- **Expanded (Charlie, 2026-10-23, "Expand both, no take", on LLP 1133.012
  §14 question 7):** speech synthesis and recognition by the Web Speech
  API's names (`speak`, `cancelSpeech`, `speechSynthesisState` and the rest
  from `@exact/speech-synthesis`; `startSpeechRecognition` and the rest
  from `@exact/speech-recognition`), on the web, iOS, macOS and Android.
  Spoken audio is a claim on the host's one audio session (LLP 1096 D8);
  recognition takes a track from `@exact/media-capture`, the microphone
  held exclusively where the recognizer must own it, or a file.
  Consumers: the recorder's transcript of each take and the Markdown
  reader's Listen. Unblocks reading a document aloud and transcribing a
  recording without app Swift. Take: none named. Still out, each until a
  consumer asks (LLP 1133.012 §12): synthesis to a file or buffers and
  SSML as tier 1 names, Now Playing for a `playback` queue, an Android
  `playback` queue in the background, recognising while the app speaks
  without hearing itself, turn-taking for a voice assistant, recognition
  in the background and keyword spotting while idle, file transcription
  on the web, speaker identification, diarization and translation,
  `boost` on Apple and grammars, and the tvOS, visionOS, watchOS, Windows
  and Linux cells. The LLP 1096 entry's "synthesis" is sound synthesis,
  and it stays out.
- **Expanded (Charlie, 2026-10-23, "Expand both, no take", on LLP 1133.013
  §14 question 7):** the device's motion by W3C Device Orientation and
  Motion's and the Generic Sensor API's names (`deviceMotion`,
  `deviceOrientation`, `accelerometer`, `gravitySensor` and the rest,
  imported from `@exact/motion`), on the web, iOS and Android: each a
  polled source whose answer carries the samples since the reader's last,
  under a cursor the runner holds; and samples at frame rate into a
  canvas's input below the runner (`startSensorInput`, the engine's
  `input.sensor`). Consumer: Beacons' tilt to move. Unblocks tilt
  steering in a game without app Swift or a commit per sample. Take: none
  named. Still out, each until a consumer asks (LLP 1133.013 §12): motion
  in the background or while the app is closed, and wake-up sensors;
  activity, light and proximity as tier 1 (steps, floors, altitude and
  pressure moved to LLP 1133.026's record below); fall detection, water submersion and movement disorder; a
  Contract reader faster than the poll's floor, and a package view that
  draws from samples below the runner; `populateMatrix` and other
  conversions as names; `deviceOrientation` and the sensor sources on
  macOS; and the Windows, visionOS and watchOS cells, and Linux through
  IIO.
- **Expanded (Charlie, 2026-10-26, "Accept all (Recommended)", waived; LLP
  1133.031 §14 question 8):** the device's battery, network type, data
  saving and power state (`getBattery`, `connection` and `powerState` from
  `@exact/device`), by the Battery Status and Network Information APIs'
  names, on the web, iOS, macOS, tvOS and Android, and on Linux and
  Windows over the desktop package builder: live facts with no permission,
  read from any origin. Consumers: Weatherlight's sky, which rests in Low
  Power Mode and under heat, and its refresh, which waits in Low Power Mode
  and Low Data Mode; Expose's real battery. Unblocks an app that does what
  Low Power Mode and Low Data Mode ask without app Swift or Kotlin. Take:
  none named; the ruling is its own waiver. Still out, each until a
  consumer asks (LLP 1133.031 §11): device and app identity;
  `chargingTime` and `dischargingTime`; `effectiveType`, `downlink` and
  `rtt`; ultra-constrained paths and a path's unsatisfied reason; battery
  health, cycle count, temperature and voltage; Android's thermal
  headroom; `prefers-reduced-data`; and a Linux or Windows thermal state.
- **Expanded (Charlie, 2026-10-23, "Admit all three, no take", on LLP
  1133.014 §13 question 8):** signing in with an identity provider by
  FedCM's names: an `identity` member on `@exact/passkeys`'
  `credentials.get`, with `getIdentityCapabilities`, `preventSilentAccess`
  and the provider's button (`exact-sign-in-button`) from
  `@exact/sign-in`, for Sign in with Apple and Sign in with Google on the
  web, iOS, macOS and Android, Apple on Android `later`. The provider's ID
  token reaches data code alone, and the app's server verifies it; on the
  web each provider's page opens in a popup through the host's auth
  session and answers the app's server, and no provider's script runs in
  the page. Consumer: the auth fixture, with Snapback's identity as the
  product consumer. Unblocks signing in with Apple or Google without app
  Swift, beside a passkey in one sheet. Take: none named. Still out, each
  until a consumer asks (LLP 1133.014 §6, §8): One Tap, Google's button
  and FedCM in a web page, and Sign in with Apple JS; a guaranteed silent
  sign-in; Google API scopes and server auth codes; linked-account
  sign-in; other identity providers and FedCM's identity-provider side; a
  token verifier for apps' servers; Apple on Android; arbitrating sheets
  across ExactKit sessions that share a window; and the Windows, Linux,
  tvOS, watchOS and visionOS cells.
- **Expanded (Charlie, 2026-10-23, "Admit all three, no take", on LLP
  1133.015 §13 question 8):** selling digital goods through the platform's
  store by the Digital Goods API's names (`getDetails`, `listPurchases`,
  `consume`, with `purchase`, `acknowledge` and `unfinishedPurchases`, the
  store's queue, from `@exact/purchases`), over StoreKit 2 on iOS and
  macOS and Play Billing on Android, the web `absent`; and the store's
  rating prompt (`requestReview` from `@exact/store-review`). Consumer:
  Weatherlight+. Unblocks a subscription or a purchase in an app without
  app Swift. Take: none named. The App Store, Mac App Store and Play
  distribution lanes stay deferred, so selling in production still waits
  for a store lane; development builds and the stores' test environments
  need none. Still out, each until a consumer asks (LLP 1133.015 §11):
  StoreKit's merchandising views as package views; External Purchase, the
  link-out entitlements and Play's alternative billing as tier 1; the web
  cell inside a TWA or a Microsoft Store PWA;
  a receipt-validation or entitlement server of exact2's, and relaying the
  stores' server notifications; and the Windows, tvOS, visionOS and
  watchOS cells.
- **Expanded (Charlie, 2026-10-26, "Accept r5, build (Recommended)",
  waived; LLP 1133.034 §13 question 9):** payment for physical goods and
  services by Payment Request's names (`showPaymentRequest`,
  `paymentRequestStatus` and the wallet's button `exact-payment-button`,
  from `@exact/payments`), through Apple Pay and Google Pay on the web,
  iOS, macOS and Android, the payment token reaching the app's data code
  alone; Wallet passes as tier 2, designed and built when a consumer asks.
  Consumer: the photo editor's Order a Print, against a fixture print
  server that charges nothing. Unblocks selling a physical good with the
  device's wallet without app Swift or Kotlin. Take: none named; the
  ruling is its own waiver. Real wallet drives wait for the Apple merchant
  ID and the Google test account (question 7). Still out, each until a
  consumer asks (LLP 1133.034 §11): card entry without a wallet and any
  processor's SDK; a payment server of exact2's; Apple Pay's in-app
  provisioning, Tap to Pay on iPhone, disbursements, identity
  verification with Wallet and Apple Pay Later's merchandising views;
  Secure Payment Confirmation and the web's payment handlers; native
  merchant session updates; an Add to Apple Wallet button view; and the
  tvOS, Windows, Linux, watchOS and visionOS cells.
- **Expanded (Charlie, 2026-10-23, "Admit all three, no take", on LLP
  1133.016 §14 question 9):** the device's own language model by the
  Prompt API's names (`languageModelAvailability`, `createLanguageModel`,
  `prompt`, `promptStreaming` and the rest, from
  `@exact/language-model`), over Foundation Models on iOS and macOS, ML
  Kit's GenAI Prompt API on Android and the browser's Prompt API on the
  web: structured output by a JSON Schema subset, tools that are the app's
  data code the model calls back at the prompt's origin narrowed to
  `tool`, and a download only as a person's act. Consumer: Agents'
  on-device conversation, its titles and its tool. Unblocks prompting the
  device's model without app Swift or a server. Take: none named. Still
  out, each until a consumer asks (LLP 1133.016 §12): image and audio
  input; partially generated structured values while streaming; tools and
  enforced structured output on Android; the summarizer, writer,
  rewriter and proofreader; Apple's dynamic profiles, session properties
  and transcript error policy; a transcript that outlives the app's
  session; `temperature` and `topK` as tier 1, and the web's
  `samplingMode`; LLP 1111's `tool` lines as the model's tools; and the
  visionOS, watchOS and Windows cells.
- Background tasks were never on the doing list (LLP 1069 §5's "Not now").
  **Expanded (Charlie, 2026-10-23, "Admit all three, no take", waived; LLP
  1133.017 §15 question 11):** the app's code run by a wake the platform
  schedules (Background Sync's `sync` and Periodic Background Sync's
  `periodicsync`, from `@exact/background`), and uploads and downloads the
  platform finishes after the app has gone (Background Fetch's names, from
  `@exact/background-transfer`), on iOS, macOS and Android, with WorkManager
  on Android; `sync` on the web while the page is open. A wake boots the
  session with no view and draws nothing (`rules/RULES.md`'s boot row
  exempts it); the host keeps the platform's entry points and the transfer
  carrier, with the seam additions of LLP 1133.017 §8. The list's generic
  job scheduler is not this: the platform's scheduler runs it, with no
  queue or pool of exact2's. Consumer: Harmony's outbox, mention
  notifications and photos. Unblocks a message written offline sent at the
  next chance, and a photo that finishes uploading after the person has
  left the app. Take: none named. Still refused (LLP 1133.017 §13): a
  runner in a service worker on the web, and with it periodic sync and
  background fetch there; push and a data-only push's wake; wakes for an
  app that is not running on macOS, Linux and Windows; Android's
  long-running workers and foreground services beyond a person-started
  transfer's; resuming an interrupted upload on Android; an app's own
  `beginBackgroundTask`; continuous background location (geofencing
  moved in, below), Bluetooth restoration and HealthKit delivery, each its
  own module over this seam; Background Fetch's `icons` and `updateUI`;
  and continued processing's GPU resource.
- Maps, geocoding and geofencing were never on the doing list. **Expanded
  (Charlie, 2026-10-26, "Accept, with Play services (Recommended)",
  waived; LLP 1133.021 §15 question 8):** maps as the `exact-map` package
  view (web, iOS, macOS, Android, tvOS), geocoding by the platform's
  geocoder (iOS, macOS, Android, tvOS), and geofencing as wakes, from a
  region carrier the host keeps on Apple and the package's receivers over
  the host's store on Android, with Play services' geofencing there (iOS,
  macOS, Android), with the seam additions of LLP 1133.021 §9. Background
  location is admitted only as the grant geofencing requires
  (`device.geolocation-background`). Consumers: map-demo, Weatherlight,
  Caltrain. Unblocks one map on every target with a map, a followed
  position named by its city, and an arrival that wakes a closed app.
  Take: none named; the ruling is its own waiver, and the expansion
  deletes map-demo's own native module (its Swift and JavaScript). Still
  refused (LLP 1133.021 §11): continuous background location, beacons,
  visits and significant-change launches, overlays, directions, search, a
  default web tile source and a web geocoder.
- **Expanded (Charlie, 2026-10-26, "Accept r5, services first
  (Recommended)", waived; LLP 1133.033 §13 question 9):** a foreground
  service the host keeps on Android, claimed by a package for a capture
  the person started (`microphone`) and for a call (`phoneCall`), declared
  by the lines `device.microphone-background` and `device.calls`, which
  also derive iOS's `audio` mode; `@exact/calls`, the system's call screen
  through CallKit and Core-Telecom and Media Session's call actions on the
  web. Consumer: the recorder's background takes; Harmony's test call,
  which stays out of App Store builds until a real call transport exists
  (question 4). Unblocks a take that goes on while the person uses another
  app or locks the phone, and a call in the system's call screen, without
  app Swift or Kotlin. Take: none named; the ruling is its own waiver.
  Still refused (LLP 1133.033 §11): foreground services for an app's own
  work, ringing an app that is not running (PushKit, FCM), a call's media
  transport, and the rest of §11's list.
- **Expanded (Charlie, 2026-10-23, "Admit all three, no take", on LLP
  1133.018 §15 question 9):** contacts and calendars (`@exact/contacts`,
  `@exact/calendar`) on the web, iOS, macOS and Android. Choosing contacts
  by the Contact Picker API's `select` is a copy and needs no grant; reading
  the address book is a grant, in the picker's shape. Adding an event
  through the system's editor (`showEventEditor`; an `.ics` file on the web
  and the Mac) needs no grant; reading and writing the person's calendars
  is a grant, in JSCalendar's shape of iCalendar's model. Limited contacts
  and write-only calendars are `access` beside `state`; an app granted
  either store keeps no answers, as one granted Health keeps none; a tool
  may read the stores and may not write the calendar. With the seam
  additions of LLP 1133.018 D13. Consumer: the Calendar example, Android
  driven on `apps/module-fixture`. Unblocks the person's own events, guests
  picked from contacts and Add to Calendar without app Swift. Take: none
  named. Still out, each until a consumer asks (LLP 1133.018 §13): writing
  contacts, groups and containers, and the system's contact card and
  new-contact editor; recurrence written or edited by span, alarms,
  attendees and reminders (reachable through the `EKEventStore` hatch); a
  change history or batch protocol for a sync engine; delivery while the
  app is closed; an app's own calendar account as a sync adapter, and
  subscribed calendars; JSCalendar 2.0's changes; and the tvOS, visionOS,
  watchOS, Windows and Linux cells.
- **Expanded (Charlie, 2026-10-23, "Admit all three, no take", on LLP
  1133.019 §13 question 7):** the photo library, read and added to, both
  halves now (`photoLibraryStatus`, `photoAssets`, `copyPhotoAsset`,
  `addToPhotoLibrary` and `managePhotoSelection` from `@exact/photos`), on
  iOS, macOS and Android, the web a visible absence. A library item
  crosses by reference: an id, a picture the host draws through the
  package's `img` scheme, or a copy in `app:/tmp`. Adding and reading are
  two grants, limited access is a fact beside the state, and an app granted
  the read keeps no answers. And image transforms (`transformImage`, and
  `rotate`, `flip` and `crop` on `compressImage`) over each host's codec,
  on the web, Apple and Android, with a new Android codec. With the seam
  additions of LLP 1133.019 D11. Consumer: the photo editor, which saves
  its edit to Photos and opens a recent photo from a strip under limited
  access. Unblocks saving an edit and opening a library photo without app
  Swift. Take: none named. Still out, each until a consumer asks (LLP
  1133.019 §9): videos in the library and a Live Photo's movie; albums as
  shared words; editing a photo in place, reverting it and deleting;
  download progress for an iCloud photo; a change-details protocol for a
  large grid; the picker's Android arm (LLP 1069.002's); filters and
  adjustments, metadata read as values, and RAW; and the tvOS, visionOS,
  Windows and Linux cells.
- **Expanded (Charlie, 2026-10-25, "Waive both (Recommended)", on LLP
  1133.026 §16 question 8):** the person's health store, read
  (`healthStatus`, `requestHealthAuthorization`, `healthStatistics` and
  `healthWorkouts` from `@exact/health`), over HealthKit and Health
  Connect; steps since a moment (`pedometerStatus`,
  `requestPedometerPermission`, `pedometer` from `@exact/pedometer`), over
  `CMPedometer` and Android's Recording API with its step counter; and air
  pressure and a change of height (`altimeterStatus`,
  `requestAltimeterPermission`, `barometer`, `altimeter` from
  `@exact/altimeter`), over `CMAltimeter` and the pressure sensor. Each on
  iOS and Android; the web, macOS, Linux and tvOS a visible absence, and
  Windows' pedometer and altimeter `later`. A type's read is `unknown`
  where HealthKit hides it, every answer names the moment it can see from
  (`since`), the pedometer and altimeter share one Motion & Fitness
  decision on Apple, and no tool or wake reads any of the three. With the
  seam additions of LLP 1133.026 D19. Consumers: the Calendar example's
  Show my activity and Fieldnotes' walk. Unblocks a person's steps,
  workouts and climb without app Swift. Take: none named. Still out, each
  until a consumer asks (LLP 1133.026 §14): writing Health samples and
  workouts, and workout sessions; Health background delivery and reads in
  the background; clinical records and required read types; samples,
  characteristics and routes as tier 1; a tool or an agent reading these
  modules, until LLP 1133.002 D12's delegation; a closed range of steps on
  Android, and motion activity; ambient light; and the Windows, visionOS,
  watchOS and Linux cells.
- **Expanded (Charlie, 2026-10-26, "Converged: build (Recommended)", on LLP
  1133.029 §13 question 4):** an orientation lock held while an element
  is displayed: `orientationlock=` on any element, with the Screen
  Orientation API's `OrientationLockType` words and the shared word
  `current`. A lock's types narrow the manifest's `orientation` and never
  widen it; the topmost sheet that holds wins, then the nearest holder,
  and unrelated holders intersect. It applies on iOS and iPadOS, Android,
  the web (both targets, in fullscreen) and Windows; macOS, tvOS, Linux
  and the terminal are a visible absence. Consumer: Beacons' tilt to move.
  Unblocks a screen that keeps its orientation while it is shown, with no
  app Swift and no lock left behind when the screen goes. Take: none
  named; the ruling is its own waiver. Still out, each until a consumer
  asks (LLP 1133.029 §12): the screen's brightness; detecting screenshots
  and recording, and keeping content out of captures; reading the
  screen's orientation in Contract or data code; a lock inside a `dialog`
  or popover; widening past the manifest at runtime by a type; Android's
  `user*` and `fullSensor` forms, Apple's
  `preferredInterfaceOrientationForPresentation`, and dotted platform
  words on an attribute.
- **Expanded (Charlie, 2026-10-26, "Accept r5, build gamepad first
  (Recommended)", on LLP 1133.022 §22 question 8; waived):** the Gamepad
  API's names over the hosts' pads (`getGamepads`, `gamepadconnected`,
  `gamepaddisconnected`, `playEffect`, `resetEffects` and the shared word
  `setLight`, imported from `@exact/gamepad`), on the web, Apple, Android,
  Linux and Windows: one pad registry per process, the host's, opened to
  packages, with effects and lights as claims a session holds on a device.
  With it, LLP 1133.001 D5's `rust-static` builder and the Linux and
  Windows loops' package dispatch ("Build it now (Recommended)", question
  1). Consumer: Beacons' prompt, rumble and light bar. Unblocks a pad's own
  button names, rumble and light from Contract without app Swift or
  Kotlin. Take: none named; the ruling is its own waiver. Still out, each
  until a consumer asks (LLP 1133.022 §20): a world driving a pad's motors
  or light below the runner; `GamepadTouch`, a pad's motion as tier 1,
  battery and player LEDs as tier 1 words, several players' pads; Linux's
  lights and Windows' trigger rumble; and the visionOS cell.
- **Expanded (Charlie, 2026-10-26, the same ruling, on LLP 1133.022 §22
  question 8; waived):** Web Bluetooth's GATT client (`bluetoothStatus`,
  `requestDevice`, `getDevices`, `connectGATT`, `characteristicValue` and
  the rest, imported from `@exact/bluetooth`), on the web, Apple, Android
  and Windows: a chooser by platform (the browser's, the module's own
  sheet on Apple, AccessorySetupKit as an app-wide tier 2 mode on iOS 18
  and later, CompanionDeviceManager on Android, `DevicePicker` on
  Windows), values in data code only, and notifications in batches under
  a cursor the runner holds. Consumer: Pulse, a heart-rate strap's
  display. Unblocks reading a Bluetooth LE peripheral without app Swift or
  Kotlin. Take: none named; the ruling is its own waiver. Still out, each
  until a consumer asks (LLP 1133.022 §20): Bluetooth in the background,
  `bluetooth-central`, iOS state restoration and Android's companion
  presence wakes; scanning for advertisements, the peripheral role and
  Bluetooth Classic; `exclusionFilters` and filters with several
  services; an app's own scan list on Android; and the Linux, visionOS and
  watchOS cells.
- NFC was never on the doing list. **Expanded (Charlie, 2026-10-26,
  "Converged: build (Recommended)", waived; LLP 1133.035 §16 question
  8):** Web NFC's NDEF reading and writing (`nfcStatus`, `scan`,
  `abortScan`, `write`, `makeReadOnly`, `abortWrite`, `ndefRecords` and
  the root's `reading` and `readingerror`, imported from `@exact/nfc`) on
  the web (Chrome on Android), iOS (Core NFC) and Android (reader mode),
  with tier 2's tag types and commands to a tag a live scan holds: one
  reader per process, claims per session, and `readerUI` telling the app
  whether the system's sheet or its own panel shows the wait. With the
  seam additions of LLP 1133.035 D16. Consumer: Shelf's Tag this book.
  Unblocks reading and writing a tag from Contract without app Swift or
  Kotlin. Take: none named; the ruling is its own waiver. Still out, each
  until a consumer asks (LLP 1133.035 §15): Android card emulation, built
  (designed in D13), and Apple's `CardSession`; the NDEF message that
  launched an app from iOS's background reading; payment-card reading and
  Apple's value-added services; ISO 15693's and FeliCa's own commands as
  tier 2; writing to the tag just read in one touch; and the Windows and
  Linux cells.
- Alarms were never on the doing list. **Expanded (Charlie, 2026-10-26,
  "Accept, tell Daehyeon (Recommended)", waived; LLP 1133.030 §15
  question 9):** alarms (`@exact/alarms`): an alert at a time or after a
  countdown that rings through silent mode and Focus, with Stop, Snooze
  or Repeat and Pause, on iOS 26 (AlarmKit) and Android (`setAlarmClock`,
  with `SCHEDULE_EXACT_ALARM`); no app code runs when one rings, and only
  the person sets one. Consumer: Metronome's practice timer. Unblocks a
  timer that is heard with the phone silent and locked. Take: none named.
  Still out (LLP 1133.030 §13): repeating alarms and local times, an
  alarm-clock app's `USE_EXACT_ALARM` and full-screen alert, the app's own
  sound as tier 1, buttons that run app code, an author-drawn countdown,
  alarms set or resumed by tools and wakes, the Windows cell until the
  desktop builder, and the web, macOS, Linux and tvOS cells.
- Finding services on the local network was never on the doing list, and
  the listener's record above keeps listening beyond loopback out.
  **Expanded (Charlie, 2026-10-26, "Accept r9, build (Recommended)",
  waived; LLP 1133.036 §15 question 8):** finding services on the local
  network by DNS-SD (`localNetworkStatus`, `browseServices`,
  `resolveService` from `@exact/local-network`) on iOS, macOS, tvOS and
  Android, and Linux and Windows over LLP 1133.022's desktop builder; and
  reaching them under a `local-network` grant source for `net.fetch` and
  `net.websocket`, with every host's local-network fact and the origin
  rule at every fetch hop and WebSocket open whose destination can show a
  prompt (LLP 1133.036 D5, D6). With the seam additions of LLP 1133.036
  D16. Consumer: Lights, a controller for WLED lights. Unblocks listing
  the devices of a known type on the person's network and reaching one
  by name or typed address, without app Swift or Kotlin, and closes a
  tool's or a wake's reach to the LAN opening the system's prompt. Take:
  none named; the ruling is its own waiver. Still out, each until a
  consumer asks (LLP 1133.036 §13): advertising and a LAN listener;
  Wi-Fi Aware, Wi-Fi Direct and DeviceDiscoveryUI pairing; browsing every
  type or an unlisted one, subtypes, wide-area DNS-SD and raw TXT bytes;
  a client socket for raw TCP or UDP, SSDP and UPnP; systemd-resolved's
  browse on Linux; and a browse in a wake or the background.
- USB, HID and serial devices were never on the doing list.
  **Expanded (Charlie, 2026-10-27, "Accept 037, hold 032 (Recommended)",
  waived; LLP 1133.037 §17 question 8):** Web Serial, WebHID and WebUSB
  (`@exact/serial`, `@exact/hid`, `@exact/usb`) on the web, macOS,
  Android (serial and USB), Windows and Linux: a chooser where the target
  draws one, the attached list where nothing asks (D3), each web
  blocklist on every target, one device per process leased to each
  session, serial input that loses no byte, and HID input reports as a
  batch. With the seam additions of LLP 1133.037 D19. Consumers: Excord's
  mention light (a blink(1)) and Bench, a serial monitor and DFU firmware
  updater. Unblocks a desk light, a board's log and a firmware update
  without app Swift or Kotlin. Take: none named; the ruling is its own
  waiver. Still out, each until a consumer asks (LLP 1133.037 §15):
  isochronous transfers; Bluetooth RFCOMM serial ports; vendor
  USB-serial bridges on Android below API 37, and launching an app when
  a device is plugged in; a background serial logger on Android; a
  chooser on Linux and a presenter-drawn one on Windows; HID report
  descriptor parsing beyond `collections`; a terminal host's cells; and
  iOS, iPadOS and tvOS, which have no device API for apps.
- **Expanded (Charlie, 2026-10-25, "Waive both (Recommended)", on LLP
  1133.027 §13 question 9):** barcodes and QR codes read in a still image
  and live in a camera track a viewfinder shows (`getSupportedFormats`,
  `detectBarcodes`, `detectedBarcodes` from `@exact/barcode-detection`),
  the text in a still image (`textDetectorAvailability`, `detectText` from
  `@exact/text-detection`), and translation and language identification
  on the device (`createTranslator`, `translate`, `createLanguageDetector`,
  `detectLanguage` and the rest from `@exact/translation`), by the Shape
  Detection API's and the Translator and Language Detector APIs' names, on
  the web, iOS, macOS, Android and tvOS where LLP 1133.027 D18 gives a
  cell. A still is a path the app may read; a live scan is a non-consuming
  output on the camera's capture that takes no lease; a create is the
  app's foreground act, and a download the person's. With the seam
  additions of LLP 1133.027 D17. Consumers: Shelf's Scan Barcode and Add
  Quote from Photo, and Excord's Translate. Unblocks adding a book by its
  ISBN, quoting a printed page and reading a message in another language
  without app Swift. Take: none; waived by that ruling, which also narrows
  the camera record's "barcode and document scanning" to document
  scanning. Still out, each until a consumer asks (LLP 1133.027 §11): text
  recognition on camera frames and a wasm OCR on the web; the system's
  scanner UI (DataScanner, Google's code scanner); document scanning; Live
  Text interaction and Apple's translation overlay; Apple's translation
  strategy; streaming translation, `measureInputUsage`, the Summarizer and
  Rewriter; Android text in scripts other than Latin; Android's platform
  `TranslationManager` and `TextClassifier`; barcode generation and
  Vision's other requests; and the Windows and Linux cells.
- **Expanded (Daehyeon Mun, 2026-10-23; trade; LLP 1133.006):** the
  clipboard's `write([new ClipboardItem(…)])` from data code, by the
  Clipboard API's names, for `image/png` (the app converts other images
  with `compressImage`) and `text/plain`, as the `@exact/clipboard`
  extension on the web, macOS and iOS; `copyText` stays a command, and
  under the agent both write a private pasteboard. Unblocks T3 Code's Copy
  image without app Swift. Take: `QUEUE.md`'s "Chrome conformance steps for
  Paste and Match Style and for a clipboard event with nothing focused"
  moves to Later. Still out, each until a consumer asks: `read` and
  `readText` outside a paste, `text/html` and RTF, custom formats, file
  promises, several items in one write, a Contract `write`, a
  clipboard-change event, and the Linux, Windows and Android cells.
- ~~No gradient style rows. A gradient with anything on it is a canvas surface with
  children (LLP 1014 §5 — the take for widening `canvas`; the three rows return when a
  host earns them).~~ Moved 2026-09-26 (Seth Webster, for grnl; LLP 1066): one CSS
  `background-image` row, `linear-gradient()` and `radial-gradient()`, painted
  natively on every host and held to Chrome. Unblocks a protection gradient under a
  floating control without a canvas. **Ratified (Charlie, 2026-09-27; take
  waived):** CSS's own row, run natively by the browser and held to Chrome's
  pixels elsewhere; kept as built.
- **Expanded (Charlie, 2026-10-12, issue-program ruling 9):** “repeating gradients: (a) admit repeating-linear-gradient + length stops held to Chrome; defer background-attachment: local”. The T3 Code clone’s split-diff hatch is the consumer. LLP 1066’s amendment implements the admitted grammar; no new row or apparatus. `local` remains deferred.
- **CSS visual properties native hosts draw cheaply (Charlie, 2026-10-02: "Waive
  the take"; LLP 1077):** `corner-shape`, `mask-image`, `text-shadow`,
  `box-shadow` lists/`inset`/spread, `conic-gradient()` and stacked
  `background-image` layers, `background-clip: text`, `-webkit-text-stroke`, and
  3D transforms without `preserve-3d`, each CSS's own grammar held to Chrome.
  Unblocks native-looking corners, edge fades and text over imagery without an
  SVG island. Also admitted ("do all of them"), LLP 1077 §5: SF Symbol rendering
  modes, variable values and effects; vibrancy; press haptics; rolling numerals; the
  scroll edge effect; iPad pointer effects; -exact-smart-invert opt-out. Still out:
  `mix-blend-mode` on boxes (LLP 1077 D9).
- No virtualList v2 (cert wires, extent demand, proxy lanes). **Admitted 2026-09-14
  (Charlie: "ok do what you think"):** a straightforward windowed list with bounded
  row/view lifetime and a separate decoded-image budget (LLP 1010 §6). Unblocks
  real Messages histories without constructing every row; memory and construction
  cost trigger it alongside 60 fps. Take: further Messages decorative Tapback/emoji
  artwork, material matching and animation-timing polish move behind list memory.
  O(N) input data is named separately from O(window) UI; recycling is no flat-memory claim.
  **Expanded (Charlie, 2026-09-27, LLP 1070: "yes delete the old windowed list
  thing, collection list is now better"):** horizontal windowed lists (`display:
  flex; flex-direction: row`) and one level of nesting, a windowed list in a
  windowed list's row, whose lifetime is its outer row's; its scroll position is
  kept by the runner, bounded, as the unvirtualized page would keep it. Unblocks
  the Extra Heavy feed's filmstrip and inbox, and any feed of carousels. Take: the
  legacy windowed list (`item-height`/`estimated-item-height` without
  `virtualized`, `runner/src/instance/window.rs`) is deleted first, before either
  capability lands. Still out: grids, masonry, wrapping and inverted lists; RTL
  row lists; nesting deeper than one level; host keep-alive of an inner list's views.
- No host keep-alive of heavy views by row key, and no reuse of a web view
  or of a native-module view without that module's opt-in (LLP 1068 §4.7,
  §4.8, §5.3; Charlie, 2026-09-27). A reused view is indistinguishable from a
  new element; state that must survive scrolling lives in keyed data.

**Actions** — an action does not call an action (LLP 1017 P4c: a child
reaches its parent through `action` props bound to elements).
**Expanded (Charlie, 2026-10-03: yes to the action-prop tail call):** a
child's action may call one of its `action` props as its last statement (or
the last statement of a last `if`/`match` branch). The compiler puts the named
action's statements there, so it runs last in the same commit (LLP 1017 §P4c,
"The tail call"). The Signal Clone's photo viewer is the consumer: its release
decides to dismiss. Unblocks a child that decides, in its own logic, to tell
its parent. Take: none offered; waived by Charlie's approval.
**Expanded (LLP 1089; waived by the orchestrator under Charlie's 2026-10-04
delegation, "make decisions without me"):** a call anywhere a statement may
stand, to an action of the same component, an `action` prop, or an injected
action, expanded into that commit; a host command keeps its name, and a
call naming both a host command and an action in its component's scope is
refused (`syntax-call-ambiguous`, amended 2026-10-05 with Charlie's approval:
a new host command must never silently rebind an app's call). Consumers:
Files' four copies of "open" (F27) and the spreadsheet's close-the-editor and
move copies (F21); Mail's swipe (F18) is the tail call, once the docs say so.
Take: none offered. Still refused: recursion, a return value, and an action
passed as an argument.

**Motion** — **in v1**, in the LLP 1002 shape: CSS's `transition` model. Targets
are kernel style rows (`translate`, `scale`, `rotate`, `opacity`); a `transition`
row on the node says how they get there; the web host emits it as CSS and does
nothing per frame; every other host runs `exact-motion`, which is held to the
browser by fixtures. One declared deviation, `-exact-spring()`, lowered to keyframes on
the web. Gestures are in as follow-and-release: the platform recognizes, the
engine holds a value and springs it back with the release velocity. The seekable
clock is in, and it is the reason motion is testable: an agent advances time to
`settle_time()` and reads; it never waits.

**Expanded (Charlie, 2026-10-05, via the lead: approved LLP 1099's design,
"move to code"):** `-exact-spring()` takes UIKit's parameterisations: duration and
damping ratio, duration and bounce, response, and a labelled `velocity` and an
end time on the physical spring, each lowered to the spring UIKit builds and
cut where Core Animation cuts it. One implementation in `exact-motion`; the
web plays its frames. Unblocks porting UIKit apps' springs by their own
numbers (the Signal clone's menu, reply icon and photo viewer, now
hand-sampled `linear()` curves). Take: the clone's hand-sampled curves are
deleted when stage 1 lands. A take off the doing-list, or a waiver in
Charlie's words, is still to be recorded here. Still out: SwiftUI's own
spring evaluator, vector initial velocities, and per-iOS-version behaviour
(LLP 1099 pins iOS 27.0).

Moved off this list 2026-08-28 (LLP 1002 D6): **delegation to CSS on the web** — it
unblocks a web host that ships zero motion bytes and a parity corpus with the
browser as the oracle, the same shape layout already has. In exchange, not in v1:

- **A gesture arena, claims, leases, compositions, or an interactive-navigation
  model.** Recognition, hit-testing, and scroll-vs-pan arbitration are the
  platform's (`touch-action`, `UIGestureRecognizer`); owning them is the
  permanent bug annuity LLP 0559 F1 describes. Scroll always wins.
  **Expanded (Charlie, 2026-09-28: approved `panrelease`, with Spark as the
  consumer LLP 1057 asked for):** `panrelease=action`, fired once when a `pan`
  that began ends, with its release velocity (vx, vy) in pan's units per
  second: the platform's where it measures one (UIKit), `VelocityTracker`'s
  elsewhere; a cancelled contact releases at (0, 0) (LLP 1057.001 §6).
  Unblocks throwing what a `pan` drags by state, a card flung off a stack
  (`apps/spark`, admitted with it). Take: none: this is 1057 phase 2's item
  with its consumer. Still no phases, pointer ids or per-frame callbacks.
  **Expanded (Charlie, 2026-10-03: yes to `pointerdown`/`pointerup`):** DOM's
  two events, a touch or the primary button going down on a node and coming
  up (a cancel is an up), with no payload and no recognition. They observe
  beside `press`, `pan` and `contextmenu` and take nothing from them (LLP 1005
  §Events). The Signal Clone's hold-to-record mic is the consumer: recording
  starts the moment the finger lands. Unblocks hold-to-act controls. Take:
  none offered; waived by Charlie's approval. Still no multi-touch.
  **Amended (Charlie, 2026-10-15, option 1 of
  `issues/closed/20261010-pointer-events-reach-ancestors.md`):** both bubble
  to every ancestor that hears them, as `key` and `wheel` do; the innermost
  node hearing any pointer event still holds the moves; `stopPropagation()`
  stops either, and `wheel`, at its handler (decided under Charlie's
  delegation, 2026-10-15, after Astra's review). Unblocks a window's
  outside-press dismiss beside a popup that hears `pointermove`. Take: none;
  Charlie's ruling.
  **Built (2026-10-04, LLP 1056 §8.6; the widening confirmed by Charlie the
  same day):** stage 3's pointer coordinates, which Canvas 2D's acceptance
  admitted for a canvas's handlers, as a `PointerEvent` record (DOM's
  offset, buttons, pressure, type and id) on these two and a new
  `pointermove`, on any node. The paint app's diary (F1) is the consumer.
  Take: none offered.
  **Expanded (LLP 1094; waived by the orchestrator under Charlie's
  2026-10-04 delegation, "make decisions without me"):** a reorder spanning
  vertical virtualized lists that share a `reorderGroup`, in one session,
  with a host-drawn top-layer ghost, a drop hold, keyboard and
  custom-action moves, the action read `elementFromPoint`, and
  `PointerEvent`'s `clientX`/`clientY`. Consumers: the two kanban builds
  (F4). Unblocks moving cards on a board without a hand-built drag. Take:
  none offered. Still out: reorder on row and nested lists (LLP 1094 §7),
  drags into or out of the app, multi-item drags, grids, a gesture arena.
  **Expanded (Charlie, 2026-10-11; reads lane):** action-only
  `elementsFromPoint(x, y)` lists own ids under a point, front to back,
  including hit ancestors, for the T3 theme editor Inspect overlay. It
  admits no reactive geometry, pixel readback or devtools inspector.
  **Expanded (Charlie's ruling 8, 2026-10-11; reads lane):** action-only
  `querySelectorAll` takes only `[data-<declared word>~="value"]` and lists
  own ids in tree order, omitting unmounted, `display: none` and inert nodes,
  for the same editor's role usage count and outlines. General selectors and
  computed-style reads remain deferred.
  **Expanded (Charlie, 2026-09-29: "Yeah add the every frame task"; take
  waived, none offered; waiver confirmed by Charlie 2026-09-30):** one root task form, `every(frame, action)`
  (LLP 1073): the action once per presented frame, never caught up, and on
  the agent's seekable clock a virtual 60 Hz display. Unblocks a ticker or
  an animation loop in step with the display (the web-framework bench's
  grid ticker). Per-node frame callbacks and frame arguments stay out.
  **Expanded (LLP 1092; waived by the orchestrator under Charlie's
  2026-10-04 delegation, "make decisions without me"):** a gated root task,
  `task NAME when COND [key=EXPR]`, whose timer exists only while its
  condition holds and restarts when its key changes; nothing runs when the
  gate changes. Consumers: ledger2's undo toast, chat's and chat2's bots and
  toasts, trivia's round (their diaries' Top-5). Unblocks an app that does
  no work at rest while nothing is showing. Take: none offered. Still out:
  a reaction to a state change (LLP 1017 P4c's `task … when [dep]`),
  component-scoped and action-started timers, computed intervals, a gate
  that reads `performanceNow()`, and cancelling a queued send.
  **Expanded (user request, 2026-10-27/28; LLP 1147):** root timers accept `wait=frame|update|scrollend|idle` and bounded scroll or idle waits, or `tolerance=` for wake coalescing. The request explicitly includes all host platforms and deterministic coverage; the follow-up admits wake tolerance. Computed intervals, combined waits and per-node timer callbacks remain deferred.
  **Qualification deferred (user request, 2026-10-28; LLP 1147):** actual energy-savings measurements, including iPhone battery and full-app Mac power measurements, and timer qualification with GPU rendering. Timer correctness, cancellation, admission instrumentation and `wait=update` performance remain in scope; Windows JavaScript qualification is not deferred.
- **A second value graph.** No shared-value plane, derived values, bindings, or
  plan node graph. The style row is the binding.
  **Expanded (Charlie, 2026-09-27, LLP 1057.003: "approve those"; take waived, none
  offered):** presentation timelines. A node's keyframes are held at the progress of
  another node's presented value (a drag's translate, a scroll offset), evaluated by
  the engine, with no app code per frame and nothing flowing back into app state;
  paint-only rows in v1. Unblocks derived presentation: a backdrop fading with a
  dismiss drag, a card's rotation with its throw, a header collapsing with its list.
  Every other derived value stays out.
- **General layout transitions.** LLP 1041 §8.12's measured projection admits
  one explicitly registered numeric-height sheet (Charlie’s four-interaction
  campaign, 2026-09-17). Take: other animated layout properties and decorative
  effects wait behind that sheet and its nested collection. This is not a
  general layout-animation system or evidence of physical 120 Hz.
  **Expanded (Charlie, Shop fidelity goal, 2026-09-19):** overlapping product
  accordion heights are admitted for the Shop clone. Their shared layout must
  move following content together when another section is tapped mid-animation.
  Take: other animated layout properties and decorative effects remain behind
  that consumer. The first increment accepts several numeric samples; automatic
  content-height measurement and host adoption remain unfinished.
  **Numeric-height issue fix (2026-10-13):** authored pixel-height transitions
  on Apple use that projection for either box sizing, without a keyword opt-in.
  This unblocks the T3 terminal drawer; other layout properties and height units
  remain deferred. `auto` retains the accordion's border-box and
  `interpolate-size: allow-keywords` requirements (LLP 1041 §8.12).
  **Expanded (Seth Webster, 2026-09-26, for grnl; LLP 1063):** `-exact-layout-transition`
  (a box's laid-out position and surface size, moved by the engine natively and by
  a measured offset on the web) and `-exact-exit-animation` (a removed node stays, out of
  layout, until its keyframes end). Unblocks siblings that slide when content
  around them changes, and things that leave instead of vanishing. **Ruled
  (Charlie, 2026-09-27, "we can try (a) for now"; take waived):** both rows stay
  as declared deviations (LLP 1001). No CSS feature animates a removed node or a
  layout move without snapshots, so the web runs them by FLIP, the technique web
  libraries use; there is no browser oracle for them, and parity is held by one
  recorded timeline compared across hosts. If the emulation's bug rate stays
  high, the fallback is dropping `-exact-layout-transition` and keeping
  `-exact-exit-animation`. Still out: animating any other layout property, and laying
  out per frame.
  **Expanded (Charlie, 2026-10-20, "ok do those things pls", of a list naming it;
  take waived, none offered):** `transition` on `font-weight` and `font-stretch`,
  the runner laying the text out again each frame (LLP 1019 §13 D10). Unblocks
  variable type that moves, as the browser and Expo SDK 58 draw it. Every other
  layout property stays out.
  **Expanded (Charlie, 2026-10-24, "fix all those things", of a list naming
  them):** `transition` on `font-variation-settings`, and `@keyframes` on all
  three font rows, played the same way.
- **Decay and sequence drivers.** A spring carries release velocity; nothing
  else needs a driver. (`@keyframes` and repeat came off; see below.)
- **Reduced-motion policy in the engine.** The producer emits `transition: none`
  when the host reports the preference, as a stylesheet's media query would.
- **A Core Animation executor for transitions.** Permitted by LLP 1002 D2, not
  built; the Apple lane measures whether it earns its place.

**Expanded (Charlie, 2026-09-26: "why wouldn't we just do a complete SVG
implementation here? i think this will be useful and we're using it now"):**
SVG, complete, as the browser renders it, and CSS `@keyframes` / `animation`
(LLP 1055). The first slice (`svg`, `g`, `path`, `polyline`, `polygon`,
`circle`, `line`, `rect`, the SVG 2 presentation properties, animations run by
the browser on the web, by Core Animation on Apple for `opacity`,
`stroke-dashoffset` and `r`, and by `exact-motion` elsewhere) serves the
crypto-list benchmark's charts; the rest of SVG follows as its own stages. Take:
none named; the dead `svgSource` prop is deleted.
  - **Carve-out: `foreignObject` on native hosts** (Charlie, 2026-09-27, LLP
    1055.000 D13 and §8). The web emits it now, with real HTML inside: "our
    general philosophy should be allow platforms to reach their full platform
    rather than sanding everything down to lowest common denominator." Native
    hosts refuse it by name until a design for boxes inside a scene exists;
    the error names the pattern that works everywhere, a box positioned over
    the `svg`.

**Expanded (Charlie, 2026-09-27: "Core Graphics everywhere", by the web's name;
admitted with "seems reasonable"):** the HTML Canvas 2D context on the `canvas`
tag (LLP 1056). A surface in the app's data module, TypeScript or Rust, draws
with `CanvasRenderingContext2D`'s own names and rules. Its recorded calls are
replayed in order by the browser, Core Graphics or tiny-skia into the canvas's
kept bitmap; on Apple a canvas that animates is replayed on the GPU by a module
loaded on demand (vello, its shaders compiled at build time; LLP 1056 §8.5;
Charlie, 2026-09-29: "we're going to want the GPU version of this"). Unblocks computed 2D drawing (charts, sparklines, maps, custom
controls) on every host without a GPU module, with Chrome as the oracle. Take:
Caltrain's line map leaves wgpu. Its `map` surface and shader are deleted, and
it is redrawn as a Canvas 2D surface in Caltrain's data crate, so one fewer GPU
path exists after than before. Still refused: a drawing language in Contract
(SVG is the declarative one), readback (`getImageData`, `toDataURL`,
`toBlob`), `ctx.filter`, and an app-visible `OffscreenCanvas`.
**Expanded (Seth Webster, 2026-09-26, for grnl; LLP 1061–1064):** paint motion
on boxes — `transition` and keyframes on `background-color`, `color`, borders,
`-exact-tint-color` and `box-shadow`, `light-dark()` pairs moving with the appearance —
through the kernel seam and Apple mechanism SVG's colours use (LLP 1055.000 D6);
press feedback (`-exact-press-scale`) and motion at a ProMotion panel's rate; the user's
motion and transparency preferences as `exactViewport` fields, which the app
reads (still no engine policy, above). Unblocks grnl's design as drawn. Take:
the branch's own `path` node, `marker` declaration and block keyframes syntax
were removed at the merge for main's SVG and keyframes; nothing is off main's
list. **Ratified (Charlie, 2026-09-27; take waived):** paint transitions as CSS
names them. `-exact-press-scale` stays as a declared non-CSS host-feedback row (LLP
1001): the press composes through CSS's `scale` property and never writes
`transform`, and it is kept under reduced motion, as a native button's
highlight is (a shrink is feedback, not motion).
- **`runOnJS` and the escape hatch / runtime graph admission** — never existed here.

**Tooling** — no Design Mode, no Guide system, no devtools UI, no blog/CMS. (The
TUI host came off on 2026-10-06: §Surfaces.)

**Admitted (Charlie, 2026-10-14; LLP 1122 §9 Q2 and LLP 1121 §9 Q3), development
builds only:** the Tamber protocol server (`tamber/1`, LLP 1121 D13) and the design tool
(LLP 1122). The tool is a web JS-target app, and every client edits through the protocol.
Each host's development build gains an **inspect mode** and a build channel: four commands
down (inspect, outline, hold, pin) and three reports up (status, a selection,
unsupported); the tool never drives an app. Release builds carry none of it: no endpoint,
no inspect mode, no build channel. Charlie's waiver; no take is named. Still refused:
editing in release builds, a production inspect tier, layout and structure editing,
captures of running apps, and a hosted relay.

**Agent API** — 10 operations, not 90:

`tree` · `screenshot` · `tap` · `type` · `state` · `layout` · `logs` · `clock` · `prefer` · `perf`

**Ruled (Charlie, 2026-09-27: "relax the rule and allow a 9th button"; LLP 1061
D5, LLP 1069.007 §3 P1):** `prefer` stays as the ninth operation, and it is the
one that sets every device fact by its web name: display preferences, and the
page and place facts of LLP 1069.000. A new fact is a form of `prefer`, never a
tenth operation.

**Ruled (Charlie, 2026-10-02: "it's probably worth a waiver to a 10th op I think";
LLP 1079 D7):** `perf` is the tenth operation and takes none away: a subtree's work
by plan site, and a host's presented frames. A new measurement is a form of `perf`,
never an eleventh operation.

**Ruled (Charlie, 2026-10-26, Excord's socket handshake):** `clock until state
<path> == <literal> within <ms>` is a form of `clock`, in the agent and in
authored tests. Virtual time stays frozen while it waits on the wall clock for
at most `<ms>`, and a miss names the last value it saw. It is for state that a
stream reaches after its first message, which settlement does not wait for
(LLP 1016.000 D5).

**An eleventh operation replaces one of the ten, same PR.** A new input is a form of
`tap` or `type` (a wheel is a `tap`; so would a drag be); a new question is answered
from `tree`, `state`, or `layout`. The old repo's six primitives grew eighty wire names
one reasonable "view" at a time; the count is the cost. (Charlie, 2026-08-29.)

Not shipping: session record/replay, causal trace, behavior diff and verify, mutation
dry-run, contract witness / dataflow / ~~source-map~~ / bindings, accessibility audit (narrowed below), plan
drag, correlate, visual query, network, perf, pasteboard, onboarding,
revalidate, code grant/resume/cancel. (`perf` shipped as LLP 1079 narrows it: work
counts and presented frames; never a causal trace, a flame chart, a devtools UI or
production telemetry.)

**Narrowed (Charlie, 2026-10-03, waiver: "I guess that trade makes sense ?
idk wdy"; LLP 1080.002):** `tree --ax` reads the platform's own exposed
accessibility tree — Chrome's computed tree, UIKit's and AppKit's
accessibility properties; Linux `unavailable` — and the driver reports four
fixed findings over it: an unnamed interactive element, an element exposed
under a declared-inert or hidden view, an element exposed outside an open
modal, and a role, a name or a state that two hosts give differently for a
`testId`-joined fixture control once each is read through its own platform's
table (UIKit's toggle value is the platform's, not a disagreement with the
web; LLP 1115.000 D14.4). Still refused: scoring, a rule catalog, WCAG checks
(contrast, target size, heading structure), reading-order claims, and
driving VoiceOver. The guessed `accessibleName` on `tree` is deleted because
the platform's tree supersedes it, not as the take.

Admitted 2026-09-10 (Charlie, LLP 1035 §5; 1035.002 D6, 1035.005 D3): **a
development-only map from plan node to its declaration and component call-site
chain**, emitted by the compiler beside the plan, keyed by the plan's digest, read by
the driver and never by a host, never in a bake. It unblocks jumping from a failing
node to its source. The take: no new Contract syntax until formatting and navigation
have landed (1035.005 §1's order), and `contract` blocks as executable assertions
(LLP 1006 §8) stay off the doing-list. The witness, dataflow and any general
provenance explorer stay refused.
**Read (LLP 1151, 2026-10-28; waiver approved by Charlie the same day, "approve waiver yes"):** formatting and navigation have landed; the authored-test grammar's `always`, `after` and `never refuses` blocks (test files, compiled to nothing an app runs) are read as outside this take; executable assertions in a component stay refused.
**Widened (Charlie, 2026-10-14; LLP 1122 §9 Q2), development builds only:** the Tamber
server may also read the map, to join a build's inspect report with its source for the
design tool (LLP 1122 D5). It records each file's source digest and each winning row's
attribute span. A host still never reads a map, and no map is in a bake. The tool shows
one selected node's provenance; a general provenance explorer stays refused.

**Expanded (user, 2026-10-28: "Implement", LLP 1079.000):** admit bounded local
frame-health counters, native menus and cards, and human/agent readouts. The menu and
card follow platform presentation and accessibility. The request "Let’s target 4KB"
admits recent-hitch capture in fixed memory and explicit Share/Copy or agent reports
capped at 4000 bytes, without automatic persistence or uploads. "Let’s do it all"
also admits live-clock scheduling, cursor reads, the development-only passive
native-scroll experiment and macOS card/inspector controls. Production default-on
requires measured adapter qualification. No production inspection endpoint or
remote telemetry is added.

`clock` replaces a `wait` operation on purpose. If the motion graph is closed-form under
a virtual clock, an agent advances time and reads the result; it never sleeps waiting for
an animation to settle. Settle timing is the single largest source of flake in the old
repo's agent loop, and under `RULES.md` a flaky check is worse than no check.

## Process

This half matters more than the feature half.

- **No governance corpus.** 20 docs, capped. No lane manifest, no verify registry with
  675 entries across 64 profiles, no contracts authority map, no issue priority scoring.
- **No speculative specs.** Specifying something you are about to implement is fine and
  expected — the deciding already happened upstream. Specifying something nobody is
  assigned to build is not written at all. The old corpus is 330 RFCs of deliberation
  against 52 specs of conclusion; the new repo imports conclusions.
- **No refine loops** (narrowed by Charlie, 2026-10-22). No rounds repeated
  until every reviewer says READY, no reviewer verdict deciding whether work
  lands, no revision ledgers, and no scope growth because a review asked for
  it. Blind review by two model families is allowed within `RULES.md`
  §Loop shape's fix-round limit.
- **No per-PR ceremony.** No framework decision log, no lane debt ledger, no provenance
  headers on hand-written files.
- **No blocking check the author did not run locally in under a minute.**
- **No 90-minute gate.** If verification takes 90 minutes it runs behind you, or it does
  not run.

## Deliberately worse

- No backwards compatibility, at all, before 1.0.
- No public API stability. Names change. (One exception, LLP 1075.003's hatch
  surface: its breaks are deliberate, below.)
- No migration guides.
- Generated files are built, never committed.
- Sparse prose. The code and the checks are the authority.
  **Except the author guides (Charlie, 2026-10-04, LLP 1086 D9):** the guides under
  `docs/` describe the shipped product to the people and agents building with it. A
  change that alters what an author writes or runs updates them in the same change.
  That is not apparatus and needs no approval; a new file there still does.

## Moving something off this list

Write one line naming what it unblocks, and take something off the doing-list in the
same PR. If nothing can come off, Charlie may waive the take in writing; the waiver
is recorded beside the entry, in his words. Without a take or a waiver, the answer
is no. **Restated (Charlie, 2026-09-27):** the take stays the default; the waiver
exists so that an exception is a visible act and not drift.

**Install-page slice, 2026-09-12 (Charlie, LLP 1030.003 D6a/D6b):** bake a
standard `/.exact/install/` page for web/iOS/macOS with configured installation
methods, and let a Mac development server invoke its existing simulator or device
build for a local Simulator or paired phone. Unblocks sharing one honest
installation URL and a local Apple development loop; the generic Go launcher,
distributable IPA, EAS/AppDrop adapter and
automatic provider provisioning remain behind proving this consumer. No new
update service or production native carrier is introduced.

**`--archive --unsigned`, 2026-10-02 (Charlie, waiver):** "Yeah sounds good. Do
1-3" — item 2 of the Signal clone diary's fixes, an ad-hoc `.ipa` with no
identity or profile for a consumer that re-signs it (AppDrop). Unblocks handing
an app to AppDrop without a stub profile; refused for apps with GPU modules,
whose baked digests a re-signer would break. No AppDrop adapter is added; the
distributable IPA and EAS/AppDrop adapter above stay deferred otherwise.

**AppDrop lane, 2026-10-14 (Eliot; Charlie, waiver, 2026-10-22):** "for decisions,
recs are ok", accepting the recommendation to waive the take for this lane and
admit AppDrop as the remote tester-install path, TestFlight later (LLP 1030.003
§6 Q4).
`deploy.appdrop` makes an iOS binary row of `exact deploy --yes` build the
`--archive --unsigned` `.ipa` against the stream's release record and publish it
to AppDrop (creating the link the first time, writing it to app.json), and the
install page offers the link.
Unblocks sending one install link whose app stays current: bundles over the
air, binaries through AppDrop. It lifts the `--unsigned` waiver's GPU refusal:
a GPU card's digest leaves out a Mach-O's code signature (`code_digest`), so a
re-signer keeps it and a code change still breaks it. The EAS adapter, signed
distribution IPAs and automatic provisioning stay deferred.

**Admitted (Charlie, 2026-10-12, issue-fix ruling 5; take waived):**
"intersect=: admit now — intersect=action {isIntersecting, intersectionRatio},
intersect-margin, root = nearest scroller, threshold 0, fires on mount + enter/leave".
Consumer: T3 sidebar subscriptions and timeline minimap. LLP 1005 records the
bounded event, browser executors and the shared AppKit/UIKit/Linux geometry
slice; native qualification remains open. Container
queries, derive-time frame and general live-position feedback stay deferred.
