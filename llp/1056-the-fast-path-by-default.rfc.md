# LLP 1056: The fast path by default

**Type:** RFC (analysis and plan)
**Status:** Draft
**Systems:** The data seam as authors meet it: Contract (roster, edge handlers, `contract build`'s report), runner (the agent's `state`), authoring notes; the Bluesky port and RealWorld as the demonstrations
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-26
**Related:** LLP 1054 (the Bluesky field report) and its children 1054.000.000 (declared refreshes), .003 (dates, numbers and durations in the roster), .004 (narrower refreshes: withdrawn in favor of this plan's P5), .005 (`trim`), .006 (arguments on list edges); LLP 1027 (TypeScript sources), 1027.003 (value transfer, measured), 1027.004 (bounded answers); LLP 1047 (pay for what you use); LLP 1012 (agent API); reviews `llp/reviews/1054.000.003-004.*`, `llp/reviews/1056-*`

## Summary

Charlie asked on 2026-09-26 whether exact2's data model is good, feels good
to write, and leads to good performance. The answer, from reading the code,
the LLPs and the Bluesky port's diary:

- **Yes on the mechanism.** One seam: a source answers now or hands back one
  request. The runner never does I/O. Nothing runs before first pixel.
- **The performance ceiling is high.** Web first paint is 70–110 ms, and a
  tap reaches the DOM in 6.4 ms (LLP 1054 §7).
- **The defaults make the slow way the easy way.** An author's first draft
  of a feed app re-sends whole lists on every tick and every tap. The fixes
  exist, but they are opt-in and found by reading LLPs.
- **Writing a source is pleasant or not depending on the side of the seam.**
  TypeScript sources are the paved path. Rust sources are laborious.

This LLP records that analysis (§1) and a plan (§3) whose principle is:
**the cheap pattern must be the one an author writes first, and an expensive
one must be visible.** Most of the plan is already specified in small child
RFCs. What this document adds is the order, the two cost reports that make
expensive patterns visible (P6), and the authoring pattern the Bluesky port and
RealWorld demonstrate (P4, P5).

## 1. Analysis

### What works, to keep

- **One seam, one loop.** A source answers now, or hands back one request.
  Its reply is folded in, and the source is asked again. The Bluesky port
  carried pagination, handle → did → thread chains, token refresh,
  notifications → posts, and an offline demo account through that single
  mechanism (LLP 1054 §7). The runner never does I/O (LLP 1016), so a run is
  deterministic, and an agent can drive and replay it (LLP 1012).
- **Numbers by construction.** No app JavaScript runs before first pixel
  (`rules/RULES.md:43, 54-57`). Constant resources are baked. Views are
  native. A plan links only the capabilities it uses (LLP 1047).
- **Language-neutral.** `exact_data::Mixed` routes each source name to Rust or
  TypeScript (LLP 1027 D8). A source can move between them without touching
  the Contract.
- **Optimistic UI falls out.** A send's answer lands before the action's
  writes settle, so an overlay in the source shows a like at once
  (LLP 1054 §7).

### R1 — The data layer becomes a view-model layer

Contract has no loops, no list indexing and no formatting (LLP 1027 §2;
LLP 1054 L5). So everything a view shows arrives from the source already
shaped for display:

- Bluesky's `Post` has 38 fields. Among them are `time: "5m"`,
  `likes: "1.2K"`, the layout flags `lineAbove`/`lineBelow`/`depth`, and a
  `role` that reuses `Post` for header, tab, loading, empty, error and end
  rows (`shapes.contract`).
- The Markdown, LLP, Exact Live and Markdown-stress apps carry a `kind`
  string on one flat `Block` record (`apps/markdown/app.contract:47-56`).

Every change to how a row looks touches data code, and every time-dependent
label forces a re-answer (R2).

### R2 — Cost scales as refreshed × size × frequency, by default

A resource is pulled whole. The runner asks it again when an argument
changes, when it is forced, or when a store it read changes revision
(LLP 1016; `runner/src/runner/settlement.rs:382-405`). Three habits multiply
that:

1. **Time as an argument.** Bluesky passes a minute clock to eight list
   resources so their labels stay current. Its `unread` and `hasNew` also
   take it, but those use it as a poll key, which is legitimate
   (`data/src/reads.rs:262-289, 651-663`). Caltrain, the v1 app, passes
   `nowMs` to `northBoard` and `southBoard`, which re-asks both every second
   (`apps/caltrain/app.contract:63-71`).
2. **Broad refreshes.** Bluesky's one `mutation change` serves like, repost,
   follow, delete, seen and markRead. It declares nine refreshes
   (`app.contract:58`), so a like re-reads all nine at the send and forces all
   nine at the reply (LLP 1054.000.000 D1). RealWorld's `change` serves six
   different writes and refreshes four (`apps/realworld/app.contract:120, 158-185`).
3. **Unbounded lists.** A feed is one resource whose answer is every page
   loaded so far. Bounded answers exist (LLP 1027.004), but only
   `apps/messages-stress` uses them.

For a Rust source this is mostly invisible. The runner keeps an equivalent
answer's object, and `Conformed` re-checks only changed list items
(`settlement.rs:557-563`, `runner/src/conform.rs`). Bluesky's crate builds
fresh records on every answer (`data/src/model.rs:509`), so even that
shortcut is lost. For a TypeScript source every answer is a JSON crossing
and a full decode. The current cost on a Mac is about 1.4 + 0.7 ms per
1,000 synthetic rows (LLP 1027.003 F5, the "path only" and "direct decode"
rows). That is affordable once, but not nine times per tap, or per tick,
over a long scroll.

### R3 — Authoring a source

- **Rust.** Records were built by position until `contract rust`
  (LLP 1054.000 R4). Multi-step requests are a hand-written continuation
  enum (Bluesky's `Step`, 24 variants, `data/src/lib.rs:39-66`). The port's
  diary: "a TS author would have hit a wall on Apple" (`DIARY.md:46-52`).
- **TypeScript.** `async`/`fetch` (LLP 1027 D1a) and generated types
  (`contract types`), with values checked by field name. It is the paved path
  "for anything with substantial app logic" (LLP 1027, Charlie). The port
  chose Rust only because Hermes wasn't provisioned (LLP 1054 O3).

### R4 — The fixes exist, but nobody meets them first

Declared refreshes (1054.000.000), bounded answers (1027.004), the corpus's
view-side formatting pattern (`contract/corpus/now-screen.contract`, LLP 1004
D4), and `refresh r` are each specified and tested. An author who reads
the README and copies an app meets none of them. RealWorld, the natural
template for an HTTP app, uses whole lists and a shared `change` mutation.
Nothing reports that a resource is re-asked by a timer, or that its answer
keeps growing.

## 2. The principle

The cheap pattern must be the one an author writes first, and an expensive one
must be visible. Concretely:

- A label that depends on time is computed in the view, from a number the
  source returns once.
- A paged list's resource answers a window, and its edge handler says which
  list and where to move.
- A write refreshes what that kind of write changes, and nothing else.
- `contract build` says which resources a timer re-asks. The agent's `state`
  says how often each resource was asked and how big its answer is.

## 3. The plan

Each item names its specification. Items P1–P3 are small exact2 changes.
P4 and P5 are authoring patterns with no runtime change, demonstrated in
apps. P6 is the visibility half.

### P1 — Formatting in the view (LLP 1054.000.003)

`formatDate`, `formatTime`, `formatRelativeTime` and `calendarDiff`, plus
`formatNumber` and `formatDuration`, are one **linked capability**, `format`
(LLP 1047 D1–D7). A plan that calls none of them links none of their code.
`formatClockTime` leaves the core. Sources return `createdAt: number` and
`likeCount: number`, and the view formats them. Clock ticks then re-evaluate
text, not resources.

The reviews showed that relative counting alone works as a Contract `fn`
today. The roster earns its place for the calendar-dependent parts
(`"Sep 3"` versus `"Sep 3, 2024"`, weekdays, local days), for number
grouping, and for one Intl-checked body instead of a copy per call site.
.003's revision trims the styles to fixtures.

### P2 — `trim` (LLP 1054.000.005)

One core roster entry, with JavaScript's whitespace set. The fixture: Bluesky's
send and post buttons are enabled on a draft of spaces.

### P3 — Lists say which edge and how far (LLP 1054.000.006)

`reachstart`, `reachend` and `refresh` take bound arguments, evaluated at
dispatch. The plan format doesn't change. This is the piece that makes
bounded answers (P4) practical for more than one list.

### P4 — Bounded feeds as the pattern

No runtime change: LLP 1027.004 is implemented. What changes is the
default an author meets:

- **The authoring notes** (the README's data-source section, LLP 1027's
  authoring notes) show a paged list as a window argument plus
  `reachend=more(id, cursor)`, not a growing page count.
- **The Bluesky port's feeds** move to windows: one resource per mounted
  list, not one `list<Feed>`, so a window move re-asks one feed, not all of
  them (the reviews' K×F point).

*Why not make the runner window automatically:* the runner can hide rows it
was given, but the cost is in producing and crossing them. Only the source
can answer less.

### P5 — Refreshes by kind of write

No runtime change. A mutation per kind of write, each declaring what that
write changes:

```
mutation liked as shape Change refreshes homeFeeds, authorFeeds, threads, notes, results, people
mutation seen as shape Change refreshes unread
```

The declaration is still per resource, but now it is per write. A like no
longer refreshes chats. RealWorld demonstrates it in-repo: `favorite`,
`follow`, the comment writes, `publish` and `deleteArticle` each get their
own mutation and refresh list.

LLP 1054.000.004's `touching` (a runtime walk that narrows refreshes to
resources that contain the edited record) is **withdrawn**. Both reviews
found its premise ("the write adds or removes no rows") broken by the very
writes it named. A like adds a row to the viewer's Likes tab and to the
post's liked-by list, and neither list contains the post beforehand. A walk
can't see membership, so it fails as silent staleness. Splitting by kind of
write gets most of the narrowing, with no new syntax and no way to go stale.

### P6 — Make an expensive pattern visible

Two reports. Neither refuses anything.

- **`contract build` names timer-driven resources.** It already prints each
  component's node cost after its summary line (LLP 1054.000 R11,
  `contract/cli/src/map.rs:104`). It adds one line when any resource's
  arguments depend, through derives, on `now()` or on a slot written by a
  timer's action:

  ```
  re-asked by time: northBoard, southBoard (tick, every 1000 ms)
  ```

  The analysis is over the plan's own tables: timers → actions → `writes`,
  derives' dependencies, and resources' argument code. It is informational,
  like the cost line. Caltrain will print it, and correctly so: its boards
  are cheap Rust, and the line lets its author decide.
- **The agent's `state` counts asks.** Each resource gains
  `"asks": N, "rows": R`: how many times its source was asked since boot, and
  the length of its current answer when that is a list (the outermost list
  for a record with one). `bun scripts/agent.mjs web "clock +60000" state`
  then shows what a minute of idle costs, and a tap's cost is the difference
  between two `state` reads. This answers a question from `state`, as
  NOT-DOING's Agent API rule requires (`rules/NOT-DOING.md:295-302`), with no
  ninth operation.

*What P6 is not:* NOT-DOING refuses "contract witness / dataflow" and "perf"
tooling (`:304-307`). The first report is one summary line computed from
tables the compiler already has, not a dataflow explorer. The second is two
counters in an existing reply, not a profiler.

### Not in this plan

- **Tagged unions for heterogeneous rows** (R1's `role` and `kind`). The
  evidence is in four apps. It needs its own LLP, and new syntax faces
  LLP 1035.005 §1's order.
- **Rust source ergonomics.** TypeScript is the paved path. `contract rust`
  already removed positional records.
- **A runner-level entity store or shared buffers.** A second application
  state graph (`rules/NOT-DOING.md:70-71, 217`), measured against JSON in
  LLP 1027.003 §9 and not selected.

## 4. Order of work

1. **P3 and P2**: smallest, independent, no format change for P3.
2. **P6**: the reports first, so P1, P4 and P5 can be shown to work (fewer
   asks, smaller answers).
3. **P1**: the `format` capability, with the one `FORMAT_DIGEST` bump shared
   with P2.
4. **P5 in RealWorld**, driven on the web: a favorite on a feed page asks
   `feed` only.
5. **P4 and P5 in the Bluesky port** (its own repository), then the diary
   and LLP 1054's findings D3, D4, D8 are updated with before and after
   `state` counts.

## 5. Measured before and after

Before and after each step, on the web host with the agent:

- **Caltrain**, idle for 60 s: `northBoard`/`southBoard` asks (expected: 60
  each, unchanged; P6 reports it).
- **RealWorld**, one favorite on the home feed: asks per resource (P5).
- **Bluesky port**, a like on a long feed (20 pages), and 60 s idle: asks
  and rows per resource (P1, P4, P5).

## 6. Consumers and trades

- P2, P3 and P1's calendar parts have their fixtures in the Bluesky port,
  which is not an admitted consumer (LLP 1054.000). Charlie asked for this
  plan on 2026-09-26, and admitting the port is his call.
- P5 and P6 need no admission: P5 changes no exact2 code, and P6's in-repo
  consumers are Caltrain and RealWorld.
- **Taken off:** `touching` is withdrawn, not added. `formatClockTime`
  leaves the core. No NOT-DOING entry is reversed.

## 7. Open questions for Charlie

1. Admit the Bluesky port as a named consumer for P1–P3?
2. P6's `asks`/`rows` in `state`: acceptable as an answer from `state`, or is
   that the "perf" NOT-DOING refuses?
3. Should Caltrain move its board filtering into the view (LLP 1004 D4's own
   recommendation), or keep re-asking every second as the cheap Rust it is?

## Implementer and date

Claude (Opus 5.5), 2026-09-26 on, after the panel's sanity check (Claude,
Astra, Grok), in the order of §4.
