# LLP 1016: Asynchronous data settlement — a resource that answers later

**Type:** RFC
**Status:** Accepted (Charlie, 2026-08-30: §4's question answered — A in the third panelist's shape, plus `refresh`; D1–D6 as written)
**Systems:** Runner (settlement, the data seam), Plan (nothing new in the tables; a resource's pending state is runtime), Contract (one expression function; possibly one statement — §4), Web host (the executor is the browser's `fetch`; one export), Apple host (the executor is `ibex2::host`; one entry and one callback), Linux host (deferred: ibex OQ2), Agent API (`clock settle` waits for requests; `state` shows them), Weird Castle (the first consumer)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Revised:** 2026-08-30 (r6 — LLP 1018 amends D1 as built: `answer` and `parse` take a `Store` the host filled before boot, and a write rides the commit out as a `store` op — the runner still touches no platform; §5's token item is resolved there.) 2026-08-30 (r4 — Accepted: the decision on §4 recorded; the working set swaps the two research links for this document.) 2026-08-30 (r3 — a third panelist, Claude Fable 5, joined at Charlie's request; §4 "The panel" rewritten for three; D1 notes the forbidden-header parity hazard; D2's cache rule is per request, not global; D5: a forgotten ticket never holds `clock settle`, an executor may abort, nothing is undone; the recommendation is A in Fable's shape — `mutation`/`send`, `option<T>`, no `else`, no stubs — plus `refresh`.) 2026-08-30 (r2 — the panel's fold: §4 "The panel" with the recommendation; D3 keep-previous-value named Exact policy; D4 promises no executor timeout; D5 says what a forgotten ticket does not undo and that a mutation's request is never take-latest; D6 beside CORS, not instead; the HTTP-cache rule in D2.)
**Related:** LLP 1005 §5 (settlement: transactional, args-keyed re-request), §7 (the seam "synchronous in v1; a settlement event in asynchronous shape is reserved, not built"), §8; LLP 1004 D4 (app data logic is a Rust data source with arguments from state — a request/response seam); LLP 1006 §1 (`resource name = source(args) as shape T`); LLP 1007 §4 (the glue, `command` ops); LLP 1008 §4 (`exact.h`), §5; LLP 1012 §2 (the clock, `settle`); `QUEUE.md` §Later (ibex2, Charlie 2026-08-29: "`ibex2::host` at the first `resource` that needs bytes from outside the process — build it together with the runner's asynchronous data settlement"); ibex LLP 0068 (the standard library for a Rust consumer: `Host`, `endow`, `Fetch::send`, synchronous by design, §4 "Exact 2"), LLP 0067 (grants); Weird Castle `llp/0000` §Authentication and session boundary (`loginV2`), its `app.contract` and `data/src/lib.rs` (the stand-in this replaces); `rules/NOT-DOING.md` §Process (this document is written because someone is about to build it), §Runtime

## Summary

Every value an app sees today came from a synchronous call: `DataSource::query`
answers before `settle` returns, at bake and at every action. That is what
made the first frame compiled data and the runner free of threads, hosts, and
timers. The first resource that needs bytes from outside the process — Weird
Castle's login, `loginV2` at `api.castle.xyz` — cannot answer that way, on any
host, and least of all on the web, where nothing blocks.

This RFC proposes the reserved shape and decides it: **the runner never does
I/O.** A data source answers *now* or hands back a *request*; the host executes
the request — the browser's `fetch` on the web, `ibex2::host` on Apple — and
delivers the response on the runner's thread through one entry point; the data
source parses it, pure; the runner settles again as if an action had run. While
the answer is out, the resource keeps the value it had and the app can ask
`pending(resource)`; a failed request is *data* the source shapes, never a
runner error. Bake stays synchronous: a source that would hand back a request
at build time is a build error, so the first frame is still compiled data.

How a **mutation** is written in Contract — Weird Castle's first request is
one — was the question this RFC put to a panel of three outside models and
then to Charlie (§4). **Decided (Charlie, 2026-08-30): A** — `mutation name
as shape T` declares an `option<T>` slot, `none` at boot, that `send name =
source(args)` fills from an action; `refresh name` re-requests a query whose
arguments did not change; no `else`, no idle stubs.

