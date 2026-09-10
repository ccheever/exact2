# LLP 1005: Plan and runner v1 — what `exact-plan` and `exact-runner` are, as built

**Type:** Spec
**Status:** Draft
**Systems:** Plan, Runner, Kernel (seam), Data seam
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1004 (the decisions this executes), LLP 1006 (the compiler that emits this format), LLP 1001 (the kernel the runner drives), LLP 1002 (the clock discipline the runner shares), LLP 0485 (the flat plan; research)

## Summary

`plan/` is the plan format and `runner/` is the loop that executes it. Together
they are LLP 0485's idea at a tenth of its size: relational tables plus
bytecode, declared once in `plan/tables/format.json`, generated into Rust at
build, validated whole on load, executed by a fixed loop that emits kernel
ops. The runner adds no threads, owns no clock, and links no host: time and
events come in through two methods, kernel ops go out through `Kernel::apply`.
Where this document and the code disagree, the code and its tests are the
authority and this document is stale.

## 1. One declaration authority (`plan/tables/format.json`)

Tables, enum vocabularies, the VM's opcodes with operand layouts, and the
stdlib roster with parameter and return types are declared in one JSON file.
`plan/build.rs` generates, into `OUT_DIR` and never committed: a row struct
and typed id/range per table, the `Plan` container, the canonical encoder,
the validating decoder, `Opcode`/`Operand`, `Stdlib` (with `params()` and
`returns()`), the enums (`TypeKind`, `RegionKind`, `BindingKind`,
`EventKind`), and `FORMAT_DIGEST` (domain-separated SHA-256 of the canonical
JSON, first 8 bytes). The generator fails closed on any table, codec, or
operand it cannot validate.

Codecs: `u8 u16 u32 i32 f64 bool`, `str` (string-pool index), `enum:<Name>`,
`idx:<table>` (validated row index), `opt:<table>` (index or `0xFFFFFFFF`),
`range:<table>` (start + len, validated), `code` (offset + len into the code
pool, validated as well-formed bytecode ending in `Return`), `bytes` (offset +
len into the data pool).

Tables: `types`, `fields`, `slots`, `derives`, `resources`, `args`, `actions`,
`params`, `writes`, `timers`, `regions`, `arms`, `nodes`, `bindings`,
`handlers`. **No kernel vocabulary is declared here**: `nodes.node_type`,
`bindings.id` are the kernel's ordinals as numbers (LLP 1004 D2); the plan
header carries the kernel's `SCHEMA_DIGEST` so a mismatch is refused at boot.
The value bridge consults the row's generated codec before interpreting `auto`:
it becomes a dimension value only for dimension rows, and remains text for enum
rows such as `align-self`, `overscroll-behavior`, and `scrollbar-width`.

## 2. Bytes (`Plan::encode` / `Plan::decode`)

Header: magic `EXPL`, `FORMAT_VERSION` u32, `FORMAT_DIGEST` u64, kernel schema
digest u64, compiler identity u64. Then the three pools (strings as count +
length-prefixed UTF-8; code and data as length + bytes), then every table in
declaration order as count + fixed-width rows. Equal plans encode to equal
bytes (`a_plan_round_trips_and_its_bytes_are_canonical`).

**Loading is a validation pass, never trusted indexing** (0485 §3.4, kept):
`decode` checks magic, version, and digest before any table; bounds every
count before allocating from it (`MAX_COUNT`); then `validate` checks every
`str`/`idx`/`opt`/`range`/`bytes` reference and walks every `code` range
(`check_code`: every opcode known, every operand well-framed, every index
operand in range, every enum operand in vocabulary, `Return` last, **every
jump forward and on an instruction boundary** — so a body always terminates).
`validate_semantics` then checks what codecs cannot: `when`/`match` regions
have exactly two arms and `each` one, every arm is owned by its region, and
no timer has a zero interval. A refusal names table, row, field, and — for
code — the pc (`PlanError::BadCode`, `CodeError`). Trailing bytes are refused.
Reservations from announced counts are capped (`bytes::RESERVE`) so a short
payload cannot make the decoder reserve more than it will fill. The same
`validate` gates `PlanBuilder::finish`, so a compiler cannot emit what a
runner would refuse. (The jump, arm, timer, and reservation rules were added
after the 2026-08-28 code review — both families found the first three.)

