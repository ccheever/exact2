# LLP 1122: The design tool — shaping a running app through Tamber

**Type:** RFC
**Status:** Draft r1, 2026-10-13. Not reviewed. Nothing built. The tool's name is open (§9 Q1); this document says "the design tool".
**Systems:**
- The design tool: an Exact app for the web's JS target, a `tamber/1` client (LLP 1121 D13, D14).
- The Tamber server (LLP 1121 D13): two additions this RFC asks for, `set-row` (D6) and the build relay (D3).
- The agent API (LLP 1012): used as it is, with one asked-for `prefer` group (D8).
- The compiler's source maps (`contract build --map`, `docs/reference.md` "Source maps") and the dev server that keeps them.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** none yet; each stage follows the Tamber stage it needs (§7).
**Related:**
- LLP 1121 (Tamber): D7 the token index, D8 state parts, D9 control roles, D13 the protocol, D14 clients and importers, D15 component params, §5 staging, §9 Q3 and Q4.
- LLP 1012 (the agent API's operations), LLP 1035.002 (`layout` explaining one node: its site and every row's source), LLP 1095 D7 (the agent pins traits).
- `rules/DEFERRED.md` §Tooling ("no Design Mode … no devtools UI"), §Agent API (ten operations).
- exact1, as research: LLP 0158 (Design Mode: the scope ladder, act first and scope after, templates not copies, provenance confidence), LLP 0200 and 0208 (the authoring platform and its advanced surfaces, both shelved), LLP 0225 (the UI Elements Lab), LLP 0304 W5 and W6 (Design Mode was DOM-only; editing security deferred). The survey is `~/.tuft/projects/exact2-theming/facet1-research.md` §10d.

## Summary

The design tool is how a designer, or an agent, shapes an exact2 app's look. It never draws
a picture of the app. It shows **the app itself, running**: on a phone, a simulator, the
Mac and the web, side by side, in every mode the design has. Select something on any of
those screens and the tool shows where its look comes from: the Contract node, its rows,
the tokens and component params behind them, and the dials behind those. Change any of
them and every connected device updates at once. Each change is a one-line, reviewable diff
in the app's own files.

It is a `tamber/1` client like any other (LLP 1121 D13, D14): it has no private channel,
and an agent has every power it has. Its parts are:
- **the canvas** (D3, D4): live builds, tiled by mode;
- **inspect** (D5): from a pixel to its provenance;
- **the scope ladder** (D6): instance, param, token, dial, after exact1's Design Mode;
- **panels** (D7) generated from the design's dials and the components' declared params;
- **states** (D8);
- **history** in git (D9);
- **agents** (D10);
- **importers'** review (D11);
- **collaboration** (D12) and **security** (D13).

What it is not: a vector drawing tool, or a layout editor in v1 (D14).

## 1. Motivation

- **Tamber makes a design data that can be swapped live** (LLP 1121 D7, D13). Something
  has to put a person's hands on that data while they look at the real app.
- **exact1 built this as Design Mode** (LLP 0158) and learned three things that shape this
  design:
  1. **The scope problem is the design problem.** "Did you mean this button, all buttons,
     or the radius token?" 0158's answer, a scope ladder with a systemic default, act first
     and re-scope after, is right and is kept (D6).
  2. **Provenance must be exact or the edit must not commit.** Write-back through a value
     that a recipe transformed was wrong. 0158 failed closed on anything short of `exact`
     confidence. Here provenance is a lookup through the compiler's source map and
     Tamber's token index, not a guess (D5).
  3. **DOM-only editing was the ceiling.** Provenance came from the web's DOM mirror, so
     native apps got themes but no editing (0304 W5). Here the canvas is every host,
     driven through the agent API, which is the same on all of them (D3).
- **The repository's own apps need it.** T3 Code's theme editor hand-built "where is this
  colour used" (`issues/closed/20261010-paint-role-node-list-read.md`). The Bluesky
  clone's Light, Dim and Dark, its font sizes and its post button are exactly the edits
  this tool makes (LLP 1121 §1).

## 2. Decisions

### D1 — What it is for, and for whom

The design tool is the **primary way to shape an exact2 app's look**, for two kinds of
co-editor:
- **designers**, who see and adjust;
- **agents**, who read the same provenance and make the same edits, through the same
  protocol (D10).

