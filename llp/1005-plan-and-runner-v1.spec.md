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
`handlers`, and `routes`. LLP 1038 D2 adds `routes(name: str, pattern: str,
parent: opt:routes, tab: bool, notfound: bool)` in declaration order, and
`router: opt:slots` in the header after `app_id`. Parents must precede their
children; at most one row is `notfound`; the header names an existing root
slot typed as a record. `resources.initial_args: bytes` accompanies compiled
values (§8). The format digest changes; every plan re-bakes.
**No kernel vocabulary is declared here**: `nodes.node_type`,
`bindings.id` are the kernel's ordinals as numbers (LLP 1004 D2); the plan
header carries the kernel's `SCHEMA_DIGEST` so a mismatch is refused at boot.
The value bridge consults the row's generated codec before interpreting `auto`:
it becomes a dimension value only for dimension rows, and remains text for enum
rows such as `align-self`, `overscroll-behavior`, and `scrollbar-width`.

## 2. Bytes (`Plan::encode` / `Plan::decode`)

Header: magic `EXPL`, `FORMAT_VERSION` u32, `FORMAT_DIGEST` u64, kernel schema
digest u64, compiler identity u64, `app_id` (length-prefixed UTF-8), and
`router` (u32 slot index or `0xFFFFFFFF`). Then the three pools (strings as count +
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
have exactly two arms and `each` one, every arm is owned by its region,
no timer has a zero interval, no site nests more than `MAX_SITE_DEPTH` (256)
levels or sits on a parent cycle (`SiteTooDeep`), and no type contains itself
(`TypeCycle`). A refusal names table, row, field, and — for
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
effects (`StoreSlot Send Refresh Command`), stack (`Pop BindLocal DropLocal`), `Return`.
`LoadItem`/`LoadBound` take a depth in region frames (0 = innermost).
An action's `writes` row is its allowlist: a `StoreSlot`, or a `Send` (whose
mutation's slot it writes), naming a slot outside it is
`Trap::WriteNotDeclared`, and code that is not an action's body (a derive, a
resource's arguments, an initializer, a handler's arguments) runs with an
empty one. Contract declares no writes (LLP 1035.005.000 D1, 2026-10-02: an
authored `writes` clause is `syntax-writes-clause`): lowering sets the row to
exactly the slots the body assigns or sends through every branch, in slot
order (`Action::effects`, LLP 1006 §2), so a compiled plan never meets the
trap; it guards a plan assembled by hand or corrupted.
A derive or resource read before it settles this update is `Trap::Pending`
(§6). A `List`, `Record` or `Some` whose expanded tree — a shared value
counted in every place it appears, as equality, shape checks and encoding
walk it — would pass 2^24 values or 64 MiB of string bytes is
`Trap::ValueTooLarge`, and one nesting deeper than `Value::decode`'s 64 is
`Trap::ValueTooDeep` (2026-09-23): a list doubled through a local was
otherwise exponential in instructions (2^24 leaves: 376 ms to compare,
636 ms to encode). Sizing is memoized per evaluation, so doubling costs one
lookup a level. Every failure is a typed `Trap` by pc; there is no undefined behavior
and no ambient read that is not an operand — `now()` reads the clock the
runner passes in. `Asm` emits opcodes by declared layout and resolves forward
jumps by label; a compiler never writes a raw byte.

## 5. The roster (`runner/src/stdlib.rs`)

Each `stdlib` entry has one body: `now`, `formatTime` (`h:mm AM`, LLP
1054.000.003), `length` (text in UTF-16 code units, as the
web's `String.length` and `maxlength` count), `isEmpty`, `toString` (integers print
as JavaScript does), `floor`, `max`, `min`, and `at` (`Array.prototype.at` as
an option, appended 2026-09-28 at the table's end so earlier ordinals hold;
LLP 1006 §3). `formatCountdownMinutes`, `formatDistance` and `formatWalk`
were Caltrain's wording and are Caltrain `fn`s since 2026-10-02 (LLP
1035.005.000 D8, LLP 1006 §5); deleting their rows moved every later entry's
ordinal down three, so the format digest changed and every host re-bakes.
Deterministic and locale-free by
design. The compiler type-checks calls against the same table (LLP 1006 §3).