## 3. Values and the data pool (`plan/src/value.rs`)

`Value::{Number, Bool, Str, Unit, Option, List, Record}` — closed; absence is
`Option` only (LLP 1004 D3). Records carry fields by position; `types` names
them. Canonical value bytes are tag + payload, decoded with a depth bound and
a NaN refusal. `Value::conforms(plan, ty)` is the `shape` check at the data
seam and on compiled data. Compiled resource values live in the data pool
(`resources.initial`, empty when the resource is requested at boot instead).

## 4. The VM (`runner/src/vm.rs`, `plan/src/asm.rs`)

A stack machine over values with a small locals stack. Opcodes: literals
(`Number Bool Str None Unit Some`), loads (`LoadSlot LoadDerive LoadResource
LoadParam LoadItem LoadBound LoadLocal`), structure (`Field Record List`),
arithmetic and comparison (`Add Sub Mul Div Rem Neg Eq Ne Lt Le Gt Ge Not
Concat`), control (`Jump JumpIfFalse JumpIfNone Unwrap`), `Call <Stdlib>`,
effects (`StoreSlot Command`), stack (`Pop BindLocal DropLocal`), `Return`.
`LoadItem`/`LoadBound` take a depth in region frames (0 = innermost).
`StoreSlot` outside an action's declared `writes` is `Trap::WriteNotDeclared`.
A derive or resource read before it settles this update is `Trap::Pending`
(§6). Every failure is a typed `Trap` by pc; there is no undefined behavior
and no ambient read that is not an operand — `now()` reads the clock the
runner passes in. `Asm` emits opcodes by declared layout and resolves forward
jumps by label; a compiler never writes a raw byte.

## 5. The roster (`runner/src/stdlib.rs`)

Each `stdlib` entry has one body: `now`, `formatClockTime` (UTC `h:mm AM`),
`formatCountdownMinutes`, `formatDistance` (miles, one decimal, `nearby` under
0.1), `formatWalk` (80 m/min), `length`, `isEmpty`, `toString` (integers print
as JavaScript does), `floor`, `max`, `min`. Deterministic and locale-free by
design. The compiler type-checks calls against the same table (LLP 1006 §3).

## 6. The runner (`runner/src/runner.rs`, `instance.rs`)

`Runner::boot(plan, data, kernel)` refuses a plan whose kernel schema digest
is not the linked kernel's, with other than one root site, or with a region at
the root (`RootRegion` — kernel roots are attach-ordered, so a keyed root could
not reorder); evaluates slot initializers in order and checks each against its
declared type (`SlotType`); **settles** derives and resources; realizes the
tree; applies the first frame as one batch.

`Runner::boot_with_delivery` takes the host's complete delivery facts before
that first settlement (LLP 1030 D7). The reserved delivery source ignores baked
and carried answers; its dependents carry device-data provenance, so a request
or conditional asset cannot first observe the bake's sequence.

**Settlement** (`settle`): derives and resources may depend on each other in
either direction, so plan order cannot order them. Each pass evaluates every
unsettled derive and resource in plan order; one that reads something unsettled
traps `Pending` and is retried next pass; the loop ends when everything settled
or nothing progressed (`RunnerError::Cycle`). On boot a resource takes its
compiled value when it has one, else queries the source. After an action, a
resource is re-requested only when its argument values changed. Settlement is
transactional: it works on a copy of the resource caches and publishes only
when the whole pass succeeds. A value that does not conform to its shape is
`RunnerError::Shape`; a derive that does not conform to its declared type is
`DeriveType`; a source refusal is `RunnerError::Data`.