It edits the design, not the program: tokens, dials, axes, component params, state looks
and, at the narrowest scope, a node's paint rows (D6). Structure, layout, data and behaviour
stay with Contract source and its authors (D14).

### D2 — Architecture: a web JS-target Exact app speaking `tamber/1`

- **It is an Exact app built for the web's JS target**, as LLP 1121 D13 requires. Only a
  TypeScript data module there can send socket frames, and that module is its protocol
  client. Its native builds, if any, come later.
- **Two connections, one token.** The data module holds two:
  - `tamber/1` to the Tamber server, for the design (reads, `subscribe`, writes,
    `propose`);
  - the **build relay** (D3) on the same server, for the running builds.

  Both use the project's local write token (LLP 1121 D13).
- **It keeps no state of its own beyond the session.** The files are the design (LLP 1121
  D13); git is the history (D9). Closing the tool loses nothing but the current selection
  and layout.
- **It is served by the dev loop**, which already runs the Tamber server (LLP 1121 D13),
  at a local URL. `bun exact.mjs tamber tool` opens it.

### D3 — The canvas: the app's own builds, live, side by side

The canvas is a grid of **tiles**. Each tile is one connected development build of the app,
on any host:
- the iOS simulator or a paired iPhone;
- the Mac app;
- a web build in a browser tab;
- a headless web build that the tool starts itself, which is the cheap way to add tiles.

**What a tile shows.** The build's latest screenshot, taken through the agent API's
`screenshot`. It is refreshed when the build reports a commit, including a live-table batch
(LLP 1121 D13), and settles under `clock` before capture, so the image is never caught
mid-animation. A phone or simulator also shows the app live on its own screen; the tile is
the copy the tool can hit-test. A continuously streamed mirror is not in r1: the agent API
has no frame stream, and DEFERRED §Agent API holds it to ten operations (§9 Q5).

**The build relay.** The Tamber server relays the agent API's operations for the builds
connected to the dev loop. It adds no operation: `tree`, `screenshot`, `tap`, `type`,
`state`, `layout`, `logs`, `clock`, `prefer` and `perf` are LLP 1012's, addressed by the
build ids that `hello` lists (LLP 1121 D13). It authenticates with the same token. The relay
exists so the tool needs one connection and one credential, not a driver per host.

**Modes side by side.** A tile is pinned to one combination of:
- **scheme and contrast**, by `prefer media` (LLP 1095 D7);
- **an app mode**, by `prefer mode` (LLP 1121 D13), for example Dim;
- **text size**, by `prefer page root-font-size` (LLP 1121 D6's percentage, or the
  platform's sizes);
- **window size**, by the host's own resize.

The default canvas for a design with Bluesky's axes is six web tiles (Light, Dim and Black,
at the default text size and at the largest), plus every device the developer connects,
each following the mode the tool last chose for it.

**One screen everywhere.** "Go to" drives every tile to the same route. Navigation inside a
tile (a `tap` relayed from a click on its screenshot) can be mirrored to the others. Each
tile keeps its own data (`--storage`).

### D4 — The app's own preview screens

A design reviewed only on screens the app happens to reach misses states and modes. The
tool therefore also shows **preview screens**: ordinary routes in the app that a developer
writes, or the tool proposes as a starter. Each lays out one component or one screen under
several `color-scheme` and `-exact-mode` pins at once (LLP 1121 D5), as the Bluesky clone's
Appearance screen does with its three previews. They run in the app like any route, so they
need nothing from the tool. A tile on a preview screen is the tool's component workbench
(D7).

### D5 — Inspect: from a pixel to its provenance

**Selecting.** Click a tile. The tool hit-tests the click against the tile's last `layout`
answer (`nodes[]` with their boxes) and takes the front-most node. The selection is the
**source node**, not the rendered copy: every copy of a node inside an `each` is outlined
in every tile, and the chip reads "All rows". This is exact1's "templates, not copies"
(LLP 0158 §3.3). Per-row styling is data, and an agent's job.

**Provenance**, in four steps, each a lookup:
1. **Node → source.** `layout <id>` gives the node's plan site and every row with its
   source (`authored`, `inherited` from an ancestor, or `initial`) (LLP 1035.002 D1). The
   dev server's source map for that plan, verified by digest (`docs/reference.md` "Source
   maps"), gives the site's file and range, its component call-site chain, and each winning
   row's origin (`own`, `class:<Name>`, `tag`).
