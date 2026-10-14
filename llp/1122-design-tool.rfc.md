# LLP 1122: The design tool — shaping a running app through Tamber

**Type:** RFC
**Status:** Draft r6, 2026-10-13. r6 restructures v1 on Charlie's word: the canvas is the running app, not captures (§0). Rulings so far (§9): Q2 (the DEFERRED admissions, for development builds only, with LLP 1121 §9 Q3), Q3 (template and style rungs: yes), Q4 (the held state: yes), Q5 (no captures in v1; the running app with hot reload). r1–r4 went through three blind review rounds by GPT-6 Astra (max) and Grok 4.7 (xhigh), every verdict RIGHT WITH CHANGES (`llp/reviews/rfc-2026-10-13-1122-design-tool-r{1,2,3}.{astra,grok}.md`); r6 is reviewed once (round 4) and r7 takes its findings. Nothing built. The tool's name is open (§9 Q1); this document says "the design tool". The amendment to `rules/DEFERRED.md` lands with this RFC.
**Systems:**
- The design tool: an Exact app for the web's JS target, a `tamber/1` client (LLP 1121 D13, D14).
- The Tamber server (LLP 1121 D13): build control over the dev loop's existing channel (D4), the selection join (D5), scrubs and the live-or-rebuild answer, `set-row` (D6), held proposals (D10), `commit` (D9).
- Every host's development build: an **inspect mode** that reports a tapped or clicked node and draws outlines (D5), and a held state (D8).
- The agent API (LLP 1012): the held state as a form of `tap` (D8), so agents get what the tool gets; the node explanation gains a held state's overlay rows and a `boundary` row source (D5, D8).
- The compiler's source maps: a source digest per file and the winning attribute's own span (D6).
- The web JS target's runtime: a row's token beside its computed value, and its own plan and map pair (D5).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** none yet. No stage is accepted until it has an implementer and a date (`rules/RULES.md` §Scope); each follows the Tamber stage it needs (§4).
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
- a few **control messages** on the dev loop's existing channel (inspect, outline, hold a
  state, pin a mode);
- **`set-row`**, a checked edit of one Contract row;
- **source maps** that name attributes and file digests.

No captures, no driven sessions, no tile grid in v1; several live copies side by side come
later (§4 T7). It is not a drawing tool and not a layout editor (D14).

## 0. Review rounds

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
  client code, speaking `tamber/1` over one socket with the project's local token.
- **The Tamber server**, in the dev loop (LLP 1121 D13), owns:
  - the design files and every write to them (`set-row`, `commit`, held proposals, scrubs);
  - the selection join: a build's report plus that build's source map (D5);
  - the control messages to builds (D4).
- **No design state of its own.** The files are the design, git the history.
- **Two URLs.** `bun exact.mjs tamber tool` opens the tool at its own local URL, served by
  the Tamber server. The app's web build is the dev loop's own URL, which the tool embeds.

### D3 — The canvas: the app, running

- **The web build, embedded.** The tool shows the app's web development build in an
  `iframe` (LLP 1020) at the dev loop's URL. It is the real app: it runs, takes input,
  navigates, and reloads on a structural edit exactly as it does in a browser tab. The tool
  sizes the frame (a phone width, a tablet, a desktop) and can open a second one at another
  size.
- **Devices and simulators, as they are.** The developer runs the app on the iOS simulator,
  a phone or the Mac in the usual way (`bun exact.mjs ios --run`, `--device`). Each
  development build connects to the dev loop as today; the tool lists them, and inspect and
  the control messages reach every one.
