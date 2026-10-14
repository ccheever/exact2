# LLP 1122: The design tool — shaping a running app through Tamber

**Type:** RFC
**Status:** Draft r4, 2026-10-13. Three blind review rounds by GPT-6 Astra (max) and Grok 4.7 (xhigh), every verdict RIGHT WITH CHANGES, rounds 2 and 3 with no blocker (`llp/reviews/rfc-2026-10-13-1122-design-tool-r{1,2,3}.{astra,grok}.md`). r2–r4 take every finding; round 3's are written in without another round, the last allowed (§0). Nothing built. The tool's name is open (§9 Q1); this document says "the design tool". It needs DEFERRED admissions before any stage starts (§9 Q2).
**Systems:**
- The design tool: an Exact app for the web's JS target, a `tamber/1` client (LLP 1121 D13, D14).
- The Tamber server (LLP 1121 D13): the build relay and its tile sessions (D3), the provenance join (D5), scrubs and the live-or-rebuild answer (D6), `set-row` (D6), held proposals (D10), `commit` (D9).
- The agent API (LLP 1012): two new forms, no new operation: `layout` at a point (D5), and a `tap` that holds a state (D8).
- The compiler's source maps (`contract build --map`): a source digest per file, and the winning attribute's own span (D6).
- The agent API's node explanation: a held state's overlay rows reported beside the base rows (D8).
- The web JS target's runtime: a row's token reported beside its computed value (D5).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** none yet. No stage is accepted until it has an implementer and a date (`rules/RULES.md` §Scope); each follows the Tamber stage it needs (§4).
**Related:**
- LLP 1121 (Tamber): D7 the token index, D8 state parts, D13 the protocol, D14 clients and importers, D15 component params, §5 staging, §9 Q3 and Q4.
- LLP 1012 (the agent API), LLP 1035.002 (`layout` explaining one node: its site and each row's source), LLP 1095 D7.
- `rules/DEFERRED.md`:
  - §Tooling ("no Design Mode … no devtools UI");
  - §Agent API (ten operations; "a new input is a form of `tap` … a new question is answered from `tree`, `state`, or `layout`");
  - the 2026-09-10 source-map admission ("read by the driver and never by a host … any general provenance explorer stay[s] refused").
- exact1, as research:
  - LLP 0158 (Design Mode);
  - LLP 0200 and 0208 (shelved for bundling too much);
  - LLP 0225;
  - LLP 0304 W5 and W6 (Design Mode was DOM-only and native never landed; editing security was deferred).
- The survey: `~/.tuft/projects/exact2-theming/facet1-research.md` §10d.

## Summary

The design tool is how a designer, or an agent, shapes an exact2 app's look. It never
draws a picture of the app: it shows **the app itself, running**, tiled side by side in
every mode the design has. Select something on a tile and the tool shows where its look
comes from: the Contract node, its rows, the tokens and component params behind them, and
the dials behind those. Change one and every tile shows it, live where the change is a
token value, by rebuild where it is source. Each change is a reviewable diff in the app's
own files.

It is a `tamber/1` client (LLP 1121 D13, D14) with no private channel; an agent can do
everything it does. It asks four things of the rest of exact2:
- **a build relay** in the Tamber server, which owns the tiles' agent sessions;
- **a point hit-test**, as a form of `layout`;
- **a held state**, as a form of `tap`;
- **`set-row`**, a checked edit of one Contract row.

It is not a drawing tool, and not a layout editor in v1 (D14).

## 0. Review rounds

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
  person's hands on that data while they look at the real app.
- **exact1 built this as Design Mode** (LLP 0158) and learned four things that shape this
  design:
  1. **Scope is the design problem.** "This button, all buttons, or the radius token?"
     0158's answer, a scope ladder with a systemic default that never defaults to the
     theme, is kept (D6).
  2. **Provenance must be exact or the edit must not commit.** 0158 failed closed below
     `exact` confidence; so does this (D5).
  3. **DOM-only was the ceiling.** Native editing was specified and never landed (0304
     W5). Here every host is reached through the agent API. But the web lands first,
     because exact1's lesson is also that bundling every host into one stage stalls (§4).
  4. **Editing security was deferred** (0304 W6). Here it is designed in (D13).
- **The repository's apps need it.** T3 Code hand-built "where is this colour used"
  (`issues/closed/20261010-paint-role-node-list-read.md`). The Bluesky clone's modes, font
  sizes and post button are exactly this tool's edits (LLP 1121 §1).

## 2. Decisions

### D1 — What it is for

The design tool is the **primary way to shape an exact2 app's look**, for designers and for
agents, as co-editors through one protocol (D10). It edits the design: tokens, dials, axes,
component params, state looks and, at the narrowest rung, a node's paint rows (D6).
Structure, layout, data and behaviour stay with Contract source (D14).

### D2 — Architecture

- **A web JS-target Exact app** (LLP 1121 D13): its TypeScript data module is its only
  client code, speaking `tamber/1` over one socket with the project's local token.
- **Everything else is the server's.** The Tamber server, running in the dev loop (LLP 1121
  D13), owns:
  - the design files;
  - the tiles' agent sessions (D3);
  - the provenance join (D5);
  - the captures;
  - `set-row` (D6);
  - held proposals (D10);
  - `commit` (D9).

  The tool asks; the server acts. A JS-target app could not spawn browsers or read a map
  itself, and should not.