2. **Row → token.** A row that holds a token reference names it: LLP 1121 D7 keeps the token
   beside the value, and D13's `explain` reports it with the `when` that won.
3. **Token → dials and params.** `explain` gives the chain (`background-color:
   surface.card ← neutral (dial)`), including a component param (`PostButton.radius ←
   radius.md`).
4. **Where else it is used.** `uses` lists every node holding the token in each connected
   build (LLP 1121 D7's index), and the tool outlines them across tiles.

**Fail closed.** The tool offers an edit only where provenance is exact: the source map's
digest matches the build's plan, and the row's source is `authored` with a known origin or a
token. Otherwise the panel shows the value read-only and says why ("inherited from
`app.contract:12`", "a computed expression"). This is exact1's lesson: never write back
through a value it can't trace.

**The inspector panel** shows:
- the selected node's type, its source location (one click opens it in the developer's
  editor), and its call-site chain;
- its paint rows, each with value, source and token chip;
- the component's params, if the node is inside a component instance (D7);
- its state parts (D8).

### D6 — The scope ladder: instance, param, token, dial

Every edit has exactly one scope. From narrowest to widest:

| Scope | Writes | Through | Available when |
|---|---|---|---|
| **instance** | a paint row on this source node | `set-row` (below), via `contract fmt` | the row is the node's own, authored |
| **param** | this component's param, for every instance | `set-param` (LLP 1121 D15) | the node is in a component that declares the param |
| **token** | the token, everywhere it is used | `set-token` | the row's value is a token |
| **dial** | a dial, and every token folded from it | `set-dial` | the token depends on the dial |

**The systemic default.** An edit lands at the narrowest scope that edits the system rather
than forking it:
- the **param** when the row reads one;
- otherwise the **token** when the row holds one;
- otherwise the **instance**.

A **dial** is offered, never the default: changing a whole brand by accident is the one
mistake the ladder must make impossible. This is exact1 0158 §3.1 with Tamber's rungs.

**Act first, scope after** (0158 §3.2):
- An edit applies at once, at the default scope, live on every tile.
- The scope is a chip on the selection ("PostButton · radius", "Just this",
  `radius.md`).
- The **blast radius is the feedback**: every node the edit reaches is outlined, in every
  tile.
- **Re-scope after the fact.** The chip on the last edit re-applies it at another rung
  without redoing the gesture, as one transaction: the inverse at the old scope plus the
  write at the new one.
- **Promote and detach.** An instance edit can be promoted to a param or a token (naming a
  new token if needed, `add-token`). A param or token edit can be detached back to the
  selected instance.

**`set-row`**, the one protocol addition this needs. It sets or unsets one paint row of
one Contract node, addressed by the source map's site and the file's digest, through
`contract fmt`:
- The rows are those LLP 1121 D8 allows in a state part (colours, opacity, shadow, tint,
  press scale), plus `border-radius` and `font` once LLP 1121 stage 6 admits those kinds.
  No layout rows (D14).
- Any edit to it rebuilds and reloads, like any other edit to a Contract file (LLP 1121
  D13).
- It is a minor-version addition to `tamber/1`, in LLP 1121 D13's table, with the same
  digest preconditions, inverse and journal.
- §9 Q3 asks whether instance edits belong in the tool at all.

### D7 — Panels from dials and component params

The tool draws no panel by hand. Every control comes from the design's declared metadata:
- **The dials panel**: each dial with its `control`, `range`, `step` and `label`, ordered
  by `group` and `priority` (LLP 1121 D4).
- **The token grid**: tokens by name group × every mode tuple and scheme, with resolved
  swatches. It shows contrast badges between declared on-pairs (a colour and its
  `on-` partner). Each badge says which resolution it assessed, the stored value or a named
  build's (LLP 1121 D13 `resolve`, `resolve-on`). Each token shows the dials it follows.
- **The component panel**: when the selection is inside a component instance, the
  component's params, grouped by `group` and ordered by `priority` (1 first, 3 behind
  "more"), each with its `control` (LLP 1121 D15). A `select` offers its `options`; a
  `token` control offers the tokens of the kind. A param pinned off its token default shows
  the mark LLP 1121 D14 defines ("no longer follows `radius.md`").
- **The component workbench**: a preview screen (D4) for one component, with its params
  panel beside it and its state parts (D8).