LLP 1038 D3 adds these signatures, preserving the existing roster entries
and ordinals. Scoped action and action-prop references take precedence over
roster names, preserving existing `open(...)` handlers when this roster grows.
Reserved shape names and `list<Router>`, `list<Entry>`,
`list<string>` are accepted type spellings in parameters and returns.

| entry | parameters | return |
|---|---|---|
| `open`, `push`, `replace`, `select`, `go` | `Router, string` | `Router` |
| `back` | `Router` | `Router` |
| `stack` | `Router` | `list<Entry>` |
| `top` | `Router` | `Entry` |
| `depth` | `Router` | `number` |
| `params` | `Router, string` | `list<string>` |
| `searchParam` | `Entry, string` | `string` |
| `encodeURIComponent` | `string` | `string` |

The VM passes the plan and its boot-checked route context to the roster.
Conversions follow the header slot's types: `Router {tab, tabs, next}` →
`Tab {name, stack}` → `Entry {id, name, url, tab, params}` → `Params` with
one string field per distinct `:name`, in first-declaration order. This is
also field order in each positional record; chunk (c)'s compiler must emit
it. Lists carry records or strings as their signatures say. The six verbs
call `exact-route`; a refused verb returns the original value and journals
each distinct intent/reason once per commit, including settlement retries
(LLP 1035.001 D6). Query reads and component encoding use the crate's web
semantics. The agent's `state` prints the slot through its declared shapes,
with field names, using its existing typed-JSON path (LLP 1038 D11).

## 6. The runner (`runner/src/runner.rs`, `instance.rs`)

LLP 1039 adds `exactViewport` beside `exactDelivery`: a host boot fact, filled by field name before settlement and never taken from a compiled or carried answer. `set_viewport` re-answers its readers through `recommit` in one commit; unchanged or unread facts return `None`, and non-finite or non-positive sizes are journaled refusals.

`Runner::boot(plan, data, kernel, viewport, launch)` refuses a plan whose kernel schema digest
is not the linked kernel's, with other than one root site, or with a region at
the root (`RootRegion` — kernel roots are attach-ordered, so a keyed root could
not reorder); evaluates slot initializers in order and checks each against its
declared type (`SlotType`); **settles** derives and resources; realizes the
tree; applies the first frame as one batch.

All boot entry points take the viewport and `launch: &str` before first settlement: `boot_carrying`
adds carried state, `boot_stored` adds a store snapshot, and `boot_with_delivery`
takes optional carried state, a snapshot and complete delivery facts (LLP 1030 D7).
The reserved delivery source ignores baked and carried answers; its dependents carry device-data provenance, so a request
or conditional asset cannot first observe the bake's sequence.

LLP 1038 D5 fills the header's router slot with `Router::launch(table, launch)`
**before any slot initializer**, even if the slot is declared later. Bake and
hosts without a location pass `/`. An unmatched launch with no `notfound`
is journaled and retried at `/`; if `/` also matches nothing, boot refuses
with `RunnerError::Router`. Boot validates the four shapes and reads the
route table once, then shares it with all VM evaluations.

A dev reload carries the router by slot name. `Carried::router` retains the
value decoded through the old plan's shapes, so Params field-order changes
cannot reinterpret a value. Every entry in every retained tab must still
match the new table to the same route name, and the tab roster (names, in
order) must be unchanged. If so, visits, stacks and ids
are retained and Params are rebound by name from each URL; otherwise the
new value is `Router::launch(new_table, old_top.url)`, with the same fallback
rule. Other root slots and matching resources retain their existing carry
behavior; row slots are never carried.

