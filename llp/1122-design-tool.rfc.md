# LLP 1122: The design tool — shaping a running app through Tamber

**Type:** RFC
**Status:** **Accepted (Charlie, 2026-10-14); name pending; not yet scheduled; builds after Tamber stage 3.** "The design tool" is a placeholder until it is named (§9 Q1). Every other §9 question is ruled. The `rules/DEFERRED.md` amendments (§Tooling, §Runtime's live table and the source-map line, all for development builds only) landed with it. History: r1–r4 had three blind review rounds by GPT-6 Astra (max) and Grok 4.7 (xhigh); r6 restructured v1 on Charlie's word (the canvas is the running app); round 4 reviewed that; r7 took its findings (§0). Every verdict was RIGHT WITH CHANGES. Reviews: `llp/reviews/rfc-2026-10-13-1122-design-tool-r{1,2,3,4}.{astra,grok}.md`.
**Systems:**
- The design tool: an Exact app for the web's JS target, a `tamber/1` client (LLP 1121 D13, D14).
- The Tamber server (LLP 1121 D13): build control over the dev loop's existing channel (D4), the selection join (D5), scrubs and the live-or-rebuild answer, `set-row` (D6), held proposals (D10), `commit` (D9).
- Every host's development build: an **inspect mode** that reports a tapped or clicked node and draws outlines (D5), and a held state (D8).
- The agent API (LLP 1012): the held state as a form of `tap` (D8), so agents get what the tool gets; the node explanation gains a held state's overlay rows and a `boundary` row source (D5, D8).
- The compiler's source maps: a source digest per file and the winning attribute's own span (D6).
- The web JS target's runtime: a row's token beside its computed value, and its own plan and map pair (D5).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** none yet. No stage starts until it has an implementer and a date (`rules/RULES.md` §Scope); each follows the Tamber stage it needs (§4).
**Related:**
- LLP 1121 (Tamber): D7 the token index, D8 state parts, D13 the protocol and the live table, D14 clients and importers, D15 component params, §5 staging.
- LLP 1007 §6 and LLP 1008 (the dev loops' reload, which carries state), LLP 1020 (`iframe`), LLP 1012 (the agent API), LLP 1035.002 (`layout` explaining one node), LLP 1095 D7.
- `rules/DEFERRED.md` §Tooling, §Agent API, and the 2026-09-10 source-map admission.
- exact1, as research: LLP 0158 (Design Mode), 0200 and 0208 (shelved for bundling too much), 0225, 0304 W5 and W6. The survey: `~/.tuft/projects/exact2-theming/facet1-research.md` §10d.

## Summary

The design tool is how a designer, or an agent, shapes an exact2 app's look. **Its canvas
is the app itself, running**: the app's web development build live inside the tool, and
the same app running normally on the developer's simulators, phone and Mac. Turn on
inspect, tap or click anything on any of those screens, and the tool shows where its look
comes from: the Contract node, its rows, the tokens and component params behind them, and
the dials behind those. Edit any of them and the tool writes the app's files through
`tamber/1`. Every running copy updates the way it already does in development: a token
or value change through Tamber's live table with no reload, anything else through the dev
loop's hot reload. Each change is a reviewable diff.

It is a `tamber/1` client (LLP 1121 D13, D14) with no private channel, and an agent can do
everything it does. It asks of the rest of exact2:
- an **inspect mode** in every host's development build;
- a **build channel** on the dev loop: five commands down (inspect, outline, hold a state,
  pin a mode, and navigate, from T7) and three reports up (status, a selection, unsupported);
- **`set-row`**, a checked edit of one Contract row;
- **source maps** that name attributes and file digests.

No captures, no driven sessions, no tile grid in v1; several live copies side by side come
later (§4 T7). It is not a drawing tool and not a layout editor (D14).

## 0. Review rounds

**Round 4** (r6 → r7, on the restructure). Astra: RIGHT WITH CHANGES, no BLOCKER (6 MAJOR,
3 MINOR). Grok: RIGHT WITH CHANGES (2 BLOCKER, 5 MAJOR, 4 MINOR). Both called the live-app
canvas the right shape and buildable. Each finding is taken:
- **The channel is one-way** (Grok B1, Astra M1): D4 now adds registration with a nonce,
  commands with a `kind` dispatched before generation handling, and three reports up by
  POST (`status`, `selected`, `unsupported`). Devices must be launched connected to the dev
  URL.
- **`outline` needed the map** (both): the server sends each build plan-qualified sites;
  the host never sees a span. A held state's file and span come from the server's join.
- **Two streams** (Grok M3): T2 and T2b run one `--wasm` loop; the JS loop joins later,
  publishing on both.
- **Stale reports** (Astra M3): a report carries its applied design digest, pin revision
  and mode tuple, and a stale one is refused.
- **Holds across reloads** (both): the server replays holds and pins with the next hello,
  only through a checked site correspondence; one hold per build.
- **Cleanup** (Astra M5): commands belong to their client and clear when it goes; inspect
  has a local way out.
- **The JS fallback** (Grok M4): wasm until the JS runtime reports origins and instance
  paths row for row; the map's origin decides the rungs for any non-runner report.
- **The iframe** (Grok M6, Astra m7): sized explicitly; never scripted by the tool;
  scrolled into view by the host; one frame in v1; an open-in-tab fallback; LLP 1121 D14
  amended.
- **Remote canvas** (Astra M6, Grok M6): an authenticated canvas gateway, revoked with the
  token.
- **Minors:**
  - text size in `pin`;
  - the workbench points at D3;
  - reports bound by nonce;
  - inspect uses the real tap's hit test;
  - Q6 says what a relay would carry;
  - the DEFERRED text lands with this RFC (Astra m9; LLP 1121 §9 Q3 is updated beside it).

**Charlie's restructure** (r5 → r6, 2026-10-13). On Q5 he asked why v1 needs captures at
all: "Can't we just have it edit the code in place and you see the hot reload". It
doesn't. r6 takes that:
- **The canvas is the running app** (D3): the web build in an `iframe` inside the tool,
  live, plus the developer's simulators and devices running normally. Edits arrive by the
  live table or the hot reload, which already exist.
- **Inspect is a mode of the running app** (D5): in a development build the host takes the
  tap or click itself, hit-tests it natively, and reports the node, with the runner's own
  explanation of it, to the Tamber server. r4's captures, its `layout`-at-a-point form and
  the capture identity and barrier are gone.
- **The build relay shrinks to four control messages** on the dev loop's channel: inspect
  on or off, outline, hold a state, pin a mode (D4). The tool drives no session; tile
  sessions, captures, their storage and their delivery are gone.
- **Security** narrows accordingly (D13): no driving of apps from the tool, no images.
- **Staging** (§4): T2 is the live canvas, T2b and T2c inspect on the web and then natively,
  and T7 is several live copies side by side (a mode grid), considered as live copies
  rather than captures.
- Q6 and Q7 are re-asked in plain words (§9).

The rounds below reviewed r1–r4, whose canvas was captures of driven sessions ("tiles"). What they settled about provenance, the scope ladder, `set-row`, scrubs, holds, `commit`, proposals and security still holds where r6 kept the mechanism.

**Round 1** (r1 → r2). Astra: RIGHT WITH CHANGES (2 BLOCKER, 8 MAJOR, 1 MINOR). Grok:
RIGHT WITH CHANGES (4 BLOCKER, 8 MAJOR, 4 MINOR). Both accepted the shape: a `tamber/1`
client, files as the only design state, fail-closed provenance, no layout editor. Both
found the machinery underspecified. Each finding is taken:
- **Hit-testing** (Grok B1, Astra M4): `layout` boxes are not paint order or hit targets.
  Now the host hit-tests a point, as a form of `layout`; each capture is taken with its
  geometry in one settle, and a stale selection is refused (D5).
- **Web tiles have no token provenance** (Grok B2, Astra M5): the JS target keeps no
  kernel, and its map is reached through the wrong producer. Now the JS runtime reports a
  row's token beside its computed value, each build advertises its own plan and map, and
  until both exist web tiles run the wasm host, which has the kernel (D5).
- **`set-row`'s address** (Grok B3, Astra B2): a plan-digest span is not a source revision.
  Now maps carry source digests; the server resolves the row in the source AST at write
  time, parses the value as the row's type, compiles the candidate before writing, keeps
  bytes for undo, and refuses tag-origin rows (D6).
- **The relay** (Grok B4, Astra M3/M10): now split into observing (for `read` and `propose`
  clients) and driving (for `write` only), from an allowlist of request forms, with
  server-owned captures. Tiles are sessions the server launches and owns (D3).
- **Held states** (Grok M5, Astra M8): a form of `tap`, not `prefer`, with full semantics;
  state tiles are separate web sessions (D8).
- **The default scope** (Grok M6): back to exact1's: the param, otherwise the source row;
  token and dial offered, never the default (D6).
- **Templates and styles** (Grok M7, Astra M6): every expansion of a source span is
  outlined; `class:` rows are edited on the style; the narrow rung is "this template"
  (D6).
- **Live versus rebuild** (Grok M8): the chip says which before the gesture (D6).
- **Unspoken surfaces** (Grok M9, Astra M9): `commit`, held proposals and tile sessions
  are named as server additions; presence and retained visual history are cut from v1;
  import has no preview before writing (D9–D11).
- **DEFERRED** (Grok M10, Astra B1): §9 Q2 asks to amend §Tooling and the source-map line
  together; the server, not a host, joins the map (D5).
- **Over-scope** (Grok M11): the canvas lands on web tiles first; native tiles only after
  that gate (§4).
- **The prompt's data** (Grok M12): it stays local unless the developer names a model (D10).
- **Inverse edits and impact** (Astra M7): an edit targets one mode branch; no inverse is
  invented for a derived colour; impact comes from the dependency closure (D6).
- **Minors:**
  - text size is `prefer root-font-size` in px (D3);
  - captures are served over loopback, not as data URLs, and navigation is mirrored by
    location, not by point (D3);
  - every row source the runner reports is named (D5);
  - captures stay in private temporary storage (D13).

**Round 2** (r2 → r3). Astra: RIGHT WITH CHANGES, no BLOCKER (9 MAJOR, 1 MINOR). Grok:
RIGHT WITH CHANGES, no BLOCKER (7 MAJOR, 3 MINOR). Both confirmed the round-1 fixes: the
host hit-test, the wasm fallback, source digests and the AST edit, the split relay,
exact1's default, server-owned commit and proposals, web before native. Each finding is
taken:
- **Live is the closure's, not the rung's** (both): the server answers live or rebuild for
  each candidate transaction from Tamber's dependency closure, and the chip shows that. A
  scrub is a **scrub session**: previews on the tiles while the gesture lasts, and one write
  on release; re-scope inverts that one transaction. The param rung moves to T3, where
  params exist (D6, §4).
- **`set-row`'s attribute** (Grok M2, Astra M5): the map records the winning attribute's
  own span (a shorthand's when the row is an expansion); the style rung addresses `style
  <Name>` and its row in the package; conditional `class:A/B` origins are refused; the
  package compiles before writing; detaching a param's last reader removes the param in the
  same transaction (D6).
- **Fail closed, consistently** (Grok M3): the template and style rungs only for an
  `authored` row of a single origin; token and dial rungs only when the node's own specified
  value is the token; inherited, dynamic and boundary-implied rows read-only, their tokens
  shown as links (D5).
- **Observers don't drive** (both): settling and pins are the server's; `read` and `propose`
  get captures, `layout` and `tree` only; `tap` and `type` carry no `ticket`, `path` or
  `files` (D3).
- **Captures travel on the authenticated socket** (both), so a paired client sees tiles;
  native window captures map through the chrome inset (D3, D12).
- **Holds are each their own state** (Grok M6), re-applied after a reload; **inspect shows
  the held overlay's rows** (Astra M4) (D5, D8).
- **The target branch** (Grok M7): an edit writes the branch that won on the tile; a new
  `when` is an explicit "only this mode" (D6).
- **Capture identity** (Astra M1): a capture is bound to the tile's carrier generation,
  incarnation, epoch, clock and applied design digest, with a barrier pairing pixels and
  geometry (D3).
- **Re-scope transformations** (Astra M3) are enumerated; others are refused (D6).
- **`commit`'s guard** (Astra M8) checks each file against the server's last written version
  and HEAD, and commits through a temporary index, so unrelated staged work is untouched
  (D9).
- **Proposal acceptance** (Astra M9) keeps the reviewed bases; a stale or partial accept is
  recomputed and shown again (D10).
- **Minors:**
  - the tool's URL and the tiles' URLs are named (D2);
  - the web hit-test is `exact_geometry.point` (D5);
  - a single mount is "this template · 1 instance" (D6);
  - a rebuild carries what the dev reload carries (D6).

**Round 3** (r3 → r4, written in without another round). Astra: RIGHT WITH CHANGES, no
BLOCKER (5 MAJOR, 2 MINOR). Grok: RIGHT WITH CHANGES, no BLOCKER (5 MAJOR, 4 MINOR). Both
confirmed round 2's fixes. Each finding is taken:
- **Styles are per file** (Grok M1): the style rung addresses the attribute on `style
  <Name>` in the node's own file; its blast radius is that file's users (D6).
- **The selection checks all five capture fields** (both): the carrier generation is stamped
  into requests and echoed; the epoch, clock and design digest are sampled after the image
  wait (D3, D5).
- **Boundary rows** (Grok M3): reported as `authored` today, so an `authored` row the map
  does not name is read-only; a `boundary` source is proposed (D5, §3).
- **`commit`** (both): a baseline per file (HEAD, index entry, working-tree digest), every
  foreign change watched, the real index's entries for committed paths updated and no
  others; `.contract` files included (D9).
- **Scrubs** (Grok M5, Astra M2): `write` only; one at a time; a competing write, a cancel,
  a disconnect, the server's loss or 30 s idle ends one; tiles get the latest committed
  snapshot; release is digest-checked; each tile resolves the branch edit for its own mode
  (D6).
- **Effective traits** (Astra M4): captures are labelled with the scheme and mode the tile
  actually shows, read back; an app's own `setScheme` is detected (D3).
- **Remote transport** (Astra M5): TLS or an explicitly trusted tunnel, or refused (D12).
- **Minors:**
  - holds are keyed by site and instance path, in a field named `holdState` (D8);
  - the JS target's hit-test is `elementsFromPoint` with its own view id (D5);
  - T2's gesture is an explicit dial or token choice, live when the server says so (§4);
  - a shorthand is edited as the shorthand, and only written attributes are editable (D6);
  - undo is a preimage restoration that bypasses fmt (D6);
  - detaching a param removes the closure it alone kept reachable (D6);
  - wasm rebuilds lose focus (D6).

## 1. Motivation

- **Tamber makes a design data that swaps live** (LLP 1121 D7, D13). Something has to put a
  person's hands on it while they look at the real app.
- **exact1 built this as Design Mode** (LLP 0158) and learned four things:
  1. **Scope is the design problem.** "This button, all buttons, or the radius token?" Its
     answer, a scope ladder whose default never reaches the theme, is kept (D6).
  2. **Provenance must be exact or the edit must not commit** (D5).
  3. **DOM-only was the ceiling**; native editing never landed (0304 W5). Here inspect is
     a host mode on every platform, but the web lands first (§4).
  4. **Editing security was deferred** (0304 W6); here it is designed in (D13).
- **Charlie's point:** in development the app already reloads on every edit, and Tamber's
  live table updates values with no reload at all. A picture of the app is a worse copy of
  something already on screen.
- **The repository's apps need it.** T3 Code hand-built "where is this colour used"
  (`issues/closed/20261010-paint-role-node-list-read.md`); the Bluesky clone's modes, font
  sizes and post button are exactly these edits (LLP 1121 §1).

## 2. Decisions

### D1 — What it is for

The **primary way to shape an exact2 app's look**, for designers and agents as co-editors
through one protocol (D10). It edits the design: tokens, dials, axes, component params,
state looks and, at the narrowest rung, a node's paint rows (D6). Structure, layout, data
and behaviour stay with Contract source (D14).

### D2 — Architecture

- **A web JS-target Exact app** (LLP 1121 D13). Its TypeScript data module is its only
  client code, speaking `tamber/1` with the project's local token. *As built in Tamber stage 3
  (LLP 1121 §10):* requests are `POST /__tamber`, and the tool follows changes by the long poll
  `POST /__tamber/events`, because a JS-target data module's `fetch` reads a whole body and
  `exactStream` merges a server-sent stream. There is no WebSocket.
- **The Tamber server**, in the dev loop (LLP 1121 D13), owns:
  - the design files and every write to them (`set-row`, `commit`, held proposals, scrubs);
  - the selection join: a build's report plus that build's source map (D5);
  - the build channel: registration, commands and reports (D4).
- **No design state of its own.** The files are the design, git the history.
- **Two URLs.** `bun exact.mjs tamber tool` opens the tool at its own local URL, served by
  the Tamber server. The app's web build is the dev loop's own URL, which the tool embeds.

### D3 — The canvas: the app, running

- **The web build, embedded.** The tool shows the app's web development build in one
  `iframe` (LLP 1020) at the dev loop's URL, framed as `localhost` on its port: another site
  than the tool's 127.0.0.1, so a browser with site isolation runs it in its own process
  (amended after T2's gate; the dev loops answer on [::1] too).
  - **It is the real app:** it runs, takes input, navigates, and reloads on a structural edit
    as it does in a browser tab.
  - **An `iframe` is a second document, not a second session in the tool's runtime.** That
    is why a JS-target app can embed it, where LLP 1121 D14 said hosting another session
    was later; LLP 1121 D14 is amended to say so.
  - **Size.** The tool sets the frame's width and height (a phone, a tablet, a desktop),
    since a bare `iframe` is 300×150 and never sizes to its guest (LLP 1020 D1).
  - **Origins.** The tool and the dev loop are different origins, so the tool never
    scripts the frame. Every control goes through the server (D4). A node selected or
    outlined off screen is scrolled into view by the host, not by the tool.
  - **One frame in v1.** Two frames of one dev URL would share storage (the canvas's
    `localhost` origin keeps its own, apart from a tab at the printed URL); several copies are
    T7, each at a loopback host of its own (`exact-copy-<n>.localhost`), so each copy is its
    own origin with its own storage (T7 as built). An "open in a tab" button covers what an embed cannot do (LLP 1020 defers
    permission delegation and navigation control). Web content the app itself embeds is a
    leaf to inspect.
- **Devices and simulators, as they are.** The developer runs the app on the iOS simulator,
  a phone or the Mac in the usual way, but **connected to the dev URL**
  (`bun exact.mjs ios --run` with the dev server as its live source), not watching a plan
  file: only a connected build gets live-table batches and commands, and reports
  back. The tool lists the builds that have registered (D4).
- **How an edit shows.** Through what the dev loop already does:
  - a token, dial or param value whose closure is `live` (LLP 1121 D13): the live table,
    with no reload, on every connected build at once;
  - anything else (a `set-row`, a state part, a structural change, a motion token): the dev
    loop's rebuild and hot reload, which carries slots, settled resources and the clock
    (LLP 1007 §6; the JS target's checkpoint also restores focus).
- **Modes.** The tool pins a build's app mode, scheme, contrast or text size with a control
  message (D4), the same facts `prefer` pins. One copy shows one mode at a time; the app's own
  preview screens show several at once inside one copy (below); a grid of live copies, one
  per mode, is T7. On the canvas and on every native build, an app's own `setScheme` beats a
  requested scheme, so the tool shows each build's **effective** scheme, contrast, mode and
  text size, from the build's status report (D4).
  - **A pinned grid copy (T7) is the exception.** There the person asked one copy to show one
    mode, so the pin holds over the app's own `setScheme`, `setMode` on a pinned axis and
    `setRootFontSize` until the copy is handed back. The tile says when the app asked
    otherwise (the status's `app`), and on hand-back the app's latest choice applies. Handing
    a pin back restores only the axes that pin set (theme2's rulings F2, 2026-10-19).
- **Preview screens.** Ordinary routes, written by the developer (the tool offers a
  starter), that lay out a component or screen under several `color-scheme` and
  `-exact-mode` pins (LLP 1121 D5). A Light preview is `color-scheme="light"`; Dim and
  Black are `color-scheme="dark"` plus the pin. The Bluesky clone's Appearance screen is
  one. A copy on a preview screen is the component workbench (D7).
- **No captures in v1.** Nothing is screenshotted, stored or sent as an image.

### D4 — The build channel: five commands down, three reports up

**Today the dev channel only goes one way.** It is server-sent events (`host/web/dev.js`'s
`EventSource`; the JS page's `/__dev/page`), and a host acts only on a generation (a new
plan) or a reload. A native or wasm session can't open `tamber/1`, since it can't send
socket frames (LLP 1121 D13). So D4 adds both directions.

- **Registration.** On connecting, a development build gets a **nonce** in its stream's
  hello. It answers with one POST to the dev loop (`/__dev/report`) carrying:
  - the nonce;
  - its host and capabilities (which commands it speaks);
  - its plan digest and applied design digest.

  The Tamber server lists registered builds by id; that list is what `hello` gives clients.
  A report without a current nonce is refused, since the dev loop accepts other POSTs with no
  token.
- **Commands, down the stream**, each a frame with a `kind` that a host dispatches before
  any generation handling, so a command can never be read as a plan push:

| Command | What the build does |
|---|---|
| `inspect` on or off | enters or leaves inspect mode (D5) |
| `outline` (a set of sites in this build's plan, or none) | outlines every mounted node with one of those sites, in inspect mode, and scrolls the first into view |
| `hold` (a site and instance path in this build's plan, a state, or none) | holds a host-known state on that node; one hold per build, a new one replacing the old (D8) |
| `pin` (scheme, contrast, an app mode, the root font size in px, or none) | pins those facts, as `prefer` would; `none` hands them back |

- **Reports, up by POST**, each with the nonce:
  - `status` (on connect and on any change): the effective scheme, contrast, app mode and
    text size, the plan digest, the applied design digest, and the pin revision it has
    applied;
  - `selected` (D5);
  - `unsupported` (a command it can't do, by name).
- **Addresses are plan-qualified.** `outline` and `hold` name sites in the receiving build's
  plan. The server computes them from a source span through that build's map (D5). A host
  never sees a span or a map.
- **Ownership and cleanup.** Each command belongs to the client that sent it.
  - When that client disconnects or its token is revoked, the server clears its inspect,
    outlines, holds and pins on every build.
  - A build that reconnects (a JS reload is a new document; a wasm program rebuild is a page
    reload) gets its current pins and holds replayed with its new hello. A hold is replayed
    only where the server finds exactly one corresponding site in the new plan, through the
    old and new maps. Otherwise it is cleared, with a diagnostic.
  - On the device, inspect always has a local way out: the banner's close control, or
    Escape.
- **One loop, one stream.** The JS loop today forwards a simulator's `/__dev` to a separate
  resident producer (`host/web-js/dev.mjs`), so its page and its devices listen on different
  streams. T2 and T2b therefore run one `dev.mjs --wasm` loop, which serves the frame and the
  devices on the same `/__dev`. The JS loop joins at T2b's second half, publishing every
  command and batch on both streams.

The tool drives no input into an app (no taps, no typing), takes no pictures and launches no
sessions. `inspect` needs `read` scope and the other four commands need `write` (D12); all are
journaled with their client.
- **`navigate {location}`** (T7, the fifth command): route a build to a path, query and
  fragment on its own origin, in place, as a link to it would (the app's navigation root or
  router), with no reload. A location with a scheme or host is refused. A build that can't
  answers `unsupported`, and the tool reloads that copy's frame at the location instead. A
  status carries the build's `location`. It is navigation, not input: no tap or key is driven,
  and it holds nothing on the build. The grid mirrors navigation by location, never by point.

### D5 — Inspect: tap the real screen

**Inspect mode** is part of every host's development build, compiled out of release builds.
- **Entering.** The tool's inspect toggle sends `inspect` on to every registered build (or
  one). Each shows a small banner, with a close control, so the person knows taps won't
  reach the app.
- **Selecting.** In inspect mode the host takes pointer and touch input itself:
  - hovering (a mouse) outlines the node under the pointer;
  - a tap or click selects it.

    The host hit-tests the real input point with the same path a real tap uses (on the web,
  the runtime's own hit test, `host/web/geometry-glue.js`), so inspect and a real tap cannot
  disagree. There is no capture to go stale.
- **The report.** The host posts a `selected` report (D4):
  - the nonce, the build id, its plan digest, its applied design digest and its pin
    revision;
  - the node's effective mode tuple;
  - the view's site and its `instance` path;
  - the runner's own explanation of the node, as `layout <id>` gives it (LLP 1035.002): each
    row with its value and source, the token beside it (LLP 1121 D7), and, while a hold is
    active, the held state's overlay rows (D8).

  The runner computes this in-process; it is the same answer the agent gets.
- **The join.** The server checks that the report is current: its nonce, plan digest,
  applied design digest and pin revision match the build's latest status. A delayed report
  from before a live-table batch or a pin is refused, and the build is asked again. The server
  then looks the site up in that build's own source map, verified by plan digest, and checks
  the map's source digest against the file on disk (D6). A held state's file and span come
  from this join, not from the host. It then publishes the selection to subscribed clients.
  It also sends each build an `outline` of that build's own sites for the source span, so
  every copy of the node (each `each` row, each expansion of a component body) is outlined on
  every screen. The source-map admission let the driver read maps and
  never a host; the server joins them here, and no host reads a map (§9 Q2, ruled).
- **Provenance**, in the inspector:
  1. **Node → source:** the file, line and component call-site chain, and each row's source:
     `authored` with its origin (`own`, `class:<Name>`, `tag`), `dynamic`, `inherited`,
     `initial`, `boundary` (a mode boundary's implied colour row), `applied: unsupported`.
  2. **Row → token**, with the `when` that won (`explain`, LLP 1121 D13).
  3. **Token → dials and params** (`explain`).
  4. **Impact:** `uses` gives the mounted nodes holding the token, labelled "mounted uses";
     the full impact of a change is the tokens whose folded values it changes.
- **The web JS target** keeps no kernel: its `layout` reads `getComputedStyle`, so a token
  arrives as `rgb(…)` (`host/web-js/agent.js`), and its dev loop's `/exact.json` answers for
  another producer's plan (`host/web-js/dev.mjs`). Its row sources are CSS's (`authored` or
  `dynamic`), not the kernel's origins, and its DOM carries a site but no instance path
  (`host/web-js/src/emit.rs`). So the embedded build is the wasm host's
  (`dev.mjs --wasm`), which has the kernel, until the JS runtime reports, row for row as the
  kernel does: each row's token beside its computed value, its origin, its instance path,
  and its own plan and map pair (§3). Whenever a report is not the runner's own, the server
  takes each row's origin from the map (`contract/cli/src/map.rs`), not from the host, to
  decide the rungs.
- **Fail closed.** An edit is offered only when the build's map matches its plan and the
  map's source digest matches the file, and then only these rungs:
  - **template or style:** an `authored` row from a single origin (`own`, or one
    `class:<Name>`; a conditional `class:A/B` is refused) whose mapped span is an attribute
    the author wrote (a compiler-derived row is reported `own` but has no attribute);
  - **token or dial:** this node's own specified value is the token. A token reached through
    inheritance, a `boundary` row or a dynamic expression is a link to the token, never this
    node's edit;
  - **state part:** while a hold is active, the held state's winning overlay row (D8).

  Everything else is read-only, with its reason. Until the kernel reports `boundary`, an
  `authored` row whose attribute the map does not name is read-only.
- **Selecting from the tool.** Clicking a token's "uses", a component in the panel, or a
  changeset entry sends each build an `outline` of its own sites, so the person sees where it
  is on screen.


### D6 — The scope ladder

Every edit has one scope and one **target branch**. The branch is the one `explain` says
won on the copy the selection came from: the token's default, or the `when` for that copy's
mode tuple. The chip names it ("`surface.page` · Dim"), because on a Dim copy the default is
hidden behind the Dim `when`, and editing it would change nothing the user is looking at.
Adding a `when` that does not exist yet is an explicit action, "only this mode". From
narrowest to widest:

| Rung | Writes | Through | Applies | Available when |
|---|---|---|---|---|
| **template** | one paint row of this source node, in every expansion | `set-row` | rebuild | the row is `authored`, origin `own` |
| **style** | the row in `style <Name>`, for every node using it | `set-row` on the style | rebuild | origin a single `class:<Name>` |
| **param** | this component's param, for every instance | `set-param` (LLP 1121 D15) | the closure's | the row reads a param |
| **token** | the token's branch, everywhere | `set-token` | the closure's | the node's own value is the token |
| **dial** | a dial, and every token folded from it | `set-dial` | the closure's | the token depends on a dial |

**Live or rebuild is the closure's, not the rung's.** Tamber rebuilds instead of applying
the live table when the change reaches anything substituted at build: a `duration` or
`easing` token, or any dial or param folded into one (LLP 1121 D13). So the tool asks the
server: `propose` answers, for the candidate transaction, `live` or `rebuild` (a minor
addition to `tamber/1`). The chip shows that answer before the gesture.

- **The default, as exact1 set it** (0158 §3.1): the **param** when the row reads one,
  otherwise the **template** or **style** rung. **Token and dial are offered, never the
  default.** With the live table, a token edit repaints every use at once, and an
  accidental one is the mistake the ladder exists to prevent.
- **"Just this" is not a rung.** There is no per-instance paint store: params are the
  design's, not props (LLP 1121 D15). The template rung is labelled "this template", and a
  node that renders once is "this template · 1 instance", since a second instance would
  inherit the edit.
- **A scrub is a scrub session**, a server addition to `tamber/1`:
  - **Who.** Opening or updating one needs `write`; a scrub drives running apps. `propose`
    answers `live` or `rebuild` and never pushes a batch.
  - **One at a time.** A second scrub is refused while one is open.
  - **During the gesture.** Each value goes to every connected build as a live-table batch
    and is written to no file. The batch is the branch edit, so each build resolves it for
    its own mode tuple: a Dim value never paints a Light copy.
  - **Release.** The server writes once, as one transaction, checked against the digests
    the scrub opened with. If that check fails, nothing is written and the tool says so.
  - **End without release.** A competing write cancels the scrub first. So do a cancel, a
    disconnect, the server's loss, and a scrub idle for 30 seconds. On any of them, every
    build gets the **latest committed** snapshot, not the gesture's starting one. A build that
    reconnects gets the committed snapshot too.
  - **While open**, the files and the running copies differ, and the tool shows
    "previewing".
  - A rebuild-class edit has no scrub: it applies on release.
- **A rebuild** carries what the dev reload carries (LLP 1007 §6: slots by name, settled
  resources and the clock). The JS target's checkpoint also restores focus; on a wasm build
  focus is lost. What it does not carry, such as scroll position, is lost, and the chip says
  "rebuilds" before the gesture.
- **Act first, re-scope after** (0158 §3.2). The chip on the last edit re-applies it at
  another rung, as one transaction: the inverse of that edit's transaction, then the new
  write. The blast radius is outlined as it happens.
- **Re-scope and promotion are an enumerated set**, each one transaction. Anything else is
  refused with the reason.
  - **Template to param:** adds the param (`add-param`) and rewrites the row to read it.
  - **Template to token:** adds a token whose default is the literal and rewrites the row.
    Offered only for an unconditional value.
  - **Param ↔ token:** between a param and the token it aliases directly
    (`radius = "radius.md"`). A param whose value is an expression offers the expression's
    inputs instead.
    - **Detach param to template:** writes the literal at this source node. If that leaves
    params unreachable (this one, and any it alone made reachable: `row → param.b →
    param.a`), the same transaction removes that closure, listed in the proposal, so
    `tamber-param-unused` never fires. If any param in that closure has an app value in the
    `.tamber` file, the detach is refused.
  - **A `when`-specific value is never moved to an unconditional literal**, which would
    change every mode.
  - **No re-scope to a dial:** a colour derived from several dials has no unique dial value.
    The tool shows the dials as inputs instead.
- **Expressions are kept.** Editing a token whose value is an expression offers that
  expression's inputs. Overwriting it with a literal is an explicit "pin", marked as LLP
  1121 D14 marks a pinned override.

**`set-row`**, the server's checked edit of one Contract row:
- **Address.** The map today records an element's span and its winning rows' origins, not
  an attribute's span (`contract/cli/src/map.rs`). So the compiler adds two things:
  - each source file's digest;
  - for each winning authored row, the span of the attribute it came from. When the row is
    an expansion (`padding-right` from `padding`), that is the shorthand's span.

  The edit names the file, that attribute span, the row and the digest the tool saw. A plan
  digest is never the address: formatting moves spans without changing it
  (`docs/reference.md` "Source maps").
- **Resolution.** The server finds the attribute in the file's syntax tree at that span,
  checks that it is the one attribute the row came from (else it refuses), parses the new
    value as the row's type, and splices that one value token.
- **What it addresses.**
  - **A shorthand:** when the row is an expansion, the control edits the shorthand's value
    and is labelled as the shorthand (`padding`), never splitting it.
  - **Written attributes only:** the template rung is offered only when the mapped span is
    an attribute the author wrote. A row the compiler derived is reported `own` but has no
    attribute, so it is read-only.
  - **The style rung:** the attribute on `style <Name>` in the node's own file. Styles are
    declared and resolved per file (`contract/lower/src/class.rs`), so two files may each
    have a `style Card`. Its blast radius is the nodes in that file whose row's origin is
    that style.
- **Check.** The server compiles the candidate package before writing; a refused compile
  writes nothing.
- **Write.** Through LLP 1121 D13's journaled commit, which runs `contract fmt` on the
  file. So the diff is one line only when the file was already formatted; otherwise the
  review shows fmt's other changes, labelled as such.
- **Undo restores the bytes.** The inverse is a **preimage restoration**: the server writes
  back the exact bytes it replaced, guarded by the digest of what it wrote, and bypasses
  fmt. A semantic inverse put through fmt would not restore an unformatted file.
- **Refused:** tag-origin rows, computed rows, layout rows (D14), and rows that LLP 1121 D8
  does not allow in a state part. `border-radius` and `font` are allowed once LLP 1121
  stage 6 admits their kinds.
- It is a minor-version addition to `tamber/1` (LLP 1121 D13's table). §9 Q3 asks whether
  the template rung belongs in the tool.

### D7 — Panels from declared metadata

The tool hand-draws no control:
- **Dials**: each dial's `control`, `range`, `step` and `label`, by `group` and `priority`
  (LLP 1121 D4).
- **The token grid**: tokens × every mode tuple and scheme, with swatches from `resolve`
  (the stored value or residual, labelled as such) or from `resolve-on` a named build. It
  has contrast badges between declared on-pairs, each saying which resolution it assessed.
- **The component panel**: the selected component's params, grouped by `group`, ordered by
  `priority` (3 behind "more"), each with its `control` and `options` (LLP 1121 D15). A
  pinned param shows LLP 1121 D14's mark.
- **The workbench**: a preview screen (D3) for one component, with its params, its states
  (D8) and its blast radius.

A component with no params shows its rows (D5) and an offer to promote one to a param
(D6).

### D8 — States

- **Runner-known states** (`:disabled`, `:checked`) are data: a preview screen sets them, or
  the person puts the running app into them.
- **Host-known states** (`:active`, `:hover`, `:focus-visible`) are held, so a designer can
  see a pressed look without pressing (§9 Q4, ruled). The tool sends `hold` (D4) for the
  selected node. The same mechanism is a new form of the agent API's `tap`, so an agent can
  do the same: `{"op":"tap","id":V,"holdState":"active"|"hover"|"focus-visible"}`, cleared
  with `"holdState":null` (DEFERRED: "a new input is a form of `tap`"). The field is
  `holdState`, so it cannot be read as `tap`'s contact phase `hold`. Semantics, the same
  either way:
  - **Each hold is its own state.** `active` holds the press on the node the press resolves
    to, the innermost with a `press` handler (LLP 1121 D8). `hover` and `focus-visible`
    hold those overlays on the named node, which needs no press handler. Each applies only
    that state's overlay, with the host's ease, under Reduce Motion as the real state would.
    - **Keyed by site and `instance` path in the build's plan**, not a view id. A reload is a
    new document, or a new plan, with nothing left to re-apply. So the **server** replays the
    hold with the build's next hello, after a `set-state` rebuild among others. It replays it
    only where it finds exactly one corresponding site in the new plan, through the old and
    new maps, and otherwise clears it with a diagnostic (D4).
  - **One hold per build**; a new one replaces the old. Several states at once wait for T7.
  - **Cleared** by an explicit clear and by real input on that node, which always wins.
  - **Disabled** nodes refuse all three.
  - **Unsupported** on a host that cannot hold that state, answered by name; iOS has no
    pointer, so it answers `hover` that way.
- **Inspect shows what is held.** While a hold is active, the node's explanation carries the
  held state's winning overlay rows beside the base rows: the state, the declaring state
  part (its file and span, from the map) and any token. This is an addition to `layout`'s
  node answer, since overlays live outside the runner's base rows (LLP 1121 D8). An edit of
  a held look goes to that state part (`set-state`, a rebuild). A host that cannot report
  overlay rows leaves the held look read-only, marked "provenance incomplete".
- **Several states at once** need several copies, so they come with T7. In v1 the tool
  steps one copy through them.

### D9 — Undo and history

- **Undo** issues each write's inverse (LLP 1121 D13). An inverse whose preconditions fail
  is refused and names who changed the file: a client, or "changed on disk" for a hand edit
  or a checkout.
- **The session changeset**: the design files' diff since the session began, as edits in
  plain words with the lines under each.
- **`commit`**, a server addition to `tamber/1`:
    - **What.** The files the session wrote, `.tamber` and `.contract` alike, with a message
    listing the edits and their authors (person or agent).
  - **The baseline.** When a session first touches a file, the server records HEAD, the
    file's index entry and its working-tree digest. It also watches the file and records
    every change it did not make.
  - **The guard.** It refuses, naming the file, if any of these holds:
    - the working tree or the index differed from HEAD at the baseline;
    - HEAD has moved since;
    - any change the server did not make was seen, even one a later server write
      overwrote;
    - the working tree now differs from the server's last write.

    So a commit never sweeps in work that isn't the session's; such a file is committed by
    hand.
  - **The index.** It commits through a temporary index holding HEAD plus exactly those
    files. It then updates the real index's entries for those paths to the new commit's
    blobs and touches no other entry, so the developer's staging elsewhere stays as it
    was.
  - It never pushes.
- **History** is the git log of the design files, read as edits. "Revert" proposes a past
  commit's inverse.
- **Not in v1:** a visual history. The review shows the diff, and the running app shows the
  result.

### D10 — Agents as co-editors

An agent is a client with the same token, reads and writes.
- **Attribution.** Every write carries its `client` (not a `hello`: clients share one server;
  as built in Tamber stage 3), and each event names it; the tool outlines an agent's
  edits in its own colour.
- **Held proposals**, a server addition. An agent's `propose` can be held: the server keeps
  the transaction in memory (with an expiry) and announces it on `subscribe`. The tool
  shows it for review (D11's view): accept all, some, or none.
  - **Accepting** applies it with the base digests it was reviewed against.
  - **If a file has moved since**, or only some items were accepted, the server recomputes
    the transaction on the current files. The tool shows the recomputed diff, and the person
    accepts that, never the old operations replayed onto new bases.
- **Prompt-as-tweak** (0158 §9). A selection-scoped request goes to an agent with a bundle:
  - the selection's provenance;
  - the rungs;
  - the relevant tokens and params.

  It carries no images. The agent answers with a proposal at an explicit rung, which the
  person reviews, and the running app shows it once accepted. **The bundle stays on the
  machine** unless the developer has named a model that runs elsewhere; the tool says so
  before sending, since provenance names the app's source. The tool holds no keys.

### D11 — Importers' review

LLP 1121 D14's importers (shadcn, Tailwind v4, DTCG, Astryx) propose; the tool reviews.
Each import shows:
- the mapped tokens, with values per mode and swatches;
- the role overrides, each off until ticked (LLP 1121 D1);
- the unmapped values, each with its reason.

Accepting writes the chosen items as one transaction, and undo is one step. An import is not
previewed on the running copies before it is written. The only preview is a scrub's, which lasts as
long as a gesture (D6); a standing preview layer would be a second truth, which exact1 found
costly. Instead the review shows each value's swatch in each mode, and an accepted import
is undone in one step.

### D12 — Collaboration

- **Local first.** Several clients on one machine edit through one server, serialized by
  digests (LLP 1121 D13). The tool lists connected clients. Live presence (each client's
  selection) is not in v1.
- **Remote, by pairing** (LLP 1121 D13), **only over an encrypted transport**:
  authenticated TLS, or a tunnel the developer explicitly trusts. The dev loop serves plain
  HTTP (`host/web-js/dev.mjs`), so a remote connection that arrives unprotected is refused,
  pairing included.
  - A paired client reads, proposes or writes through `tamber/1` as its scope allows, and
    receives selections and outlines' sources like any client. Its access ends when its
    token is revoked or expires.
  - **Seeing the app** remotely goes through an **authenticated canvas gateway**: the Tamber
    server proxies the app's dev URL under the paired client's token, over the encrypted
    transport, on **its own origin** (a second TLS port on the same listener and certificate),
    so script in the framed canvas can't read the tool's DOM, storage or token (LLP 1121 stage
    10 C; theme2's ruling, 2026-10-19).
    - The tool enters the canvas with a single-use grant carried in the frame's `#g=` fragment;
      the gateway trades it for a session cookie bound server-side to the token, and the
      fragment leaves the frame's address before any request. No wire request reads a cookie,
      and a cookie that page code sets with the gateway's name grants nothing.
    - The gateway needs `read`.
    - A remote frame whose `src` was the developer's loopback URL would load the remote
      browser's own loopback, so the gateway rewrites the app's loopback URLs to itself. That
      rewrite is functional, not a security boundary: it never touches a non-loopback host,
      and exotic encodings are a known limit. The boundary is the token and the origin.
    - The dev gate checks host, origin and peer, never a paired credential
      (`scripts/install-page.mjs`).
    - Revoking the token ends canvas access too, on both ports, separately from the socket.

    Without the gateway, a remote client works on the design and sees selections, but has no
    canvas.
  - Control messages (D4): `inspect` needs `read`, since it only reports what is under the
    pointer, so a `propose` client inspects read-only; `outline`, `hold`, `pin` and `navigate`
    (T7) need `write`, since they change what the developer's screens show (LLP 1121 stage 10
    A's scope table).
- **No relay in v1** (§9 Q6): remote use is pairing only. A hosted relay is LLP 1121 stage
  10c, later: a blind pass-through of the client's TLS to the developer's server, which changes
  nothing here (the same pairing, tokens, scopes and gateway).

### D13 — Security

- **Development builds only.** Release builds have no protocol endpoint, no inspect mode, no
  build channel, no live table and no held state; inspect is compiled out.
- **One credential.** The project's token, in the server's memory, is given only to a
  verified-local request (LLP 1121 D13). Paired clients have scoped, revocable tokens, only
  over an encrypted transport (D12).
- **No driving.** The tool sends no taps, keystrokes or file answers into an app. Its only
  control is D4's five commands (`inspect` needs `read`, the rest `write`; D12), which are
  journaled with their client and cleared when that client goes. Inspect mode takes input only
  while it is on, with a banner and a local way out.
- **Reports are bound to their build.** A `status` or `selected` POST must carry the nonce
  from that build's stream hello. The dev loop accepts other POSTs with no token
  (`host/web-js/dev.mjs`), so without it a page that can reach the dev server could forge a
  selection.
- **No images.** Nothing is captured or stored. A selection report carries node data and
  source locations, which only token holders receive.
- **Writes only through the server**, under its digests, compile check and journal (D6, LLP
  1121 D13). `set-row` writes only files inside the project root that the map names.
- **The embed.** Locally, the tool's `iframe` loads the app from the dev loop as a browser
  tab does, under the dev gate as it is, cross-origin to the tool, which never scripts it.
  Remotely it goes through the canvas gateway (D12).
- **Prompts** stay local unless a remote model is named (D10).

### D14 — What it is not

- **Not a drawing tool.** No shapes, artboards or free canvas: everything shown is the
  running app.
- **Not a layout editor in v1.** Dragging boxes, flex direction, gap or padding by handle,
  and reordering are structural source edits. exact1 found drag intent in a flex layout
  ambiguous (0158 §14): is a drag gap, padding or basis? Lengths a design names are tokens
  and params, editable at those rungs once LLP 1121 stage 6 lands. `set-row` refuses
  layout rows. Layout editing is a later RFC.
- **Not a component editor.** It edits declared params and state parts, not parts or slots.
- **Not Figma.** Figma is a client (LLP 1121 D14), with no canvas sync.

## 3. What it needs

| Need | From | Status |
|---|---|---|
| reads, writes, `propose`, `subscribe`, inverses, token | LLP 1121 D13, stage 3 | accepted, not built |
| live table | LLP 1121 stage 4 | accepted; admitted for development builds (LLP 1121 §9 Q3) |
| `explain`, `uses`, token beside value | LLP 1121 D7, stage 2 | in progress |
| params; state parts and `set-state`; importers; pairing | LLP 1121 stages 7, 8, 3a–3d, 10 | accepted, not built |
| inspect mode in each host's development build, and the `selected` report | this RFC D5 → the hosts | proposed |
| the build channel: registration with a nonce, five commands down (navigate from T7), `status`, `selected` and `unsupported` reports up | this RFC D4 → the dev loops, the hosts, the Tamber server | proposed |
| devices launched connected to the dev URL, one `--wasm` loop for frame and devices | this RFC D3, D4 → `host/apple/build.mjs`, the dev loops | proposed |
| the canvas gateway for paired clients | this RFC D12 → the Tamber server | built (LLP 1121 stage 10 C) |
| `iframe` embedding stated as a second document, not a second session | this RFC D3 → LLP 1121 D14 | proposed |
| the held state, as `hold` and as a form of `tap` | this RFC D8 → the hosts, LLP 1012 | ruled yes (§9 Q4) |
| a held state's overlay rows, and a `boundary` row source, in the node explanation | this RFC D5, D8 → LLP 1035.002, LLP 1121 D7 | proposed |
| attribute spans and source digests in maps | this RFC D6 → the compiler | proposed |
| each build's own plan and map pair; the JS runtime reporting tokens, origins and instance paths row for row | this RFC D5 → both dev loops, the web JS target | proposed |
| `set-row`, `commit`, held proposals, scrub sessions, `propose`'s live-or-rebuild answer | this RFC D6, D9, D10 → LLP 1121 D13 | proposed (`set-row` ruled yes, §9 Q3) |
| `iframe` | LLP 1020 | built |
| source maps; `layout <id>` with sources | `contract build --map`; LLP 1035.002 D1 | built |

Gone since r5: the build relay's tile sessions, captures, `layout` at a point, and the
capture identity and barrier.

## 4. Staging and gates

Each stage needs an implementer and a date; the DEFERRED admissions are ruled (§9 Q2). Each
lands with or after the Tamber stage it needs.

1. **T1, files** (with Tamber stage 3):
   - the tool, with dials and the token grid (D7);
   - undo, the changeset, `commit` and history (D9);
   - attribution and held proposals (D10).

   **Gate:**
   - every control comes from declared metadata;
   - an edit to a formatted file is a one-line diff, and undo restores the bytes exactly,
     even for a file that was not formatted;
   - an agent's write appears attributed;
   - a stale inverse names who changed the file;
   - `commit` refuses a file that was dirty or staged at its baseline, or hand-edited between
     two server writes; it leaves other staging untouched, and never pushes.
2. **T1a, import review** (with Tamber 3a–3d), D11. **Gate:** a shadcn theme reviews with
   overrides off until ticked, every unmapped value listed, and one transaction to accept.
3. **T2, the live canvas** (with Tamber stage 4):
      - the web build embedded (D3), on one `dev.mjs --wasm` loop serving the frame and the
     devices (D4);
   - devices launched connected to the dev URL, registration and `status` reports (D4);
   - the `pin` command, with ownership, cleanup and replay;
   - the token and dial rungs, scrub sessions and the live-or-rebuild answer (D6).

   **Gate:**
   - the Bluesky clone runs in the tool's frame and on the iOS simulator and the Mac;
   - a token or dial scrub, chosen explicitly, updates all three with no reload whenever the
     server classified it `live`;
   - a structural edit hot-reloads all three and keeps their slots;
   - a cancelled scrub restores every copy;
      - `pin` shows Dim on one copy, and the tool shows each copy's effective mode;
   - closing the tool clears its pins on every build, and a reloaded build gets its pins
     back;
   - a report without the current nonce is refused.
4. **T2b, inspect on the web**:
   - inspect mode in the web hosts (wasm first);
   - the `selected` report and the server's join;
   - `outline` (D5).

   **Gate:**
   - a click in the frame selects the node the browser hit-tests, and shows its source,
     rows, sources and token chain;
   - every copy of the node is outlined;
   - inherited, dynamic and boundary rows are read-only with their reason;
   - taps don't reach the app while inspect is on.

   **Then the JS target:** once its runtime reports tokens and its own map pair, a JS-target
   frame answers inspect row for row as the wasm frame does.
5. **T2c, inspect natively**: the iOS simulator and the Mac, then a phone. **Gate:** T2b's
   gate on each native host; a tap on platform chrome selects nothing.
6. **T3, components** (with Tamber stage 7): the param rung, the component panel and the
   workbench (D6, D7). **Gate:**
   - `PostButton`'s params appear as declared;
   - a param edit re-scoped to the token it aliases is one transaction and one undo;
   - a duration param's chip says "rebuilds".
7. **T4, states** (with Tamber stage 8): `hold` and the `tap` form (D8). **Gate:**
   - the post button's pressed look shows without a touch, on the web and the Mac;
   - real input clears a hold, and a disabled node refuses one;
   - inspect on a held node shows the state part's rows;
   - editing `:active` rebuilds and the hold is re-applied.
8. **T5, template and style rungs** (with `set-row`): D6. **Gate:**
   - a template colour edit writes one line and hot-reloads;
   - a stale source digest is refused;
   - a candidate that fails to compile writes nothing;
   - promote to param and to token are each one transaction;
   - a `class:` row edits the style in its own file.
9. **T6, remote** (with Tamber stage 10): D12. **Gate:**
      - a paired `propose` client cannot write or send commands;
   - revocation cuts off its socket and, separately, its canvas through the gateway;
   - an unencrypted remote connection is refused.
10. **T7, several live copies**: a grid of the app running in several frames at once, each
    pinned to one mode (Light, Dim and Black at two text sizes), plus the states side by
    side.
    - Copies are live builds, not captures.
    - The questions this stage settles: how many web copies a machine runs well, whether
      each needs its own `--storage`, how navigation is mirrored across them (by location,
      never by point), and whether native copies (several simulators) are worth their cost.
      **Settled (T7, 2026-10-19):** six copies by default (the largest that holds a sustained
      60 fps scrub on the gate's Mac), four through the remote gateway; each copy its own
      origin and storage; navigation mirrored by `navigate` (D4); native copies not in T7
      (QUEUE).
    - Captures come back only if a later need (a review's before and after, a visual
      history) proves live copies insufficient.

    **Gate:** six live copies of the clone update together on a scrub; each shows its own
    mode's value.

## 5. Non-goals

- Drawing, artboards, free canvases (D14).
- Layout and structure editing in v1 (D14).
- Editing in release builds; a production inspect tier.
- Design state outside the files (D2).
- Driving apps from the tool: taps, typing, file answers (D4).
- Captures and visual history in v1 (D3, D9).
- Components from imported libraries (D11).
- A standing preview layer before writing (D11).

## 6. Risks

- **One copy, one mode.** Until T7, comparing modes means pinning one copy in turn, or using
  preview screens. That is slower than a grid, and cheaper.
- **Inspect touches every host's input path.** It is development-only, behind a banner, and
  lands one host at a time (§4), web first.
- **Wrong scope.** Mitigated by exact1's default, blast-radius outlines, re-scope after the
  fact, and one undo (D6).
- **Provenance gaps** make some rows read-only (D5), with the reason shown.

## 7. Cost

- **The tool:** one web JS-target app.
- **The Tamber server:** the selection join, the build channel, the canvas gateway, scrub
  sessions and the
  live-or-rebuild answer, `set-row`, `commit`, held proposals.
- **The hosts:** inspect mode (input, banner, outlines, the report) and holds, per host, in
  development builds.
- **The agent API:** the held `tap` form, and overlay rows and `boundary` in the node
  explanation.
- **The compiler:** attribute spans and source digests in maps. **The dev loops:** each
  build's own map pair. **The JS runtime:** the token report.
- **Docs:** the tool's page, and the protocol reference's new rows.

## 8. Not in this RFC

- Layout and structure editing.
- Native builds of the tool.
- The component library's workbench beyond params and states (LLP 1121 stage 12).
- A production inspect tier.
- Live presence.

## 9. Open questions for Charlie

1. **The name.** Candidates if a start helps: **Atelier**, **Loupe**, **Bench**,
   **Swatchbook**.
2. **DEFERRED.** **Ruled (Charlie, 2026-10-13): amend them, for development builds only,
   together with LLP 1121 §9 Q3.** §Tooling admits the design tool and inspect mode, and the
   2026-09-10 source-map admission is widened so that the Tamber server (never a host) joins
   maps for inspect. Release builds keep both refusals. This is Charlie's waiver, so no take
   is named. The `rules/DEFERRED.md` text lands with this RFC.
3. **The template and style rungs** (D6, `set-row`). **Ruled (Charlie, 2026-10-13): yes.**
4. **The held state** (D8). **Ruled (Charlie, 2026-10-13): yes**, as `hold` for the tool and
   as a form of `tap` for agents.
5. **Captures** (D3). **Ruled (Charlie, 2026-10-13): none in v1.** The canvas is the running
   app, updated by the live table and the hot reload. Several live copies come in T7.
6. **People who share no machine or network with the developer.** **Ruled (Charlie,
   2026-10-13): no hosted relay.** Remote use is pairing only, over an encrypted connection
   (D12). **Revisited (Charlie, 2026-10-19): "We will prob need one yes."** A hosted relay is
   LLP 1121 stage 10c, later: blind TLS pass-through, so the relay never sees a token,
   a design or a canvas, and the tool's side is unchanged.
7. **How the tool controls running apps.** **Ruled (Charlie, 2026-10-13): yes, for now.** The
   tool sends four commands (inspect, outline, hold, pin) and receives three reports
   (status, a selection, unsupported), and it never drives an app (D4). T7 added a fifth,
   `navigate`, navigation by location, not input. Taps and typing
   stay with agents, through the agent API.

## 10. As built

### T1, commit 1: the server additions, `tamber/1.1` (`6281652df`..`a0129f4ad`)

- **`history`** is the design files' git log, with token-level edits per commit.
- **`revert`** proposes, held, the inverse of a past commit as semantic operations
  (`set-token`, `unset-token-when`, `add-token`, `remove-token`, axis values), planned on
  today's files. It is never a byte restore, since a client can't send one.
  - **Refused, naming each edit and why,** where an edit has no inverse operation: the commit
    created or removed a design file, added or removed a dial, added an axis, reordered an
    axis's values, changed an axis's `under`, or changed a kind, a range, metadata, the
    design's name or its exports. A removed axis is restored with `add-axis`.
  - **Accepted after its files moved,** it is planned again from the commit, never replayed. A
    partial revert keeps its selection when planned again.
  - A client can't forge a revert's review. A removed dial counts as restored only with its
    whole declaration.
- **`import`** takes `{kind, source, name?}`, or `sources: [{name, text}]` for multi-file DTCG
  such as Figma's per-mode files. Astryx is refused there and keeps its CLI.
- **`requires: {path: digest}`**, on a transaction or a proposal, names files a write was
  planned on but doesn't edit. It is checked on the snapshot the write uses.
- **`/__tamber/tool/`** serves the tool's files to local requests only, through no symlink or
  dot file.

### T1, commit 2: the skeleton and `tamber tool` (`b516197e1`..`9bdf8cf62`)

- **The tool is served at a fixed local port, 127.0.0.1:7740.** A data module's grant names one
  origin and port, and its `fetch` needs an absolute URL.
  - So one tool runs per machine, on one project at a time. To switch, stop it and run `tamber
    tool <other app>`.
  - The port binds loopback only.
  - A port already held is refused, naming what holds it: another app's design server, or a
    process found with `lsof`. A design server for the same app, serving this checkout's
    build, is reused.
- **`bun exact.mjs tamber tool [<app>] [--via <dev url>]`** builds the tool if stale. With no
  `--via`, it starts the app's design server on that port. With `--via`, it forwards
  `/__tamber` to the dev loop that holds the design (one writer per project). `--via` takes only
  a loopback address, and only a loop serving this app.
- **The data module:**
  - it fetches the token once, and asks again once after a 404, for a restarted server;
  - its grant names the tool's origin only;
  - every write is `design-tool:<person>`, with the person asked once and kept;
  - it follows changes by the long poll, first asking with `wait: false`;
  - an epoch change or `missed` starts the list again and reads the design again.
- **Wire additions (`tamber/1.1`):**
  - `hello` carries the app's directory;
  - `wait: false` makes the long poll answer at once;
  - while the design server is down, the long poll answers its `down` event, then 503, never
    an empty answer to ask again at once.

### T1, commit 3: the dials panel, the token grid and contrast (`d1704b462`, `567ca6099`, `e2dc10b70`, `d508bba97`, `d9ffffcb8`)

- **The panel is drawn from declared metadata only.** A control that doesn't fit its dial's kind
  falls back to the kind's own control, and its help says so. Groups appear in the order they
  are first declared, priority defaults to 2, and priority 3 goes behind "More" (LLP 1121 D15).
  `control=none` is not drawn.
- **Grid columns** are app-mode tuples × light and dark. The light column is left out for the
  values of an `under scheme=dark` axis, since it repeats the default's. A residual shows its
  fallback, labelled.
- **Contrast** is between any two tokens, on stored values (each side clipped to 8-bit sRGB),
  labelled so. Declared pairs (`pair=`, LLP 1121 D3) are not built yet.
- **Additions to `tamber/1.1`:** `resolve` carries `srgb {light, dark}` and `files` (the digests
  it read), `explain` carries `files`, and `hello` carries `epoch`. The tool re-reads a grid
  whose reads span two revisions, and it binds undo and redo to one server epoch.
- **Known limitation:** a colour dial's `select` options written as `oklch()` or `hsl()` are
  matched against the current value as written, not folded.

### T1, commit 4: editing and undo (`f707933a7`, with its fixes in `e2dc10b70`, `d508bba97`, `b51155dd3`)

- **A cell writes the branch the checker picks for its column** (D6's target branch). On a light
  column, a `when` naming an under-dark axis is passed over, as the checker does. "Only this
  mode" adds a `when` naming every axis, and a light column with an under-dark axis offers none.
- **Apply writes the proposal it shows:** its edit, the digests `propose` read, and the server
  run that read them.
- **Expressions are kept.** Overwriting a value that names tokens, dials or a platform role is an
  explicit "Pin a value".
- **The chip** reads "Rebuilds" unless `hello` says `live: true`. Stage 4 adds that field once
  the live table works.
- **Undo and redo** are the session's stack of inverse ids, bound to one run of the server by its
  token. A different run on the port refuses them as unknown, and the stacks start again. A
  refused inverse names who changed the file. Redo sends the original operations against the
  current digests.
- **Open:** if the wire answers an undo with 504 after 120 s and the undo lands later, its entry is
  set aside, with a message, instead of moving to redo. The fix is in the wire (a write is held
  until the server answers), queued with Tamber stage 4.

### T1, commit 5: the changeset, `commit` and history (`7c68b2935`, `710959c37`, `f4c957321`, `3b5376d36`, `5ce08cfc8`)

- **The changeset** lists every write since the tool opened, its own and others', hand edits
  included.
  - Each row gives the token values before and after, measured file by file between the reads
    either side of the write. The tool's own rows show their diff.
  - After a gap in the long poll, a row says the list may be incomplete. Old rows aren't
    reconciled.
- **`commit`** composes its message from a dry run of the server's scope (`commit {dry}` →
  `{files, edits, scope}`), so writes from before the tool opened are named. It sends the scope
  back, and the server refuses the commit (409, `stale: true`) if the session wrote in between.
  Refusals show by file and reason. It never pushes.
- **`write`** numbers each write, on its reply, its `changed` event and the session's edits.
  `hello` carries `design`.
- **History** reads as token edits, ten to a page. Revert holds a past commit's inverse as a
  proposal.

### T1, commit 6: agents' edits and held proposals (`b98b34628`, `f4c957321`, `84b87121a`, `7b83ebd68`)

- Each change names its client, with a colour per client in the order clients appear.
- **The grid outlines only the cells a change moved,** each credited to the latest client that
  changed it, measured between the nearest reads around the change. A hand edit gets the tokens
  that moved in its stretch that no other change claims.
- Highlights fade oldest first, one every 3 s.
- **The inbox** shows each proposal's proposer, diff, tokens and items, each item naming its file.
  - Accept, Accept picked, or Drop. A partial or stale accept comes back recomputed with its new
    diff, never replayed.
  - An accepted proposal is one undo step. Its redo is written as the proposer's, and the tool's
    row says who redid it.

### T1a, commit 7: import review (`782bf06b4`, `84b87121a`, `7b83ebd68`, `1df7c4a12`; `1d83f9794`)

- **Through the file picker:** one stylesheet, or several DTCG files, each a mode. Several
  stylesheets are refused (400).
- **The review** lists each operation by kind:
  - a token, with each mode's value and swatches, references followed;
  - an axis;
  - a dropped mode value.
- **Role overrides are off until ticked.** One the importer can't offer shows its
  `tamber-role-axis` note. Every unmapped value is listed with its reason and source block, and
  the count shows as source = mapped + reported.
- **Each mode's value is editable** before accepting, and what Accept writes is what the card
  shows. Accept is one transaction, and undo one step.
- **The server refuses** a write or propose that would make a role override vary by an app axis
  (`tamber-role-axis`, as the compiler's lowering would), and nothing is written. A role that
  varied before the write is left to the build.
- **Known gap:** a reference whose target has modes the reference lacks gets no rows for them.

T1 and T1a are complete.

### T2, steps 1–3: the canvas, builds and pins, scrubs (`9bfa9f50f`, `952302694`, `694df527b`)

- **The canvas** is one sandboxed `iframe` on `hello.page`'s port (*1.3*, the dev loop's own
  origin, or null with no loop), framed as `localhost` since the follow-up below (the dev loops
  answer on [::1] too), with phone, tablet and desktop sizes and "open in a tab", which opens
  `hello.page` as printed. The canvas's cookies and storage are its own. The dev loops'
  HTML carries `frame-ancestors 'self' http://127.0.0.1:7740`. `'self'` is there because the
  directive checks the whole ancestor chain, so an app framing its own pages needs it.
- **Builds and pins:** the build list comes from the server, with each build's effective facts from
  its status, and pinning, pinned or handing back by revision (`pinRevision`, *1.3*). Each build's
  last refusal is listed (`unsupported`). The tool names itself on its long poll, so its pins hold
  while it runs and are handed back about 35 s after it closes.
- **Scrubs:** "Scrub live" sits beside Apply, an explicit choice and never the default. A
  scrub opens on that button rather than on pressing a control, since no press reaches the tool.
  - One frame is in flight at a time, the latest value.
  - Keep (or letting go of a slider) writes once and is one undo step.
  - Cancel and Escape wait for the server to let go, and leaving the edit cancels first.
  - It is not offered for a rebuild edit or with no loop, and the tool says why.

### T2's gate: met, the web page's structural reload tracked apart (2026-10-16; the run is `~/x2theme/t2-gate-run.txt`)

The run was on the Bluesky clone, on the wasm loop with the tool driven in headless Chrome, plus
the iOS simulator and the Mac.

| Gate line | Result |
|---|---|
| The clone runs in the tool's frame and on the iOS simulator and the Mac | **Met.** |
| A token or dial scrub, chosen explicitly, updates all three with no reload whenever `live` | **Met.** `primary_500`: about 0.6 s per preview, all three design digests moved, no reload, the file untouched until Keep wrote one line. |
| A structural edit hot-reloads all three and keeps their slots | **Met.** iOS and the Mac in the run. The wasm page held its generation while the app had requests pending, which was so before T2. reload1's `777c5e526`..`7a71e5bf7` (LLP 1007 §6) has the dev loop swap after a 1 s settle window, and the clone then shows a Contract edit 2.8–4.7 s after the save, with its slots, navigation stack and URL kept. |
| A cancelled scrub restores every copy | **Met.** |
| `pin` shows Dim on one copy, and the tool shows each copy's effective mode | **Met.** |
| Closing the tool clears its pins on every build, and a reloaded build gets its pins back | **Met.** The pins were handed back 38 s after the tool closed. |
| A report without the current nonce is refused | **Met** (403). |

- **The lead's ruling:** the gate is met. The web page's structural reload is tracked as its own
  fix (reload1, `~/x2theme/reload-brief.md`), which theme2 signs off when it comes.
- **On the JS loop,** the clone's JS-target page busy-loops in headless Chrome (queued at stage 4).
  Being same-site with the tool, it stalled the tool's renderer, so the live lines were driven over
  the wire there, and they reached iOS and the Mac.
- **Follow-ups before T2b:**
  - frame the page at a different site (`localhost`), so a heavy app can't stall the tool (done
    after T2b step 6: "The canvas frames a 127.0.0.1 page as localhost, in its own process");
  - serialize the canvas host's refusal and status posts (done: `5f9e9fd75`).

### T2b (web, wasm): inspect mode, the server's join, outline, the inspector (`ed60a4824`, `b7955343e`, `30a4e1da9`; report order `5f9e9fd75`; framing `169407db1`)

- **Inspect mode on the wasm page** (`host/web/dev.js`, development only):
  - `inspect {client, build?, on | refresh}` (*1.3*) is numbered, owned by a present client, and
    replayed to a build that registers again;
  - a banner with a close control, and Escape;
  - a pointer cover plus a gate on keys, focus, change, scroll and drag, so the app gets no input,
    and guest frames none either;
  - a released press selects what the **browser's own hit test** finds. That is the wasm page's
    real tap path, in place of D5's "the runtime's own hit test, `geometry-glue.js`", which is the
    app's `elementFromPoint` import.
- **The `selected` report** carries the plan, design and pin revision, plus the node, site,
  instance, the node's own tuple, and its rows (value, source, token), trimmed to 64 KiB and said so.
  Reports go one at a time per stream, in order.
- **The server's join** (`scripts/tamber-inspect.mjs`):
  - **Currency:** a report whose plan, design or pin revision isn't the build's latest status is a
    409, and the build is asked again once per status.
  - **The map:** the site is looked up in the named plan's source map, verified by digest. The
    compiler now records each source's SHA-256 in the map, and a changed or missing file makes the
    selection read-only, with the reason.
  - **Editability fails closed:** the token rung is offered only where the row's own whole value
    is one declared token. Every other row says why and links its token.
- **`outline {sites, plan}`** goes to inspecting builds, each drawing one box per mounted copy.
  Outlines are owned by their client.
- **The inspector:** a selection is pinned to its build's facts, the design's axes and its scheme,
  and goes stale on any move. The token rung opens the winning branch's cell with its column's full
  place. Impact shows `uses` for its own token.
- **The canvas frames the page as `localhost:<port>`,** a different site from the tool's, so the
  browser runs it in its own process. The dev loops also answer on `[::1]`.
- **Known limits:**
  - the server keeps one map, the running generation's, so a build on another plan gets no outline
    and a read-only selection (lifted by T2c's maps per plan);
  - a popover's light dismiss happens before inspect can stop it;
  - a gesture already under way isn't cancelled.

### T2b's gate, the wasm half: met (2026-10-16; the run is `~/x2theme/t2b-gate-run.txt`)

| Gate line | Result |
|---|---|
| A click in the frame selects the node the browser hit-tests, and shows its source, rows, sources and token chain | **Met** on the clone, e.g. "Feeds ✨" at `home.contract:203 · HomeScreen`. |
| Every copy of the node is outlined | **Met:** 3 marks on the 3 mounted posts' author names. |
| Inherited, dynamic and boundary rows are read-only, with their reason | **Met** for inherited and dynamic on the clone. Boundary rows are met on the theme fixture by tests, since the clone's signed-out screens have no boundary. |
| Taps don't reach the app while inspect is on | **Met.** |

The JS target's half followed (below, after T2c), with the same gate on the clone's JS loop.

### T2c: native inspect, maps per plan (`abc6490e9`, `9d24e4ebc`, `efd03e95a`; optional *1.9* fields, no bump)

T2c went before T2b's JS half (the lead's order, 2026-10-17).

- **One source map per plan digest** (step 1).
  - The server resolves each build through the map of the plan it says it runs (`planDigest`).
  - The JS loop's resident producer announces each published plan's map on a pipe (fd 3), and the
    loop pulls the bytes by their hash. A native on the JS loop therefore has its own plan's map.
  - A selection on one plan outlines the matching sites on every inspecting build, each from its
    own map. A build whose plan has no map here gets none, and is said so.
  - A build the server skips is sent `{clear}` for the outlines it drew for that client.
- **iOS and the Mac inspect** (steps 2–3). They answer `inspect` and `outline` as the wasm page
  does, compiled into development builds only: Swift's `EXACT_INSPECT` from the trust policy, plus
  the runtime gate. `host/apple/inspect-gate.test.mjs` checks the define, the `#if` cover, and a
  production bake's strings and symbols.
  - Input: iOS's window takes every touch and press. The Mac's event monitor takes every mouse,
    wheel, hover, gesture, key and modifier event in every window of the app, and menus close.
    Each sequence is owned to its end, entering or leaving inspect mode included.
  - The report is the runner's own `node` op, so its rows are the kernel's. Status and report
    name one plan digest: the runner's.
  - The walk goes up from the platform's own hit view to the nearest plan node and stops at a
    system bar or the title bar. A node the bar hosts is selected: the title, a bar or toolbar
    item, a tab, title segments, the search field. The bar's own parts select nothing, and the
    banner says why.
  - Outlines follow where each copy shows, a native bar's stand-in included, in its own window.
    The copy just tapped never scrolls (the click token, *1.7*).
- **Linux answers `inspect` unsupported** (step 4), as do tvOS and a development JS page.
- **Known limits:**
  - an AppKit control's tracking loop already running when inspect mode comes on is not cancelled;
  - a grouped list's cell, a Mac toolbar segment and the collapsed segments menu are not boxed;
  - two sessions in one build running different plans give one status digest;
  - no tests send synthesized UIEvents (UIKit has no public way to make one): iOS interception is
    checked end to end on the simulator. The Mac's tests send constructed NSEvents.

### T2c's gate: met for the simulator and the Mac; the phone pending (2026-10-17; the run is `~/x2theme/t2c-gate-run.txt`)

| Gate line | Result |
|---|---|
| A tap selects the node the platform hit-tests, with source, rows and token chain | **Met** on iOS and the Mac. "Feeds ✨" gives `home.contract:203`, the same as the wasm page. On the JS loop the native's selection is editable: text colour offers "Edit text". |
| Every copy is outlined | **Met,** across builds too: a Mac click outlined the tab on the iOS simulator. |
| Inherited, dynamic, initial and boundary rows are read-only, with their reason | **Met** on both for inherited, dynamic and initial. A boundary row is host-independent: the server's join decides it from the kernel's rows, whichever host reports them. It is met for T2b by the theme fixture's test, and the clone's signed-out screens have no boundary. |
| Taps don't reach the app while inspect is on | **Met** on both, with real touches on iOS. |
| A tap on platform chrome selects nothing; a node a bar hosts is selected | **Met by the host tests on real bars** (UINavigationBar's title and Back, UITabBar's items and space, the Mac title bar and close button). The clone draws its own chrome, and its back glyph is correctly selected as the app's view. |
| A real phone | **Pending:** it needs Charlie's device. |

### T2b, the JS target's half (`b19d1e1a2`, `316744fc3`, `731357ac2`; optional *1.9* field `computed`)

- **The page says its plan** (`planDigest`, the SHA-256 of dist's app.plan). The JS loop keeps
  each plan's map by that digest, so a selection on the JS page is joined to, and outlined from,
  the JS plan's own map. `designServed` is set only for a design the builds take: a value-only
  edit, a kept build, or a reload whose shape held.
- **The inspect table** (host/web-js/src/inspect.rs, `exact-web-js inspect-table`) says of each
  site what the runner's `node` op says of each view made from it. That is the type, the
  regions above it (arms; an `each` row's key is the page's), and every own row: its value in
  each app tuple in `node`'s words, read from per-tuple static trees, plus dynamic or heading,
  the tokens it reads and its text. It also gives the text-style sources, the initial values
  (a control's start included), the boundaries and their implied rows, and each row's CSS
  property.
  - The dev loop makes the table when a page first asks for it (1.8 s on the Bluesky clone),
    keyed by plan. No build makes it, so release bytes are unchanged: production builds of
    theme-fixture, realworld and the clone, and all 88 conformance contracts, were compared
    against main byte for byte.
  - rt.js itself has no hook. A `--dev` copy gains two lines (module.mjs `withInspectHooks`):
    the view id, and every `each` region with its rows and key function.
- **The page's inspect mode** (host/web-js/inspect-dev.js, served by the dev loop) works as the
  wasm page's does.
  - Its gate is the dev client's, on the window before the app's module runs, with a cover, a
    guest-frame shield and Escape while the module loads.
  - Each report is row for row `node`'s. Static and token rows are in the kernel's format; a
    token's row follows the page's current design after a live edit (theme2's ruling).
  - A row an expression binds is own only while the page has it set, and its value is the
    computed CSS, marked `computed: true`.
- **Known limits:**
  - a text style's size (a heading's too) is its ramp at the 16 px root, so a pinned root moves
    it in `node` and not on the page;
  - a text style or heading chosen by an expression is not a style source the table knows;
  - a control start or a style fallback under an ancestor row that only an expression sets is
    not recorded;
  - tokens an expression names are recovered only through custom properties, so not family,
    type or inline tokens, and not the exact text of an expression that mixes them with other
    text;
  - an expression row's ownership is read from what the page wrote: an animation's `none`, a
    tint's `unset`, and SVG geometry written as attributes disagree with `node`;
  - `css_property` misses a few rows the emitter writes elsewhere (the press scale, the control
    size, legacy line clamps, timelines);
  - a row root a host replaces (the markup editor's textarea) loses its key;
  - outline marks follow document order, and `from` and `node` are the page's own view ids.

### T2b's gate, the JS half: met (2026-10-17; the run is `~/x2theme/t2b-js-gate-run.txt`)

| Gate line | Result |
|---|---|
| The same node on the clone answers the same on the JS and wasm loops | **Met** for five nodes: both tabs, the hamburger, the wordmark and a post's avatar. Identity, rows, sources, tokens, joins and editability match. Static and token values match exactly; expression rows match by source and token (theme2's ruling). Seven theme-fixture nodes, two of them boundaries, are compared on every test run. |
| A live token edit, then the node inspected → the new value | **Met** on the theme fixture's JS loop. On this machine the loop served the edit by a rebuild and reload, so the value came from the new plan's table; the value-only batch path was not driven. |

### Stage 6's kinds in the tool (`b436b529b`; `cf00a0d47`..`bbc26d770`, `f16377fd6`; reads `76efb7f0a`, `5443e977b`, `0beb826a9`, `904db0513`)

- **Each kind loads and shows as `resolve` gives it.** Before this, a design with any non-colour token
  failed to load. A cell shows a length in px (with its rem, em or named value labelled by its px),
  a number, a shadow (a small preview on its column's ground, plus its text), a type's longhands
  (the unsaid ones named), a family, a duration or an easing. A length dial's slider and stepper
  write `Npx`.
- **Typed fields by kind** (`parts.ts`, a byte-exact cut of the value as written): a length's amount
  and unit, a number, each shadow's x, y, blur, spread, inset and colour, and a type's longhands.
  Each field edits only its own part, and the edit is still the one `set-token` line. Words it
  doesn't touch stay as written: token colours, an unsaid spread, a family's quoting. A draft goes
  stale when its branch's value moves under it, whichever field typed it.
- **Kinds that rebuild say so before the gesture** (D6): a motion token reads "Rebuilds", Apply
  writes it, and it has no scrub. On the JS target's dev loop, `propose` answers `rebuild` for an
  edit that moves a token the pages write inline (a `font-weight` number, an SVG attribute's length
  or number), as the loop's design reader marks it (theme2's 18:00 ruling). Without the reader, the
  session's class stands.
- **Icons** are listed read only, with their sources, since no operation writes one.
- **Reads on a big design** (found by the gate): the design and axes reads follow the files (a
  `changed` event, a gap, a new run), not every event. A read asks four resolves at a time and
  stops once a newer read overtakes it. A proposal is asked again on each poll answer, except while
  a scrub is shown. Before this, the clone's 73 resolves (about 207 ms each while the serve process
  parsed `app.contract` per request) were reread on every build status and overran their 15 s.
- **Known limits:**
  - the design reader getting ready sends no event, so a proposal shown before then can stay
    Live until the next poll answer (25 s at most);
  - Expose's lengths are an inline Contract theme, which no design server serves;
  - a batch `resolve` would make a read one request (QUEUE).

### Stage 6 kinds' gate: met (2026-10-18; the run is `~/x2theme/kinds-gate-run.txt`)

| Gate line | Result |
|---|---|
| A length scrubs live on the web and the natives; Keep writes one line | **Met** on a clone copy's `space.header`: the JS loop, the wasm loop, the iOS simulator and the Mac. Each native build's design digest followed every frame, with no reload. |
| A shadow list (inset, token colours) scrubs live; Keep writes one line | **Met** on `shadow.header`. A blur and an unsaid spread were edited; the untouched words were kept as written. |
| A motion edit rebuilds | **Met**: "Rebuilds" before the gesture, no scrub, Apply only. |
| Icons read only | **Met**: the clone's 64 icons, listed with their sources. |
| theme-fixture's JS-loop e2e | **Met** (`kindsOnTheJsLoop`, every run of scripts/design-tool-scrub.test.mjs, split from design-tool.test.mjs). |

### T3, components (step 1 `4600797f5`, `fcfca3753`, `80544178c`, `5684f39d3`, `a8cfb59f7`, `18bf8d9d8`; steps 2-3 `a8c3a089a`, `31290cd65`, `ed742c171`, `5dc1d9dc4`; step 4 `212c41b73`, `8e20080a6`, `b372bf692`, `f45cc65ed`, `8209bcef9`; optional *1.11* rung `param`)

- **The param rung** (D6, step 1, the server's join): a row the node itself sets to one component
  param joins as `{rung: "param", param, short, token?, modes}`. `token` is the token the param
  aliases directly (its value as the app has it, the line's or the default, when that names one
  token, a role override by any spelling of its role; none for a param with `when` lines). The
  server reads the app's components by `describe` when its files change, for a new plan, and
  after a failed read. A row's names and text, in `selection`, `selected` and a held state's rows,
  name a param in full, never by the IR token a plan names it by (`param.f<hash>.…`), also inside a
  synthetic token's expression or the custom property a JS page writes for it, and in a plan
  reply's `changed` (a held proposal, a `changed` event).
- **The component panel** (D7, step 2; `components.ts`): every component with params, a native
  view's declared in the design (`app.tamber#Name`) too, by group and priority (3 behind "more"),
  each param with its declared control. `control=token` (the default), and every kind the dials
  have no control for, is a field for a token or a literal; a duration's slider or stepper writes
  ms; an icon param shows its sources. An edit is `set-param`, proposed with its chip, scrubbed
  when live; a duration or easing param rebuilds. A set param says so, with Reset (`unset-param`,
  one undo), and a literal that no longer follows the tokens its default reads is marked pinned.
  The panel is the app design's: it shows while that is open.
- **The param as the default rung** (step 3): a selected row that reads a param opens the param in
  the branch editor on the branch that won for the copy, its edit `set-param`, its typed fields and
  Only this mode as a token's (a param's first `when` line writes its own line first, in the one
  reviewed transaction, which has no scrub). The token it aliases is offered beside it, never the
  default. A param whose value is an expression offers its inputs there (Follows / Pin).
- **Re-scope** (step 4; `rescope.ts`): the last edit of a param whose value there aliases one token
  offers "Apply to the token … instead"; the last edit of a token one param aliases there offers
  "Apply to … instead". One transaction: the edit's inverse, then the same value at the other rung,
  on the same `when`; the other rung's value is the one the moved line's copies show (its most
  specific `when` the edit's holds). It is worked out from the revision the edit was reviewed on,
  and it `requires` every file it read, so another write, a hand edit or a component's source
  changed since refuses it, and a refusal withdraws the offer. It is a write of its own: one undo
  takes back the move, a second the edit. A redo offers none.
- **The workbench** (D7) in T3 is the component panel and the canvas with the component's
  instances outlined (theme2's 10:36 ruling). A preview screen of one component alone needs a host
  route that renders it, which stage 12's component library brings: it moves there.
- **Known limits:**
  - a param's first `when` line (two operations) offers no re-scope;
  - a `when` of the other rung that only partly overlaps the edited line's (it covers some of its
    copies, not all) does not count: the move takes the most specific `when` that covers the
    edited line, else the unconditional value, and two equally specific ones offer no move;
  - a loop through another token (`radius.alias = radius.md`, then the param set to `radius.alias`
    and moved to `radius.md`) is refused by the server rather than foreseen;
  - promote a row to a param and detach a param to a template wait for T5's `set-row`;
  - the clone declares no native component, so a native's params in the panel are shown by the
    stub's `MapCard` only.

### T3's gate: met (2026-10-18; the run is `~/x2theme/t3-gate-run.txt`)

On a fresh copy of the clone (`dt3-gate`, its main `f473895` with a `PostButton.fade` duration param
added).

| Gate line | Result |
|---|---|
| `PostButton`'s params appear as declared | **Met**: Fill (a select of `primary_500`, `contrast_900`, `positive_500`), Height (a slider, 24px – 48px · step 1px) and Fade (a field), in their groups, with Controls and Drawer listed beside it. |
| A param edit re-scoped to the token it aliases is one transaction and one undo | **Met** on the JS loop, the wasm loop and with the iOS simulator's and the Mac's builds attached: `PostButton.fill = contrast_900`, moved, wrote `primary_500 = contrast_900` and dropped the param's line in one write. One undo gave the param edit's bytes back; a second gave the original. |
| A duration param's chip says "rebuilds" | **Met**: `PostButton.fade` to 300ms says Rebuilds, with Scrub disabled. |
| (review2) Reset restores the file byte for byte in one undo | **Met**: `Controls.hit`'s line, after a Keep, went with Reset; one undo put it back byte for byte. |
| (review2) A native-declared component's params | The clone declares none: the stub's `MapCard` (`app.tamber#MapCard`) shows them (contract tests). |
| A live param scrubbed on the web and the natives | **Met**: `Controls.hit` (the post action buttons' padding) 5 → 10px, the frame's padding following and every build's design digest the served one: the JS loop, the wasm loop, the iOS simulator, the Mac. Keep wrote one line. |
| The param rung from the canvas | **Met** on the JS and wasm loops and with the iOS simulator attached: a post's reply button inspected offers "Edit Controls.hit" on its padding rows and "Edit Controls.radius" on its radius rows. |



### T4, states (step 1 and step 2 `6fbc0264fa` with README `9d488627c5` and fixes `e7ccae826d`; the hold replay `2ca697fc0b`, `86bcbefcbc`, `28ce115532`, `7df268ad8c`; the pages' glue wait `49af400183`, `a747b9e2ee`, `fbbba0cf0d` and the JS page's glue load `82c4602f66`, `479a5fa29e`, `236b0186ed`, `902c31665a`; optional *1.12* rung `state`)

- **Held states** (D8, step 1; `holds.ts`): Pressed, Hovered or Focused holds that state on the
  selected copy (`hold`), one a build, a new one replacing the last; Let go ends it. The status
  says Holding until the build's `held` report on that hold frame, then Held (the server forgets a
  report at each hold frame, so a listing's `held` is its hold's); where it is (here, the press's
  node the build names, `held.site`, or another node of the build, which a hold here replaces);
  why the last one went (real input, disabled, no single view there, another hold, its client
  gone, no single site in a new plan) and a state a build cannot hold. While it is held here the
  selection's page is asked again for the held look's rows (an inspect refresh, once a hold; a
  selection's held report counts only as the answer to the hold it was asked under), each row at
  its part's place; a host that reports none, or not their tokens, says provenance incomplete.
- **The state rung** (D6, step 2, the server's join): a style's `:active` part row (the map's
  `state` and `at`, origin `class:<Name>`) joins as `{rung: "state", style, state, row, file,
  text?, token?}`, the default: the style by the name its file declares it with (the compiler may
  rename a second `style Card`), the row as its author wrote it, the style's file relative to the
  app, the value as written (only a whole one literal), and the token its whole value is, offered
  beside it. A held row carries it where the build names the held node; a style outside the app's
  own Contract sources (a package's, vendored or linked) is read-only; an inline `:active` row on a
  node points at `set-row` (T5). The editor starts from the value as written, its draft bound to
  the copy and the files' digests it was opened on (a write since, or a stale or read-only copy,
  refuses Apply until the copy is selected and the row opened again); `set-state` rebuilds.
- **The hold replay** (stage 8 E, fixed here by the lead's 18:30): after a new plan, a held site
  corresponds by its place and call-site chain: the same calls take the same chain; as many copies
  whose calls moved go by rank where each file's calls moved as one edit moves them; else the
  unmoved kept and the rest so among themselves; copies added or removed keep the unmoved chain;
  else, or on a throw, cleared with the diagnostic.
- **The workbench's state parts** (moved here from stage 8's gate): a selected component's
  `:active` part rows, each with its state rung, are the workbench's state parts; T3's component
  panel lists params, not state parts.
- **Known limits:**
  - only `:active` parts carry their place in the map: a `:hover`, `:focus-visible`, `:disabled` or
    `:checked` part's rows are offered no state rung, so a held hover or focus look is read-only (a
    gap against D8: QUEUE, the compiler to record `at` for every state part);
  - the replay goes by places alone (the map has no other identity of a call or an element): a
    reorder that keeps its spacing, a call or element moved onto another's old place, or a copy
    added or removed while one keeps its line can carry the hold to the other copy (the host lets
    it go, `gone`, where the copies' instance paths differ); a slot filled at two outlets is one
    copy, its outlets told apart by their order;
  - a hold sent while a page's input glue loads (a page reloading after a rebuild) waits for the
    glue to say it has (`exact-hold`) up to 60 s, then says it is not loaded. The gate found two
    causes, both fixed above: the pages' fixed 2 s wait, and the JS page loading its glue only after
    an animation frame, which a canvas scrolled out of view in the tool never gets (it now loads a
    second on as well). The wasm shell's own activation still waits on frames (QUEUE), and the
    agents' own `holdState` waits are still 2 s;
  - a page that answers a refresh with no selection leaves "asked" said until the next selection
    (data sources have no timers); a hold ended because its client went says nothing (the server
    sends no reason);
  - before the server's first file read the join has no own-sources list and uses the path rule
    (the write refuses a vendored file regardless).

### T4's gate (2026-10-18; the run is `~/x2theme/t4-gate-run.txt`)

On a copy of the clone (`dt4-gate`, its main `95d7d41` with two Post buttons in the home header,
the second disabled).

| Gate line | Result |
|---|---|
| Hold pressed on the Post button shows its `:active` look with no touch | **Met** on the JS loop and the wasm loop (#0059d6, "Held pressed."), the iOS simulator (#0059d6 against #006aff at rest) and the Mac (held `active` on the wire; its screen capture was covered on the shared desktop, the look met in stage 8's gate). |
| Real input on it lets the hold go | **Met** on the JS and wasm loops ("real input on the node let it go"). |
| The disabled Post button refuses a hold | **Met** on the JS and wasm loops, the iOS simulator and the Mac (`disabled`). |
| Hovered: held where a pointer is, unsupported where none | **Met**: held on the Mac; the iOS simulator answers `unsupported` ("this host holds no hover"), to the owning client only. |
| Inspect on the held node shows the state part's rows | **Met** on the JS loop with stage 9 E: the held row ":active background-color" at its part (`screens.contract:582`) with its token, no provenance incomplete, "Edit PostButton :active background-color" on it. |
| The workbench's state parts, editable through `set-state` | **Met**: the selection's `:active` row offers the state rung (the default) and "Edit the token primary_600 instead"; the field starts at `primary_600`; Rebuilds; Apply wrote one line (`screens.contract:582`, `primary_600` → `positive_500`). |
| After the rebuild the hold is held again | **Met** on the JS loop (00:49Z, at main with the glue fixes): the server re-sent it at the corresponding copy ("Holding pressed.", where before the replay fix every PostButton cleared), the reloaded page held it 2 s later ("Held pressed.") with the canvas out of view, and in view its ground was the new `:active` value, `positive_500`, with no touch. **Met** on the wasm loop too (01:17Z, in a quiet window): Held again after its restart, `positive_500` in view. |

### T5, template and style rungs (step 1 `1c6434a090`, `ee2d85d546`; step 2 `f0f1917a25`; step 3 `95332bb9de`; the map's `layout` `7553483dd7`; fixes `c74d313a22`, `7eec1a9a90`, `fb943c06df`, `47d74f03b7`, `e1d7526acc`, `7311d0f039`; optional *1.12* fields)

- **The template and style rungs** (D6, step 1, the server's join): a row its author wrote, where
  the map places it (D1's `at` and `attr`), answers the template rung `{file, at, sha256, row,
  state?, instances, text, component?, token?}` on the node or the style rung `{style, row, file,
  text, token?}` in its class's `style` by the name its file declares, the default unless the row
  reads a param (whose rung then carries its `template` address, for a detach); the token its
  whole value is offered beside. `row` is the attribute as written (a shorthand's whole for a
  longhand). Only a plain value is offered, read at the map's UTF-8 byte column as Contract writes
  one (`plainAt`: a string, its escapes decoded as the lexer does, a bare `#…` colour, a number,
  in any parentheses, then a comment, another attribute or a positional argument); an expression
  or a name in scope, a file outside the app's own sources, a conditional class and a layout row
  (the map's `layout`, D14, set-row's own test: `paint_row`, now the library's) are refused with a
  reason, a whole token still offered. A state row written under the node takes the template rung
  with `state`, held ones too where the build names the held node; `instances` counts call-site
  chains (a `button "…"`'s label is no copy).
- **The tool's editor** (step 2): "Edit this template · N instances" / "Edit style X" as the
  bordered default, a field from the value as written, proposed as `set-row` (Rebuilds, no scrub),
  its draft bound to the copy (with the build's facts) and the files' digests, and refused at
  propose when the files moved since the copy was selected; the server's refusals (a moved file, a
  compile failure with its diagnostics, D14) shown, nothing written.
- **Moves** (step 3, D6's enumerated set; `moves.ts`): a template row to a new param (`add-param`
  in the template's file, its kind the row's, its default the value as written) or, an
  unconditional value, a new token (`add-token`), the row reading it; a param row detached to its
  template (the param's value written there; `remove-param` for it unless the server says another
  row still reads it, and for each param it alone left read, as the compile names them; refused
  where the design sets it or its default is an expression over another param). Each one
  transaction, one undo; checked against the params read the ops were made from.
- **Known limits:**
  - a promotion's kind is read from the row's name (a colour row's, opacity's number, shadows,
    `font-family`, durations, easings, else a length); a row whose value the server types
    otherwise is refused by the server, not guessed again;
  - a detach copies a token default as the token's name (the row then reads the token, D6's
    "expressions are kept"), and refuses a default that is an expression over another param;
  - before the server's first file read the join has no own-sources list and uses the path rule
    (the server's write refuses a vendored file regardless);
  - a positional argument written against the value with no space (`opacity=0.5("hi")`) reads as
    an expression;
  - a param colour on the JS page reads as a dynamic value (a CSS variable the page resolves), so
    its row offers neither the param rung nor a detach there.

### T5's gate (2026-10-19; the run is `~/x2theme/t5-gate-run.txt`)

On a copy of the clone (`dt5-gate`, its main `95d7d41` with the header's right slot given a literal
ground and the Post button a one-reader `corner` param).

| Gate line | Result |
|---|---|
| A template colour edit writes one line and hot-reloads | **Met** on the JS loop: `#f1f3f5` → `#ffb3b3` at `home.contract:331` only, Rebuilds, the page rebuilt to that colour; one undo restored it. |
| A stale source digest is refused | **Met**: the file edited after the copy was selected; the server refused (`tamber-contract-edit`: the file changed since line 331 was selected), nothing written. |
| A candidate that fails to compile writes nothing | **Met**: `not-a-colour` refused with its diagnostic (`tamber-unknown-token`), the files unchanged. |
| Promote to param and to token are each one transaction | **Met**: `drawerGround` (the param's line and the row reading it, one write, one undo); `gate.drawer` (`app.tamber`'s new token and the row naming it, two files, one write, one undo). |
| A `class:` row edits the style in its own file | **Met**: the Post button's `color` row wrote `style PostButton`'s line in `screens.contract`; undone. |
| (review2) Detach is one transaction, its closure listed | **Met**: `PostButton.corner` detached: its line removed and the row written `999px`, one write, one undo. |
| (review2) A layout row is refused with D14's reason | **Met**: width, height, margins, paddings, alignment and font-weight rows each say D14's reason. |

### T6, collaboration (step 1 `cd6300a22a`; step 2 `68515147ac`, `91b875075f`, `3ea24dbfe8`, `5570a77ed5`, `513f050bd1`; step 3 `cfdf491e12`, `4087df91ba`, `80815754b1`, `814c00e289`; the entry word `8f90a463db`, `a21184f903`, `927178760a`, `2ca911ea7f`; *1.13*, LLP 1121 stage 10)

- **The local tool pairs and revokes** (step 1, D12): a Clients panel, shown only to the local token (hello's
  `local`). A scope picker says what each scope may do. "Pair a client" mints a code (`pair-code`) and shows it,
  good once until its expiry, with each remote door's origin and certificate fingerprint to check on the other
  machine (the tunnel said as trusted), or how to open one. The paired clients (`clients`: scope, peer, last seen)
  are read again as one pairs or goes and when the server starts again, and retried after a refusal. Each is
  revoked here (`revoke`), bound to the server run that listed it.
- **Remote mode** (step 2): with no server on this machine, the tool pairs with another machine's server by its
  address and the code its tool shows.
  - It pairs only through an https door or a tunnel to this machine's loopback, and refuses any other address
    before anything is sent. The code goes once, in the body of `POST /__tamber/pair`.
  - From then on every request goes to that door at the paired scope; the client is the name the server gave.
  - `wire.ts` is the tool's one client (moved out of app.ts). The rest of the tool holds a handle for a token,
    never the token, so no state, log or reply carries one. A token goes only to the server it came from: a write
    bound to another (this machine's, a review made before pairing) is refused unsent, and an answer from before
    a pairing, or from an older one, changes nothing.
- **Scope** (D12; ruling 5): nothing above the paired scope is sent. `NEEDS` mirrors LLP 1121's table, and a test
  holds it to `METHOD_SCOPE`. `inspect` is `read` only as a refresh, and a drop of another client's held proposal
  is `write`.
  - Below `write`, what writes is greyed, and one line says why.
  - At `propose`, Apply is "Propose for review": the edit is held (`propose` with `hold`) for someone with `write`.
  - Below `propose`, every drafting control is greyed.
  - A reader's or proposer's selection is read from the server (a read-only `inspect` refresh), shown as the build
    reported it, and offers no edit.
- **Revoked** (D12): a 401 drops the token and sends nothing more. The tool says Revoked, a Keep, Cancel or write
  in flight is shown as the revoke, and the canvas goes with the rest of the view.
- **The canvas through the gateway** (step 3, LLP 1121 stage 10 C): a paired tool asks for a grant (`canvas`,
  `read`) once per server run and frames the gateway's entry with the grant in the `#g=` fragment.
  - The frame is built from parts, never the server's string: only through B's TLS door, on that door's host,
    which must not be loopback (any other port on this machine may be another service), another port than the
    door (another origin than the tool, which the door serves), at `/canvas/<this client>/`, with a 43-character
    base64url grant.
  - Paired, the tool is never local, whatever the server's hello says, so it never frames a page of this machine.
  - It mints one grant a key (client, server run, tick); a second ask shares it, and a mint a newer ask overtook
    is answered without its grant and kept nowhere. The frame mounts only once a fresh grant has settled, and
    Reload Canvas or Try Again asks for another.
  - C2's entry page posts `{type: 'exact-canvas-entered'}` to the tool's origin once it has entered. The tool takes
    it only from the minted canvas's origin and from a direct frame of its own window (`event.currentTarget`;
    never a window the canvas opened, a frame inside it, or a window gone). It then drops the grant from the
    frame's URL, its state and its memory: one load of the gateway's root under the session.
- **Tests:** the stub answers A2 (pair-code with a fixed series a test can type, pair, clients, revoke, hello's
  scope, `local` and client, a 401 after revoke) and checks every request with A1's table.
  - The contract tests cover the panel, refusals, remote mode end to end, a hello that lies about `local`, and the
    canvas refused through a tunnel.
  - `scripts/design-tool-scope.test.mjs` covers the scope table, `doorOf`, `canvasFrame`'s refusals, `wire.ts`'s
    hold-backs with fetch stubbed, and the entry word's sources and mints.
  - `scripts/design-tool-pair.test.mjs` pairs through a real `tamber serve --trust-tunnel`. It allows only `read`
    and `propose` through, revokes while the tool runs, and finds the code and token in no URL, drive reply,
    server output or browser profile file.
- **Known limits:**
  - The tool can't learn its page's address (no `location` in the data module). The person types the door's
    address, and pairing works from the tool that door serves: B refuses another origin.
  - A tunnel has no canvas (stage 10), and neither does a TLS door on loopback.
  - Until the entry page's word comes (at most 40 one-second polls a canvas), the frame's `src`, and so the tool's
    state, holds the grant. It is single-use and dead after 30 s.
  - The data module can't see the iframe element (`document` is hidden from it), so the word is bound by window
    topology, not to the element. A second direct canvas-origin frame would pass; the remote view has one.
  - `tamber tool --via`'s 2 s probe of a dev loop can time out under load (QUEUE).

### T6's gate: met (2026-10-19; the run is `~/x2theme/stage10-gate-run.txt`, LLP 1121 stage 10's)

The tool's own lines ran in LLP 1121 stage 10's gate. They were driven in its second Chrome profile, which trusts
only the run's certificate, against B's TLS door bound to this Mac's LAN address for the run (theme2's 13:40Z
ruling). exact2 main at `29ec5ae613` (`~/x2theme/t6-gate-dlines.mjs`).

| Gate line | Result |
|---|---|
| The tool's own pair panel pairs a second browser over TLS, by the door's address and the code | **Met** (1, D): paired as `remote:Gate#1` at `propose`; the code field empty after, no URL held it. |
| A `propose` client can't write or send commands; what writes is greyed and says why | **Met** (2, D): Inspect, Commit, Undo and Redo disabled, the scope line said; the wire's own refusals are line (2). |
| The canvas comes through the gateway on its own origin, and the grant leaves the frame | **Met** (5, D): the tool set the frame's `src` to `/canvas/remote%3AGate%231/#g=…`, then to the gateway's root once entered; the tool's page held no grant. |
| Revocation cuts the tool off, socket and canvas | **Met** (3, D): "Revoked" shown, the canvas frame gone, no wire request after the revoke. |
| No regressions | **Met** (7): design-tool 18; the contract tests 56. |

**First final run:** 21 of 22. (5, D) read the frame's first `src` after the entry word had already dropped the
grant: a timing fault in the gate's check, not the tool. The check now records each `src` as the tool sets it,
and the re-run met every line.

### T7, several live copies (T7a `f3dba4c097`..`04232cb3bd`; T7a-2 `6f5f5dd3ba`..`0739d2c4e7`; T7b `d287ba04c6`..`244db8615c`; *1.14*)

- **Which frame is which build** (theme2's 18:11Z Q1, 18:14Z): no URL marker. Each copy is an iframe the tool names
  `exact-copy-<n>`. The dev page posts `{type: "exact-build", id}` to its parent each time it registers. The tool
  takes the word only from its own child frame of that name (`window.frames[name]`, the message's source), from
  that tile's origin, and only while the frame shows the page the tool gave it. A copy keeps its build id under
  its frame's name, which the page only reads.
- **Copies at loopback names of their own** (the gate's F1, theme2's 21:04Z ruling):
  - Each local copy is at `http://exact-copy-<n>.localhost:<port>/`, because a browser carries six HTTP/1.1
    connections a host and each copy holds a stream. The dev loop admits those names (`n` 1 to 99), never as local.
  - Each copy is its own origin, with its own storage (Q3 as ruled then: "each copy starts with its own data").
  - Through a paired server's gateway the copies share its one origin, and at most 4 show, as the grid says.
- **`navigate`** (Q2): a fifth build-channel command and a write method.
  - It routes one build in place to a location (a path, query and fragment on its own origin, spelled as URLs and
    the router write it, at most 2 KiB) through the app's navigation root, else its router's `go`, with no reload
    and the app's state kept.
  - A build without the capability is 409 `unsupported`, and a page that can't route answers `unsupported` later.
    Either way the tool reloads that copy's frame at the location instead and says so.
  - It holds nothing on the build; it is journaled with its client. A status says its `location`.
- **The grid** (S1): Light, then dark at each value of an axis (the first by default), at the build's own text size
  and again larger (22 px by default, and a third size where 8 copies are asked for). The default is 6 copies (4, 6
  or 8). Each copy is pinned on its own build only, once that build has said which it is. The grid gives back
  only what it set and still holds: a pin by the revision the server gave it, a hold by its state and instance.
  One reconciling pass runs at a time, bound to its server run.
- **The states side by side** (S2): the selection as it was when chosen, at rest, pressed, hovered and focused, every
  copy in its mode. Each copy is held on the selection's site in the selection's plan, once it shows the
  selection's screen. The views are exclusive, so the states' copies are never the grid's pins.
- **Mirror navigation**: a copy the person moves leads, and the others are sent `navigate`. A copy sent somewhere
  has gone once it reports any new place (a router's rewrite, a Back), or five passes on, and isn't sent there
  again. A copy's first report never leads. Mirror off moves nothing.
- **A pinned copy holds its pin over the app's own** (the gate's F2, theme2's 21:04Z ruling; D3 is amended for
  pinned grid copies alone):
  - The app's `setScheme`, `setMode` on a pinned axis and `setRootFontSize` are kept as asked and not applied: in
    the runner on the wasm page (`hold_facts`, ABI `exact_hold_facts`; no host mirrors a held command), and at
    the command sites on the JS page.
  - A smaller pin releases what it drops, and hand-back releases all, each to the app's latest choice, in one
    commit that is rolled back whole if refused.
  - What the app asked against the pin is said in the copy's status (`app`) and on its tile.
  - The single canvas and every native build keep D3.
- **Tests:**
  - `scripts/dev-copies.test.mjs`, both web loops in Chrome:
    - ids per frame, surviving a reload of all frames and of one;
    - `navigate` in place with state kept, a redirect, and a later refusal;
    - leader moves by a link, Back and a fragment;
    - six copies at their own names all registering and reporting;
    - a pinned copy holding its scheme, mode and root size over the app's own (what paints included), a smaller
      pin, and hand-back.
  - `contract/cli/tests/it/tamber_held.rs` covers the runner's held facts.
  - The contract tests cover the grid (copies registering out of order, the leader, both fallbacks, Mirror off, a
    smaller grid and the hand-back, the app's own choice on a tile) and the states, against the stub's copies
    served from a dev-loop origin of its own.
- **Known limits:**
  - A dev reload (JS, or the wasm loop's resident reload) forgets a choice the app made by a press before it. Its
    launch choices are asked again.
  - While a hand-back the runner refused waits for its retry, the page shows the release.
  - The server's `pin` has no revision condition, so another client's pin landing during the grid's own release
    can be cleared.
  - A listing has no hold revision: another client's identical hold reads as the grid's.
  - Native copies (several simulators) and `navigate` on the native hosts are QUEUE (Q4).

### T7's gate: met (2026-10-20; the run is `~/x2theme/t7-gate-run.txt`)

The Bluesky clone's JS dev loop, the tool's grid in headed Chrome, exact2 main at `244db8615c` (the run took
T7b's tool at `a51522a217` with the app-note line). The scrub was through the wire as the tool sends it: one
request in flight, the latest value next, 60 Hz gestures. It changed `contrast_0`'s light and dark sides and its
`dark-style=black` line each step.

| Gate line | Result |
|---|---|
| Six live copies of the clone update together on a scrub | **Met**: at 4, 6 and 8 copies every copy showed about 41 values a second (the scrub's own request rate, 263 requests in 6 s), its last value the scrub's last. |
| Each shows its own mode's value | **Met**: Light, Dim and Black at 16, 22 and 28 px each showed only its mode's values, none of another's. Black held over the clone's own `setMode("dark-style", "dim")` at launch (F2). |
| Q5: the default is the largest count that holds a sustained 60 fps scrub | **6**: frame p50 16.7 ms at 4, 6 and 8; p95 18.8 ms at 4 and 6, 23.6–25.5 ms at 8 (14–19 frames over 25 ms). Chrome 99%/141%/217% CPU and 3.6/4.9/6.0 GB; the dev loop 54–93% CPU, about 1 GB. |

**Findings on the way, now fixed:** six copies of one origin starved their own reports (F1), and the clone's own
`setMode` beat the Black copy's pin (F2). Headless Chrome on this Mac composites six large frames at about 15 fps
idle, so the gate runs headed.