**Later (LLP 1016, built 2026-08-30).** A source may answer a resource with
a *request* instead of a value (`DataSource::answer` → `Answer::Later`): the
resource keeps the value it had — its last answer, or its compiled boot
value — is **pending** under a fresh ticket, and the request is in
`Runner::take_requests()` for the host to run once the pass has published
(a pass that fails hands out nothing). One request per resource: newer
arguments forget the older ticket. `Runner::fulfill(ticket, outcome)` is the
reply: the source's `parse` makes the value, the resource takes it, and a
settlement pass follows as after an action — one commit; a ticket no longer
held is dropped with a journal line. `refresh` (an action statement) makes a
resource re-request with its current arguments, coalesced with any argument
change in the same transaction. A **mutation** (`plan.mutations`: a slot of
`option<T>`, `none` at boot, and `T`) is never queried by settlement: an
action's `send name = source(args)` asks the source once — an answer now
lands in the slot inside the action's commit; a request goes out with the
commit under one ticket per mutation, the newest `send` winning on
acceptance — and an assignment to the slot forgets its ticket in flight.
`pending(x)` in an expression reads the ticket flags. Boot with a `Later`
and no value to keep is `RunnerError::Data` — bake's refusal of a resource
that answers later at boot.

**The instance tree** realizes sites: a node → one kernel view with a
last-emitted value per binding; `when`/`match` → the active arm and its roots;
`each` → rows by key in item order. An update re-evaluates every site and
emits only what changed: a prop or style op when a binding's value differs
from the last emitted, `SetChildren` when a child list differs, create/destroy
when a key appears or goes away. A keyed row keeps its views across reorders
(`a_press_selects_a_station_re_requests_the_board_and_keeps_rows_by_key`).
Style values go through the kernel's own `StyleProps::set_dynamic`
(`runner/src/bridge.rs`; LLP 1001 gained it in this landing) — a string is an
enum name, `auto`, `N%`, or a hex color; a number is the row's number.