- **No design state of its own.** The files are the design and git the history. Closing the
  tool loses the selection and the canvas layout, nothing else.
- **Two URLs.** `bun exact.mjs tamber tool` opens the tool at its own local URL, served by
  the Tamber server. Tiles open the app's development build at the URL of the dev loop that
  built it: the wasm host's page until T2b, the JS target's after.

### D3 — The canvas: tiles the server launches and owns

**A tile is an agent session the Tamber server launches**, keeps open and addresses by an
id. The agent driver opens a session by launching its own browser, and on a phone or
simulator it restarts the app (`scripts/agent.mjs`). So a tile can't be "a build that
happens to be connected": the server owns each tile's carrier, its plan generation, and its
reload and disconnect. The server holds a table of tiles keyed by id: host, carrier, pinned
modes, current plan digest and map, last capture.

- **Order of hosts.** First, headless web tiles (cheap, many). Then the iOS simulator and the
  Mac app, launched with the platform's own chrome. Then a paired phone. A native tile comes
  only after the web canvas has passed its gate (§4).
- **What a tile shows: a capture.** The server settles the tile (`clock settle`, bounded;
  an unsettled tile is shown as unsettled, not hidden), then takes `screenshot` and
  `layout`.
    - **Identity: five fields.** A capture is identified by its carrier generation, its
    incarnation, its epoch, its clock and its applied design digest.
    - A JS page restarts at incarnation 1 with a fresh epoch, so the **carrier generation**
      is the server's own counter. It is bumped on every relaunch or reload, stamped into
      each request and echoed in each reply, and a reply from an earlier carrier is
      dropped.
    - The pair is accepted only if both replies carry the same five fields.
  - **The barrier.** The driver today stamps a screenshot reply's tags before it waits for
    images and takes the PNG (`scripts/agent.mjs`). So the capture form samples the epoch,
    clock and design digest **after** the image wait, in the same step as the pixels, and
    then takes `layout`. While a capture is in flight, the server holds back live-table
    batches for that tile. It retakes the pair if the fields disagree.
  - **Retaken** after each commit the tile reports (a live-table batch included) and after
    every driving request, so a scroll or a press never leaves a stale picture.
  - **Delivery.** The image travels to the tool as bytes on its authenticated socket (at
    `--scale 1`), never as a `data:` URL (capped at 1 MiB) and never on an unauthenticated
    URL.
  - **Native captures.** A native tile's capture is its window, platform chrome included.
    The server records the chrome's inset and maps a point through it; a hit on chrome is
    the platform's, and the tool offers no edit there.
- **A continuous mirror is not in r1** (§9 Q5): the agent API has no frame stream, and
  DEFERRED §Agent API holds the count at ten.