## 1. Where this sits

LLP 1004 D4 put app data logic in a Rust data crate with arguments from state
and a request/response seam, and LLP 1005 §7 built the seam synchronous with
the asynchronous shape reserved. The ibex2 decision (2026-08-29) named the
trigger and the executor: the first out-of-process resource, through
`ibex2::host` on native, built together with this. Weird Castle (its
`llp/0000`; restarted on exact2 2026-08-30) is that resource: `POST
https://api.castle.xyz/graphql` with `mutation { loginV2(usernameOrEmail,
password) { userId username token } }`, then `X-Auth-Token` on every call
after (`me`, `logout`, decks). Its data crate is a stand-in today — the shape
of `loginV2`'s answer, produced in-process — so the form, the screens, and the
agent's drive are real on three hosts and only the server is not.

What holds this design to the ground:

- **The web is the standard.** On the web the browser is the only executor
  there is: a request is `fetch`, the answer arrives on the main thread as an
  event, nothing blocks. Whatever the runner's model is, it must be the shape
  the web already has — and then the Apple host does what the browser does.
- **The first frame is compiled data** (LLP 1004 D4, LLP 1005 §5). Bake
  evaluates every resource at build time. Nothing in this RFC may make a build
  reach the network, or make the first frame wait.
- **Settlement is transactional** (LLP 1005 §5). An answer arriving later is a
  second commit, not a hole in the first.
- **No threads, no host, no timers in the runner** (LLP 1005 §7). The runner
  builds for `wasm32-unknown-unknown`; `ibex2` does not.