Runner commits continue to return `exact_kernel::CommitReceipt` unchanged.
`Runner::take_router_change() -> Option<RouterChange>` is a separate drain
beside `take_commands()`, `take_requests()`, and `take_store_writes()`
(LLP 1038 D7; orchestrator ruling, 2026-09-14). `RouterChange` is `{top: u64,
url, removed: Vec<u64>}`: the selected top and ids removed from any tab.
A successful commit that changes the router slot publishes it, including
boot's initial change with no removed ids; unchanged and refused commits
leave the pending change alone. Taking clears it; absent a router, or after
a take until navigation changes again, the result is `None`. Multiple commits
before a take retain the latest top/url and every removed id once, in
first-removal order (each commit traverses old tab/stack order). Hosts drain
it beside commands and translate it into the `router` op in chunk (d).

**Settlement** (`settle`): derives and resources may depend on each other in
either direction, so plan order cannot order them. Each pass evaluates every
unsettled derive and resource in plan order; one that reads something unsettled
traps `Pending` and is retried next pass; the loop ends when everything settled
or nothing progressed (`RunnerError::Cycle`). On boot a resource takes its
compiled value only when its evaluated arguments equal `initial_args`, else
queries the source; a source not ready at boot uses the compiled value as a
stale placeholder regardless of arguments and is asked again with current
arguments at `data_ready` (LLP 1038 D5; LLP 1027 D4; §8). After an action, a
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
and no value to keep shows the resource's placeholder (LLP 1048.003 D6); with
none, it is `RunnerError::Data` — bake's refusal of a resource that answers
later at boot.

**The instance tree** realizes sites: a node → one kernel view with a
last-emitted value per binding; `when`/`match` → the active arm and its roots;
`each` → rows by key in item order. An update visits only the sites whose
reads changed (§8, the dependency table) and emits only what changed: a prop or style op when a binding's value differs
from the last emitted, `SetChildren` when a child list differs, create/destroy
when a key appears or goes away. A keyed row keeps its views across reorders
(`a_press_selects_a_station_re_requests_the_board_and_keeps_rows_by_key`).
Style values go through the kernel's own `StyleProps::set_dynamic`
(`runner/src/bridge.rs`; LLP 1001 gained it in this landing) — a string is an
enum name, `auto`, `N%`, or a hex color; a number is the row's number.

`Navigate` (LLP 1038 D8/D11, 2026-09-14) carries one string location to the
navigation root. Apple and web dispatch kind **14** follows scroll (13);
Linux uses `Event::Navigate` directly. The handler chooses the router verb.
An action taking no parameters ignores the location; otherwise it takes one string.
A URL before boot is the launch fact, with no navigate dispatch.