- **How an edit shows.** Through what the dev loop already does:
  - a token, dial or param value whose closure is `live` (LLP 1121 D13): the live table,
    with no reload, on every connected build at once;
  - anything else (a `set-row`, a state part, a structural change, a motion token): the dev
    loop's rebuild and hot reload, which carries slots, settled resources and the clock
    (LLP 1007 §6; the JS target's checkpoint also restores focus).
- **Modes.** The tool pins a build's app mode, scheme or contrast with a control message
  (D4), the same facts `prefer` pins. One copy shows one mode at a time; the app's own
  preview screens show several at once inside one copy (below); a grid of live copies, one
  per mode, is T7. An app's own `setScheme` beats a requested scheme, so the tool shows each
  build's **effective** scheme and mode, as the build reports them.
- **Preview screens.** Ordinary routes, written by the developer (the tool offers a
  starter), that lay out a component or screen under several `color-scheme` and
  `-exact-mode` pins (LLP 1121 D5). A Light preview is `color-scheme="light"`; Dim and
  Black are `color-scheme="dark"` plus the pin. The Bluesky clone's Appearance screen is
  one. A copy on a preview screen is the component workbench (D7).
- **No captures in v1.** Nothing is screenshotted, stored or sent as an image.

### D4 — Control messages: four, on the channel builds already hold

Every development build already holds a channel to the dev loop (the event stream that
tells it a plan changed, which Tamber's live table also uses, LLP 1121 D13). The Tamber
server sends four more messages on it, each with the build id `hello` lists:

| Message | What the build does |
|---|---|
| `inspect on`, `inspect off` | enters or leaves inspect mode (D5) |
| `outline` (a source span, or none) | draws an outline around every mounted node whose site maps to the span, in inspect mode |
| `hold` (a node's site and instance path, a state, or none) | holds a host-known state on that node (D8) |
| `pin` (scheme, contrast, an app mode, or none) | pins that fact, as `prefer` would; `none` hands it back |

That is the whole build relay. The tool drives no input into an app (no taps, no typing),
takes no pictures, and launches no sessions. A build that does not speak a message (an older
host) says so by name, and the tool shows it.

### D5 — Inspect: tap the real screen

**Inspect mode** is part of every host's development build, compiled out of release builds.
- **Entering.** The tool's inspect toggle sends `inspect on` to every connected build (or
  one). Each shows a small banner so the person knows taps won't reach the app.
- **Selecting.** In inspect mode the host takes pointer and touch input itself:
  - hovering (a mouse) outlines the node under the pointer;
  - a tap or click selects it.

  The host hit-tests the real input point natively, as it does for a real tap. There is no
  capture to go stale.
- **The report.** The host sends the Tamber server a `selected` message:
  - the build id and its plan digest;
  - the view's site and its `instance` path;
  - the runner's own explanation of the node, as `layout <id>` gives it (LLP 1035.002): each
    row with its value and source, the token beside it (LLP 1121 D7), and, while a hold is
    active, the held state's overlay rows (D8).

  The runner computes this in-process; it is the same answer the agent gets.
- **The join.** The server looks the site up in that build's own source map, verified by
  plan digest, and checks the map's source digest against the file on disk (D6). It then
  publishes the selection to subscribed clients, and sends `outline` for the source span to
  every build, so every copy of the node (each `each` row, each expansion of a component
  body) is outlined on every screen. The source-map admission let the driver read maps and
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
  another producer's plan (`host/web-js/dev.mjs`). So the JS runtime must report a row's
  token beside its computed value and advertise its own plan and map pair (§3). Until it
  does, the embedded build is the wasm host's (`dev.mjs --wasm`), which has the kernel.
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
  changeset entry sends `outline` to every build, so the person sees where it is on screen.


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
- **The workbench**: a preview screen (D4) for one component, with its params, its states
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
  - **Keyed by site and `instance` path**, not a view id, which does not survive a reload.
    After a reload (a `set-state` rebuild among them), the build re-applies its holds to the
    mount with that site and path, and drops a hold whose mount is gone.
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
  - **Seeing the app** remotely means reaching the developer's dev URL for the embed, over
    the same encrypted transport. Without that, a remote client works on the design and
    sees selections, but has no canvas.
  - Control messages (D4) need `write`: inspect, outline, hold and pin change what the
    developer's screens show.
- **A team relay** for people who share no machine is open (§9 Q6).

### D13 — Security

- **Development builds only.** Release builds have no protocol endpoint, no inspect mode, no
  control messages, no live table and no held state; inspect is compiled out.
- **One credential.** The project's token, in the server's memory, is given only to a
  verified-local request (LLP 1121 D13). Paired clients have scoped, revocable tokens, only
  over an encrypted transport (D12).
- **No driving.** The tool sends no taps, keystrokes or file answers into an app. Its only
  control is D4's four messages, which need `write` and are journaled with their client.
  Inspect mode takes input only while it is on, with a banner on the screen.
- **No images.** Nothing is captured or stored. A selection report carries node data and
  source locations, which only token holders receive.
- **Writes only through the server**, under its digests, compile check and journal (D6, LLP
  1121 D13). `set-row` writes only files inside the project root that the map names.
- **The embed is the dev URL.** The tool's `iframe` loads the app from the dev loop the same
  way a browser tab does, under the dev gate as it is. It gains nothing a tab would not have.
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
| four control messages on the dev channel | this RFC D4 → the dev loops, the hosts, the Tamber server | proposed |
| the held state, as `hold` and as a form of `tap` | this RFC D8 → the hosts, LLP 1012 | ruled yes (§9 Q4) |
| a held state's overlay rows, and a `boundary` row source, in the node explanation | this RFC D5, D8 → LLP 1035.002, LLP 1121 D7 | proposed |
| attribute spans and source digests in maps | this RFC D6 → the compiler | proposed |
| each build's own plan and map pair; the JS runtime's token report | this RFC D5 → both dev loops, the web JS target | proposed |
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
   - the web build embedded (D3);
   - connected simulators and devices listed;
   - the `pin` message;
   - the token and dial rungs, scrub sessions and the live-or-rebuild answer (D6).

   **Gate:**
   - the Bluesky clone runs in the tool's frame and on the iOS simulator and the Mac;
   - a token or dial scrub, chosen explicitly, updates all three with no reload whenever the
     server classified it `live`;
   - a structural edit hot-reloads all three and keeps their slots;
   - a cancelled scrub restores every copy;
   - `pin` shows Dim on one copy, and the tool shows each copy's effective mode.
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
   - a paired `propose` client cannot write or send control messages;
   - revocation cuts it off;
   - an unencrypted remote connection is refused.
10. **T7, several live copies**: a grid of the app running in several frames at once, each
    pinned to one mode (Light, Dim and Black at two text sizes), plus the states side by
    side.
    - Copies are live builds, not captures.
    - The questions this stage settles: how many web copies a machine runs well, whether
      each needs its own `--storage`, how navigation is mirrored across them (by location,
      never by point), and whether native copies (several simulators) are worth their cost.
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
- **The Tamber server:** the selection join, control messages, scrub sessions and the
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
6. **People who share no machine or network with the developer.** Today a remote designer
   pairs with the developer's machine over an encrypted connection (D12). Should we also run
   a hosted service that connects people who can't reach each other directly, so a designer
   anywhere can work on the app with the developer? Recommended: not yet, until pairing has
   been used.
7. **How the tool controls running apps.** In r6 the tool never presses buttons or types
   into an app. It sends four messages to development builds over the channel they already
   hold: turn inspect on or off, outline these nodes, hold this state, pin this mode (D4).
   Is that the right limit? Recommended: yes. Driving apps (taps, typing) stays with agents
   through the agent API, where it is already scoped and journaled.