- **The data crate is pure and testable** (D4's point): a test drives the app
  with a fake transport and never touches a network.

## 2. Decisions

### D1 — A data source answers now, or hands back a request

```rust
pub enum Answer { Now(Value), Later(Request) }

pub struct Request  { pub method: String, pub url: String, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
pub struct Response { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
pub enum Outcome { Response(Response), Failed { kind: FailureKind, message: String } }   // no connection, TLS, timeout, refused by grant

pub trait DataSource {
    /// Answer a resource's request — now, or with a request the host will run.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Answer, DataError>;
    /// The value of a resource from what the host brought back. Pure: no I/O, no host.
    fn parse(&mut self, source: &str, args: &[Value], outcome: Outcome) -> Result<Value, DataError> {
        let _ = (source, args, outcome);
        Err(DataError::UnknownSource(source.into()))
    }
}
```

**Browser module extension (Codex, 2026-09-07):** the implemented `Request`
also has `continuation: Option<u64>`. `None` is the HTTP shape above;
`Request::continuation(token)` is an executor-local microtask turn, never a
network URL. It uses the same `Later` / ticket / `fulfill` lifecycle, and a
settlement may issue another request (the implemented `parse` returns `Answer`).
Module-authored HTTP JSON cannot set this field. Apple/Linux refuse this kind
as `Unsupported`; Hermes drains its microtasks synchronously instead.

`Request` and `Response` are the runner's own two structs, the fields of
ibex2's `stdlib::fetch::{Request, Response}` (LLP 0068 §1) minus what a plan
runner does not decide (redirect mode; the final URL). They are not ibex2's
types because the runner builds for wasm and depends on nothing (LLP 1005
§7); the Apple host converts, one line each way. Two parity facts the data crate must know
(the panel, r1/r3): the browser's `fetch` silently drops forbidden request
headers (`Cookie`, `Host`, `Origin`, …) where ibex2 sends them — a source
must not rely on them; and a request the browser refuses (CORS) arrives as a
rejected `fetch` — `Failed`, status 0 by the glue's convention — while
`Refused` is only ever the app's own grant check (D6). `DataError` from `query` or
`parse` means what it means today — the source is wrong about its own contract
(unknown source, bad arguments, an answer it cannot shape) — and is still
`RunnerError::Data`. A network that failed is not that (D4).

### D2 — The host executes, and delivers on the runner's thread

A `Later` answer during settlement leaves the resource **pending** with a
ticket, and the batch that settlement produces carries one op per new request:

```json
{"op":"request","ticket":7,"resource":"session","method":"POST","url":"https://api.castle.xyz/graphql","headers":[["content-type","application/json"]],"body":"<base64>"}
```

The host runs it with the executor it has, and brings the outcome back:

- **Web** (`glue.js`): `fetch(url, {method, headers, body})`; on settle,
  `exact_fulfill(ticket, status, len)` with the body in the input buffer (a
  failure is `status 0` and a message — the glue's convention for a rejected
  `fetch`, not Fetch's response model). The browser is the executor and the
  authority (CORS); the glue holds the grant list too (D6) so the refusal is
  the same on both hosts. An ordinary request keeps the browser's HTTP-cache
  semantics (the web's default; never a global `no-store`); a request the
  app forced (§4's `refresh`) goes with `cache: "reload"`; a source marks a
  mutation-adjacent read `cache-control: no-cache` itself, as a web app does;
  and a mutation is a `POST`, which the cache never serves — so a re-requested
  balance is never a cached one (the panel, r2/r3).
- **Apple** (`exact-apple`, Rust): the host crate owns one `ibex2::host::Host`
  and the app's `Bindings` (D6), and a worker thread; a `request` op never
  reaches Swift. The worker calls `Fetch::send`, stores the outcome, and calls
  the **wake callback** `exact_boot` now takes beside the measure callback —
  from the worker thread, carrying nothing. The presenter hops to the main
  thread (`DispatchQueue.main`) and calls `exact_pump(now_ms)`, which delivers
  every stored outcome to the runner and returns the batch. One wake per
  outcome; a pump with nothing stored returns an empty batch.
- **Linux**: the same host-crate shape, on ibex2's Linux transport when it
  exists (ibex OQ2: the development TCP transport speaks no TLS). Until then a
  `request` on Linux fails with `FailureKind::Unsupported` — data, not a
  crash (D4).

Both entries end in the runner's one method: `Runner::fulfill(ticket, outcome,
now_ms) -> Receipt`. A ticket the runner no longer holds (D5) is dropped with a
journal line, not an error.

**Browser continuation execution (Codex, 2026-09-07):** `op: "continue"`
carries a ticket and a realm-local token. The loader serializes each realm's
turns, drains microtasks to a MessageChannel task checkpoint, and returns a
typed step through ordinary fulfillment. The glue tracks this work alongside
HTTP in `inflight`, so agent `clock settle` waits for both. Fetch still emits a
real grant-checked HTTP request; continuation turns do not bypass that path.
The host incarnation guard and disposal discard late completion after reload.

The alternative — the data source blocks on a worker thread and the runner
waits — was rejected because it cannot exist on the web, and the web is the
standard.

**Browser redirect restriction (2026-09-24):** all app fetches, including early
module GETs, use `redirect: "error"`. Fetch's manual mode returns an opaque
redirect without its Location, so the browser host cannot re-admit the next
origin before sending a request. Same-origin redirects are refused too; apps
must request the final URL. Native executors retain per-hop grant admission.
Ordered browser responses use the native 64 MiB streaming ceiling; independent
requests retain their declared smaller limit.

### D3 — Pending: the resource keeps its value, and the app can ask

While a request is out, the resource's value is the one it had — the compiled
boot value, or its last settled answer — so every read site keeps its type
and nothing that works today changes. Contract gains one stdlib function,
`pending(resource) → bool`, usable in expressions (`when pending(session)`), and
the runner exposes the set (`Runner::pending()`; the agent's `state` lists it).

This is Exact's policy, not a browser default (`fetch` retains no
application value; SWR and React Query keep `data` beside a parallel
`isValidating`/`isFetching` flag, configurably): the page keeps showing what
it has until the next thing arrives, and `pending(x)` is that flag. `option<T>`
for every resource that might be remote was rejected: it retypes every read site of every resource
that ever gains a remote source (`station.name` today), and it conflates "not
yet" with "none", which `match` would then have to tell apart with a second
signal anyway.

Bake is unchanged and synchronous: `contract::bake` runs `query` for every
resource's boot arguments and a `Later` there is a build error naming the
resource — *the first frame is compiled data* stays a build-time fact. A
resource whose boot value is inherently remote declares boot arguments for
which the source answers `Now` (Weird Castle: `attempt = 0` answers the idle
session). Whether the language should carry an explicit boot value instead
(`as shape T else <expr>`) is §4's fourth question for the panel; it is not
needed for the first consumer.

### D4 — Failure is data

`Outcome::Failed` reaches `parse` like a response does; the source decides
what the app sees, in the resource's own shape (Weird Castle: `error:
"Couldn't reach Castle"`). A non-2xx status is a `Response` with that status —
`parse` reads it. The runner never turns a network outcome into
`RunnerError`; `DataError` from `parse` is reserved for the source being wrong
about its shape, as today. No timeout is promised: the browser's `fetch` has
none, and ibex2's transport is the platform's; whatever ends a request without
a response arrives as `Failed`, and a timeout policy, if one is ever wanted,
is a later decision for both executors at once.

### D5 — One request per resource; the newest arguments win

A resource has at most one request in flight. When settlement finds a pending
resource whose arguments changed, the old ticket is **forgotten** (its outcome
is dropped on arrival, D2) and a new request goes out. Forgetting is display
policy for a query — the newest arguments are what the view means — and it
undoes nothing on the wire: a `POST` that was sent was sent, and an
executor that can abort (the web's `AbortController`) may do so
opportunistically without that being a guarantee of anything. A forgotten
ticket is not in flight for `clock settle` (LLP 1012 §2) — it never holds the
agent. What the newest-wins rule means for a mutation is §4's. Fulfilment is a
settlement pass like an action's: the resource takes its value, derives
recompute, resources whose arguments changed because of it request — so
`resource me = me(session.token)` follows a login by itself — and the pass is
transactional (LLP 1005 §5). Fulfilment runs no action; there is no callback.

A reload (`boot_carrying`, LLP 1005 §6) drops every ticket; the carried
arguments re-request what has no compiled value. The agent's `clock settle`
(LLP 1012 §2) waits for in-flight requests as it waits for motion and timers,
so a scripted drive against a fake transport is deterministic and one against
the network says how long it took.

### D6 — Grants: the app names the hosts it may reach

`ibex2::host` endows bindings from a `GrantSet` (LLP 0067; 0068 §1) and
refuses the rest. A `net.fetch` grant is an **origin** — scheme, host, port —
matched whole, never a prefix (as built: the web glue matches `URL.origin`
the same way). The app's data crate declares its grants as one constant —
`pub const GRANTS: &str = "net.fetch https://api.castle.xyz\n";` — that the
`host!` macros hand to `Host::endow` on Apple and that `glue.js` receives at
boot (a string list) and checks before `fetch`, so a request outside the grant
fails identically everywhere, as `Failed { kind: Refused }`. The grant is a
boundary beside CORS, not instead of it: the browser still enforces its own
(and can send some cross-origin requests whose responses it will not let the
page read); the grant is what the app declared it would reach. Exact has no
manifest yet; when it has one (LLP 0068 §4 says the same grammar), the
constant becomes a section of it. A grant in `app.contract` itself was
considered and deferred: the language does not name hosts today, and a
capability is the host crate's business, not the view's.

**As built (2026-08-30, runner and compiler):** the seam is `query` (now,
required — bake and every in-process source) beside `answer` (default:
`query` now; a source that reaches outside overrides it) and `parse`, so no
existing source changed; the source is named at each `send`, not on the
declaration (Fable's `logout`), so the `Send` opcode carries it and the
`mutations` row is name, slot, `T`; the assignment-forgets rule runs before
settlement so `pending()` is right in the same commit. **The hosts, the
agent, and the first consumer, the same day:** the web executor is the
browser's `fetch` in `glue.js` with `grants`/`request` ops and
`exact_fulfill` (LLP 1007 §4); the Apple executor is `ibex2::host` on a
thread inside `exact-apple`, with a wake callback and `exact_pump` (LLP 1008
§4); `clock settle` waits on requests and `state` lists them (LLP 1012 §2);
Weird Castle's data crate sends `loginV2` and `logout` to
`api.castle.xyz`, parses the reply, and declares `net.fetch
https://api.castle.xyz` — a wrong password from Chrome, the macOS app, and
the iOS simulator comes back as Castle's own refusal in `session.error`.
The Linux executor followed the same day (LLP 1015 §1; ibex OQ2 resolved
with a rustls transport), and the token across launches is LLP 1018.

## 3. What changes, where

| Where | Change |
|---|---|
| `runner/src/runner.rs` | `Answer`, `Request`, `Response`, `Outcome`, `parse`; a resource state gains `pending: Option<Ticket>`; settlement leaves pending resources and emits requests; `Runner::fulfill`; `Runner::pending()`; the journal lines |
| `plan/tables/format.json` | nothing — no new table; `pending` is runtime state |
| `contract/…` | the stdlib function `pending(resource)` in the roster (analyze: its argument is a resource name; lower: one op) |
| `host/web/src/{abi,batch}.rs`, `glue.js` | the `request` op; `exact_fulfill`; the grant list at boot; the fetch |
| `host/apple/src/{abi,batch}.rs`, `include/exact.h`, `swift/Bridge.swift`, both presenters | `ibex2` (`default-features = false`) in `exact-apple`; the executor thread; the wake callback in `exact_boot`; `exact_pump` |
| `scripts/agent.mjs` | `clock settle` waits for requests; `state` shows `pending` |
| `contract/cli` (bake) | `Later` at bake is a build error naming the resource |
| Weird Castle `data/` | `login` answers `Later` with the GraphQL request when `attempt > 0`; `parse` reads `loginV2`; `GRANTS` |
| Tests | a fake transport in the runner's and the app's tests; the web smoke against a stub Castle served by `dev.mjs` on loopback |

Sizes, by the code that exists: runner ~250 lines; each host ~150; the
Contract function ~40; Weird Castle's data crate ~120. LLP 1005 §5/§7/§8,
1006 §1, 1007 §4, 1008 §4/§5, 1012 §1/§2 are amended as built.

## 4. The open question: how a mutation is written

A resource is a **query**: `resource x = source(args) as shape T`, evaluated
from state, re-requested only when the arguments change, memoized, bakeable.
A login is a **command** with an effect on the server. Four ways to write it:

**P — a resource whose arguments are the submitted state** (the proposal; what
Weird Castle does today against the stand-in). An action writes
`submittedWho`, `submittedPassword`, and `attempt = attempt + 1`; the resource
`login(submittedWho, submittedPassword, attempt)` requests because its
arguments changed; the answer is the resource's value; logout resets the
arguments. No new language. It chains (`me(session.token)`). The counter is
the wart: it exists only so that a retry with the same credentials is a new
request. Its result is memoized like a query's — which also means a login
survives a dev reload for free.

**A — an explicit effect statement in actions.** `login(who, password) into
session` inside an action: the action *sends*; `session` is a declared
resource-like slot the answer lands in, with the same pending/failure shape as
D3/D4. A retry is another run of the action. New: a statement, a plan table
for effects, runner state for effects not keyed by arguments, and — if an
effect is allowed to *continue* (`… then onLogin`) — a way for an action to
run on completion with a payload, which today's actions cannot branch on
(LLP 1006: assignment and command statements only). Elm's `Cmd`/`Msg`,
roughly.

**B — a `refresh` statement.** Keep P's resource, drop its counter: `refresh
session` in an action re-requests that resource with its current arguments
whether or not they changed (SWR's `mutate()`). One statement, no new table
(a bit on the action), no callbacks; the query model intact; the wart gone.
The mutation is still spelled as "state the arguments, then refresh".

**C — commands with reply events.** The existing `command` op (LLP 1005 §3)
already leaves the runner for the host; a reply would come back as an event
that runs a named action with the payload. The host is the executor either
way. This is A in host clothing, and it puts the effect's contract in the
host rather than the data crate — against D4 of LLP 1004.

The questions for the panel, then Charlie:

1. P, A, B, C, or something else — for Weird Castle's login *and* for the
   next mutation (buying bricks: LLP 0004's Stripe checkout), which is the
   real test. Which keeps one mental model (state → resources → view) and
   which will the app author reach for correctly without reading a spec?
2. Does D3 hold — a pending resource keeps its value — or should pending be
   visible in the type? (Weigh the read-site retyping cost against the
   "stale error shown during a retry" cost.)
3. Should the language carry a boot value (`as shape T else <expr>`) now, or
   is "boot arguments the source answers `Now`" enough until a resource is
   inherently remote at boot (the decks list will be)?
4. Anything in D1–D6 that the web's own model contradicts.

### The panel (2026-08-30; `llp/reviews/1016-async-data-settlement.{codex,grok,fable}.md`)

Three panelists. codex (`gpt-5.6-sol`, ultra) and grok (`grok-4.6`, xhigh)
took positions blind, then each answered the other; Claude Fable 5 joined at
Charlie's request, took its position blind to both, then answered both while
the first two answered it. Nobody moved on the spelling: codex **A**, Fable
**A** (in a sharper form), grok **B**. Everybody moved on the ground.

**Agreed by all three.**

- `attempt` goes. The submitted snapshot is real — it is the form's request
  body — but a counter is a cache-buster for a memoized GET.
- A mutation is an *occurrence*: two submits are two requests, and a `POST`
  must never fire because its arguments exist. grok conceded that its round-1
  checkout on the live token would re-`POST` on a token change, and that the
  action must snapshot token and pack.
- `refresh <resource>` is needed regardless of the spelling: after the
  Stripe return, `bricks(token)` is the same key it was and nothing in LLP
  1005 §5 can make it fresh. codex called this the strongest thing said.
  Invalidation is not the mutation. Coalescing costs nothing: `settle` runs
  once per action, so an argument change and a `refresh` in the same action
  are one request.
- Pending stays out of a resource's type (D3); `when pending(session)` hides
  the stale error. C is out (LLP 1004 D4). No completion callback and no
  `then`: the reply is data, the view routes on it, resources chain
  (`me(token)`), and the Stripe URL is a link the user taps — Safari's popup
  blocker refuses a `window.open` outside a user gesture anyway.
- The narrowings now in D1–D6: keep-previous is Exact policy, not a browser
  default; no timeout is promised; a forgotten ticket undoes no `POST` and
  never holds `clock settle`; grants sit beside CORS; ordinary requests keep
  the HTTP cache and a forced one bypasses it; forbidden headers and
  CORS-as-rejection are parity facts for the data crate.
- `as shape T else <expr>` waits (grok, Fable; codex dissents): a placeholder
  that is then requested is a third lifecycle against D3, and LLP 1006 §2 has
  no record or list literal to write the expression with. `none` answers the
  question for mutations; bake answers it for queries; the first
  no-argument resource that is remote at boot reopens it.

**Seen by the third panelist and conceded by the other two.** Under P or B,
`boot_carrying` keeps a resource only while its carried value still conforms
and then `settle(false)` re-queries the rest (`runner/src/runner.rs`): editing
`shape Session` while signed in — the ordinary dev-loop moment — re-`POST`s
the credentials on every save, and editing `shape Checkout` creates a Stripe
session per save. The browser's rule is the opposite ("Confirm Form
Resubmission", POST-redirect-GET). And under P or B the `POST` body lives in
slots for the session's life — `submittedPassword` cannot be cleared after
success without changing the arguments and logging the user out, so a
password sits in `state`, in the agent's `state` dump, and in the carry.

**Still disagreed — the spelling of the `POST` itself.**

- **A, two panelists.** Fable's form: `mutation session as shape Session`
  declares a slot of type `option<Session>`, `none` at boot; `send session =
  login(who, password)` in an action snapshots the arguments and makes the
  one request through the same `query`/`parse` (D1–D4 unchanged; a `Now`
  writes the slot inside the action's commit, so the stand-in keeps
  working); fulfilment is D5's pass; a reload carries the slot and never
  re-sends; nothing runs at bake. codex's form keeps the cell a `T` with an
  idle boot value (`else`, or an idle resource) and rejects `option<T>` as
  transport state in the value type. Their case: every re-request rule the
  runner has — arguments changed, carry, shape-miss, newest-wins — is a GET
  rule, and the runner cannot see the method (D1 builds the `Request` inside
  `query`, after the runner has decided to send); `mutation` is the one fact
  it needs. B needs four disciplines (copy the live fields; snapshot the
  token; never clear the submitted slots; never edit the shape while signed
  in), and `send` makes all four impossible.
- **B, one panelist**, refined to answer the above: keep login and checkout
  as resources of *submitted* snapshots, add `refresh`, coalesce, and write
  D5 so that a request whose method is `POST` is **never replayed** — on
  reload, shape-miss, focus, or any later retry rule — using `Request.method`
  as the audit. PRG without a keyword; one authoring model (NOT-DOING). Its
  case against A: a second way to fill a `T`, a `send` opcode beside
  `Command`, every remote value classified by the author, and `option<T>` on
  the cell is the retyping D3 refused (three `match` derives in the login).
  Its objection to Fable's `logout` — `send session = logout(token)` then
  `session = none` on one slot — is real under D5 as written.

**Recommendation (the orchestrator's, for Charlie): A in Fable's shape,
plus `refresh`.**

- `mutation name as shape T` — a slot, `option<T>`, `none` at boot, never
  queried at bake, carried across a reload like any slot, never re-sent.
- `send name = source(args)` in an action: `query(source, args)` builds the
  request (or answers `Now`); `parse` reads the answer; one ticket per slot;
  the newest `send` wins on *acceptance* — both requests were sent, as two
  `fetch`es are, and `pending(name)` is how the view disables the button. An
  **assignment to a mutation slot forgets its ticket in flight** — that is
  Fable's `logout`: the `POST` goes, the local session drops now, the late
  reply is dropped. (Refusing a `send` while one is pending was considered;
  newest-wins is D5's rule already and the web's behaviour.)
- `refresh name` re-requests a *resource* with its current arguments,
  coalesced with an argument change in the same transaction, sent with
  `cache: "reload"`.
- No `else`, no idle stubs. Reads of a mutation are `match` (or roster
  helpers); the retyping is on a new slot, not on `station.name`.

Why A over B′: B′'s exception list is the argument against it. Carry,
shape-miss, focus, retry — each is a rule the runner has or will have, and
each needs a `POST` carve-out; a shape-miss on a `POST` resource then needs a
value it cannot request, so it needs *another* rule; and the runner learns the
method only after `query` has built the request, so the audit runs after the
decision it audits. A is one declaration whose meaning *is* the carve-out,
and it removes the password from `state`. The cost grok names is real — a
second declaration form and a `send` opcode — and it buys the one guarantee
that matters when the next mutation is a ledger debit (LLP 0004): this does
not run unless the action says so. The web tie-break, stated honestly:
`fetch` is one function; the platform splits by method everywhere it matters
(the cache, form resubmission, RFC 9110's safe methods), and every mainstream
data library with a reactive query construct keeps its mutation apart from
it. If Charlie prefers B′, it is B′ with the method rule, the snapshot rule,
and the shape-miss rule written into LLP 1005 §5 as normative text — and the
password stays in state.

**Decision (Charlie, 2026-08-30): A, as recommended.** Q2: pending stays out
of a resource's type; a mutation's reply is `option<T>`. Q3: no `else`. Q4:
the narrowings folded into D1–D6. The build follows §3 plus: a `mutation`
slot kind and `send`/`refresh` statements in Contract (LLP 1006), their
opcodes in the plan (LLP 1005), the runner's mutation state beside its
resource state. `0486` and `0491` leave `llp/current` for this document
(Charlie, the same day).

## 5. Not in this RFC

`then`/completion callbacks and actions branching on a payload (both
panelists: not needed for login or checkout; the view routes on the cell);
streaming and sockets (the relay in Weird Castle `llp/0002` is later);
caching headers, retries and backoff (the app's `attempt` is the retry);
request cancellation on the wire (D5 forgets, it does not abort); an async
runtime in the runner or in ibex2 (0068 OQ3); the Linux transport (ibex OQ2);
a manifest (LLP 0068 §4 — the grant constant is its seed); secure storage of a token across launches (Weird Castle's next ask — LLP 1018, built the same day: the store, snapshot in and `store` ops out, D1 standing literally); the exact1 data island
(LLP 0518, research).

## 6. Process note

`rules/NOT-DOING.md` §Process says no refine loops and no READY verdicts. This
RFC gets one **panel** at Charlie's request (2026-08-30) — two outside models
argue §4's question, each sees the other's position once, and the fold is
recorded here — not a loop, and no verdict binds. The artifacts are
`llp/reviews/1016-async-data-settlement.{codex,grok}.md`.