Listener lookup reads the view-to-site map the id allocator keeps (pruned
against the kernel's live views). Event dispatch finds the instance along the
kernel's parent chain, crossing only the arms and rows on that path, and
reconstructs their frames for curried arguments.

**Events.** `dispatch(view, Press | Change(text) | Hover(over) | Focus | Blur
| Key(name) | Submit | Load | Message(text) | Contextmenu | Dblclick | Swiperight | Scroll(left, top) | Navigate(location))` finds the site and the frames in force at that view, evaluates
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
rollback, never a poisoned runner. An action's `writes` bound `StoreSlot`
and `Send` (§4);
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
`share(title=, text=, url=)` (LLP 1069.003) takes named arguments, lowered
positionally as `(title, text, url)` with `none` for an absent one; every
`Command` carries `source`, the view whose event ran the action (`None` for a
timer or an answer), and the runner's `share::arm` rules for every host:
refused into the journal, held for the agent, or presented.
`saveFile(id, from, suggestedName)` (LLP 1069.010 D3) takes three strings; the
runner's `save_file::arm` refuses a `from` outside the app's `fs.read` grants
or an unknown `id` (the host then fires `cancel` there), holds `export` for the
agent, or says present. The chosen name arrives as `change` on the element
`id` names, a dismissed panel as `cancel`, which any element may now take.
Hosts reach both rulings through one door, `commands::request`.
`showOpenFilePicker(id[, multiple])`, `showDirectoryPicker(id)` and
`showSaveFilePicker(id, suggestedName)` (LLP 1069.010 D2) rule through the
same door (`file_pickers::arm`): held as `open-file`, `open-directory`,
`save-file` for the agent; the chosen `doc:` handles arrive as `change` on
the element `id` names, one per line.
`selectText(html-id)` focuses and selects an editor after commit using the
host’s native selection API; it accepts one string (LLP 1007 §4, 1008 §5).
Linux reports this unsupported; it does not emulate a text selection surface.
Keyed rows use one key rule:
strings, finite numbers (`-0` is `0`), bools; NaN is refused (`KeyKind`).

The runner indexes immutable node/region child sites once at boot. A compact
sorted group table maps each exact `(parent, arm)` pair to a range in one child
array. Realization, dependency analysis and list validation borrow those ranges
instead of rescanning and sorting the whole plan for every row. Within a group,
`order` then site index retain the prior ordering; a tied node precedes a region.
The index stores plan structure only, is rebuilt with each runner, and uses
memory proportional to plan sites rather than mounted rows or history length.

**Atomicity.** An action, a reply and a recommit each take one checkpoint —
slots, the store, pending requests, commands, and the flags settlement sets
on its way (a deferred resource's staleness, store provenance, requested
refreshes) — and a refusal puts all of it back, leaving the kernel untouched. A failure after the
instance tree has begun to change poisons the runner
(`RunnerError::Poisoned`; `is_poisoned()`) and clears any queued commands:
the host restarts it (LLP 1004 D5 — a reload is a restart). A bad bound
value is not a reason to restart (Charlie, 2026-09-22). A binding is a
declaration whose value is computed from state, as a `var()` reference is,
so a value its prop or style row refuses (a colour that is not one,
`aria-level=1.5`) is invalid at computed-value time (CSS Custom Properties
§3.1): the row is unset — `ClearProp`/`ClearStyle`, so inherited or initial
— and one journal line names it. It is not CSSOM's `setProperty`, which
keeps the earlier declaration: the view would then depend on history rather
than state. A repeated key in an `each` or a virtualized list is the
data's error too (Charlie, 2026-09-22, a separate decision): the later
rows take the identities `d1:<key>`, `d2:<key>` in order — no canonical key
text starts with `d` — and each such row's creation writes one journal
line. With values conforming at every boundary, what remains is a plan
defect or a trap while the tree changes; for a type-checked plan, a string
past `MAX_STRING` built by a binding, or a route segment a binding asks
`encodeRouteSegment` to encode and it refuses.

**The clock.** `advance(now_ms)` fires every due timer in order (earliest
first, plan index on ties), each at its own due time, then moves the clock.
It is a seek: the countdown after `advance(60_000)` equals the countdown after
sixty `advance`s of a second; a backwards call is a no-op and a non-finite one
is refused (`NonFiniteClock`).

## 7. Boundaries

The runner depends on `exact-kernel`, `exact-plan`, and the `exact-route` leaf; the plan crate on
nothing. The data seam is one trait, `DataSource::query(source, args) →
Result<Value, DataError>`, synchronous, beside `answer(source, args) →
Result<Answer, DataError>` (default: `query`, now) and `parse(source, args,
outcome) → Result<Value, DataError>` for a source that hands the host a
request (LLP 1016 D1; `Request`, `Response`, `Outcome` are the runner's own
structs, ibex2's fields). The runner still does no I/O: requests leave through `take_requests` and replies enter through `fulfill`. Durable client state (LLP 1018) is a `Store` the host fills before boot (`Runner::boot_stored`; a reload carries it in `Carried::store`) and drains after each commit (`take_store_writes`); `answer` and `parse` receive it — a read is a map lookup, a write is a `StoreWrite` for the host, rolled back with a refused action or reply — so the rule holds literally. Bake gives a resource that read the store no compiled value (`resource_reads_store`): it answers from the device at boot. No threads, no host, no timers
of its own. Both crates build for `wasm32-unknown-unknown`.

## 8. Compiled arguments and remaining limits

`resources.initial_args` (LLP 1038 D5) is a canonical `Value::List` of the
arguments evaluated by bake, using the same value encoding as `initial`.
Both references are empty when there is no compiled value. A compiled
zero-argument resource has an encoded empty list, not empty bytes. Plan
validation requires the encoded list's length to equal the argument count.
`PlanBuilder::set_resource_initial_args` writes it;
`Runner::resource_args(name)` exposes the settled arguments bake records.

With a ready source at boot, only equal evaluated arguments admit the compiled
value. A deferred source uses its compiled placeholder regardless of arguments,
marks it stale, and is asked again at `data_ready`. A deep
launch that changes a resource's query asks its source; unrelated resources
retain their baked first frame. Delivery, viewport and store provenance keep
their stricter existing rules. If a source answers later, a previous settled
value may remain pending; compiled data is a fallback only for matching
arguments. A fresh boot with a ready source, different arguments, a later answer, and no
value to retain refuses, as any boot without an available first value does.

Still outside v1:

Per-instance
derives or resources inside `each` rows (per-instance *state* landed
2026-08-30, LLP 1017 P4c: `slots.owner` names an `each` region and the row
holds the value on its `Frame` — `RowSlots` — read through the frames like
`LoadItem`, written on commit with the same rollback, never carried; a
child's derive is a substituted expression, and a child's resource is
refused); carrying per-instance state across reload (root state carries under
LLP 1007 §6); a request's cancellation on
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
The full evaluation walk remained then (see the dependency table below).
Bulk listener discovery and bounded motion-slot removal reduce
web runner-plus-batch topology cost from 566.02 to 19.59 ms. These are
in-process desktop measurements, not a browser or phone frame budget.

### The dependency table (2026-09-22)

Built after the 2026-09-22 review measured the full walk (a keystroke beside a
`when`-wrapped 10,000-row list visited 50,004 nodes). At boot the runner scans
every binding, surface argument, region subject and key, derive and resource
argument once (`runner/src/instance/deps.rs`): a bitset over root slots,
derives, resources, pending flags and the clock, plus a mask of the enclosing
row and arm frames it reads by relative depth; each site carries the union
over its subtree. An update diffs the environment against what the tree last
showed (identity first, then bit-exact equivalence) and visits a child only
when its subtree's reads meet that set, an enclosing row or arm whose value
changed, or a row an action wrote (a row-slot write dirties only its own row
and the rows enclosing it); a visited node evaluates only its stale bindings,
and a parent rebuilds its child list only when a region's roots changed. An
`each` keyed from the same subject object with unchanged key inputs is not
re-keyed; a windowed list with unchanged items only refreshes its mounted rows.
Settlement keeps a derive's value, and a resource its arguments, when every
input has settled to the value it had when they were last computed — dynamic
`Pending` retries and cycle refusals are unchanged; a derive whose value is the
same but whose store provenance changed still counts as changed. Equal results
keep their previous objects, so identity survives downstream.

`set_full_evaluation` makes every site stale (evaluation only: what a host
sees of a virtualized list — ending a reorder preview, re-measuring rows, the
revision — follows inputs that actually changed, in both modes).
`runner/tests/incremental.rs` runs every app plan and a synthetic one both ways
in lockstep through seeded random events, clock seeks, late replies, store
reads and writes, deferred activation, collection feedback (stale reports
included) and reorder gestures; every run must boot, commit and never poison,
and receipts, kernel trees, carried state, effects, journals and collection
snapshots must be identical after every step. A commit's checkpoint copies the
old values of what it writes, not the store. `last_instance_work()` counts
nodes visited, bindings and derives evaluated, rows keyed and rows scanned
(including the rows compared to find the event's view) and store bytes copied.
Measured on the review's harness (release): that
keystroke visits 3 nodes and allocates 39 times (was 50,004 and 402,531); a
Messages composer keystroke visits 8 nodes and evaluates 5 bindings (was
2,237 and 21,049), 0.006 ms at 50, 500 or 2,000 messages per list. A change
read by every row still visits every row.

## 9. Checks that hold this

`plan/tests/format.rs` (canonical bytes, validation refusals by table/row/
field and by pc, value shapes, jump resolution), `runner/tests/now_screen.rs`
(a hand-built plan: boot, press → rollback, action → re-request → keyed
reorder, `when` flip, timers, commands, refusals leave the kernel untouched,
schema mismatch), the `math` pins. All under `cargo test --workspace`; clippy
`-D warnings`, fmt, wasm, and `caps` green on 2026-08-28.

**`pointerdown` and `pointerup`** (2026-10-03, Charlie: yes, with the Signal
Clone's hold-to-record mic as the consumer; DEFERRED under Motion's gesture
arena). These are DOM's names for a touch or the primary button going down on
a node, and coming up or being cancelled. A cancel is delivered as
`pointerup`, so an action that started something hears the end, unless its
node is removed while the pointer is down. A removed node has no handler
left to run, and the hosts then forget the pointer (an app that starts
something on `pointerdown` keeps the node mounted until the up).
Neither carries a payload, and neither is recognized: the down fires before
any gesture has decided, and both sit beside `press`, `pan` and
`contextmenu` without taking anything from them. On the web a cancel is
DOM's `pointercancel`. DOM's order holds: down, up, then the click's
`press`. The innermost enabled node that hears either one takes the pointer, on
every host (a disabled one passes it to an enabled ancestor), and only the
primary pointer counts, and its
up arrives wherever the pointer lifts: heard on the document on the web (a
pointer capture would also retarget the click and press on a lift
elsewhere), through AppKit's own mouse-up routing, and with the touch on
UIKit.
- **Web** (`glue.js` with `input-glue.js` `pointer`; the JS target's
  `pointer.js`): the element's own events. The innermost claims the event
  as it bubbles, so its ancestors' handlers leave it.
- **iOS** (`IOS/PointerIOS.swift`): a gesture recognizer that only observes.
  It never recognizes and can neither prevent nor be prevented, so a press,
  a pan, a long press and the scroll view keep their touches. Each node's
  recognizer skips a touch that a nearer enabled pointer node takes. An idle
  one does not keep a row out of the node pool.
- **macOS** (`MouseChainMac`): `mouseDown`/`mouseUp`, on the innermost
  enabled node from the hit view up, held on the presenter until the button
  comes up. A native button's own tracking loop reports both (its up
  before the action it sends). A held node's release, or a reset, clears
  the hold.
- **Linux:** not built; the events never fire.
- **ABI:** dispatch kinds 29 and 30.
- **Agent:** `tap` remains an activation (`press`). The pointer events are
  driven by real touches (LLP 1080.000's `touch: platform`) or by the hosts'
  tests.

Tests: `contract/cli/tests/it/pointer.rs` (the runner and DOM's order),
`testAPointerNodeObservesItsTouchWithoutPreventingAnything` (iOS), and
`testPointerDownAndUpReachTheNearestPointerNodeAroundThePress` (macOS), and
`host/web/tests/pointer.test.mjs` (Chrome: order, a lift elsewhere, the
secondary button, a disabled node).

`swiperight` is the next EventKind after `dblclick`: a recognized, payload-free
host event. It preserves authored action arguments and journals once on a
completed swipe. Move/cancel samples do not enter the runner.