The tool surfaces what a component declares and nothing else. A component that declares no
params shows its rows (D5) and an offer to add params (`add-param`), which writes the
`params` section in the component's file.

### D8 — States

A designer has to see the pressed, hovered, focused, disabled and checked looks without
pressing anything.
- **Runner-known states** (`:disabled`, `:checked`) are data. A preview screen (D4) sets
  them, or the tool drives a tile into them with `tap` and `type`.
- **Host-known states** (`:active`, `:hover`, `:focus-visible`) are the host's own
  tracking (LLP 1121 D8), and the agent API can tap but not hold a press. So this RFC asks
  for **a fifth `prefer` group, `state`**, beside `media`, `page`, `fold` and `mode`:
  `prefer state <id> active` pins one node's host state, in development builds only,
  until it is cleared. It is held like a press, so overlays, ease and Reduce Motion behave
  as they do for a real press (LLP 1121 D8).
- **The states strip.** For the selected node, the tool shows one tile per state the node
  declares, each pinned with `prefer state`. Next to each is that state part's rows, edited
  through `set-state`, which rebuilds (LLP 1121 D13).
- **Without `prefer state`** (if it is refused, §9 Q4), the strip shows runner-known states
  only, and host-known looks are edited blind, with a live tap to check.

### D9 — Undo and history: the session in memory, the record in git

- **Undo.** Every write returns its inverse transaction (LLP 1121 D13). The tool keeps the
  session's stack, and undo and redo issue those inverses. A change another client made is
  never undone by mine: an inverse whose preconditions fail (the file changed since) is
  refused, and the tool says who changed it.
- **The session changeset.** The working tree's diff of the design files since the session
  began, shown as a list of edits in plain words ("`radius.md` 8 → 12px") with the
  line-level diff under each.
- **Commit, when asked.** "Commit" writes one git commit of the design files touched. Its
  message lists the edits and who made them (person or agent), and its author is the
  developer. Nothing is committed without that action, and the tool never pushes.
- **History.** The git log of the design files, readable as edits. "Revert" proposes the
  inverse of a past commit as a transaction (`propose`), which the person accepts.
- **Review.** A changeset or commit has a review view: per edit, before and after
  screenshots of the affected tiles, taken when the edit was made. A reviewer sees what
  changed on screen, not only in text.

### D10 — Agents as co-editors

An agent is a client with the same token, the same reads and the same writes (LLP 1121 D13,
D14). The tool adds three things on top:
- **Attribution.** Every event in `subscribe` names its client. The tool shows an agent's
  edits as they arrive, outlined in its own colour, in the changeset under its name.
- **Proposals.** An agent may write directly, or `propose` for review. The tool lists
  pending proposals with the same review view as an import (D11): accept all, accept some,
  decline.