- **Modes side by side.** A tile is pinned to one combination of:
    - scheme and contrast (`prefer prefers-color-scheme dark`, LLP 1095 D7);
  - an app mode (`prefer mode dark-style=dim`, LLP 1121 D13);
  - the platform's text size (`prefer root-font-size 21`, in px, a page fact);
  -   its window size (a web `resize` tap; a native tile keeps its device's size).

  **Requested is not effective.** An app's own `setScheme` beats the device preference
  (`scripts/agent.mjs`), so a tile asked for Light may stay dark. The server reads the
  effective scheme, contrast and app mode back from the tile, through `layout`'s
  environment and the root's rows, and labels each capture with the **effective** traits.
  A tile whose effective traits differ from those requested is marked, not relabelled. For
  an app that overrides the scheme, use D4's preview screens, or drive the app's own
  appearance setting. After a reload, the server restores the pins and checks them again.

  The app's own text-size setting (LLP 1121 D6's percentage) is the app's, reached by
  driving the app, not by `prefer`. For a design with Bluesky's axes, the default canvas is
  six web tiles: Light, Dim and Black, each at the default and the largest text size.
- **One screen everywhere.** "Go to" drives every tile to a route by `type` on the
  navigation root's location, never by replaying a click point onto tiles of other sizes.
  Each tile keeps its own `--storage`.

**The build relay** is how the tool reaches tiles. It is an allowlist of request forms,
never a pass-through of LLP 1012:

