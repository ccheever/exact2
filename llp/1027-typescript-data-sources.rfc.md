# LLP 1027: TypeScript data sources — app logic in TS, behind the one seam

**Type:** RFC
**Status:** Accepted (Charlie, 2026-09-03 — "accept all recs": every §10 leaning and every §8 trade as written, with the rulings recorded inline the same day: Rolldown (Q1), `console` (Q2), 1.8 MB given optional (Q3), the lean VM at bake (Q4), Caltrain stays Rust (Q5), the pure tier as ibex2's Rust bindings (Q9), `fetch` as the normal thing (Q8), iOS numbers not a blocker (Q7). **Implementer: Claude (Fable 5.1). Date: 2026-09-03**, stage 1 begun the same day — §9.)
**Systems:** Runner (a second `DataSource` executor: the app's logic as a TypeScript module, compiled to Hermes bytecode, run by the lean VM), Contract (unchanged above the seam; a generated `app.d.ts` below it), Build (bake compiles TypeScript to bytecode and to page JavaScript, and evaluates constant resources through the same engine), Web host (the browser is the executor; the module loads after first paint through one wasm import), Apple host and Linux host (link `exact-js`, a separate crate, after first pixel), Delivery (LLP 1026's `module` card carries bytecode; the runtime version gains the bytecode version), ibex (what exact2 links from it, and what it never links — Charlie's "split it in two" question, answered as three)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-03
**Related:** LLP 1004 D4 (app computation is a Rust data crate — the costing this document reverses, and the seam it keeps: "expressions call the roster; data comes from a data source; nothing else crosses"; its alternative (i), LLP 0517's TypeScript provider seam, rejected 2026-08-28 "on that fork, not on `RULES.md`'s first-pixel rule"), LLP 1005 §3 (canonical value bytes; `Value::conforms`, the shape check at the seam — value.rs:88), LLP 1006 §2 (what Contract is, scoped — §2 here says what it cannot do, by design), LLP 1016 D1/D2/D6 (the runner never does I/O; a request is a value the host runs; grants — the properties that make a module with no globals safe), LLP 1017.000 P5 ("the first `fn` that wants a loop is evidence for the data crate" — this document is where that evidence goes), LLP 1018 D4/D5 (bake sees an empty store; the token stays below the seam — both kept, in TypeScript), LLP 1023 D5 (`app_id` in the plan header), LLP 1024 D3 (one app artifact after the paint gate — the placement rule D4 copies), LLP 1026 D2/D3/D4/D10/D12 (the executor slot, the bytes-only ABI, pairing, digest identity, Level A/B — this is the second executor in that slot; §7's "A JS engine" refusal is withdrawn here; §10 Q6 and Q7 are answered by construction), LLP 1007 §6 (what a reload carries), the 2026-08-29 ibex2 decision (`QUEUE.md` §Later: `ibex2::host` at the first out-of-process resource — landed, LLP 1016/1018; "the engine only at a measured call site, after v1, as another `DataSource` loaded on demand after first pixel … on the web the browser is the executor, so one module runs under two loaders; design that first" — this is that design), `rules/RULES.md` §Scope (no app JS before first pixel; the boot path executes and compiles nothing; modules ship as bytecode — every one kept, D4/D5) and §Agents, `rules/NOT-DOING.md` §Authoring models (one authoring model for the UI — kept; a second language *below* the seam is the trade §8 names) and §Runtime ("HBC compilation" — moved for the bake, §8), ibex LLP 0057 §5.2 (Exact 2 "only where the plan and Rust are not sufficient" — this names where), LLP 0068 (`ibex2::host`, the no-engine standard library exact2's hosts already link — host/apple/Cargo.toml:27, host/linux/Cargo.toml:30), ibex2 `src/bytecode.rs` (bytecode is version-coupled to the engine; the compiler's identity is in the cache key), `metrics/ibex2-speed.jsonl` (the engine's floor and the 45 ns synchronous host call). External: Hermes (`facebook/hermes` 260318099.0.0-stable, the vanilla build in `~/projects/ibex/ios/Frameworks-vanilla/`, receipt `hermes-input-receipt.json`; `hermesvmlean` is the bytecode-only VM), the App Store's interpreted-code clause (the one Expo Updates lives under; a JIT is impossible on iOS and Hermes has none). Predecessor, research never authority: exact1 LLP 0517 (the wasm host interface and its TypeScript provider seam: `callSync`, one argument envelope, "a provider adapter MUST NOT construct/evaluate source text per call" — the same shape, reached from the other side), LLP 0508 §9 (`resource` in Contract), exact1's `data.ts` (202 lines — what LLP 1004 §4 rewrote into Rust; §3 here says why it can come back).

**Accepted amendment (2026-09-04):** [LLP 1027.000](1027.000-explicit-time-and-randomness.rfc.md)
addresses the stage-3 clock gap and implicit randomness, including bake and
cache semantics. Charlie requested the described fixes on 2026-09-04:
explicit time/seed arguments and refusal of ambient reads replace D1a's
unimplemented clock shadow. The macOS executor implementation and actual
verification are recorded in the child; the browser executor remains unbuilt.

## Summary

Charlie's ruling, 2026-09-03, in his words: *"we want TS support to be
optional but to be the default paved path for anything with substantial
app logic/business logic."* And, the same hour: *"maybe we want the
stdlib from ibex2 (stuff like fetch etc) to be available to rust? So
maybe ibex2 needs to be sort of split into 2 so you can have the rust
stdlib and you can have the Hermes/TS support and choose to include
either of those?"*

This document is the design for the first and the answer to the
second. It changes nothing above the data seam. A Contract app still
declares `resource`, `send`, `mutation`, and `shape`; the runner still
asks one `DataSource` and refuses anything not in the declared shape
(LLP 1004 D4, LLP 1005 §3). What changes is what may sit behind that
seam: **a TypeScript module**, compiled at bake to Hermes bytecode for
native hosts and to plain JavaScript for the web, loaded after first
pixel on every host, run by the lean Hermes VM through a new crate,
`exact-js`, that a host links or does not. The Rust data crate stays,
for compute and for apps that want it, behind the same seam; an app
may have both (D8).

**The seam in TypeScript** is four exports — `appId`, `grants`,
`answer(source, args, store)`, `parse(source, args, outcome, store)` —
with values as JSON and the plan's own shapes directing the conversion
both ways (D1, D2). No `fetch`, no timers, no filesystem: the module
has no globals that reach outside the process, so a request is a value
the host runs under the grants, exactly as in Rust (LLP 1016 D1). That
is not discipline; it is the absence of a binding.

**Measured on this Mac (§5), with a probe that ports Caltrain's data
crate to TypeScript line for line and drives both through the seam:**

| | |
|---|---|
| Answers, all twenty cases including six error paths | **byte-identical** to the native crate |
| The module | 9.8 KB TypeScript → 6.0 KB JavaScript → **11.1 KB bytecode**, in **20 ms** |
| Runtime create · bytecode load (p50 of 20) | **0.32 ms · 0.008 ms** |
| One call through the seam, JSON both ways (p50 of 2000) | **1.2–41 µs**, against 0.04–5.5 µs native: **7.5–29×** |
| The lean engine, linked and stripped, on macOS arm64 | **+1.81 MB** (+785 KB gzipped); wasmi was +1.0 MB (LLP 1026 §5) |
| `tsc --strict` on the module | 0.43 s, clean |

**What it reverses and what it keeps.** LLP 1004 D4 rejected a
TypeScript seam on a three-part bill: an engine in every native host, a
baked first frame, and staleness machinery for values that arrive
late. The second and third have since been built as ordinary features
(bake, LLP 1016's settlement). The remaining item is the engine, and it
is 1.8 MB, once, after first pixel. The 2026-08-29 ruling — "zero app
JS until a measured call site" — is superseded by Charlie's of
2026-09-03; the "design that first" it asked for is D6. Kept, to the
letter: no app JS before first pixel (D4), the boot path compiles
nothing (D5: bytecode, compiled at bake), one authoring model for the
UI (Contract; §7 refuses JS above the seam), and the React door.

**ibex, in three, not two (D9).** The split Charlie asked about already
exists: `ibex2::host` is the Rust standard library with no engine, and
exact2's hosts link it today. What a TypeScript data source needs is
not ibex2's JavaScript runtime — the ESM loader, the `fetch` binding,
the task queue — because the module is a function with no I/O, not a
program. It needs the **engine build**: the lean VM, `hermesc`, and the
receipt that binds them, per platform. So: the standard library
(linked by hosts, today), the engine as a `-sys`-shaped crate (linked
by `exact-js`, proposed), and ibex2's runtime layer on top of the
engine (never linked by exact2). The probe in §11 is the proof: ~100
lines of C++ against the vanilla headers, nothing from ibex2.

This is a proposal with no implementer and no date. §8 names the
trades; §10 asks what only Charlie can answer.

## 1. The ruling, and the question it settles

Three questions were asked on 2026-09-03 in this order, and the
answers are this document's premises:

1. *Can an app write Rust today, or only Contract?* Rust, in three
   places: the data crate (Caltrain's schedule model; Weird Castle's
   `loginV2`, its token book in the store, its account switcher), the
   GPU surfaces (`apps/caltrain/gpu`, wgpu), and native modules (LLP
   1024, Rust and Swift). Contract is the UI and its state machine,
   and §2 lists what it cannot do, on purpose.
2. *Wouldn't substantial logic want TypeScript rather than Rust?* Yes,
   for the reason that matters and not for the one usually given. §3.
3. *Optional, but the default paved path for substantial logic.* The
   ruling. This document does not re-argue it; it builds it, and §8
   records the trades a ruling owes.

What "paved path" means here, concretely: an app is `app.contract`
beside `app.ts`, and that is the whole app until it needs something a
`.ts` cannot be (D8, D10). The Rust crate is the same seam with a
different file, chosen when the logic is hot or when the app is the
runtime's own fixture. Neither is a second UI.

## 2. What Contract cannot do, by design

LLP 1006 §2 says what Contract is. This is the complement, so the
boundary this document draws is visible from both sides. Each is LLP
1004 D4's choice — "computation enters at one place" — not a gap:

- **No list or record literals, and no list operations.** No map,
  filter, sort, reduce, index, or slice. Every list and record comes
  from a resource or a mutation; an `each` under a `when` filters what
  is *shown*, never what is *computed*.
- **Expressions are small.** Arithmetic, comparison, boolean logic,
  ternary, templates, field access, option match, and the roster's
  eleven entries (stdlib.rs:14–56: four formatters, `length`,
  `isEmpty`, `toString`, `floor`, `max`, `min`, `now`). No string
  splitting or case changes, no date parsing, no regular expressions,
  no JSON.
- **Actions branch but never loop:** assignments, `send`, `refresh`,
  capability commands, `if`/`match` (LLP 1017 P2). No `let`, no
  `await`, no second request chained off a reply inside one action.
- **A `fn` is one expression**, inlined, non-recursive (LLP 1017 P5):
  "the first `fn` that wants a loop is evidence for the data crate."
- **Only the root** holds resources, mutations, and tasks; the one
  task form is a repeating timer.
- **No I/O.** Contract names a source; the source decides the request;
  the host runs it under grants (LLP 1016). Contract cannot fetch,
  read a file, touch a secret, or call a platform API.

Everything on that list is what "substantial app logic" is made of.
Under LLP 1004 D4 it lives in Rust. Under this document it lives in
TypeScript by default, behind the same seam, and the list above does
not change by one line: the seam is what keeps Contract small.

## 3. Why TypeScript for logic — the honest version

The argument that is right: **logic that shapes data wants
TypeScript.** Parsing replies, string and date handling, flows, state
machines, JSON in and out — Rust charges the type system and the
borrow checker for code that needed neither. For agents this is
larger still: TypeScript is the language they write best, and a fix
loop that includes borrow-checker rounds is a slower loop. The edit
loop compounds it: `bun build` plus `hermesc` is **20 ms** for the
probe's module (§5) at any size a data module will reach, while a
Rust leaf crate rebuilds in seconds and grows.

The argument that is backwards: **binaries do not get big because of
Rust logic.** The whole Caltrain web app — runner, kernel, data crate,
plan — is 513 KiB of wasm, 215 KiB gzipped (`metrics.mjs`,
2026-09-03). Logic in Rust compiles to roughly the size of its
bytecode. What makes a binary big is the engine, a fixed cost paid
before the first line of TypeScript: **1.81 MB** stripped for the lean
VM (§5). One pays it once, and every app on the paved path gets
TypeScript for it; but it is the engine's cost, not Rust's absence.

The part of "a lot of logic" that still wants Rust: **the hot part.**
Hermes is an interpreter — no JIT anywhere, and none possible on iOS
— and the probe's per-call tax is 7.5–29× on calls that cost
microseconds natively (§5). Search over a large list, geometry,
diffing, anything per-item and per-frame, is Rust's; D8 lets an app
say so per source. The split is by kind of work, not by amount.

So the paved path is TypeScript because of who writes the logic and
how fast the loop runs, with Rust one file away for the same seam when
a source is hot. That is the ruling's shape, and the rest of this
document is its mechanism.

## 4. Design

### D1 — The module: four exports, the seam as JSON, no globals that reach out

An app on the paved path has `app.ts` beside `app.contract`. Its
entry evaluates to one object on `globalThis.exact` (what the bundler
emits for a module's exports; the name is the executor's to look up):

```ts
export const appId = "xyz.castle.weird";                      // LLP 1023 D5
export const grants = "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n";

export function answer(source: string, args: Args, store: Store): Answer { … }
export function parse(source: string, args: Args, outcome: Outcome, store: Store): Answer { … }
```

with the seam's types, generated per app (D5) from the Contract's
`shape` declarations and its `resource`/`send` declarations:

```ts
type Answer = { value: unknown } | { request: Request } | { error: DataError };
type Request = { method: string; url: string; headers: [string, string][]; body: string };
type Outcome = { response: { status: number; headers: [string, string][]; body: string } }
             | { failed: { kind: "Network" | "Refused" | "Unsupported" | "Aborted"; message: string } };
type DataError = { kind: "UnknownSource" | "BadArguments" | "Unavailable"; message: string };
interface Store { get(name: string): string | null; set(name: string, value: string): void; forget(name: string): void; }
```

That is the trait (runner.rs:23–79) with each Rust type given its
JSON spelling; nothing is added and nothing is dropped. `answer`
returns now, or a request, or an error; `parse` turns what the host
brought back into the declared shape, or an error; the store is the
one thing either may write (LLP 1018 D5), and a write outside the
grants is refused by the executor exactly as `Store::set` refuses it
today (store.rs:111). Weird Castle's login, from the probe (§11),
which is the whole shape of a request-answering source:

```ts
case "login": {
  const who = text(args, 0).trim(), password = text(args, 1);
  if (who === "" || password === "") return { value: { ok: false, username: "", error: "Enter a username and a password" } };
  return { request: { method: "POST", url: "https://api.castle.xyz/graphql",
                      headers: [["content-type", "application/json"]],
                      body: JSON.stringify({ query: LOGIN, variables: { who, password } }) } };
}
// …and in parse:
const user = json?.data?.loginV2;
if (user?.token) { store.set("castle.session", …); return { value: { ok: true, username: user.username, error: "" } }; }
return { value: { ok: false, username: "", error: json?.errors?.[0]?.message ?? `Login failed (HTTP ${status})` } };
```

**The module has no I/O because it has no bindings.** The executor
creates a runtime with `eval` closed and installs nothing: no `fetch`,
no timers, no `console` unless D10 admits it, no filesystem, no
`URL`. A module that reaches for one throws a `ReferenceError` at the
call, which the executor reports as `DataError::Unavailable` naming
the source — the LLP 1026 D3 discipline ("not 'the data crate does no
I/O' but 'the data crate *cannot*'"), here by the emptier mechanism of
an empty global object. A request is how it reaches out, and the host
runs it under the grants (LLP 1016 D6) on every host, before any
executor sees it.

**Synchronous, on purpose.** `answer` and `parse` return values, never
promises: there is nothing to await, because I/O is a request the
host runs and the reply comes back through `parse` (LLP 1016 D1). An
`async` seam function returns a `Promise`, which the executor refuses
as `Unavailable("answer returned a promise")` — the same refusal
exact1's LLP 0517 spelled as "a provider adapter MUST NOT …", reached
from the other side. Inside the module, `async` is simply pointless.

**One entry, bundled.** Hermes runs scripts, not ES modules. `app.ts`
may import other `.ts` files and packages; the bake bundles them to
one script (D5). A package is admissible exactly when it bundles to
code with no reaching-out global — the bundler does not know, the
type check does not know, the call does: `ReferenceError`, refused by
name at the first call, which the fixture drives at bake (D5).

### D1a — `fetch` as the normal thing (proposed 2026-09-03, recommended)

Charlie, the same day: *"there is so much training data where authoring
involves calling fetch from ts, etc. maybe we should just make it the
normal thing?"* Yes, and it costs less than D1's purity suggested,
because the host already owns the request machinery:

- **`fetch(url, init)` is a binding that does no I/O.** It checks the
  grants in Rust at the boundary, hands the request to the host's
  ticket-and-fulfill path (LLP 1016 D2 — `exact_fulfill`, abi.rs:296;
  the page's grant-checked `fetch` at glue.js:358–384; ibex2's `fetch`
  on native), and returns a Promise the executor resolves when the host
  fulfills. The runner still never does I/O; every request still
  crosses the one Rust boundary; the agent still sees every request.
- **`answer` may return a value or a `Promise` of one.** A Promise is
  "pending" exactly as `Answer::Later` is; when it settles, the value
  takes D2's shape check. For TypeScript modules `parse` is gone — the
  continuation is the code after `await`. The Rust seam keeps
  `Later`/`parse`; a Rust source has no engine to hold a continuation.
- **Sequential requests** inside one action — which neither Contract
  (§2) nor today's seam can express — become `await` lines.
- **The web** shadows `fetch` with the page's grant-checked path rather
  than the browser's own — which closes D6's globals asymmetry better
  than deleting globals does.
- **The executor gains:** one Promise-returning binding; a `Response`
  with `status`, `headers`, `text()`, `json()`, `arrayBuffer()`;
  draining the microtask queue after each fulfill (the lean VM has the
  queue; ibex2's `fetch.js` binding and task pump are this, written);
  a re-request while an answer is in flight lets the old Promise run
  out and drops its result by ticket, as a stale reply is dropped today.
- **What stays out:** timers, and an ambient clock or random generator.
  LLP 1027.000 (accepted and implemented 2026-09-04) replaces the proposed
  `Date.now` shadow with explicit time/seed arguments and clear refusal of
  ambient reads before module initialization and after asynchronous work.
  `AbortSignal` has its trigger.
- **Bake** treats a constant resource whose `answer` returns a Promise
  as it treats a store-reading one: no compiled value, answered on the
  device after first pixel (D4).

D1's "no bindings" becomes "no binding that reaches *past the host*":
`fetch` reaches the host, which is where it reached before.

**As built (stage 3, 2026-09-03).** The seam from the module's side is
`exact.answer(source, args, store)` returning the value or a Promise of
it and throwing for a refusal (an object with `kind` and `message`);
`fetch`, `Headers`, `Response`, and `store` come from a prelude
(`js/src/prelude.js`, bytecode, evaluated before the module) that
reaches Rust through one host function with four ops — record a
request, store get, set, forget — so a store read is counted and a
write grant-checked by the runner's own `Store`. The executor calls
three prelude functions: `__exact_call(source, argsJson)` (a value, a
refusal, or "pending: drain and settle"), `__exact_settle(id)` (the
value, the refusal, or the ticket of the fetch the answer awaits), and
`__exact_fulfill(ticket, outcomeJson)` (resolves that fetch; the
continuation runs in the microtask drain that follows). **The runner's
`parse` now returns an `Answer`**, so a reply may hand back one more
request on the same target with the same arguments — the runner
enqueues it under a new ticket and commits nothing else — which is how
one `answer` awaits two fetches in a row; every Rust source wraps its
value in `Answer::Now`. `query` (the bake's path) has no store — reads
are empty, writes refused — and a fetching answer is `Unavailable`
there, exactly as `Later` at boot is refused today. Owed: the pure tier
as ibex2 bindings (D10; `URL`, `TextEncoder`, base64, `Headers` from
Rust), and `AbortSignal`. The clock gap was closed by LLP 1027.000 on
2026-09-04 with explicit time/seed arguments and ambient-read refusal.

### D2 — Marshaling is shape-directed, both ways, from the plan's own tables

Values cross as JSON, and the *plan* says what the JSON means: a
`resource … as shape T` and a `send … as shape T` name a type in the
plan's `types` table, whose record rows carry named fields in order
(`plan.fields`, builder.rs:178–191). So the executor converts without
guessing:

| Plan type | JSON out to the module | JSON in from the module |
|---|---|---|
| `number` | a finite number | a finite number; anything else refused |
| `string`, `bool` | as is | as is |
| `option<T>` | `null` or T | `null` or T |
| `list<T>` | an array | an array |
| a record (`shape`) | an object keyed by field name, in declared order | an object; every declared field present, none extra; converted in *declared* order into `Value::Record` |

Arguments go out the same way, typed by the declaration's argument
expressions (the type pass already typed them, LLP 1006 §2). The
result is decoded into a `Value` and then checked by `conforms`
(value.rs:88) exactly as a Rust answer is — the shape check at the
seam is unchanged, and it is the second line of defense behind the
conversion. A malformed reply is `DataError::Unavailable` naming the
source and the field; never a panic (LLP 1005 §2's refusal
discipline).

Why JSON and not a walk over the engine's objects through JSI: the
walk would be faster (the probe's per-call cost is mostly the two
JSON passes, §5), and it is an optimization the executor can make
later behind the same seam. JSON first because the *web* executor
(D6) crosses a wasm boundary where JSON is the only sane shape, and
one marshaling on three hosts is worth more than microseconds on two.
The ABI is versioned by number, as LLP 1026 D3's is; `exact_abi` is
the module's fifth export, set by the generated types file (D5), and a
mismatch is refused by number.

The store crosses as a snapshot in and writes out: the executor hands
`answer` an object over the snapshot (`get`/`set`/`forget` recording
into a list), and after the call applies the recorded writes to the
runner's `Store` — where the grant check lives — so the module cannot
write what the crate could not (LLP 1018 D3). Pure in, pure out, the
LLP 1026 D3 shape.

### D3 — The executor: `exact-js`, a separate crate, the lean VM, and about a hundred lines of C++

`exact-js` implements `DataSource` for a bytecode module: `Module::new(hbc: &'static [u8])`
creates a runtime (`RuntimeConfig` with `eval` off and the microtask
queue on — ibex2's choice, hermes_shim.cc:120–131), evaluates the
bytecode once, reads `appId` and `grants` once, and answers
`query`/`answer`/`parse` through three calls: look up
`exact.answer`, call it with strings, read the string back. The
probe's `shim.cc` (§11) is that executor's C++ in full: `probe_create`,
`probe_load`, `probe_call`, `probe_string`, `probe_free`,
`probe_destroy`. An app's host crate says

```rust
exact_apple::host!(exact_js::Module, PLAN);     // instead of caltrain_data::Caltrain
```

with the module's bytes `include_bytes!`'d beside the plan — the host
macros and `Runner::boot` are generic over `D: DataSource` today and
do not change.

**A crate, never a feature.** `exact-js` depends on `exact-runner`
for the trait and on the engine build (D9) and nothing else; a host
links it or does not (LLP 1000's "another executor" clause; the
optional-capability rule in `CLAUDE.md`; Charlie, 2026-09-03: "JS engine
should be optional" — ruled). It is not a cargo feature
on `exact-runner`, `exact-apple`, or `exact-linux`, so there is no
build matrix, and a Rust-only app's binary contains no engine — the
"optional" half of the ruling, by construction.

**The lean VM.** `libhermesvmlean_a.a` is Hermes without its
compiler: it runs bytecode and refuses source. That is the shipping
shape under `rules/RULES.md` ("modules ship as bytecode, never source
strings transpiled per module at boot"), and it is what §5 measured:
the linked, dead-stripped engine is 1.81 MB against the full VM's
2.79 MB of text. A runtime that cannot compile cannot be handed source
by anyone, which closes the door exact1's Hermes path left open.

**Determinism.** Time and seed inputs are arguments, as for the Rust crate
(`board(id, dir, nowMs)`); LLP 1027.000 enforces that boundary in the existing
macOS executor. Repeated inputs and supplied external outcomes reproduce
answers in the fixture. This does not claim cross-host locale/timezone
parity or execution on the future browser/iOS/Linux TypeScript loaders.

### D4 — After first pixel, on every host, and one frame of `pending`

`rules/RULES.md`: *no app JS before first pixel*, a **[check]** on the
web (`boot.mjs` counts the module graph reachable before first paint)
and a review rule elsewhere. This document keeps it to the letter, and
the way it does so is the one behavioral difference between a
TypeScript source and a Rust one:

- **The first frame is compiled data.** Bake evaluates every resource
  whose arguments are constant and that does not read the store (LLP
  1018 D4, cli/src/lib.rs:450–476), through the module (D5), and
  writes the values into the plan. The first frame needs no executor.
- **The executor is created after the first pixel** — on Apple and
  Linux in the slot the GPU module and native modules use (LLP 1009
  D2, LLP 1024 D3: after the paint gate), on the web by the same
  `requestAnimationFrame` that loads `gpu-glue.js` (glue.js:797–849).
  It costs 0.33 ms (§5).
- **A resource the bake could not answer** — one that reads the store,
  Weird Castle's `remember()` — is `pending` until the executor
  arrives: one frame, under a millisecond on native. LLP 1016 already
  gives the app `pending(x)` and an `option` for exactly this shape;
  no new state appears. A Rust crate answers such a resource
  synchronously at boot today; the TypeScript one answers it a frame
  later. That is the price of the rule, stated.

**As ruled and built (2026-09-03, "ok let's do that").** Building stage 5
showed the frame was not the only cost: a store-reading resource
(Weird Castle's `remember()`) is *answered at boot*, and a runner whose
source is not ready cannot answer it at all — boot would fail. The data
such an answer needs is one store entry and its result is a fifty-byte
record, so the runner now **keeps the answer**: the last fresh value of
every store-reading resource is written beside the app's secrets under
`exact.kept.<resource>` (arguments and value, canonical bytes; never
granted to the app, never counted as a read, never a revision; capped at
8 KB), persisted by the host like any store write. At a boot whose source
is not ready (`DataSource::ready`, false for an unloaded module) the
first frame shows the kept answer if its arguments still match and its
value still fits, else the compiled **empty-store placeholder** the bake
now writes for every reader (`resources.reader`, plan format 4; LLP 1018
D4 amended), and the host calls `Runner::data_ready()` after loading the
engine, which asks every placeholder-shown resource again in one commit.
A returning user sees "signed in" on the first frame; a fresh install sees
the empty-store frame, which is right for it; a keychain cleared underneath
shows the kept answer for one frame and is corrected. No JavaScript before
first pixel, no bend in the rule, and the shape every native app already
uses for cached login state. `js/tests/castle.rs` drives all three.

The alternative — create the engine before boot, since it is 0.3 ms —
was considered and not taken, not because the number is large but because the rule
is a count, not a timer, and the reason it is a count is that the
old repo's 5.47 MB of boot-path JavaScript began as a small number
too. If Charlie wants the frame back, it is a ruling on the rule, not
a change to this design (§10 Q3).

### D5 — Bake: TypeScript to bytecode and to JavaScript, constants through the same engine, types from the Contract

Bake gains three steps, all build-time, all reproducible:

1. **Bundle and strip.** `app.ts` and what it imports → one script,
   by `bun build --format=iife` (the toolchain the probe used; ibex2's
   own `typescript::strip` — Oxc — is the single-file fallback with no
   bundler in the loop; §10 Q1). 6 KB of JavaScript for the probe.
2. **Compile.** `hermesc -O -emit-binary` → `app.hbc`, 11 KB for the
   probe, with the `hermesc` that ships beside the VM it targets (the
   receipt, D9): bytecode is version-coupled to the engine, and a
   mismatch is refused outright (ibex2 bytecode.rs). Steps 1–2: 20 ms.
3. **Evaluate constants.** `contract::bake(plan, exact_js::Module::new(&hbc))`
   — the same function, the same lean VM, the same bytecode the phone
   will run. LLP 1026 §10 Q7 asked whether bake should run the module
   through the interpreter so its boot values are provably the
   module's; for TypeScript there is no other way, so the question
   answers itself.

Two outputs from one source, as LLP 1026 D9 has it: `app.hbc` embedded
in the native binaries and named by the update bundle's `module` card
(D7); `app.js` — step 1's output, unminified — served to the web (D6).

**The types are generated.** `contract` already knows every `shape`,
every resource's argument types, and every `send`'s; bake emits
`app.d.ts` beside the module: the `Args` per source as a
discriminated union, the `Answer` value type per source, the seam's
own types, and the ABI number. `tsc --noEmit --strict` against it
(0.43 s for the probe) is the module's type check, run by bake — part
of `build`, not a sixth check — so a module that answers `board` with
a `Station` is refused before the shape check at the seam ever sees
it. Generated files are built, never committed.

**The fixture is the same one.** The byte-equality test LLP 1026 D3
names as the ABI's fixture becomes this document's: Caltrain's Rust
crate and its TypeScript twin (§11) must answer the same bytes on
every source and every error path — twenty cases today. The Rust
crate stays as the Rust seam's fixture; the twin is the TypeScript
seam's; the test is one test.

**The dev loop.** `dev.mjs` watches `app.ts` beside `app.contract`
(dev.mjs:104's list gains the app's `.ts` files); a save runs steps
1–3 and pushes `{rebuilt}` — a module change, classified as LLP 1026
D4 classifies it: a restart with carry (LLP 1007 §6), resources
restarted. 20 ms of build inside the 100 ms edit-to-present budget.

### D6 — The web: the browser is the executor; one wasm import; the same module under two loaders

**Implementation status (2026-09-04):** this section is the accepted design
for a future executor, not an as-built claim. No `exact-js-web` loader is
present yet. Its guarded module environment must satisfy LLP 1027.000
without changing host-page or iframe-guest Date/Math globals.

The 2026-08-29 note asked for this design first: "on the web the
browser is the executor, so one module runs under two loaders."

The web host's wasm is built with `host!(exact_js_web::Module, PLAN)`:
a `DataSource` whose `answer`/`parse` call **one import** —
`exact_js.call(name, a, b, c) -> (ptr, len)` — that `glue.js`
supplies. glue.js loads `app.js` as a classic script after first paint
(the `loadGpuIfNeeded` pattern, glue.js:816–821), and the import
calls `globalThis.exact[name]` with the strings the wasm wrote into
its input buffer (abi.rs:241 already owns that buffer). The wasm goes
from zero imports (glue.js:827 instantiates it against `{}`) to one
import module with one function; `boot.mjs` counts nothing new,
because `app.js` is a script tag inserted after paint, exactly as the
GPU module is.

Before `app.js` has loaded, the import answers "not yet": the runner
treats it as `Answer::Later` with a ticket the page fulfills when the
script arrives — LLP 1016 D2's machinery, one more kind of request the
page runs — and D4's one frame of `pending` is the same on the web as
on native. No engine is shipped: the browser's is the fastest one
there is, and the JavaScript is the same source `hermesc` compiled,
from step 1 of D5, never a second implementation.

**The web's globals.** D1's "no I/O because no bindings" is the
*engine's* guarantee on native and only a convention in a browser,
where `globalThis.fetch`, `window`, and timers exist whether the
module wants them or not. Two ways to close it, in order of cost. The
bundle wraps the module in a function whose parameters shadow the
reaching-out names (`fetch`, `XMLHttpRequest`, `setTimeout`,
`setInterval`, `window`, `document`, `localStorage`, `WebSocket`), so a
module cannot reach one by accident; a determined author still can
through `globalThis`, and the native bake (D5) and the native smoke are
the enforcement — the web is the dev loop, native is the sweep. The
stronger form is a Worker with those globals deleted in its prelude,
which makes every answer on the web a `Later` fulfilled on the next
tick (the same machinery as the not-yet ticket), at one frame per
answer — the guarantee, if it is ever worth the frame. Shadowing
first; the Worker has its trigger.

Answering LLP 1026 §10 Q6 for this executor: the web loads the module
separately, *because it must* (there is no wasm form of it), and a
`{rebuilt}` of `app.ts` is a restart with carry on the web too —
`glue.js` re-inserts the script under a new URL and reboots the
runner, as it does for a plan change today.

### D7 — Delivery: the module card carries bytecode; the runtime version carries the bytecode version

LLP 1026 D9–D12 apply verbatim with one substitution: the update
bundle's `module` card names `app.hbc` instead of a wasm module —

```json
"module": { "url": "./app.hbc", "sha256": "<hex>", "bytes": 11143, "abi": 1, "bytecode": "hermes-96" }
```

— and the runtime version (LLP 1026 D9: kernel schema, `formatVersion`,
ABI number, native-module roster digest) gains the engine's bytecode
version, because a bytecode file for one Hermes is refused by the
next (ibex2 bytecode.rs), and refusal must happen at the envelope,
before download, not at load.

**Digest identity (LLP 1026 D10) holds without change**: the binary
embeds `app.hbc`; a bundle whose module digest matches runs what is
embedded; a bundle whose digest differs runs the downloaded bytecode.
What differs from the wasm case is that *both* run in the interpreter
— there is no native form of a TypeScript module — so the "interpreted
window" of D10 is always open for TypeScript logic, at the cost §5
measures, and there is nothing to close at the next release.

**Level A and Level B (LLP 1026 D12) restated for the paved path:** a
TypeScript app's binary always links `exact-js` (it is the app's
logic), so a code update costs it no extra binary — Level B is free
where Level A was. A Rust-logic app is Level A with no interpreter, as
before. The one interpreter a phone binary carries is chosen by the
app's language (§8 records what this takes off LLP 1026's list).

Signing, anti-rollback, crash fallback, assets by digest, apply at next
launch, static files, no service: unchanged. The App Store clause is
the same one: interpreted code, downloaded, not changing the app's
primary purpose — the shape Expo Updates has shipped under for years.

### D8 — Rust and TypeScript side by side: one seam, routed by source name

An app may have `app.ts` and a `data/` crate. The bake composes them as
one `DataSource`: `exact_js::Either<Rust, Module>` routes each source
name to whichever declares it, and a name declared by both is refused
at bake by name (`bake-source-twice`). The TypeScript module declares
its sources by the `Args` union the generated types give it — that
is, by answering; a Rust crate declares them by its `match`. `appId`
must agree, or bake refuses (`bake-app-id`). Grants are each side's
own, and the app's grants are the union, checked by the host as today
(the runner sees one string).

This is how "optional" and "paved path" coexist without a mode: the
paved path is `app.ts` alone; the hot source moves to Rust one name at
a time, the fixture (D5) holding both to the same bytes; a Rust-only
app never links the engine (D3). There is no per-platform override
(`rules/NOT-DOING.md` §Authoring models), because the seam is the
same on every host.

### D9 — ibex, in three: the standard library, the engine build, the runtime layer

Charlie's question — should ibex2 be split so the Rust standard
library and the Hermes/TS support can be included independently —
has the answer that the split already exists, plus one more cut.

1. **The standard library, no engine: `ibex2::host`** (ibex LLP 0068).
   `fetch`/`fs`/`env`/`secrets`/`kv` behind grants, `default-features
   = false`, no Hermes linked. exact2's Apple and Linux hosts link
   exactly this today for the request executor and the Keychain
   (host/apple/Cargo.toml:27, host/linux/Cargo.toml:30). This is "the
   stdlib available to Rust," and it is done.
2. **The engine build** — the vanilla Hermes lean VM and `hermesc`,
   per platform, with the receipt that binds bytecode to VM
   (`ios/Frameworks-vanilla/`: `macos-static/libhermesvmlean_a.a`,
   `hermes.xcframework`, `tools/hermes-vanilla/hermesc-macos-arm64`,
   `hermes-input-receipt.json`). Today it is a directory and a path in
   a `build.rs`. **Proposed: a crate** — `hermes-lean-sys` for this
   text — that carries the archives (or fetches them by digest at
   build), the headers, the link lines (build.rs:5–16 of ibex2's, less
   the Objective-C++), and `hermesc`'s path, and that both ibex2's
   engine layer and `exact-js` depend on. Owed for it: a Linux
   `x86_64`/`aarch64` lean build (the Hetzner builders, the Linux host)
   and an iOS device measurement (§10 Q7).
3. **ibex2's runtime layer** — `engine/hermes.rs`, `hermes_shim.cc`,
   the bindings (`fetch.js`, `timers.js`, `url.js`, `esm.js`,
   `harden.js`), the loader, the task queue, the pool — is what a
   JavaScript *program* needs. Refined 2026-09-03 by D1a and D10:
   `exact-js` links the engine adapter, the `fetch` binding and its
   pump, and the pure tier's bindings; it never links the module
   loader, the ESM lowering, `require`, or timers. A data source is
   still a function, not a program; the line is *modules and time*,
   not *bindings*. The probe is the demonstration: ~100
   lines of C++ against the vanilla headers, no ibex2 symbol in the
   binary.

So the "choose either" Charlie wants is: hosts choose (1) — already;
TypeScript apps choose (2) through `exact-js`; nobody in exact2
chooses (3). What could migrate from (3) to the paved path is ibex2's
*pure* tier — `url.js`, `headers.js`, base64, text — published as an
ordinary TypeScript package a module bundles in (D1's rule: no
reaching-out global), never linked (§10 Q6).

### D10 — What the module can and cannot use; `console` as the one binding

- **Language:** Hermes's — ES2015 and most of what followed; no
  `eval`/`Function` (closed at construction); the current lean build does
  include `Intl.DateTimeFormat` (verified 2026-09-04; its default-time calls
  are guarded by LLP 1027.000); `fetch` per D1a; timers never. Whatever is missing is
  missing at the first call, by name, in the fixture the bake drives.
- **The pure tier, as Rust bindings (proposed 2026-09-03, recommended;
  Charlie: "would it make sense to just implement the wintercg spec in
  rust?" — that is ibex2's thesis, LLP 0057, and the pure tier exists:
  `stdlib::url`, `headers`, `base64`, `text`, WPT-tested).** The
  WinterCG minimum minus time and streams: `URL`, `URLSearchParams`,
  `TextEncoder`/`TextDecoder`, `atob`/`btoa`, `Headers`,
  `structuredClone`, `console`, `fetch` (D1a). Bound, not bundled, for
  three reasons: the grant checker and the module must parse a URL with
  *one* parser (ibex LLP 0059.000 §3.4: a partial URL parser is a
  security bug); the Rust is already linked in every native host
  through `ibex2::host`, so a binding is nearly free where a JavaScript
  polyfill is ~100 KB of source; and WPT conformance keeps native
  matched to the browser, which has these built in. Not: `setTimeout`,
  `performance.now` (the clock is the runner's), WebSocket and streams
  (a trigger), `crypto` (deliberately later). Marginal size over the
  lean engine: unmeasured; ibex2's binding bytecode is 28 KB.
- **`console`**, admitted as the one host binding (Charlie, 2026-09-03;
  the alternatives — `fetch`, timers, a clock, secrets, files — are each
  refused by D1's rule, and the pure helpers are §10 Q6): `console.log` into the runner's `logs` (the agent API's,
  LLP 1012), 45 ns a crossing (ibex2's measured synchronous host
  call). It reaches nothing outside the process, so D1's rule holds;
  it is the affordance an agent editing `app.ts` in the cloud will
  reach for first.
- **npm packages:** admissible when they bundle to no reaching-out
  global — a date library yes, a GraphQL client no (it fetches; the
  module *describes* the request instead). The bundler resolves; the
  fixture judges.
- **Size and time:** the module runs under the executor's budgets — a
  wall-clock bound per call and a heap bound on the runtime (Hermes
  `RuntimeConfig` has both), the LLP 1026 D5 fuel-and-limits shape by
  the engine's own means; a call over budget is `Unavailable` and the
  last value stays on screen.
- **Not before first pixel, never in Contract, never touching the tree**
  — §7.

## 5. Measured, 2026-09-03

The probe (§11, in the scratchpad, outside the repo — no apparatus
added): `caltrain.ts`, a port of `apps/caltrain/data/src/lib.rs` source
by source and message by message; `bun build` to one script,
`hermesc -O` to bytecode; a Rust binary that links the **lean** VM
through ~100 lines of C++, loads the bytecode, and drives twenty cases
through `exact.answer` against the native crate in the same process,
comparing `Value::to_bytes` (or `DataError`). This Mac, warm, release,
p50 where repeated.

| | |
|---|---|
| Source → script → bytecode | 9,808 B `.ts` → 6,037 B `.js` → **11,143 B `.hbc`**; 20 ms, three runs |
| `tsc --noEmit --strict --target es2020` | 0.43 s, no errors |
| `hermesc` alone (2026-09-03, Charlie's question): the 6 KB module, process start included | **7 ms** (10 runs, 70 ms) |
| `hermesc` on synthetic bundles, `-O0` / `-O` | 100 KB: 20 / 20 ms · 1 MB: 0.21 / 0.27 s · 5 MB: 1.0 / 1.7 s — about 3–5 MB/s of source; `-O` dead-code-eliminates, so those bytecode sizes are not comparable; the real module is 1.85× its script |
| Runtime create · bytecode load (n=20) | **0.322 ms · 0.008 ms** |
| Cases byte-identical (values) or equal (errors) | **20 / 20** — six sources, three argument sets of `board` (a full day, a terminus, mid-day), three of `search` (one hit, three, all), six error paths with the crate's exact messages |
| `login`: `answer` → request; `parse(200)` → a `Session` record; `parse(Refused)` → the crate's exact refusal text | as designed (D1) |

Per call through the seam, JSON both ways, decoded to `Value` (p50 of
2,000, the native crate on the same cases):

| Source | TypeScript µs | Rust µs | × |
|---|---|---|---|
| `defaultLocation()` | 1.2 | 0.04 | 29 |
| `station("paloalto", loc)` | 2.1 | 0.08 | 25 |
| `search("Palo", loc)` | 3.0 | 0.29 | 10 |
| `nearest(loc, 3)` | 9.6 | 0.79 | 12 |
| `board("mv", "north", noon)` — 26 departures, from a 48-entry day | 40.9 | 5.46 | 7.5 |

The ratio falls as the work grows, because the fixed part is the two
JSON passes and the JSI string crossings (~1 µs), not the interpreter.
Caltrain's worst second is two `board` calls per tick: 82 µs. LLP
1026's wasm module measured 8–10× on the same crate; the two
interpreters are in the same band.

The engine's cost, linked:

| | |
|---|---|
| Probe binary, stripped (lean VM, JSI, the shim, the Rust) | 2,213,856 B |
| Same Rust, no engine referenced, stripped | 403,600 B |
| **The lean engine, dead-stripped, macOS arm64** | **+1,810,256 B** (+784,637 B gzip -9); `__TEXT` +1.70 MB |
| For scale: `libhermesvmlean_a.a` text, arm64 | 2.79 MB (the full `libhermesvm_a.a`: 4.78 MB) |
| For scale: the iOS device slice of `hermes.xcframework` (full VM, debugger variant) | 4.86 MB — not the shipping shape; the lean static link on device is owed (§10 Q7) |
| For scale: LLP 1026's wasmi executor | +1.0 MB |
| For scale: the whole Caltrain web app | 513 KiB wasm, 215 KiB gzip |

Unmeasured, and said so: the per-call number on a phone (Hermes on an
A-series core is slower than on this M5; the ratio to native should
hold, the absolute will not); the lean link on iOS; a module with a
real npm dependency; a store-writing `parse` (the probe's `parse`
reads only).

## 6. Costs and budgets

- **Binary:** +1.8 MB per native binary that links `exact-js` —
  TypeScript apps only; a Rust-only app pays nothing (D3). Against
  `rules/RULES.md`'s budgets it is not on any; against the phone it
  is the one number §10 asks about first, as LLP 1026 did for its 1.0.
- **Boot:** +0.33 ms, after first pixel (D4); the first-frame budget is
  untouched by construction. The one frame of `pending` for a
  store-reading resource is the observable cost.
- **Per call:** microseconds to tens of microseconds, per resource
  change, never per frame (§5).
- **Edit loop:** +20 ms in the bake; the 100 ms edit-to-present budget
  holds with room.
- **Checks:** none added. The type check is bake's; the byte-equality
  fixture is one test in the runner's suite; `boot.mjs` counts what it
  counts.
- **The second language.** The real cost, and the ruling's. Two seams'
  worth of fixtures (one test, two implementations), two toolchains in
  the bake, two places an agent might put logic. D8 keeps it to one
  seam and one routing rule; the paved path keeps the default clear.

## 7. What this refuses, with the trigger to revisit

- **JavaScript in Contract expressions or `fn` bodies.** A second
  evaluator in the plan splits the derive graph, puts JS on the boot
  path, and makes the VM two VMs. Trigger: none; LLP 1017 P5's rule
  ("the first `fn` that wants a loop is evidence for the data crate")
  now points at `app.ts`.
- **JavaScript driving views, the tree, or native modules.** LLP 1026
  D7's boundary: a native module is platform code; the tree is the
  plan's. The module answers questions; it does not touch what is on
  screen. Trigger: none.
- **A JavaScript UI tier.** The React door stays open as
  `rules/NOT-DOING.md` has it — designed against, not built. This
  document makes it neither closer nor farther: the seam is below the
  UI, and a React tier would be above it.
- **ibex2's runtime layer in the process** — the ESM loader, `fetch`,
  timers, the task queue (D9). A data source is a function. Trigger: a
  module that needs to be a program, which is a different thing and
  would be its own document.
- **JavaScript before first pixel**, on any host, by any amount (D4).
  Trigger: a ruling on the rule, not a design.
- **An asynchronous seam** (D1). Trigger: none foreseeable; LLP 1016
  is the asynchrony.
- **Source at runtime.** The lean VM cannot compile; nothing ships
  `.ts` or `.js` to a native host. Trigger: none.
- **Node or bun as the *runtime* executor for bake** — bake runs the
  module under the engine that will run it (D5). The faster-to-set-up
  alternative is §10 Q4.
- **JavaScript on the server** as the seam's other end — LLP 1023 §8
  and LLP 1026 §7 already refuse the proxied seam; the store is on the
  device.

## 8. The trades this RFC owes (Charlie's, before Acceptance)

The rule (`rules/NOT-DOING.md` §Moving something off this list): name
what it unblocks, and take something off the doing-list in the same PR.

- **LLP 1004 D4 "the source is app code in Rust"** → "the source is
  the app's, in TypeScript by default or in Rust, behind one seam."
  Unblocks: substantial app logic in the language people and agents
  write it in, on every host, without a compiler in the loop — the
  ruling. **The take: LLP 1026 D2's wasm data module leaves 1026's
  staging.** With TypeScript as the paved path for logic, over-the-air
  *logic* travels as Hermes bytecode (D7); a Rust data crate is Level A
  — native, no interpreter, updated by binary — and the phone carries
  at most **one** interpreter, chosen by the app's language. wasmi
  returns only if LLP 1026 D6 (surfaces as wasm) is ever built, and
  then for surfaces. One executor slot, one engine per app, not two.
- **`rules/NOT-DOING.md` §Runtime "HBC compilation"** → moved, for
  the *bake* only: `hermesc` runs at build, in the same step that
  compiles `app.contract`; nothing compiles at runtime, which is what
  the line was guarding (the parenthetical about reload is untouched;
  a module change is a restart with carry). Unblocks: D5. Take:
  covered above. **Ruled 2026-09-03: moved.**
- **`rules/NOT-DOING.md` §Authoring models "One authoring model, one
  set of bugs"** → stays, with one added sentence proposed: *"Logic
  below the data seam is TypeScript by default or Rust; nothing runs
  JavaScript above it."* It is the honest statement of the second
  language's cost (§6), written where the cost is accounted. **Ruled
  2026-09-03: OK.**
- **`QUEUE.md` §Later, the ibex2 paragraph** → the engine's trigger
  ("a measured call site, after v1") is replaced by this ruling; the
  paragraph's "design that first" is D6, done. The `ibex2::host`
  half is unchanged and landed.
- **LLP 1026 §7 "A JS engine"** → withdrawn; D2–D3 of 1026 are the
  wasm executor, now for surfaces if ever; this document is the
  data-seam executor. 1026 §10 Q1 (is 1.0 MB acceptable) becomes §10
  Q3 here (is 1.8 MB), and Q6/Q7 are answered by D6/D5.
- **The 2026-08-29 ruling "zero app JS through v1"** → superseded by
  the 2026-09-03 ruling; recorded here so the two dates are one line
  apart.

### 8a. The first trade, at length (Charlie asked, 2026-09-03: "this is a big deal")

**What LLP 1004 D4 fixed, and what it only assumed.** D4 fixed the
*seam*: one place computation enters; arguments from state; a typed
request/response boundary; values refused unless they match the
declared shape; the first frame baked. That is the invariant every
later document leaned on — LLP 1016's settlement, LLP 1018's store,
LLP 1023's identity, LLP 1026's executor slot — and none of them
depend on the language behind the seam. D4 also *assumed* a language:
Rust, because the alternative's bill was an engine in every host plus
two features that did not exist yet. The features now exist; the
engine is 1.8 MB, once, after first pixel, optional. The sentence
changes because its premise did.

**What exact1's failure actually was.** Not JavaScript. It was
JavaScript *as the runtime*: 5.47 MB of ESM parsed at every boot, a
bridge that drove native views from JS, a module system and three
layers of defaults disagreeing under it. Every one of those is still
refused here — nothing before first pixel (D4), nothing touching the
tree (§7), no module system in the process (D9), one Rust runtime with
one set of defaults. What is admitted is JavaScript *as a library to
the Rust runtime*: a function the runner calls through a typed seam,
with no way to reach past it. That is the difference between the old
repo and this one, stated in one sentence, and it is why the ruling
does not reopen the old failure.

**Why the take is 1026's data module.** The rule wants a real
removal. LLP 1026 D2 existed to make *Rust* logic travel over the air
so the cloud loop could change logic without a rebuild. Under the
ruling the logic that changes often is TypeScript, and it travels as
bytecode through the same envelope (D7); Rust logic is what remains
Rust for a reason — hot sources, the runtime's own fixtures — and
changes at the cadence of a binary, which the cloud builds anyway
(1026 D8). Keeping D2 would mean a phone binary with two interpreters,
2.8 MB, for the case of a Rust source that wants an over-the-air
update and will not move to TypeScript. That case exists; its answer
is D8 (move the changing source) or a binary. So: one executor slot,
one engine per app, chosen by the app's language; wasmi is not planned
for data. It stays a possibility for *surfaces* (1026 D6, deferred with
its own trigger), and the one future case with two interpreters —
a TypeScript app with GPU surfaces travelling as wasm — is named here
so it is decided on purpose if it arrives.

**Alternatives for the take, and why not.** Taking the Rust data crate
itself (TypeScript-only) fails the ruling's "optional" and the compute
argument (§3). Taking nothing and calling the engine "replacement
apparatus" is not a removal. Taking LLP 1023's DNS-SD is already 1026's
take. Taking the React door is not a take; it is not on the doing list.

**What the change costs that the numbers do not show.** Two places an
agent may put logic — the seam and the paved-path default keep that to
one habit. The engine as an org dependency: exact2's TypeScript apps
build against a Hermes build the ibex repo produces, pinned by receipt,
vanilla only (D9); upstream cadence and Static Hermes are ibex's to
track. A hot TypeScript source on a phone at 10–30× — D8 and the
budgets. And the web asymmetry D6 names: the no-globals guarantee is
the engine's on native and discipline plus shadowing on the web, with
the bake and the native smoke as the sweep.

**What it buys, concretely.** Weird Castle's 607 lines of Rust become
an `app.ts` of about the size of exact1's `data.ts` (202 lines), with
JSON as JSON, strings as strings, no borrow checker, and an edit loop of
20 ms; its logic updates over the air as a signed bytecode file with
the binary unchanged; an agent in the cloud edits `app.contract` and
`app.ts` and never needs a compiler. Caltrain, all Rust, is untouched
and links no engine. The rules that made exact2 fast are the same
rules, and the check that counts the boot graph reads the same number.

## 9. Staging

Each stage ships alone. **Implementer: Claude (Fable 5.1); stage 1
begun 2026-09-03**, the day of acceptance. The order below is revised
from r1 for D1a: `fetch` and the pure tier are stage 3, before the web
and before Weird Castle, because both need them. Two foundations
surfaced in the first hour of building and are stage 1's: **the plan
carries the seam's signatures** (a `sources` table — name, parameter
types, result type — unified across every `resource` and `send` that
names the source, so the executor names record fields from the plan
and `app.d.ts` is generated from the plan, one authority), and **the
module's `appId` and `grants` are bake outputs** beside the bytecode,
so boot reads them without an engine (the identity gate and the store's
grants are read at boot, runner.rs:440 and :466).

1. **`exact-js`** (D2, D3, D10): the crate, the shim, the
   shape-directed marshaling from `plan.fields`, the budgets, `console`
   if admitted. Caltrain's TypeScript twin (§11) kept as `exact-js`'s own
   test fixture — never an app file: Caltrain stays all Rust and other
   apps carry the TypeScript experiments (Charlie, 2026-09-03) — the
   byte-equality test as the runner's. The Apple and Linux hosts link it after the paint gate;
   `metrics.mjs` gains the engine's create-and-load row. Verified by
   driving it: `smoke.mjs macos` and `linux` green on the twin; a
   module that calls `fetch` refused by name; a looping module refused
   by budget with the last value on screen; a `Promise` refused.
2. **Bake and the dev loop** (D5): bundle, compile, evaluate through
   the engine, `app.d.ts`, the `tsc` step; `dev.mjs` watching `.ts`;
   the pending-one-frame rule (D4). Verified: edit `board` in `app.ts`,
   the macOS window shows it in under 100 ms; a wrong-shaped answer
   refused at bake by `tsc` and, with the check bypassed, at the seam.
3. ~~**`fetch` and the store**~~ — **landed 2026-09-03** with stage 1's
   commit's successor: D1a as built (above), `parse → Answer`, the store
   as host-door ops, `js/tests/castle.rs` (login, a failed fetch, a
   two-fetch chain, refusals before and after a fetch, an answer pending
   on nothing, and the same through a compiled Contract and the runner).
   Owed from it: the pure tier from ibex2. The clock shadow was superseded
   and the ambient-input gap fixed by LLP 1027.000 on 2026-09-04.
4. **The web** (D6): the one import, `app.js` after first paint, the
   not-yet ticket, restart with carry. Verified: `smoke.mjs web` green
   on the twin; `boot.mjs` unchanged at one module; parity of every
   answer with the native hosts through the agent's `state`.
4. **Weird Castle in TypeScript** — the paved path's first real app:
   `loginV2`, `remember`, `accounts`, `switch`, `logout`, `stillLife`,
   the token book in the store (LLP 1018 D5), as `app.ts`; the Rust
   crate kept until the twin answers the same bytes, then deleted
   ("delete; don't deprecate"). This is the call site the 2026-08-29
   ruling asked to be measured; it is measured here as the fixture.
5. **Delivery** (D7): the `module` card with `bytecode`, the runtime
   version's new component, a signed TypeScript update to an installed
   Caltrain twin at the next launch. Verified as LLP 1026 Stage 2/3
   verify, with the bytecode-version mismatch refused at the envelope.
6. **The engine crate** (D9): `hermes-lean-sys` in the ibex repo; Linux
   lean builds; the iOS device link and its numbers into §5.

Deferred, each with its trigger in the text: the JSI walk instead of
JSON (D2 — when a per-call number asks); the pure-tier package (D9 —
when a module wants `URL`); the Rust seam's own bytes ABI for
over-the-air Rust (LLP 1026 D2/D3 — if surfaces-as-wasm is built).

## 10. Open questions (for Charlie)

1. **The bake's TypeScript toolchain:** `bun build` (bundles, strips,
   fast; one more tool on the build machine and the fleet) or ibex2's
   Oxc strip alone (already a Rust dependency; single file, no
   imports, no npm)? Leaning was bun, because "substantial logic" will
   import files. **Charlie leans Oxc (2026-09-03)**, and the shape that
   follows is: one file and no imports first; when a module imports a
   relative file, ibex2's ESM lowering plus `oxc_resolver` concatenated
   into one script with a ten-line `require` shim (small Rust, no second
   runtime on the builders); npm packages are the trigger for more, and
   Rolldown keeps that in Rust. **Ruled 2026-09-03: Rolldown** — the
   Oxc-family bundler, Rust, one step for strip, bundle, and npm
   resolution, emitting one IIFE script lowered to the syntax `hermesc`
   accepts (what it lacks fails at bake, which is the check). Owed:
   whether the Rust crates are consumable from a `build.rs` or the npm
   CLI (node is already required) is the integration.
2. **`console` as the one binding** into `logs` (D10)? Leaning: yes;
   it is 45 ns and reaches nothing. **Ruled 2026-09-03: yes.**
3. **Is +1.8 MB acceptable in release iOS** for a TypeScript app? It
   replaces LLP 1026's 1.0 MB question for the paved path. And the
   one frame of `pending` (D4): acceptable, or is a sub-millisecond
   bytecode load before boot a ruling you want to make on the rule?
   Leaning: accept both; the frame is the rule's honest price.
   **Ruled 2026-09-03: the size is acceptable given the engine is
   optional per app (D3).** The frame stands as designed.
4. **Bake's executor:** the lean VM (D5, one engine everywhere, needs
   the engine crate on every build machine) or `bun` (already there,
   faster to stand up, a second engine whose answers the fixture must
   hold equal)? Leaning: the VM; the fixture would catch a divergence
   but "provably the module's" is worth the setup. **Charlie,
   2026-09-03: "the lean VM seems right."**
5. **Does Caltrain keep its Rust crate** once the twin exists, as the
   Rust seam's fixture (D8's both-sides case), or does it become
   TypeScript-only and Weird Castle carries the Rust fixture? Leaning
   was both. **Ruled 2026-09-03: Caltrain stays all Rust; other apps
   experiment with TypeScript.** The twin is a test fixture only (§9).
6. **The pure tier as a package** (D9): should ibex2's `url.js`,
   `headers.js`, base64, and text helpers become a TypeScript package
   a module can bundle, or does a module bring its own from npm?
   Leaning: leave it to npm until a module asks.
7. **The iOS numbers:** the lean static link on a device and the
   per-call tax on a phone are owed before Acceptance, or is the macOS
   number enough to rule on? Leaning: owed; it is an afternoon with
   `build.mjs --device`. **Charlie, 2026-09-03: not a blocker;**
   measured when a device build happens.
8. **`fetch` as the normal thing** (D1a)? Leaning: yes; the training-data
   argument is Charlie's and it costs one binding and a pump the host
   machinery already implies.
9. **The pure tier as ibex2's Rust bindings rather than bundled JavaScript**
   (D10)? Leaning: yes, for the one-parser reason first.
10. **Naming:** `exact-js`, `hermes-lean-sys`, `Either` — or the names
   the implementer picks.

## Ratification note

Draft r1, 2026-09-03, written the day of the ruling and after the
probe in §5 ran; **accepted the same day** by Charlie with every
recommendation, after the questions of §10 were each argued in
conversation and their answers written inline. Unreviewed by a panel.
The numbers are this Mac's; the iOS numbers are owed and not blocking.
An implementer and a date are named (§9); the trades of §8 land with
stage 1, as the rule says.

## 11. Appendix — the probe, verbatim

In the scratchpad, outside the repo, built against `~/projects/ibex/ios/Frameworks-vanilla/` (the lean VM) and `tools/hermes-vanilla/hermesc-macos-arm64`. `build.sh` makes the bytecode; `cargo build --release` in `hostprobe/` makes the two binaries; `./target/release/hostprobe ../caltrain.hbc` prints §5.

### `caltrain.ts`

```ts
// Caltrain's data source in TypeScript: the same seam as
// apps/caltrain/data/src/lib.rs, source by source, message by message, so
// the probe can compare answers byte for byte. No imports, no I/O: a
// request is a value the host runs (LLP 1016 D1), exactly as in Rust.

type Location = { lat: number; lon: number };
type Station = { id: string; name: string; zone: number; distance: number };
type Departure = { id: string; train: number; service: string; headsign: string; at: number };
type Session = { ok: boolean; username: string; error: string };

type Request = { method: string; url: string; headers: [string, string][]; body: string };
type Outcome =
  | { response: { status: number; headers: string; body: string } }
  | { failed: { kind: "Network" | "Refused" | "Unsupported" | "Aborted"; message: string } };

type Answer =
  | { tag: 0; value: unknown }
  | { tag: 1; request: Request }
  | { tag: 2; kind: "UnknownSource" | "BadArguments" | "Unavailable"; message: string };

class DataError extends Error {
  constructor(public kind: "UnknownSource" | "BadArguments" | "Unavailable", message: string) {
    super(message);
  }
}

type Row = { id: string; name: string; zone: number; lat: number; lon: number; offsetMin: number };

const STATIONS: Row[] = [
  { id: "sf", name: "San Francisco", zone: 1, lat: 37.7765, lon: -122.3947, offsetMin: 0 },
  { id: "22nd", name: "22nd Street", zone: 1, lat: 37.7573, lon: -122.392, offsetMin: 5 },
  { id: "millbrae", name: "Millbrae", zone: 2, lat: 37.6003, lon: -122.3868, offsetMin: 20 },
  { id: "sanmateo", name: "San Mateo", zone: 2, lat: 37.568, lon: -122.324, offsetMin: 27 },
  { id: "redwood", name: "Redwood City", zone: 3, lat: 37.4855, lon: -122.231, offsetMin: 38 },
  { id: "paloalto", name: "Palo Alto", zone: 3, lat: 37.4436, lon: -122.1647, offsetMin: 45 },
  { id: "mv", name: "Mountain View", zone: 4, lat: 37.3947, lon: -122.0763, offsetMin: 52 },
  { id: "sunnyvale", name: "Sunnyvale", zone: 4, lat: 37.3784, lon: -122.0312, offsetMin: 57 },
  { id: "sj", name: "San Jose Diridon", zone: 4, lat: 37.3297, lon: -121.9023, offsetMin: 70 },
];

const DAY_START_MS = 1_787_875_200_000;
const DEFAULT_LOCATION: Location = { lat: 37.3947, lon: -122.0763 };
const SERVICES: [string, number, number][] = [
  ["Local", 5, 1],
  ["Limited", 25, 0.8],
  ["Express", 45, 0.6],
];

function station(id: string): Row | undefined {
  return STATIONS.find((s) => s.id === id);
}

/** Great-circle distance in meters (haversine). */
function distanceM(a: Location, b: Location): number {
  const r = 6_371_000;
  const lat1 = a.lat * (Math.PI / 180);
  const lon1 = a.lon * (Math.PI / 180);
  const lat2 = b.lat * (Math.PI / 180);
  const lon2 = b.lon * (Math.PI / 180);
  const dlat = lat2 - lat1;
  const dlon = lon2 - lon1;
  const s1 = Math.sin(dlat / 2);
  const s2 = Math.sin(dlon / 2);
  const h = s1 * s1 + Math.cos(lat1) * Math.cos(lat2) * s2 * s2;
  return 2 * r * Math.asin(Math.sqrt(h));
}

function stationValue(s: Row, location: Location): Station {
  return { id: s.id, name: s.name, zone: s.zone, distance: Math.round(distanceM(location, s)) };
}

function departures(s: Row, direction: string): Departure[] {
  const terminus = direction === "north" ? STATIONS[0] : STATIONS[STATIONS.length - 1];
  if (terminus.id === s.id) return [];
  const sign = direction === "north" ? -1 : 1;
  const out: [number, Departure][] = [];
  SERVICES.forEach(([service, every, speed], si) => {
    let minute = 5 * 60 + si * 7;
    let n = 0;
    while (minute < 23 * 60) {
      const at = minute + sign * s.offsetMin * speed;
      if (at >= 0 && at < 24 * 60) {
        const train = 100 + si * 100 + n * 2 + (direction === "north" ? 1 : 0);
        const headsign = direction === "north" ? "San Francisco" : "San Jose";
        out.push([at, { id: `${s.id}-${direction}-${train}`, train, service, headsign, at: DAY_START_MS + at * 60_000 }]);
      }
      minute += every * 6;
      n += 1;
    }
  });
  out.sort((a, b) => a[0] - b[0]);
  return out.map(([, d]) => d);
}

// --- argument checks, the crate's, message for message ---------------------

function arity(args: unknown[], expected: number): void {
  if (args.length !== expected) {
    throw new DataError("BadArguments", `expected ${expected} arguments, got ${args.length}`);
  }
}

function loc(args: unknown[], i: number): Location {
  const v = args[i] as Partial<Location> | undefined;
  if (
    v && typeof v === "object" && Object.keys(v).length === 2 &&
    typeof v.lat === "number" && typeof v.lon === "number" &&
    Number.isFinite(v.lat) && Number.isFinite(v.lon) &&
    v.lat >= -90 && v.lat <= 90 && v.lon >= -180 && v.lon <= 180
  ) {
    return { lat: v.lat, lon: v.lon };
  }
  throw new DataError("BadArguments", "location");
}

function text(args: unknown[], i: number): string {
  const v = args[i];
  if (typeof v === "string") return v;
  throw new DataError("BadArguments", `argument ${i}`);
}

function finiteNumber(args: unknown[], i: number): number {
  const v = args[i];
  if (typeof v === "number" && Number.isFinite(v)) return v;
  throw new DataError("BadArguments", `argument ${i}`);
}

// --- the sources -------------------------------------------------------------

function query(source: string, args: unknown[]): unknown {
  switch (source) {
    case "defaultLocation": {
      arity(args, 0);
      return DEFAULT_LOCATION;
    }
    case "stations": {
      arity(args, 1);
      const location = loc(args, 0);
      return STATIONS.map((s) => stationValue(s, location));
    }
    case "nearest": {
      arity(args, 2);
      const location = loc(args, 0);
      const count = finiteNumber(args, 1);
      if (count % 1 !== 0 || count < 0 || count > STATIONS.length) {
        throw new DataError("BadArguments", "count");
      }
      const all = STATIONS.slice();
      all.sort((a, b) => distanceM(location, a) - distanceM(location, b));
      return all.slice(0, count).map((s) => stationValue(s, location));
    }
    case "station": {
      arity(args, 2);
      const id = text(args, 0);
      const location = loc(args, 1);
      const s = station(id);
      if (!s) throw new DataError("Unavailable", `station ${id}`);
      return stationValue(s, location);
    }
    case "board": {
      arity(args, 3);
      const id = text(args, 0);
      const direction = text(args, 1);
      const nowMs = finiteNumber(args, 2);
      if (direction !== "north" && direction !== "south") {
        throw new DataError("BadArguments", "direction");
      }
      const s = station(id);
      if (!s) throw new DataError("Unavailable", `station ${id}`);
      return departures(s, direction).filter((d) => d.at >= nowMs);
    }
    case "search": {
      arity(args, 2);
      const q = text(args, 0).toLowerCase();
      const location = loc(args, 1);
      return STATIONS.filter((s) => s.name.toLowerCase().includes(q)).map((s) => stationValue(s, location));
    }
    default:
      throw new DataError("UnknownSource", source);
  }
}

// A request-shaped source, Weird Castle's login in miniature: `answer` hands
// the host a request; `parse` reads what came back. The token would go to
// the store (LLP 1018 D5), which this probe does not model.
const LOGIN = "mutation Login($who: String!, $password: String!) { loginV2(who: $who, password: $password) { token username } }";

function login(args: unknown[]): Answer {
  arity(args, 2);
  const who = text(args, 0).trim();
  const password = text(args, 1);
  if (who === "" || password === "") {
    return { tag: 0, value: { ok: false, username: "", error: "Enter a username and a password" } satisfies Session };
  }
  return {
    tag: 1,
    request: {
      method: "POST",
      url: "https://api.castle.xyz/graphql",
      headers: [["content-type", "application/json"], ["accept", "application/json"]],
      body: JSON.stringify({ query: LOGIN, variables: { who, password } }),
    },
  };
}

function parseLogin(outcome: Outcome): Session {
  if ("failed" in outcome) {
    const { kind, message } = outcome.failed;
    const error =
      kind === "Refused" ? "Castle is not a host this app may reach"
      : kind === "Unsupported" ? "This host cannot reach Castle yet"
      : `Couldn't reach Castle (${message})`;
    return { ok: false, username: "", error };
  }
  const { status, body } = outcome.response;
  let json: any;
  try { json = JSON.parse(body); } catch { return { ok: false, username: "", error: `Castle answered HTTP ${status} without JSON` }; }
  const user = json?.data?.loginV2;
  if (user && typeof user.token === "string" && user.token !== "") {
    return { ok: true, username: String(user.username ?? ""), error: "" };
  }
  const message = json?.errors?.[0]?.message ?? `Login failed (HTTP ${status})`;
  return { ok: false, username: "", error: String(message) };
}

// --- the seam ------------------------------------------------------------------

function answer(source: string, argsJson: string): string {
  const args = JSON.parse(argsJson) as unknown[];
  try {
    if (source === "login") return JSON.stringify(login(args));
    return JSON.stringify({ tag: 0, value: query(source, args) } satisfies Answer);
  } catch (e) {
    if (e instanceof DataError) return JSON.stringify({ tag: 2, kind: e.kind, message: e.message } satisfies Answer);
    throw e;
  }
}

function parse(source: string, argsJson: string, outcomeJson: string): string {
  const outcome = JSON.parse(outcomeJson) as Outcome;
  if (source === "login") return JSON.stringify({ tag: 0, value: parseLogin(outcome) } satisfies Answer);
  return JSON.stringify({ tag: 2, kind: "UnknownSource", message: source } satisfies Answer);
}

(globalThis as any).exact = { appId: "com.exact.caltrain", grants: "net.fetch https://api.castle.xyz\n", answer, parse };
```

### `build.sh`

```sh
#!/bin/sh
set -e
cd "$(dirname "$0")"
bun build caltrain.ts --outfile caltrain.js --target=browser --format=iife --minify-whitespace >/dev/null
~/projects/ibex/tools/hermes-vanilla/hermesc-macos-arm64 -O -emit-binary -out caltrain.hbc caltrain.js
ls -la caltrain.ts caltrain.js caltrain.hbc
```

### `hostprobe/Cargo.toml`

```toml
[package]
name = "hostprobe"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
exact-runner = { path = "/Users/ccheever/projects/exact2/runner" }
caltrain-data = { path = "/Users/ccheever/projects/exact2/apps/caltrain/data" }
serde_json = "1"

[build-dependencies]
cc = "1"

[patch.crates-io]
taffy = { path = "/Users/ccheever/projects/exact2/vendor/taffy" }

[profile.release]
opt-level = 3
```

### `hostprobe/build.rs`

```rust
fn main() {
    let engine = "/Users/ccheever/projects/ibex/ios/Frameworks-vanilla";
    println!("cargo:rerun-if-changed=shim.cc");
    cc::Build::new()
        .cpp(true)
        .file("shim.cc")
        .include(format!("{engine}/hermes-headers"))
        .flag("-std=c++17")
        .flag("-stdlib=libc++")
        .compile("probe_shim");
    println!("cargo:rustc-link-search=native={engine}/macos-static");
    // The lean VM: bytecode only, no compiler — the shipping shape.
    println!("cargo:rustc-link-lib=static=hermesvmlean_a");
    println!("cargo:rustc-link-lib=static=jsi");
    println!("cargo:rustc-link-lib=static=boost_context");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
```

### `hostprobe/shim.cc`

```cpp
// The smallest executor: a Hermes runtime, one bytecode module, and calls
// into `exact.answer` / `exact.parse` with strings — what an `exact-js`
// DataSource would do per call. No ibex2, no stdlib, no bindings: the
// module has nothing to import.
#include <hermes/hermes.h>
#include <jsi/jsi.h>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

using namespace facebook;

namespace {
class OwnedBytes : public jsi::Buffer {
public:
  explicit OwnedBytes(std::vector<uint8_t> bytes) : bytes_(std::move(bytes)) {}
  size_t size() const override { return bytes_.size(); }
  const uint8_t *data() const override { return bytes_.data(); }
private:
  std::vector<uint8_t> bytes_;
};
struct Probe { std::unique_ptr<facebook::hermes::HermesRuntime> rt; };
char *dup(const std::string &s) {
  char *p = static_cast<char *>(std::malloc(s.size() + 1));
  std::memcpy(p, s.c_str(), s.size() + 1);
  return p;
}
}  // namespace

extern "C" {
void *probe_create() {
  auto config = ::hermes::vm::RuntimeConfig::Builder().withEnableEval(false).withMicrotaskQueue(true).build();
  auto rt = facebook::hermes::makeHermesRuntimeNoThrow(config);
  if (!rt) return nullptr;
  return new Probe{std::move(rt)};
}
int probe_load(void *h, const uint8_t *data, size_t len, char **out) {
  auto *p = static_cast<Probe *>(h);
  try {
    auto buffer = std::make_shared<OwnedBytes>(std::vector<uint8_t>(data, data + len));
    p->rt->evaluateJavaScript(buffer, "app.hbc");
    return 0;
  } catch (const jsi::JSError &e) { *out = dup(e.getMessage()); return 1; }
  catch (const std::exception &e) { *out = dup(e.what()); return 2; }
}
int probe_string(void *h, const char *name, char **out) {
  auto *p = static_cast<Probe *>(h);
  auto &rt = *p->rt;
  try {
    jsi::Object exact = rt.global().getPropertyAsObject(rt, "exact");
    *out = dup(exact.getProperty(rt, name).getString(rt).utf8(rt));
    return 0;
  } catch (const jsi::JSError &e) { *out = dup(e.getMessage()); return 1; }
  catch (const std::exception &e) { *out = dup(e.what()); return 2; }
}
int probe_call(void *h, const char *name, const char *a, const char *b, const char *c, char **out) {
  auto *p = static_cast<Probe *>(h);
  auto &rt = *p->rt;
  try {
    jsi::Object exact = rt.global().getPropertyAsObject(rt, "exact");
    jsi::Function fn = exact.getPropertyAsFunction(rt, name);
    jsi::Value result = c == nullptr
        ? fn.call(rt, jsi::String::createFromUtf8(rt, a), jsi::String::createFromUtf8(rt, b))
        : fn.call(rt, jsi::String::createFromUtf8(rt, a), jsi::String::createFromUtf8(rt, b), jsi::String::createFromUtf8(rt, c));
    *out = dup(result.getString(rt).utf8(rt));
    return 0;
  } catch (const jsi::JSError &e) { *out = dup(e.getMessage()); return 1; }
  catch (const std::exception &e) { *out = dup(e.what()); return 2; }
}
void probe_free(char *p) { std::free(p); }
void probe_destroy(void *h) { delete static_cast<Probe *>(h); }
}
```

### `hostprobe/src/main.rs`

```rust
//! The TypeScript data-source probe: the Caltrain seam under Hermes
//! bytecode, compared byte for byte with the native crate, and timed.
use caltrain_data::{Caltrain, DAY_START_MS, DEFAULT_LOCATION};
use exact_runner::{DataError, DataSource, Value};
use serde_json::{json, Value as Json};
use std::ffi::{c_char, c_void, CStr, CString};
use std::time::Instant;

extern "C" {
    fn probe_create() -> *mut c_void;
    fn probe_load(h: *mut c_void, data: *const u8, len: usize, out: *mut *mut c_char) -> i32;
    fn probe_string(h: *mut c_void, name: *const c_char, out: *mut *mut c_char) -> i32;
    fn probe_call(h: *mut c_void, name: *const c_char, a: *const c_char, b: *const c_char, c: *const c_char, out: *mut *mut c_char) -> i32;
    fn probe_free(p: *mut c_char);
    fn probe_destroy(h: *mut c_void);
}

struct Engine(*mut c_void);

fn take(out: *mut c_char) -> String {
    if out.is_null() { return String::new(); }
    let s = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
    unsafe { probe_free(out) };
    s
}

impl Engine {
    fn new() -> Engine {
        let h = unsafe { probe_create() };
        assert!(!h.is_null(), "no runtime");
        Engine(h)
    }
    fn load(&self, hbc: &[u8]) {
        let mut out: *mut c_char = std::ptr::null_mut();
        let status = unsafe { probe_load(self.0, hbc.as_ptr(), hbc.len(), &mut out) };
        assert_eq!(status, 0, "load: {}", take(out));
    }
    fn string(&self, name: &str) -> String {
        let name = CString::new(name).unwrap();
        let mut out: *mut c_char = std::ptr::null_mut();
        let status = unsafe { probe_string(self.0, name.as_ptr(), &mut out) };
        assert_eq!(status, 0);
        take(out)
    }
    fn call(&self, name: &str, a: &str, b: &str, c: Option<&str>) -> Result<String, String> {
        let name = CString::new(name).unwrap();
        let a = CString::new(a).unwrap();
        let b = CString::new(b).unwrap();
        let c = c.map(|c| CString::new(c).unwrap());
        let mut out: *mut c_char = std::ptr::null_mut();
        let status = unsafe {
            probe_call(self.0, name.as_ptr(), a.as_ptr(), b.as_ptr(), c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()), &mut out)
        };
        let text = take(out);
        if status == 0 { Ok(text) } else { Err(text) }
    }
}
impl Drop for Engine {
    fn drop(&mut self) { unsafe { probe_destroy(self.0) } }
}

/// A declared shape, as the plan's types table would give it to the executor.
enum Shape { Num, Str, Bool, List(Box<Shape>), Rec(Vec<(&'static str, Shape)>) }

fn location_shape() -> Shape { Shape::Rec(vec![("lat", Shape::Num), ("lon", Shape::Num)]) }
fn station_shape() -> Shape { Shape::Rec(vec![("id", Shape::Str), ("name", Shape::Str), ("zone", Shape::Num), ("distance", Shape::Num)]) }
fn departure_shape() -> Shape { Shape::Rec(vec![("id", Shape::Str), ("train", Shape::Num), ("service", Shape::Str), ("headsign", Shape::Str), ("at", Shape::Num)]) }
fn session_shape() -> Shape { Shape::Rec(vec![("ok", Shape::Bool), ("username", Shape::Str), ("error", Shape::Str)]) }

/// JSON → Value, directed by the shape: records by declared field order.
fn from_json(j: &Json, s: &Shape) -> Result<Value, String> {
    Ok(match (s, j) {
        (Shape::Num, Json::Number(n)) => Value::Number(n.as_f64().ok_or("number")?),
        (Shape::Str, Json::String(t)) => Value::str(t),
        (Shape::Bool, Json::Bool(b)) => Value::Bool(*b),
        (Shape::List(inner), Json::Array(items)) => Value::list(items.iter().map(|i| from_json(i, inner)).collect::<Result<_, _>>()?),
        (Shape::Rec(fields), Json::Object(map)) => {
            if map.len() != fields.len() { return Err(format!("record has {} fields, shape has {}", map.len(), fields.len())); }
            Value::record(fields.iter().map(|(name, fs)| map.get(*name).ok_or(format!("missing {name}")).and_then(|v| from_json(v, fs))).collect::<Result<_, _>>()?)
        }
        _ => return Err("shape mismatch".into()),
    })
}

fn decode(text: &str, shape: &Shape) -> Result<Value, DataError> {
    let j: Json = serde_json::from_str(text).expect("the module answers JSON");
    match j["tag"].as_u64() {
        Some(0) => Ok(from_json(&j["value"], shape).unwrap_or_else(|e| panic!("{e}: {text}"))),
        Some(2) => {
            let m = j["message"].as_str().unwrap_or("").to_string();
            Err(match j["kind"].as_str() { Some("UnknownSource") => DataError::UnknownSource(m), Some("Unavailable") => DataError::Unavailable(m), _ => DataError::BadArguments(m) })
        }
        other => panic!("unexpected tag {other:?}: {text}"),
    }
}

fn loc_json(lat: f64, lon: f64) -> Json { json!({"lat": lat, "lon": lon}) }
fn loc_value(lat: f64, lon: f64) -> Value { Value::record(vec![Value::Number(lat), Value::Number(lon)]) }

/// One case: the source, its arguments as the module sees them (JSON) and
/// as the crate sees them (Values), and the declared shape.
struct Case { source: &'static str, args_json: Vec<Json>, args: Vec<Value>, shape: Shape }

fn cases() -> Vec<Case> {
    let (lat, lon) = DEFAULT_LOCATION;
    let noon = DAY_START_MS + 12.0 * 3_600_000.0;
    let list = |s: Shape| Shape::List(Box::new(s));
    vec![
        Case { source: "defaultLocation", args_json: vec![], args: vec![], shape: location_shape() },
        Case { source: "stations", args_json: vec![loc_json(lat, lon)], args: vec![loc_value(lat, lon)], shape: list(station_shape()) },
        Case { source: "nearest", args_json: vec![loc_json(lat, lon), json!(3)], args: vec![loc_value(lat, lon), Value::Number(3.0)], shape: list(station_shape()) },
        Case { source: "nearest", args_json: vec![loc_json(37.7, -122.4), json!(9)], args: vec![loc_value(37.7, -122.4), Value::Number(9.0)], shape: list(station_shape()) },
        Case { source: "station", args_json: vec![json!("paloalto"), loc_json(lat, lon)], args: vec![Value::str("paloalto"), loc_value(lat, lon)], shape: station_shape() },
        Case { source: "board", args_json: vec![json!("mv"), json!("north"), json!(noon)], args: vec![Value::str("mv"), Value::str("north"), Value::Number(noon)], shape: list(departure_shape()) },
        Case { source: "board", args_json: vec![json!("mv"), json!("south"), json!(DAY_START_MS + 7.0 * 3_600_000.0)], args: vec![Value::str("mv"), Value::str("south"), Value::Number(DAY_START_MS + 7.0 * 3_600_000.0)], shape: list(departure_shape()) },
        Case { source: "board", args_json: vec![json!("sf"), json!("north"), json!(noon)], args: vec![Value::str("sf"), Value::str("north"), Value::Number(noon)], shape: list(departure_shape()) },
        Case { source: "board", args_json: vec![json!("sj"), json!("north"), json!(DAY_START_MS)], args: vec![Value::str("sj"), Value::str("north"), Value::Number(DAY_START_MS)], shape: list(departure_shape()) },
        Case { source: "search", args_json: vec![json!("Palo"), loc_json(lat, lon)], args: vec![Value::str("Palo"), loc_value(lat, lon)], shape: list(station_shape()) },
        Case { source: "search", args_json: vec![json!("san"), loc_json(lat, lon)], args: vec![Value::str("san"), loc_value(lat, lon)], shape: list(station_shape()) },
        Case { source: "search", args_json: vec![json!(""), loc_json(lat, lon)], args: vec![Value::str(""), loc_value(lat, lon)], shape: list(station_shape()) },
        // The error paths, message for message.
        Case { source: "nearest", args_json: vec![loc_json(lat, lon), json!(10)], args: vec![loc_value(lat, lon), Value::Number(10.0)], shape: list(station_shape()) },
        Case { source: "nearest", args_json: vec![loc_json(lat, lon), json!(1.5)], args: vec![loc_value(lat, lon), Value::Number(1.5)], shape: list(station_shape()) },
        Case { source: "stations", args_json: vec![], args: vec![], shape: list(station_shape()) },
        Case { source: "stations", args_json: vec![loc_json(91.0, 0.0)], args: vec![loc_value(91.0, 0.0)], shape: list(station_shape()) },
        Case { source: "station", args_json: vec![json!("nowhere"), loc_json(lat, lon)], args: vec![Value::str("nowhere"), loc_value(lat, lon)], shape: station_shape() },
        Case { source: "board", args_json: vec![json!("mv"), json!("east"), json!(noon)], args: vec![Value::str("mv"), Value::str("east"), Value::Number(noon)], shape: list(departure_shape()) },
        Case { source: "board", args_json: vec![json!(7), json!("north"), json!(noon)], args: vec![Value::Number(7.0), Value::str("north"), Value::Number(noon)], shape: list(departure_shape()) },
        Case { source: "bogus", args_json: vec![], args: vec![], shape: location_shape() },
    ]
}

fn median(mut v: Vec<f64>) -> f64 { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] }

fn main() {
    let hbc_path = std::env::args().nth(1).expect("usage: hostprobe <app.hbc>");
    let hbc = std::fs::read(&hbc_path).expect("read hbc");

    // The floor: a runtime and the module, fresh, twenty times.
    let mut creates = vec![]; let mut loads = vec![];
    for _ in 0..20 {
        let t = Instant::now(); let e = Engine::new(); creates.push(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now(); e.load(&hbc); loads.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let engine = Engine::new();
    engine.load(&hbc);
    println!("module   {} bytes of bytecode; app_id {:?}; grants {:?}", hbc.len(), engine.string("appId"), engine.string("grants").trim());
    println!("floor    runtime create p50 {:.3} ms · bytecode load p50 {:.3} ms (n=20)", median(creates), median(loads));

    // Byte equality against the native crate, every case.
    let mut native = Caltrain;
    let mut equal = 0; let mut total = 0;
    for c in cases() {
        let args_json = serde_json::to_string(&c.args_json).unwrap();
        let js = decode(&engine.call("answer", c.source, &args_json, None).expect("call"), &c.shape);
        let rs = native.query(c.source, &c.args);
        let same = match (&js, &rs) { (Ok(a), Ok(b)) => a.to_bytes() == b.to_bytes(), (Err(a), Err(b)) => a == b, _ => false };
        total += 1; if same { equal += 1; }
        let describe = |r: &Result<Value, DataError>| match r { Ok(Value::List(l)) => format!("list of {}", l.len()), Ok(v) => format!("{} bytes", v.to_bytes().len()), Err(e) => format!("{e:?}") };
        println!("  {:<6} {:<16} {:<44} ts={} rs={}", if same { "same" } else { "DIFF" }, c.source, args_json.chars().take(44).collect::<String>(), describe(&js), describe(&rs));
    }
    println!("equal    {equal}/{total} cases byte-identical (values) or equal (errors)");

    // The request-shaped source: answer hands out a request; parse reads the reply.
    let login_args = r#"["alice","hunter2"]"#;
    let r = engine.call("answer", "login", login_args, None).unwrap();
    println!("login    answer → {}", r.chars().take(160).collect::<String>());
    let ok = json!({"response": {"status": 200, "headers": "", "body": json!({"data": {"loginV2": {"token": "t0k", "username": "alice"}}}).to_string()}}).to_string();
    let bad = json!({"failed": {"kind": "Refused", "message": "not granted"}}).to_string();
    let r1 = decode(&engine.call("parse", "login", login_args, Some(&ok)).unwrap(), &session_shape());
    let r2 = decode(&engine.call("parse", "login", login_args, Some(&bad)).unwrap(), &session_shape());
    println!("login    parse(200) → {:?}", r1.map(|v| v.to_bytes().len()));
    println!("login    parse(refused) → {:?}", r2);

    // Timing: per call, p50 over 2000, the module against the crate.
    let (lat, lon) = DEFAULT_LOCATION;
    let noon = DAY_START_MS + 12.0 * 3_600_000.0;
    let timed: Vec<Case> = cases().into_iter().filter(|c| matches!((c.source, c.args_json.len()), ("defaultLocation", 0) | ("station", 2) | ("nearest", 2) | ("board", 3) | ("search", 2))).collect();
    let _ = (lat, lon, noon);
    println!("{:<16} {:<44} {:>10} {:>10} {:>6}", "timing p50", "args", "ts µs", "rust µs", "×");
    let mut seen = std::collections::HashSet::new();
    for c in timed {
        if !seen.insert(c.source) { continue; }
        let args_json = serde_json::to_string(&c.args_json).unwrap();
        let mut ts = vec![]; let mut rs = vec![];
        for _ in 0..2000 {
            let t = Instant::now();
            let text = engine.call("answer", c.source, &args_json, None).unwrap();
            let v = decode(&text, &c.shape);
            ts.push(t.elapsed().as_secs_f64() * 1e6);
            std::hint::black_box(v);
            let t = Instant::now();
            let v = native.query(c.source, &c.args);
            rs.push(t.elapsed().as_secs_f64() * 1e6);
            std::hint::black_box(v);
        }
        let (a, b) = (median(ts), median(rs));
        println!("{:<16} {:<44} {:>10.1} {:>10.2} {:>6.1}", c.source, args_json.chars().take(44).collect::<String>(), a, b, a / b);
    }
}
```

### `hostprobe/src/bin/baseline.rs`

```rust
//! The same Rust, no engine: the size delta between this binary and
//! `hostprobe` is what the linked engine costs.
use caltrain_data::{Caltrain, DEFAULT_LOCATION};
use exact_runner::{DataSource, Value};
fn main() {
    let (lat, lon) = DEFAULT_LOCATION;
    let mut d = Caltrain;
    let v = d.query("nearest", &[Value::record(vec![Value::Number(lat), Value::Number(lon)]), Value::Number(3.0)]).unwrap();
    println!("{}", v.to_bytes().len());
}
```