- **Prompt-as-tweak** (0158 §9). A selection-scoped request ("make the pressed state a
  little darker", "match this to the system blue") goes to an agent with a context bundle:
  - the selection and its provenance (D5);
  - the scope ladder's options;
  - the relevant tokens and params;
  - the tile screenshots.

  The agent answers with a transaction proposed at an explicit scope, which the person
  reviews. Which agent and model is the developer's configuration, not the tool's.

### D11 — Importers, with a review view

The importers of LLP 1121 D14 (shadcn, Tailwind v4, DTCG, Astryx) are proposal-only
clients. The tool is where their proposals are reviewed. An import shows three lists:
- **Mapped tokens**: name, value per mode, and a swatch. Each can be accepted or edited
  before accepting.
- **Role overrides**, offered separately (`--background` → `-exact-background`), each off
  until ticked: the design stays the platform's unless the person chooses (LLP 1121 D1).
- **Unmapped values**, each with the importer's reason, so nothing is dropped unseen.

Accepting applies the chosen items as one transaction, previewed live on every tile before
it is written. Importing components is a non-goal, as in LLP 1121.

### D12 — Collaboration

- **Local first.** Several clients on one machine (the tool in two windows, an agent, a
  hand edit in an editor) edit one design through one server, serialized by digest
  preconditions (LLP 1121 D13). The tool shows who else is connected and their selections.
- **Remote, by pairing** (LLP 1121 D13; its §9 Q4). A designer on another machine pairs
  once with a code shown in the local tool. They get a token scoped `read`, `propose` or
  `write`, revocable, with every write attributed. Their tool shows the same canvas through
  the relay, so the screenshots come from the developer's builds.
- **A team relay**, for people who share no machine, is open (§9 Q6).

### D13 — Security

- **Development builds only.** A release build has no protocol endpoint, no relay, no live
  table and no `prefer state` (LLP 1121 D13).
- **One credential, local by default.** The project's write token is held in the server's
  memory. It is given only to a verified-local request (LLP 1121 D13), and it gates both
  `tamber/1` and the relay.
- **The relay can drive the app.** `tap` and `type` act as the user, on real devices. A
  paired client gets the relay only with `write` scope, and every relayed operation is
  journaled with the client that sent it.
- **The tool writes only through the protocol.** It never edits files itself, so every
  write has the server's checks, digests and journal.
- **Screenshots are the app's data.** The tool keeps tile images in memory and in
  `.exact/` for review views. They are listed in the session and deleted with it unless
  committed into a review.

### D14 — What it is not

- **Not a vector drawing tool.** There is no free-form canvas, no shapes, no artboards and
  no pixel nudging. Everything on the canvas is the running app.
- **Not a layout editor in v1.** Dragging boxes, changing flex direction, gap or padding by
  handle, and reordering children are Contract source edits. They are structural, and
  exact1's 0158 found drag intent in a flex world ambiguous: is a drag gap, padding or
  basis? The case for layout in v1 is weak. Lengths a design names (spacing, radius) are
  tokens and params, so they are editable at those scopes (LLP 1121 stage 6). Instance
  `set-row` excludes layout rows. Layout editing is a later RFC once instance edits have
  been used (§9 Q3).
- **Not a component editor.** It edits a component's declared params and state parts. It
  does not add parts, slots or structure. The component library (LLP 1121 stage 12) is its
  own RFC.
- **Not a Figma replacement for Figma files.** Figma is a client (LLP 1121 D14). There is no
  live two-way canvas sync.

## 3. What it needs from elsewhere

| Need | From | Status |
|---|---|---|
| reads, writes, `propose`, `subscribe`, inverses, the token | LLP 1121 D13, stage 3 | accepted, not built |
| the live table | LLP 1121 stage 4 | accepted, not built; DEFERRED admission open |
| `explain`, `uses`, the token index | LLP 1121 D7, D13; stage 2 | stage 2 in progress |
| component params | LLP 1121 D15, stage 7 | accepted, not built |
| state parts, `set-state` | LLP 1121 D8, stage 8 | accepted, not built |
| importers | LLP 1121 D14, stages 3a–3d | accepted, not built |
| pairing | LLP 1121 stage 10 | proposed (its §9 Q4) |
| `set-row` | this RFC D6 → LLP 1121 D13 | proposed |
| the build relay | this RFC D3 → LLP 1121 D13 | proposed |
| `prefer mode` | LLP 1121 D13 | accepted, not built |
| `prefer state` | this RFC D8 → LLP 1012 | proposed |
| source maps on the dev server | `contract build --map` | built |
| `layout <id>` with site and row sources | LLP 1035.002 D1 | built |

## 4. Staging and gates

Each tool stage lands with, or after, the Tamber stage it needs. Each needs an implementer
and a date (`rules/RULES.md` §Scope), and all of it needs DEFERRED §Tooling's admission
(§9 Q2).

1. **T1, the files** (with Tamber stage 3):
   - the tool as a web JS-target app;
   - the dials panel and token grid (D7);
   - undo, the changeset, commit and history (D9);
   - agents' attribution and proposals (D10).

   **Gate:**
   - every control the tool shows comes from declared metadata;
   - an edit is a one-line diff, and undo restores the file byte for byte;
   - an agent's write appears attributed within one event;
   - a stale inverse is refused with the other client named;
   - commit writes one commit with the edits listed, and never pushes.
2. **T1a, import review** (with Tamber stages 3a–3d): D11.
   **Gate:** a shadcn theme imports through the review view; role overrides are off until
   ticked; every unmapped value is listed; accepting is one transaction.
3. **T2, the canvas and inspect** (with Tamber stage 4, and the relay):
   - tiles on the web, iOS simulator, a phone and the Mac (D3);
   - modes side by side;
   - inspect with provenance (D5);
   - the token and dial rungs of the scope ladder (D6).

   **Gate:**
   - a dial drag updates six web tiles, the simulator and the Mac within one refresh of
     each, with no reload;
   - clicking a row in the Bluesky clone on any tile selects its source node, outlines
     every copy, and shows its file and line, its rows' sources and its token chain;
   - an inherited or computed row is read-only, with the reason;
   - `uses` outlines every node using the token across tiles;
   - a relayed `tap` without the token is refused.
4. **T3, components** (with Tamber stage 7): the component panel and workbench (D7), the
   param rung (D6). **Gate:** the clone's `PostButton` params appear grouped and ordered as
   declared; a `set-param` re-scoped to the token is one transaction and undoes as one; a
   pinned param shows its mark.
5. **T4, states** (with Tamber stage 8, and `prefer state` if admitted): D8. **Gate:**
   the post button's pressed, disabled and default looks show side by side without a
   touch; editing `:active` rebuilds and the strip shows the new look.
6. **T5, instance scope** (with `set-row`, if admitted): the instance rung, promote and
   detach (D6). **Gate:** an instance colour edit writes one Contract line and rebuilds;
   promoting it to a new token writes the token and replaces the literal in one transaction;
   detach reverses it.
7. **T6, remote** (with Tamber stage 10): pairing and remote tiles (D12). **Gate:** a
   paired `propose` client cannot write or drive; revoking it cuts it off; its proposals
   arrive in the review view.

## 5. Non-goals

- Vector drawing, artboards, free-form canvases (D14).
- Layout and structure editing in v1 (D14).
- Editing in release builds, or a production inspect tier.
- A second source of truth: the tool keeps no design state outside the files (D2).
- Components from imported libraries (D11).
- A live mirror stream in r1 (D3).

## 6. Risks

- **Stale screenshots mislead.** A tile is a capture. Each shows its age and the commit it
  reflects, and a tile whose build missed a batch is marked stale until it catches up.
- **Wrong-scope surprises.** Mitigated by the systemic default, the blast-radius outlines,
  re-scope after the fact, and uniform undo (D6). exact1 0158 found the same.
- **Provenance gaps.** Inspect refuses rather than guesses (D5). A row with no exact path
  can't be edited here, and it says why.
- **Many tiles are many builds.** Six headless web tiles are cheap; simulators are not. The
  default canvas uses web tiles for modes and real devices as the developer connects them.
- **The relay widens what a token can do.** It can drive the app on real devices. It is
  scoped and journaled (D13).

## 7. Cost

- The tool itself: one web JS-target app, with its TypeScript data module as the protocol
  client.
- Two protocol additions on the Tamber server: `set-row` and the build relay.
- One `prefer` group, `state`, on every host that tracks a press.
- Docs: a page in `docs/` on driving the tool, and the protocol reference's new rows.
- Nothing in the runner or kernel beyond what LLP 1121 already builds.

## 8. Not in this RFC

- Layout and structure editing (D14). It is a later RFC once instance edits have been used.
- A continuous live mirror (D3).
- Native builds of the tool.
- The component library's workbench beyond params and states (LLP 1121 stage 12).
- A production inspect tier.

## 9. Open questions for Charlie

1. **The name.** The tool is unnamed. Candidates, if a starting point helps:
   - **Atelier** (the studio);
   - **Loupe** (looking closely at the real thing);
   - **Bench** (where components are tried);
   - **Swatchbook**.
2. **DEFERRED §Tooling.** The tool is a devtools UI, which §Tooling defers. LLP 1121 §9 Q3
   asks the same for the protocol server. Admit both together, for development builds only?
3. **Instance scope** (D6, `set-row`). Should the tool edit one node's paint rows, or only
   params, tokens and dials, leaving every instance edit to source and agents? Recommended:
   yes, paint rows only, with promote and detach, because a designer's first edit is often
   "just this one". Layout rows stay out of v1.
4. **`prefer state`** (D8): a fifth `prefer` group pinning one node's host-known state in
   development builds. Recommended: yes. Without it, pressed and hover looks are edited
   blind.
5. **A live mirror** (D3). Screenshot tiles refreshed on each commit are enough for design
   edits. A continuous stream needs a new agent surface, against §Agent API's ten
   operations. Recommended: not in r1.
6. **A team relay** (D12) for people who share no machine. Recommended: not in this
   program until pairing has been used.
7. **The relay** (D3): relaying the agent API's operations through the Tamber server, under
   its token, so the tool has one connection. Recommended: yes. The alternative is the tool
   driving each host's agent carrier itself.