| Group | Forms | Who |
|---|---|---|
| **observe** | the tile's latest capture, `layout` of a node, `layout` at a point (D5), `tree` | `read`, `propose`, `write` |
| **drive** | `tap` (including D8's held state) and `type`, with no `ticket`, `path` or `files` field | `write` only, each journaled with its client |

Settling, recapturing and a tile's `prefer` pins are the server's own acts, done for its
tiles. No client sends them: `clock settle` fires timers and commits their actions, and
`prefer` changes device facts (`online`, `visibility-state` among them), so neither is an
observation.

Not relayed, ever:
- a client-chosen `path`;
- `logs`, `perf` and `state`;
- a `ticket` (which answers a file picker or an auth prompt), or `files`;
- any form that reads or writes a file.

The server picks every file it writes and hands back handles.

### D4 — The app's own preview screens

Some states and modes the app's normal screens never reach. So the tool also uses
**preview screens**: ordinary routes, written by the developer (the tool offers a starter),
that lay out a component or screen under several `color-scheme` and `-exact-mode` pins
(LLP 1121 D5). A Light preview is `color-scheme="light"`; Dim and Black are
`color-scheme="dark"` plus the pin. The Bluesky clone's Appearance screen is one. A tile on a
preview screen is the tool's component workbench (D7).

### D5 — Inspect: a point, a node, its provenance

**Selecting.** A click on a tile becomes **`layout` at a point**, a new form of `layout`
(DEFERRED: "a new question is answered from … `layout`"): `{"op":"layout","at":[x,y]}`. The
host answers with the view id its own hit test finds front-most at that point:
- the web, `exact_geometry.point` (`host/web/glue.js`): `elementsFromPoint`, then the
  nearest `[data-view]`, answered as a view id (the geometry ABI's `id + 1` is converted);
- native hosts, the same hit test `tap` uses.

The point is in viewport space, not `tap`'s target-local `at`. The server converts it from
the capture's coordinates, the scale and any chrome inset. The point-pick carries the
capture's five fields (D3), and the tile answers only if all five still match. Otherwise
the selection is refused and the tile is recaptured. Boxes from `layout` are used for
outlines only.

On the JS target (T2b), the hit-test is `elementsFromPoint` with the JS runtime's own view
id, since its elements carry no `data-view`. The wasm host's `exact_geometry.point` answers
`id + 1`, with `0` for a miss.

**The selection is the source.** The tool selects a source span, not a rendered copy. It
outlines every mounted node whose site maps to that span: each copy in an `each`, and each
expansion of a component body, across every tile. The chip says how many ("12 rows",
"PostButton · 3 instances"). This is exact1's "templates, not copies", extended to
components.

**Provenance, joined by the server.** The source-map admission lets the driver read maps
and never a host. Here the Tamber server joins them (§9 Q2 asks to widen that line) and
hands the tool provenance records:
1. **Node → source.** `layout <id>` gives the node's site and each row with its source.
   The sources are:
   - `authored`, with an origin: `own`, `class:<Name>`, or `tag`;
   - `dynamic`;
   - `inherited` from an ancestor;
   - `initial`;
   - a boundary-implied colour row (LLP 1121 D7);
   - `applied: unsupported`.

   The server looks the site up in **that tile's own map**, verified by plan digest. Each
   build advertises its plan and map pair on its own envelope. Today the JS loop's
   `/exact.json` answers for a different producer's plan (`host/web-js/dev.mjs`), so this is
   owed before web inspect (§3).
2. **Row → token.** On a kernel host, the token kept beside the row (LLP 1121 D7), and
   `explain` with the `when` that won. On the JS target, **the runtime reports a row's
   specified token beside its computed value**, in the runner's row shape. Its `layout`
   reads `getComputedStyle` today, so a token arrives as `rgb(…)` (`host/web-js/agent.js`).
   Until the JS runtime reports tokens, web tiles run the wasm host (`--wasm`), which has
   the kernel.
3. **Token → dials and params**, by `explain` (LLP 1121 D13).
4. **Impact.** `uses` lists the mounted nodes that hold a token. The tool labels these
   "mounted uses", not "everything affected": a descendant that inherits the colour is
   affected and isn't listed. The full impact of a change is computed from the tokens whose
   folded values it changes (the dependency closure) and shown as those tokens.

**Fail closed.** An edit is offered only when the tile's map matches its plan digest and
the map's source digest matches the file on disk (D6), and then only these rungs:
- **template or style:** the row is `authored`, from a single origin (`own`, or one
  `class:<Name>`; a conditional `class:A/B` is refused);
- **token or dial:** this node's own specified value is the token. A token reached through
  inheritance, a boundary-implied row or a dynamic expression is shown as a link to the
  token, never as this node's edit;
- **state part:** while a hold is active, the held state's winning overlay row (D8).

**Boundary rows.** A mode boundary writes inherited colour tokens onto the boundary node as
its own rows (LLP 1121 D7), so `layout` reports them today as `authored`, with a token
beside them and no attribute in the map. Preview screens are boundaries (D4), so this
matters. The kernel will flag an implied boundary row, and `layout` will report it as its
own source (`boundary`), as §3 proposes. Until then, any `authored` row whose attribute the
map does not name is read-only, and its token is shown as a link.

Everything else is shown read-only, with its reason ("inherited from
`app.contract:12`", "computed", "the tag's default", "set at a mode boundary"). This is
exact1's rule: never write back through a value it cannot trace.

### D6 — The scope ladder

Every edit has one scope and one **target branch**. The branch is the one `explain` says
won on the tile the gesture is on: the token's default, or the `when` for that tile's mode
tuple. The chip names it ("`surface.page` · Dim"), because on a Dim tile the default is
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
  - **During the gesture.** Each value goes to the tiles as a live-table batch and is
    written to no file. The batch is the branch edit, so each tile resolves it for its own
    mode tuple: a Dim value never paints a Light tile.
  - **Release.** The server writes once, as one transaction, checked against the digests
    the scrub opened with. If that check fails, nothing is written and the tool says so.
  - **End without release.** A competing write cancels the scrub first. So do a cancel, a
    disconnect, the server's loss, and a scrub idle for 30 seconds. On any of them, every
    tile gets the **latest committed** snapshot, not the gesture's starting one. A tile that
    reconnects gets the committed snapshot too.
  - **While open**, the files and the tiles differ, and the tool shows "previewing".
  - A rebuild-class edit has no scrub: it applies on release.
- **A rebuild** carries what the dev reload carries (LLP 1007 §6: slots by name, settled
  resources and the clock). The JS target's checkpoint also restores focus; on a wasm tile
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
  (the stored value or residual, labelled as such) or from `resolve-on` a named tile. It
  has contrast badges between declared on-pairs, each saying which resolution it assessed.
- **The component panel**: the selected component's params, grouped by `group`, ordered by
  `priority` (3 behind "more"), each with its `control` and `options` (LLP 1121 D15). A
  pinned param shows LLP 1121 D14's mark.
- **The workbench**: a preview screen (D4) for one component, with its params, its states
  (D8) and its blast radius.

A component with no params shows its rows (D5) and an offer to promote one to a param
(D6).

### D8 — States

- **Runner-known states** (`:disabled`, `:checked`) are data. A preview screen sets them,
  or the tool drives a tile into them.
- **Host-known states** (`:active`, `:hover`, `:focus-visible`) use a new form of `tap`,
    **a held state**: `{"op":"tap","id":V,"holdState":"active"|"hover"|"focus-visible"}`. It
  holds that host state on one node until cleared (`"holdState":null`), in development
  builds only.
  DEFERRED: "a new input is a form of `tap`". Its semantics:
      - **The field is `holdState`**, so it cannot be read as `tap`'s existing contact phase
    `hold`: `{"op":"tap","id":V,"holdState":"active"}`.
  - **Each hold is its own state.** `active` holds the press on the node the press
    resolves to, the innermost with a `press` handler (LLP 1121 D8). `hover` and
    `focus-visible` hold those overlays on the named node, which needs no press handler.
    Each applies only that state's overlay, with the host's ease, under Reduce Motion as
    the real state would.
  - **Ownership.** The server's tile session that set it; one held state per node.
  - **Cleared** by an explicit clear, by the node's retirement, and by real input on that
    node, which always wins. A hold is keyed by its site and its `layout` `instance` path, not
    by a view id, which does not survive a reload. After a reload (a `set-state` rebuild
    among them), the server re-applies it to the mount with that site and instance path, so
    the strip shows the new look. It is dropped if that mount no longer exists.
  - **Disabled** nodes refuse all three holds, as they take none of those states.
  - **Unsupported** on a host that cannot hold that state, answered by name. iOS has no
    pointer, so it answers `hover` that way.
- **Inspect shows what is held.** While a hold is active, the node's explanation reports
  the held state's winning overlay rows beside the base rows: the state, the declaring state
  part (its file and span, from the map) and any token. This is an addition to `layout`'s
  node answer, since overlays live outside the runner's base rows (LLP 1121 D8). The
  tool routes an edit of a held look to that state part (`set-state`). A host that cannot
  report overlay rows leaves the held look read-only, marked "provenance incomplete".
- **The states strip** uses one web tile per state, so pressed, hovered and default show at
  once. A native host shows them one after another on its one tile.
- **Editing a state part** is `set-state` (LLP 1121 D13), a rebuild.
- If the held state is refused (§9 Q4), host-known looks are edited and checked with a real
  press.

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
- **Not in v1:** a retained visual history. The review view shows before and after
  captures for the current session only.

### D10 — Agents as co-editors

An agent is a client with the same token, reads and writes.
- **Attribution.** Each event in `subscribe` names its client; the tool outlines an agent's
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
  - the relevant tokens and params;
  - captures.

  The agent answers with a proposal at an explicit rung. **The bundle stays on the machine**
  unless the developer has named a model that runs elsewhere, and the tool says, before
  sending, that captures are included. The tool holds no keys.

### D11 — Importers' review

LLP 1121 D14's importers (shadcn, Tailwind v4, DTCG, Astryx) propose; the tool reviews.
Each import shows:
- the mapped tokens, with values per mode and swatches;
- the role overrides, each off until ticked (LLP 1121 D1);
- the unmapped values, each with its reason.

Accepting writes the chosen items as one transaction, and undo is one step. An import is not
previewed on the tiles before it is written. The only preview is a scrub's, which lasts as
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
  pairing included. Captures reach it as bytes on its authenticated
  socket, so it needs no URL on the developer's machine, and its access ends when its token
  is revoked or expires. A paired client gets the observe group of the relay
  at `read` and `propose`, and the drive group only at `write`. It sees the developer's
  tiles.
- **A team relay** for people who share no machine is open (§9 Q6).

### D13 — Security

- **Development builds only.** Release builds have no endpoint, no relay, no live table and
  no held state.
- **One credential.** The project's token, in the server's memory, is given only to a
  verified-local request (LLP 1121 D13). Paired clients have scoped, revocable tokens.
- **The relay is an allowlist** (D3). Driving needs `write`, and each driving request is
  journaled with its client. No request form names a file.
- **Writes only through the server**, under its digests, compile check and journal (D6, LLP
  1121 D13). `set-row` writes only files inside the project root that the map names.
- **Captures are private.** They go to a temporary directory created `0700` per session,
  reach clients only as bytes on their authenticated sockets, and are deleted when the
  session ends. `.exact/` is
  not ignored by git, so nothing is kept there.
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
| live table | LLP 1121 stage 4 | accepted; DEFERRED admission open |
| `explain`, `uses`, token beside value | LLP 1121 D7, stage 2 | in progress |
| params; state parts and `set-state`; importers; pairing | LLP 1121 stages 7, 8, 3a–3d, 10 | accepted, not built |
| `prefer mode` | LLP 1121 D13 | accepted, not built |
| build relay with tile sessions | this RFC D3 → LLP 1121 D13 | proposed |
| `layout` at a point | this RFC D5 → LLP 1012 | proposed |
| `tap` holding a state | this RFC D8 → LLP 1012 | proposed |
| each build's own plan and map pair | this RFC D5 → both dev loops | proposed |
| the JS runtime reporting a row's token | this RFC D5 → the web JS target | proposed |
| `set-row`, `commit`, held proposals, scrub sessions, `propose`'s live-or-rebuild answer | this RFC D6, D9, D10 → LLP 1121 D13 | proposed |
| a held state's overlay rows in `layout`'s node answer | this RFC D8 → LLP 1035.002 | proposed |
| a `boundary` row source in `layout`, flagged by the kernel | this RFC D5 → LLP 1121 D7, LLP 1035.002 | proposed |
| attribute spans and source digests in maps | this RFC D6 → the compiler | proposed |
| source maps; `layout <id>` with `authored`, `dynamic`, `inherited` and `initial` sources | `contract build --map`; LLP 1035.002 D1 | built |

## 4. Staging and gates

Each stage needs an implementer, a date and the DEFERRED admissions (§9 Q2). Each lands with
or after the Tamber stage it needs.

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
      - `commit` refuses a file that was dirty or staged at its baseline, or hand-edited
     between two server writes; it leaves other staging untouched, and never pushes.
2. **T1a, import review** (with Tamber 3a–3d), D11. **Gate:** a shadcn theme reviews with
   overrides off until ticked, every unmapped value listed, and one transaction to accept.
3. **T2, the web canvas and inspect** (with Tamber stage 4):
   - headless web tiles owned by the server, running the wasm host (D3);
   - `layout` at a point; captures paired with geometry;
   - provenance (D5);
      - the token and dial rungs, scrub sessions and the live-or-rebuild answer (D6).

   **Gate:**
   - six web tiles in Light, Dim and Black at two text sizes;
      - a dial or token drag, chosen explicitly (token and dial are never the default rung,
     and the param, template and style rungs come at T3 and T5), updates every tile with no
     reload whenever the server classified the change `live`, each tile showing its own
     mode's value;
   - a click on an overlapped or clipped node selects the node the host hit-tests;
   - a stale capture's click is refused and recaptured;
      - a token row shows its chain;
   - a scrub previews on every tile and writes once on release, and a cancelled or
     disconnected scrub restores every tile;
   - a capture whose screenshot and `layout` disagree on identity is retaken;
   - an inherited, dynamic or boundary-implied row is read-only with its reason;
   - an observe-only client cannot `tap`;
   - no client can name a capture path.
4. **T2b, JS-target tiles**: the runtime's token report and each build's own map pair.
   **Gate:** a JS-target tile answers inspect as the wasm tile does, row for row, on the
   Bluesky clone.
5. **T2c, native tiles**: the iOS simulator and the Mac, then a phone, each owned by the
   server, with platform chrome. **Gate:** T2's inspect gate passes on each native host.
6. **T3, components** (with Tamber stage 7): the param rung, the component panel and the
   workbench (D6, D7). **Gate:** `PostButton`'s params appear as declared; a param edit
   re-scoped to the token it aliases is one transaction and one undo; a duration param's
   chip says "rebuilds".
7. **T4, states** (with Tamber stage 8 and the held `tap`): D8. **Gate:** pressed, hovered,
   disabled and default show at once on web tiles; real input clears a hold; a disabled
   node refuses one; inspect on a held tile shows the state part's rows; editing `:active`
   rebuilds and the hold is re-applied.
8. **T5, template and style rungs** (with `set-row`): D6. **Gate:**
   - a template colour edit writes one line and rebuilds;
   - a stale source digest is refused;
   - a candidate that fails to compile writes nothing;
   - promote to param and to token are each one transaction;
   - a `class:` row edits the style.
9. **T6, remote** (with Tamber stage 10): D12. **Gate:** a paired `propose` client observes
   but cannot drive, scrub or write; revocation cuts it off; an unencrypted remote
   connection is refused, pairing included.

## 5. Non-goals

- Drawing, artboards, free canvases (D14).
- Layout and structure editing in v1 (D14).
- Editing in release builds; a production inspect tier.
- Design state outside the files (D2).
- Components from imported libraries (D11).
- A continuous mirror, live presence and retained visual history in v1 (D3, D9, D12).
- A preview layer before writing (D11).

## 6. Risks

- **A capture is not live.** Each tile shows its age and the commit it reflects, and a tile
  that missed a batch is marked stale until recaptured.
- **Wrong scope.** Mitigated by exact1's default, blast-radius outlines, re-scope after the
  fact, and one undo (D6).
- **Provenance gaps** make some rows read-only (D5). That is the point, but it can frustrate:
  the reason is always shown.
- **Tiles cost.** Web tiles are cheap; simulators are not. Native tiles come one host at a
  time (§4).
- **The relay drives real devices.** It is scoped, allowlisted and journaled (D3, D13).

## 7. Cost

- **The tool:** one web JS-target app.
- **The Tamber server:** the relay and its tile sessions, the provenance join, scrub
  sessions and the live-or-rebuild answer, `set-row`, `commit`, held proposals.
- **The agent API:** two forms, `layout` at a point and the held `tap`, on each host that
  answers them.
- **The compiler:** source digests and attribute spans in maps. **The dev loops:** each
  build's own map pair. **The node explanation:** a held state's overlay rows.
- **The JS runtime:** reporting a row's token.
- **Docs:** the tool's page, and the protocol reference's new rows.

## 8. Not in this RFC

- Layout and structure editing.
- A continuous mirror.
- Native builds of the tool.
- The component library's workbench beyond params and states (LLP 1121 stage 12).
- A production inspect tier.
- Live presence.

## 9. Open questions for Charlie

1. **The name.** Candidates if a start helps: **Atelier**, **Loupe**, **Bench**,
   **Swatchbook**.
2. **DEFERRED.** The tool is a devtools UI (§Tooling). Its inspect joins source maps in the
   Tamber server and shows provenance, which the 2026-09-10 admission keeps to the driver and
   refuses as "a general provenance explorer". Amend both, for development builds only,
   together with LLP 1121 §9 Q3? RULES asks for a take or your waiver; none is offered here.
3. **The template and style rungs** (D6, `set-row`). Paint rows on one source node, with
   promote and detach, or only params, tokens and dials? Recommended: yes, paint rows only,
   in T5, after the system rungs have been used.
4. **The held `tap`** (D8). Recommended: yes. Without it, pressed and hover looks are edited
   blind.
5. **A continuous mirror** (D3). Recommended: not in r1. It would need a new agent surface.
6. **A team relay** (D12). Recommended: not until pairing has been used.
7. **The build relay** (D3): the Tamber server owning tile sessions and relaying an
   allowlist of agent forms under its token. Recommended: yes. The alternative is the tool
   driving each host's carrier itself, which a JS-target app cannot do.
