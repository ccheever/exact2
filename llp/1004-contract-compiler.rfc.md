# LLP 1004: The Contract compiler — Rust, five crates on one table authority

**Type:** RFC
**Status:** Review (super-refine loop 2026-08-28 at Charlie's request, closed the same day at his instruction after two completed rounds — final verdicts bind to r2, both families NOT READY on in-delta findings only, no material pre-existing concern; r3 folds those findings and is unreviewed; see `llp/reviews/1004-contract-compiler.{codex,grok}.md`. Built the same day: LLP 1005 (plan and runner) and LLP 1006 (compiler) transcribe the landing and §7 of 1006 names every deviation from this text; the §4 sequencing is superseded by that landing.)
**Systems:** Contract (compiler), Plan, Runner, Dev loop
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Revised:** 2026-08-28 (r3 — round-2 fold; dispositions in `llp/reviews/1004-contract-compiler.{codex,grok}.md`. D4: app data logic is a Rust data source with arguments from state, a request/response seam, and build-time evaluation of constant resources; the 0517 alternative costed per branch. D5: dev reload is a full teardown and reset — no state migration, no generations. D3: the refusal tuple binds every table the plan encodes. §4: the NOT-DOING trade for this loop recorded as Charlie's obligation.) 2026-08-28 (r2 — round-1 fold: kernel-schema edge, roster/encoder owners, call-site inventory, "the v1 app".)
**Related:** LLP 1000 (the map; lane order), LLP 1001 (kernel v1 — `schema.json` and `SCHEMA_DIGEST`, the vocabulary the compiler emits), LLP 1002 (the RFC/build/spec shape this lane follows), LLP 0508 (Contract v1 Edition 1 — the language basis; research), LLP 0485 (the flat plan — the output's shape; research), LLP 0517 (the wasm host interface and its TypeScript provider seam — the alternative D4 rejects; research), LLP 0518 (the three island lanes — D4 takes Lane 2's shape; research), LLP 0500 / LLP 0553 (state-preserving reload — not taken in v1, §2 D5; research), LLP 0542 (resident checker; research), `rules/RULES.md` §Time budgets, `rules/NOT-DOING.md` §Process, §Runtime, §Authoring models

## Summary

The Contract compiler turns a `.contract` source into the plan the runner
executes: parse → infer closed types → analyze dependencies and effects →
lower to tables and bytecode, byte-identically. It runs at build time and in
the dev loop, never on a device. This RFC decides the six things that shape
the lane before anyone writes it — the language it is written in, its crate
structure, the table authorities it shares, what it admits at the JavaScript
boundary, its budget, and how it is held to a language that will keep changing
— because one of them (D4) also gates the runner, which is the next lane. It
decides nothing about the compiler's internals; those are the spec's, written
when the code exists.

## 1. Why these decisions come first

Three facts from the 2026-08-28 census of exact1 at `a8e633907` (method in
the Appendix; the numbers motivate, they do not bind):

1. **The compiler and the runner are two ends of one format.** exact1 kept the
   plan format in a JSON schema inside its TS compiler package and generated
   readers for TS and Rust — the Rust one 13,294 lines, committed. That package
   was ~220K lines, of which the compiler proper (parser, inference, analysis,
   lowering) was ~22K; the rest was a second runtime and other subsystems.
2. **The v1 app's logic lives in JavaScript.** The Caltrain Contract twin (237
   lines) imports twenty helpers from `data.ts` and calls them 33 times, from
   derives and from view expressions (Appendix). 0508 admits import-rooted
   calls (§6.6); this RFC does not, and what replaces them is the lane's
   first-order decision.
3. **Hot-reload speed is design, not language.** The 100 ms edit → present
   budget is decided by the levers in D5; raw compile time for a file this size
   is milliseconds in either language.

## 2. Decisions

**D1 — Rust.** The compiler is Rust, in the workspace, sharing the toolchain
with the kernel, motion, and runner. Chosen for one build, one set of
generators, one determinism story, and exhaustive `match` over the AST — a
new construct fails to compile until every pass handles it. Not chosen for
speed (§1 fact 3). The known cost is slower iteration on the compiler itself,
bounded by the 30 s crate-rebuild budget. Editor support is a later lane.

**D2 — Five compiler crates on one shared table crate; cargo checks
acyclicity, review checks layering.** `exact-plan` is created by the runner
lane and owns the plan format: `plan/tables/format.json`, `opcodes.json`, and
`stdlib.json` (the enumerated roster: each entry's name, signature, and
opcode), with the encoder and decoder both generated from them at build,
never committed (the kernel's `schema.json`/`build.rs` pattern). `exact-plan`
depends on nothing and declares no kernel vocabulary: plan rows carry the
kernel's ordinals as numbers, and the crates that give them meaning —
`contract-lower` when emitting, the runner when applying — take the
vocabulary from `exact-kernel`, LLP 1001 §1's one authority. Nothing is
redeclared.

| Crate | Owns | Depends on |
| --- | --- | --- |
| `contract-syntax` | lexer, parser, AST, source spans | nothing |
| `contract-types` | closed-type inference; stdlib signatures come from the roster | `contract-syntax`, `exact-plan` |
| `contract-analyze` | dependency DAG, effect signatures, `writes` checking, region structure | `contract-types` |
| `contract-lower` | tables and bytecode, byte-identically, in the kernel's vocabulary; build-time evaluation of constant resources (D4) | `contract-analyze`, `exact-plan`, `exact-kernel`, the app's data crate |
| `contract` | the driver: CLI, resident dev-loop server, diagnostics rendering | all of the above |

What cargo enforces is that this graph has no cycle. It does not stop a
sideways edge (`exact-plan` → `contract-syntax` would be acyclic); layering
is a `Cargo.toml` edit a reviewer sees, and the 1,500-line file cap keeps each
crate legible. No new apparatus.

**D3 — The language basis is Contract v1 Edition 1 (LLP 0508), scoped to
the v1 app and owned here.** Decided: exactly these constructs —
`state`/`derive`/`action`/`task`/`resource`, components with `props`,
`when`/`match`/keyed `each`, closed inferred types, `Option`-only absence,
actions as reducers with `writes` and 0508 §10.3's closed command vocabulary,
the stdlib roster of D2. 0508 is research: the implementation spec
transcribes the semantics it adopts for these constructs and names each
deviation; its edition machinery, flags, and migration rules are not
imported. Pre-1.0 there is no compatibility. A plan's header carries every
authority it encodes — `exact-plan`'s table digest (format, opcodes, roster,
one domain-separated digest over the three canonical tables), the kernel's
`SCHEMA_DIGEST`, and the compiler identity (crate version plus configuration
digest); a runner refuses a plan unless all three match its own, and a
changed opcode or ordinal can never keep old bytes meaningful. Nothing is
migrated.

**D4 — The JavaScript boundary: expressions call the roster; data comes from
a Rust data source; nothing else crosses.** No `use … from "*.ts"`; no call
from an expression into JavaScript. In its place, four fates for app logic:

- **A `resource` names a data source and its arguments.** `resource board =
  boards(station.id, "north") as shape DirectionBoard` (0508 §9's form): the
  arguments are expressions over state; the source is **app code in Rust** —
  the app's data crate (`data.rs`: the schedule model, search, nearest,
  boards, lookup), one implementation, linked into every host through the
  runner's one typed request/response seam (`DataSource::query(name, args) →
  bytes`, `shape`-validated at the seam, refused fail-closed). The runner
  re-requests when arguments change; a value arrives as a settlement event
  (0508 §7.1). This is 0518 Lane 2's provider shape, admitted deliberately:
  it is the one place computation enters, it is Rust, and it is the app's.
  It is not a Rust UI root and needs no ABI generator (`rules/NOT-DOING.md`
  §Authoring models): one trait, implemented by the app.
- **Constant resources are compiled data.** `contract-lower` links the same
  data crate and evaluates every resource whose arguments are constant at
  build time — the station list, the seeded location, the boards for the
  initial state — into the plan's data segment, so the first frame needs no
  host and no seam.
- **Formatting is a roster entry** (`formatClockTime`, countdown minutes,
  distance and walk text), implemented once in Rust, evaluated by the VM,
  added by fixture (D6). Filtering "upcoming" departures against `nowMs` is
  in-language (`each` with `when`), so a ticking clock never re-queries.
- **Ambient host state is a command.** `setScheme` is a `capability-call`.

*Alternatives costed.* (i) **LLP 0517's TypeScript provider seam** (0518
Lane 1) keeps `data.ts`. Its web branch (`wasm-page-js`) has no native
counterpart, so native needs either its Hermes branch — a JavaScript engine in
every native host, not on the doing-list, plus a baked first frame and a
staleness window whose staged-reload machinery NOT-DOING excludes — or a
second implementation of every helper. Rejected on that fork, not on
`RULES.md`'s first-pixel rule, which a baked frame satisfies literally.
(ii) **Schedule math in Contract:** honest, but grows the language (sorting,
geodistance) before the app. Chosen: a Rust data source plus the roster —
one implementation, boot-safe, and the plan side sees only data. The Appendix
maps all twenty helpers to the four fates; the map is the check on "small."

**D5 — The budget is edit → present; the compiler owns one slice; a reload
is a restart.** `RULES.md`'s row is "dev restart, request to present, 100 ms
p50," and v1 takes it literally: an edit yields a new plan; the runner tears
the old one down — in-flight tasks and requests cancelled, late settlements
dropped — and boots the new one from initial state. No state migration, no
patch format, no generations, no identity matching: `rules/NOT-DOING.md`
§Runtime excludes hot revision surfaces and staged reload, and v1 needs
neither. State-preserving reload (LLP 0500/0553's shape) is a later trade
against that list, never a silent extension of this one. The compiler's
slice — source change observed → plan ready — is allocated **≤ 20 ms p50**
for an edit that does not change exported types or component interfaces (one
that does recompiles the invalidated importer closure and is measured
separately); an initial allocation, bound when the resident driver exists and
measured on the v1 app. The levers: **resident** (one long-lived process; no
per-edit spawn — LLP 0542's shape) and **incremental** (recompile the changed
module and its invalidated closure). Whether restart-to-present fits the
remaining ~80 ms is the web host lane's first measurement, not this RFC's
claim.

**D6 — The corpus is how the language evolves.** `contract/corpus/` holds,
per construct, an accept fixture with its expected plan, and per diagnostic
id a reject fixture; where constructs compose (`each` inside `match`, a
`writes` list against a derive), an interaction fixture. Comparison is on
canonical bytes, so byte-identity is tested, not asserted. Every accept
fixture and every roster entry also runs on the runner with an observable
expectation (exported tree, state, or command), so a construct is proven at
both ends, not only lowered. Adding a construct touches syntax, types,
analysis, lowering, and the corpus; a construct with no fixture does not
exist. Diagnostics carry a stable id per rule, a span, and — where mechanical
— a fix. The runner lane's hand-built plans are the corpus's first expected
outputs, so the compiler's exit, evaluated once that corpus exists, is
"compiling the v1 app yields the tables the runner already runs."

## 3. Not decided here

- Parsing strategy, inference algorithm, incremental-cache design, bytecode
  encoding — the spec's, when built.
- The editor experience (LSP, formatter) — its own lane.
- Roster entries beyond the v1 app's — grown by fixture, never by speculation.
- Whether the compiler also runs in the browser as wasm — plausible under D1,
  unneeded until the web host exists.
- **Which app is the v1 app.** `rules/NOT-DOING.md` leaves it open with
  Caltrain recommended. Naming it, and re-running the Appendix's census on
  it, is a prerequisite of the runner lane; this RFC's scope, roster, budget,
  and exit bind to whichever is named.

## 4. Sequencing

After the runner (which creates `exact-plan` and fixes its tables against
hand-built plans) and the web host (which makes edit → present measurable on
the surface the loop runs on), before the Apple and Linux hosts — LLP 1000's
order as revised 2026-08-28. The lane starts when an implementer and a date
are on the spec. Charlie authorized this document ahead of its lane, and the
review loop on it, because D4 is consumed by the runner lane, which is next.
**Obligation (Charlie, before Acceptance):** `rules/NOT-DOING.md` §Process
forbids refine loops and its move-off rule asks for a recorded trade; either
record the trade (the proposed line: loops only for a document whose lane has
an implementer and a date, capped at three rounds) or record this loop as a
one-off exception. This document cannot edit a rules file.

## 5. Costs

- No TS reader; the web host's thin TS seam to the wasm runner consumes
  generated bindings from the same tables and is glue, not a reader.
- D4 makes the first Caltrain port a rewrite of `data.ts` (202 lines) into a
  Rust data crate of about that size, four roster entries, and one command
  (Appendix); the compiler links that crate.
- D5 gives up state-preserving reload in v1; a developer five screens deep
  returns to the first screen on every edit.
- Rust exhaustiveness taxes every language change with edits across four
  crates. That is the guardrail working.

## Appendix — the Caltrain census (exact1 `a8e633907`, 2026-08-28)

`js/src/caltrain-contract/app.contract`: 237 lines, using `state`, `derive`,
`action`, `task` (one: `every(1000, tick)`), components with `props`,
`when`/`else`, `match`, keyed `each`; `resource` is new under D4. Its `use …
from "./data.ts"` list is the twenty names below, call sites by `grep -o`.
`data.ts`: 202 lines. Package counts: `packages/exact-contract/src` ≈ 220K
non-test lines (`compiler/parser.ts` 4,280; `closed-types/inference.ts`
4,517; `analyze.ts` 9,330; `plan/lower.ts` 4,252);
`contract-native/src/flatplan/format_reader.generated.rs` 13,294.

| Fate under D4 | Helpers (call sites) |
| --- | --- |
| Data source, constant arguments → compiled data | `getStations` (1), `getDefaultUserLocation` (1) |
| Data source, arguments from state → requested at runtime (and compiled for the initial state) | `nearestStationIds` (1), `getSelectedStation` (2), `getDirectionBoard` (2), `getTrainRunById` (1), `searchStations` (1) |
| Roster entries | `formatClockTime` (4), `formatCountdownMinutes` (2), `distanceSummary` (1), `walkSummary` (1) |
| Already Contract (`match`, ternaries, style literals, subtraction) | `caltrainColors` (1), `boardSurface` (1), `boardAccent` (1), `directionTitle` (1), `getServiceColor` (4), `serviceBadge` (2), `unstyledButton` (3), `elapsedMilliseconds` (1) |
| Command | `setScheme` (2) |

## Ratification note

Drafted 2026-08-28 at Charlie's request after the plan-runner assessment;
r2 and r3 fold rounds 1 and 2 of the review loop. Draft-equivalent until he
confirms D1–D6 and discharges §4's obligation; D4 gates the runner lane, so
it is the one to confirm first.