**Events.** `dispatch(view, Press | Change(text) | Hover(over) | Focus | Blur
| Key(name) | Submit | Load | Message(text) | Contextmenu | Dblclick | Swiperight | Scroll(left, top))` finds the site and the frames in force at that view, evaluates
the handler's curried arguments there at dispatch time, appends the event
payload — a change's text, a hover's `over` (in or out: one kind, one handler,
one action), a key's web name (`Enter`, `Escape`, `ArrowDown`, `a` — the
DOM's `KeyboardEvent.key`), nothing for press, focus, blur, submit, load, contextmenu or dblclick — and
runs the action. `Submit` (2026-08-30) is Enter in an input with a `submit`
handler: the web's implicit submission (HTML forms §4.10.21.2) without a
form, so an action need not branch on a key. `Contextmenu` and `Dblclick`
(2026-09-09, Messages) carry no payload: the platform recognizes secondary
activation / a long press and a double click / tap. The `EventKind`s are
`plan/tables/format.json`'s. Not events
(2026-08-30, the minimal set first): pointer coordinates and moves (a drag),
`keyup`, and a wheel's offsets reaching the runner (a scroll
container's position is the host's, LLP 1007 §6).
`Scroll` (2026-09-09, Messages) appends two number arguments, `scrollLeft`
and `scrollTop`, in CSS pixels, after the authored arguments. It reports the
host's changed position, including programmatic changes, and does not bubble.
The host still owns scrolling. The ABI's dispatch kind 13 carries two finite
numbers as UTF-8 `left,top`; malformed coordinates are refused.
`act(name, args)` runs an action by name (tests; an agent goes through the host's input path, LLP 1012 §1). Arguments must
conform to the parameters' declared types (`ArgumentType`) and every write to
its slot's (`SlotType`) — so an authored `width = 1/0` is a typed refusal with
rollback, never a poisoned runner. An action's `writes` bound `StoreSlot`;
its commands (`capability-call`) are collected and returned by
`take_commands()` after commit, in order — and every host reads them after
each commit and carries them to its presenter as `command` ops (2026-08-30;
before that they were journaled and nothing executed them): `setScheme(s)`
is the host's colour scheme — the document's `color-scheme` on the web,
`NSAppearance` on macOS, the window's interface style on iOS — and a name
no presenter knows is refused on its stderr. `copyText(text)` writes one string
to the host clipboard after commit (Apple: LLP 1008 §5; web: LLP 1007 §4).
It has no return value and neither reads clipboard contents nor changes focus.
Invalid arguments and unsupported/denied writes are reported by the host.
`selectText(html-id)` focuses and selects an editor after commit using the
host’s native selection API; it accepts one string (LLP 1007 §4, 1008 §5).
Linux reports this unsupported; it does not emulate a text selection surface.
Keyed rows use one key rule:
strings, finite numbers (`-0` is `0`), bools; NaN is refused (`KeyKind`).

**Atomicity.** A failure during settlement rolls back the action's slot
writes and commands and leaves the kernel untouched. A failure after the
instance tree has begun to change poisons the runner
(`RunnerError::Poisoned`; `is_poisoned()`) and clears any queued commands:
the host restarts it (LLP 1004 D5 — a reload is a restart). With values
conforming at every boundary, the remaining way there is a duplicate `each`
key from a data source.

**The clock.** `advance(now_ms)` fires every due timer in order (earliest
first, plan index on ties), each at its own due time, then moves the clock.
It is a seek: the countdown after `advance(60_000)` equals the countdown after
sixty `advance`s of a second; a backwards call is a no-op and a non-finite one
is refused (`NonFiniteClock`).

## 7. Boundaries

The runner depends on `exact-kernel` and `exact-plan`; the plan crate on
nothing. The data seam is one trait, `DataSource::query(source, args) →
Result<Value, DataError>`, synchronous, beside `answer(source, args) →
Result<Answer, DataError>` (default: `query`, now) and `parse(source, args,
outcome) → Result<Value, DataError>` for a source that hands the host a
request (LLP 1016 D1; `Request`, `Response`, `Outcome` are the runner's own
structs, ibex2's fields). The runner still does no I/O: requests leave through `take_requests` and replies enter through `fulfill`. Durable client state (LLP 1018) is a `Store` the host fills before boot (`Runner::boot_stored`; a reload carries it in `Carried::store`) and drains after each commit (`take_store_writes`); `answer` and `parse` receive it — a read is a map lookup, a write is a `StoreWrite` for the host, rolled back with a refused action or reply — so the rule holds literally. Bake gives a resource that read the store no compiled value (`resource_reads_store`): it answers from the device at boot. No threads, no host, no timers
of its own. Both crates build for `wasm32-unknown-unknown`.

## 8. Not in v1 (and where each is declared)

A Deps table and dirty-set sweep (the runner re-evaluates every site; 0485
§8.3's incremental sweep is a measured optimization for later); per-instance
derives or resources inside `each` rows (per-instance *state* landed
2026-08-30, LLP 1017 P4c: `slots.owner` names an `each` region and the row
holds the value on its `Frame` — `RowSlots` — read through the frames like
`LoadItem`, written on commit with the same rollback, never carried; a
child's derive is a substituted expression, and a child's resource is
refused); state-preserving reload (LLP 1004 D5); a request's cancellation on
the wire (a forgotten ticket is dropped on arrival, LLP 1016 D5);
cursors, cells, confidentiality, cost claims, speculation (LLP 1004 §3).

### Measured scaling correction (2026-09-04)

Implemented by Codex at Charlie's request; investigation and exact samples:
[runner scaling issue](../issues/closed/20260904-measure-runner-update-scaling.md).
On the M5 Max release fixture, a 10,000-row local edit took 29.70 ms and a
reorder 64.79 ms p50. Canonical-key lookup and constant-time child membership
bring those to 6.47 and 7.62 ms. Final child lists precede the runner's
unique-id destroys in the same atomic batch, removing repeated sibling
rebuilds; the same topology replacement falls from 164.49 to 9.36 ms.
A replacement can temporarily keep old and new kernel nodes live in the
commit; peak allocation is unmeasured.
The full evaluation walk remains. No Deps table or scheduling semantics
changed. Bulk listener discovery and bounded motion-slot removal reduce
web runner-plus-batch topology cost from 566.02 to 19.59 ms. These are
in-process desktop measurements, not a browser or phone frame budget.

## 9. Checks that hold this

`plan/tests/format.rs` (canonical bytes, validation refusals by table/row/
field and by pc, value shapes, jump resolution), `runner/tests/now_screen.rs`
(a hand-built plan: boot, press → rollback, action → re-request → keyed
reorder, `when` flip, timers, commands, refusals leave the kernel untouched,
schema mismatch), the `math` pins. All under `cargo test --workspace`; clippy
`-D warnings`, fmt, wasm, and `caps` green on 2026-08-28.

`swiperight` is the next EventKind after `dblclick`: a recognized, payload-free
host event. It preserves authored action arguments and journals once on a
completed swipe. Move/cancel samples do not enter the runner.
