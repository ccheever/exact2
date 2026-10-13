# LLP 1005: Plan and runner v1 — what `exact-plan` and `exact-runner` are, as built

**Type:** Spec
**Status:** Draft
**Systems:** Plan, Runner, Kernel (seam), Data seam
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Amended:** 2026-10-13, Astra: scroll completion measurements, time ownership, bounded catch-up and regression reconciliation (§Scroll completion); implementation remains parked.
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
behavior; row and arm slots are never carried (a `late` root slot is).

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
| Keycapture(record) | Key(record) | Keyup(record) | Submit | Load | Message(text) | Contextmenu | Dblclick | Swiperight | Scroll(left, top) | Navigate(location))` finds the site and the frames in force at that view, evaluates
the handler's curried arguments there at dispatch time, appends the event
payload — a change's text, a hover's `over` (in or out: one kind, one handler,
one action), a key's web name (`Enter`, `Escape`, `ArrowDown`, `a` — the
DOM's `KeyboardEvent.key`), nothing for press, focus, blur, submit, load, contextmenu or dblclick — and
runs the action. `Submit` (2026-08-30) is Enter in an input with a `submit`
handler: the web's implicit submission (HTML forms §4.10.21.2) without a
form, so an action need not branch on a key. `Contextmenu` and `Dblclick`
(2026-09-09, Messages) carry no payload: the platform recognizes secondary
activation / a long press and a double click / tap. The `EventKind`s are
`plan/tables/format.json`'s.

**Bubbling focus events** (Charlie, 2026-10-08; implementer: focus lane,
2026-10-12). `focusin` and `focusout` follow DOM focus transitions: the old
target's `focusout` bubbles before the new target's `focusin`, including a move
between descendants of the same wrapper. Both offer an optional `FocusEvent`
record with `target` (the focused element's `id`, or `""`) and `relatedTarget`
(`some(id)` for the other focused element, including `some("")` when it has no
`id`, or `none` when focus enters from or leaves to no element). A handler on
an ancestor does not make that ancestor focusable or a Tab stop. Programmatic,
pointer and Tab moves use the same transition; removing the focused node
delivers `focusout` through ancestors still connected when removal begins.
The web targets use DOM focus events; native hosts dispatch the same path
from their focus transition, retaining the old path for a removed node.
Autofocus uses that transition too. IDs are read
at the transition, including an ID changed while focused. `stopPropagation()`
stops either bubbling event at its current handler; it does not stop the other
event in the transition or an enclosing event. Early web focus replay preserves
that event scope too (focus lane review fixes, 2026-10-12). An edited field's
transition follows Chrome's observed `change`, `blur`, `focusout`, `focus`,
`focusin` order. UIKit read-only textarea selection sessions announce responder
entry even when UIKit omits its editing delegate callback. The host ABI kinds
are 46 (`focusin`) and 47 (`focusout`), after 45 (`intersect`).

If a `blur` handler destroys its wrapper, the subsequent `focusout` stays
inside that detached subtree, as in Chrome; former connected ancestors do
not hear it. The runner retains the removed handlers' instance scopes through
that outgoing event, independently of nested focus transitions, and consumes
each binding once. Linux fixes the detached path when blur finishes, before
commit processing can autofocus another field or a focusout handler can remove
more ancestors. Both web targets commit ordinary blur before DOM focusout;
the wasm host preserves the detached DOM subtree and its outgoing listeners.
Exit-animation ghosts count as removed: the outgoing event ends at that
subtree and keeps its IDs after presence strips the DOM IDs. A blur raised
during a JS tree flush runs before that commit's focus commands or autofocus.
Apple waits for the next
responder (or the deferred blur flush) before delivering it, preserving the
other element's `relatedTarget`.

Not events
(2026-08-30, the minimal set first): pointer coordinates and moves (a drag)
and a wheel's offsets reaching the runner (a scroll
container's position is the host's, LLP 1007 §6); `keyup` became one on
2026-10-07 (§9).

`Scroll` (2026-09-09, Messages) appends two number arguments, `scrollLeft`
and `scrollTop`, in CSS pixels, after the authored arguments. It reports the
host's changed position, including programmatic changes, and does not bubble.
The host still owns scrolling. The ABI's dispatch kind 13 carries two finite
numbers as UTF-8 `left,top`; malformed coordinates are refused.

`Scrollend` (2026-10-13, the Bluesky clone) is DOM's `scrollend`: no
arguments, sent after the `Scroll` belonging to a completed scroll. It is
not a notification of every offset change, nor a promise that a handler
cannot start another scroll before it arrives. The web hears the browser's
own event. Apple uses native completion callbacks and, for moves those
callbacks do not bracket, a 100 ms quiet fallback. That fallback is not
Chrome's definition of completion. [Scroll completion](#scroll-completion)
records the measured cases, native differences and Linux design. A node
without a handler takes it as nothing. ABI dispatch kind 51, no payload.
Linux and Android remain unimplemented on main.
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
Complete row-field projections, string-default expressions and literal choices
from projected booleans are also recognized once per binding. A one-byte kind
keeps operands in the original bytecode; each evaluation still checks its runtime
scopes, field kinds and string pool. Lowered functions still read both arguments
before branching, and only the selected branch reads its literal. Every other
expression uses the existing VM.

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
Ordinary `each` rows omit their empty state table when they own no slots and
their body's dependencies contain neither row-slot reads nor opaque reads.
An ancestor of a state-reading subtree retains its table as an identity for
the written row's scope chain; windowed collection rows retain their tables.
The decision is made only when creating rows, once per region update.
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
Each hands its action a `PointerEvent` when the action takes one (LLP 1056
§8.6, 2026-10-04, with `pointermove`, the pointer's moves while held and a
free pointer's over the node), and neither is recognized: the down fires before
any gesture has decided, and both sit beside `press`, `pan` and
`contextmenu` without taking anything from them. On the web a cancel is
DOM's `pointercancel`. DOM's order holds: down, up, then the click's
`press`. The innermost enabled node that hears either one takes the pointer, on
every host (a disabled one, a control or any node with `disabled`, passes it to an enabled ancestor), and only the
primary pointer counts, and its
up arrives wherever the pointer lifts: heard on the document on the web (a
pointer capture would also retarget the click and press on a lift
elsewhere), where the pointer leaving the document (out of the window, into
a frame) or the window losing focus also ends it, and a node the tree has
removed is never called, through AppKit's own mouse-up routing, and with the touch on
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
- **Linux** (`presenter/pointer.rs`, 2026-10-04): beside the contact, on the
  innermost enabled node under the press; a cancel is an up.
- **ABI:** dispatch kinds 29, 30 and 31 (`pointermove`), each with the
  record's line.
- **Agent:** `tap` remains an activation (`press`). The pointer events are
  driven by real touches (LLP 1080.000's `touch: platform`) or by the hosts'
  tests.

Tests: `contract/cli/tests/it/pointer.rs` (the runner and DOM's order),
`testAPointerNodeObservesItsTouchWithoutPreventingAnything` (iOS), and
`testPointerDownAndUpReachTheNearestPointerNodeAroundThePress` (macOS), and
`host/web/tests/pointer.test.mjs` (Chrome: order, a lift elsewhere, the
secondary button, a disabled node).

**`keyup`, and `KeyboardEvent.code` and `.repeat`** (2026-10-07, #140: a
coding-agent desktop app's ⌘-held hints, a held ⌘W that closes one panel,
shortcuts by physical key under a non-Latin layout; admitting `keyup` to
`rules/DEFERRED.md` is the owner's call, as `pointerdown`/`pointerup` were).
`keyup` is DOM's: a key's release at the focus, bubbling to every `keyup`
handler from the focus out, as `key` (keydown) does, `stopPropagation()`
included, with the same payload (the key's name) and record. A modifier's
release is one (`"Meta"`), and DOM's flags hold: a modifier's own keydown
holds it, its keyup no longer does, so an app knows ⌘ was released. A keyup
reaches the focus whatever took the keydown (a shortcut, a
`preventDefault()`); it has no default on a native host. The record
(`exact_runner::KeyboardEvent`) gains `code`, the physical key
(`KeyB`, `Digit1`, `MetaLeft`; "" where the host cannot tell), and
`repeat`, true on the keydowns the platform repeats while a key is held
(never on a keyup). An element with a `keyup` handler takes the focus, as
one with `key` does.

- **ABI:** kind 6 (`key`) and 43 (`keyup`), one wire for both
  (`KeyboardEvent::parse`): the chord the hosts already wrote
  (`Shift+Meta+b`), then, where the host knows them, `\n` and the code,
  `\n` and `true` or `false`. A chord alone is code "" and no repeat (the
  terminal host's, a native module's); anything else is refused by name.
- **Web** (`glue.js` `keyChord`, `document-glue.js`'s early queue; the JS
  target's `onKey`): the element's own `keyup`, and `event.code` and
  `event.repeat` as Chrome reports them.
- **macOS** (`KeyEvents.swift`): the session's local monitor routes AppKit's
  keyUp and a flags change that releases a modifier (the NX_DEVICE bits tell
  the sides apart) through `Presenter.keyUp`; `code` is the virtual key's
  (`KeyCodes.mac`, the game canvas's map) and `repeat` `isARepeat`. An input
  method's composition keeps its keys, keydowns and keyups, but for a
  modifier's own (Chrome forwards every flags change to the page) and a ⌘
  chord, whose keydown commits the composition first
  (`Presenter.passesComposition`, `endComposition`; iOS's
  `NodeView.passesComposition` the same).
- **iOS** (`pressesEnded` on a node, a field, a textarea): `code` from the
  press's HID usage; UIKit's presses carry no auto-repeat, so `repeat` is
  false.
- **Linux** (`presenter/events.rs` `key_event`, `key_up`): the evdev and VNC
  keyboards' edges and the agent's, with their code and repeat.
- **Agent:** `type X key K up` delivers the keyup; a lone modifier's down
  holds it on every carrier; `key K for <ms>` repeats the key while held, a
  keydown with `repeat` at macOS's default rate on the virtual clock (500 ms,
  then every 83 ms; a modifier alone none), LLP 1012 §1.

Tests: `contract/cli/tests/it/keyup.rs` (the wire, the record, the
refusals), `runner/src/runner/event/keyboard.rs`, `KeyUpMacTests` (AppKit's
keyUp and a modifier's flags change), Linux's
`keyup_hears_a_release_and_both_carry_code_and_repeat`, and the JS target's
`keys` conformance plan (Chrome, against the wasm runner). A reserved
held-modifier fact remains out (#140).

**Capture-phase keys** (Charlie, 2026-10-11; #140, the coding-agent desktop
app's window shortcuts). `keycapture=action` hears DOM `keydown` on the path
from the outermost ancestor through the focused target before any `key=`
handler. Its string payload and optional `KeyboardEvent` are the same as
`key=`. The target's capture handler precedes its bubble handler regardless
of attribute order. `disabled` excludes only HTML form controls from the key
path; the walk skips a disabled control and continues at its parent, while a
disabled container still hears and passes keys (LLP 1088 D7.3). A capture
handler that calls `stopPropagation()` prevents target and bubble handlers,
including the target's `key=` handler when called at the target.
This stops Exact's handler walk while leaving the key's default action,
including Enter submission, available. `preventDefault()` at any phase
cancels the key's native default, including a field edit. The path is fixed before dispatch and
actions run in phase order. `keycapture` on a container does not itself add a
Tab stop; held modifiers remain `keyup` plus `exactPage().hasFocus`, with no
keyboard fact. The ABI kind is 44, after `keyup` in
`plan/tables/format.json`.
The compiler/runner test in `contract/cli/tests/it/keyup.rs` holds the wire
and record; Linux's `capture_precedes_target_and_bubble_and_controls_the_default`,
`KeyUpMacTests` and `PressFocusIOSTests` hold the native order and stop; the
`capture-keys` web conformance plan drives both browser runtimes in Chrome.

**A chord during an input method's composition, and held modifiers**
(2026-10-07, #140). Chrome hands the page a ⌘ chord's keydown while the text
is still composed (`isComposing`; CDP's `Input.imeSetComposition` then the
chord, and `RenderWidgetHostViewCocoa`, which forwards the keydown before the
input method's result), with the composed text already in the field and
reported to `input`. AppKit reports composed text only once committed, so on
macOS (and iOS with a hardware keyboard) the chord's keydown commits the
composition first: the field's `input` hears the text, then the chord's
`key` handlers, which read the value Chrome's would, and a value one writes
lands (a composer's ⌘Enter sends what was typed and clears the field). On
macOS such a chord presses no `aria-keyshortcuts` button, by key or menu
item, as the web's listener skips a composing keydown (iOS's key commands are
UIKit's, unchanged). The input method is asked to let go of the text first
(`discardMarkedText`, which it answers by inserting it) and what it leaves
marked is committed (Firefox's order), so no syllable is inserted twice. The
web host is unchanged: Chrome hands `key` handlers every key while composing. A modifier's own press and release
pass a composition untouched, so a Shift let go mid-syllable is a keyup and
no held-Shift state sticks. Any other key stays the input method's. Held
modifiers are the web's: keydown and keyup say what is held, and a key let go
while another app or window has the focus sends none (Chrome, the web host and
macOS alike, checked with a real ⌘ held while TextEdit came forward), so an
app forgets them when `exactPage().hasFocus` turns false (a gated task, the
page's `window` `blur`). Tests: `KeyUpMacTests` (the chord, ⌘Enter and the
Send button, Shift's release), `FieldCompositionIOSTests`, and
`contract/cli/tests/it/keyup.rs`'s held-modifier reset.

`swiperight` is the next EventKind after `dblclick`: a recognized, payload-free
host event. It preserves authored action arguments and journals once on a
completed swipe. Move/cancel samples do not enter the runner.

## Intersection visibility (Charlie, 2026-10-12)

Implementer: Codex, 2026-10-12; web targets and native geometry slice.
Charlie's ruling: "intersect=: admit now — intersect=action
{isIntersecting, intersectionRatio}, intersect-margin, root = nearest scroller,
threshold 0, fires on mount + enter/leave". Consumer: T3 sidebar subscriptions
and timeline minimap. This admits no container queries or reactive geometry.

`intersect=action` offers an optional final `IntersectionObserverEntry` record,
after authored arguments: `isIntersecting: bool`, `intersectionRatio: number`.
It does not bubble. The root is the nearest ancestor CSS scroll container
(`overflow: auto`, `scroll` or `hidden` on either axis), or the viewport.
`intersect-margin` is a literal rootMargin string: one to four signed `px` or
`%` lengths, default `"0px"`; the compiler normalizes case-insensitive units
and requires CSS number syntax (digits after a decimal point or exponent).
Percentages follow Chrome: root height for top/bottom, root width for left/right. The margin expands only the root;
intervening clips still apply. This is geometric intersection, not occlusion or painted visibility.

The browser's IntersectionObserver with threshold 0 supplies initial delivery
(including an offscreen node), then enter/leave. Edge contact may report true
with ratio 0; a zero-area target reports ratio 1 when intersecting. A ratio
change alone does not deliver. Removed nodes stop observing without a synthetic
leave; a new mount gets a fresh initial delivery. Callback entries retain browser
order within a root/margin observer; different observers have no ordering
guarantee. Static rendering dispatches nothing.
A changed nearest root restarts observation with a fresh initial delivery.

Host coverage: both web targets use the same browser adapter. AppKit, UIKit
(including tvOS) and Linux use one runner sampler over laid-out frames and
reported scroll offsets. Native hosts sample after the resize loop and scroll turns,
mark entries before dispatch, and generation-check queued entries so removal
or recycling cannot deliver to a replacement. A handler that changes geometry
is observed on the next host turn. Apple samples at the next executor pump,
after the presenter has applied the previous batch and reported authored scroll
offsets. The runner preserves registration order within a root/margin group,
including when keyed targets move; a changed root re-registers at the end.
The browser adapter starts after first paint. A failed observer construction
retires that target's registration without interrupting other targets.

The sampler applies `Kernel::sticky_constraint` at each ancestor's reported
scroll offset, outside its own transform, so a sticky target and its descendants
remain intersecting while pinned and leave at their containing block's end.
For a root scrolled by the page, the sampler subtracts the root's frame origin
from the page offset to use the constraint's local coordinates.
The Chrome and runner tests also agree on partially clipped rotated roots,
a counter-rotated target, and a rotated/scaled root.

Native qualification remains incomplete: inline text runs, presentation-time
motion, nested/transformed sticky constraints and SVG clip references are not
qualified by this slice. `position: fixed` is not an admitted schema value.
Native transforms use the committed two-dimensional rows, as `frame()` does,
not animation samples. Percentage margins follow the local Chrome oracle's
per-axis basis. This differs from the W3C draft's width
rule and is recorded as a spec deviation in LLP 1001. Windows and Android inherit the Linux
sampler but their carriers are unqualified; terminal does not call the sampler. These deviations are also in LLP 1001; the issue
stays open until native presenter tests and drives and the remaining geometry
cases are qualified.

## Scroll completion

**Amendment, 2026-10-13; design: Astra, `q1013/scrollend-rule`.** This
clarifies §6's event added by `46f300937`. The implementation to resume is
the Linux lane `q1013/linux-scrollend`, parked at `cbd15e31a`; this amendment
changes no host. The rules below are its target, not a claim that the
current Apple or web agent implementations satisfy every case.

The cause is two conflations: an offset change is not necessarily a scroll
operation, and an unfinished operation is not an already queued completion.
The parked host also advances the runner while the presenter still holds
commands and scroll events from earlier instants. Neither a later event
flush nor a longer quiet timeout repairs that ordering.

### Browser evidence

Measured on this worktree at `64e779683`, 2026-10-13, with Bun 1.4.2,
`playwright-core` and headless Chrome **155.0.8059.40**, selected by
`scripts/agent-launch.mjs`'s `chromium()`. The throwaway probe and JSON trace
are `target/scrollend-rule/chrome-scrollend.mjs` and `chrome-results.json`;
run `bun target/scrollend-rule/chrome-scrollend.mjs`. They are local
measurement artifacts, not a new check. That trace has 37 cases, each in
its own browser context. The wheel follow-up at `353c9a715`, on the same
Chrome and Bun versions, is `chrome-wheel-results.json`: run
`bun target/scrollend-rule/chrome-scrollend.mjs --wheels` for the single
notch, three spaced notches and two six-notch bursts. The fixture has two 200 × 100 CSS-pixel overflow ports,
2,000-pixel children and `overflow-anchor: none`. Listeners record event,
offset, layout-box existence, connectedness, `performance.now()` and an
animation-frame counter. Positive cases await events; absence is observed
over 250–800 ms of animation frames, including a show after hiding. Those
windows and the measured milliseconds below are observations, not timing
assertions a host test should copy.

`S` means `scroll`, `E` means `scrollend`; subscripts identify scrollers.
The runtime cases import this checkout's `rt.js` and `agent.js`, using real
`act`, command dispatch and `agentSettled({op: "clock", settle: true})`
over the same DOM fixture. Only generated names metadata is empty; this is
a runtime probe, not a compiled app drive.

| Case and drive | Chrome observation |
|---|---|
| Wheel notches: Playwright `mouse.wheel(0, 60)`, once and three times separated by two animation frames | In `chrome-wheel-results.json`, `wheel-1` arrived at 51.5 ms and produced `S E` at 74.6 ms. `wheel-3` arrived at 49.8, 118.0 and 179.1 ms, ending separately at 78.7, 130.2 and 196.8 ms. Each notch ended on its own, despite gaps below 100 ms. |
| Fast wheel bursts: six 60-pixel ticks | `wheel-fast-burst` sends serial Playwright calls without deliberate waits: six `S E` pairs, offsets 60 through 360, with observed wheel spacing 28.7–35.5 ms. `wheel-frame-burst` sends CDP ticks on six consecutive rAFs: wheels at 50.2–133.3 ms, about 16.7 ms apart, six `S E` pairs at 66.8–151.1 ms. No quiet-window merging in either burst. These are discrete injected ticks, not a physical trackpad's phases or momentum. |
| Programmatic jump: `scrollTop = 120` | Assignment returned at 1.4 ms; `S E` arrived at 20.7 ms in the next rendering turn. Neither event was synchronous; completion did not wait 100 ms. |
| Animated scroll: `scrollTo({top: 600, behavior: "smooth"})` | Many `S`, then one `E`, about 450 ms after the call and 30–40 ms after the last `S`. No 100 ms wait. Native duration is not a cross-host constant. |
| Key handler: prevent ArrowDown's default and call `scrollBy(0, 120)` | The same `S E` as an instant API jump. A key invoking an API is not a held native keyboard-scrolling gesture. |
| A `scroll` handler jumps another 120 pixels, once | After an instant first jump: `S₁ [jump₂] E₁`, then `S₂ E₂` next turn. `E₁` reads the new offset. After the first sample of a smooth scroll: the jump interrupts the animation; `S₂ E` follows, with only one end. |
| A `scrollend` handler jumps another 120 pixels, once | `S₁ E₁ [jump₂]`, then `S₂ E₂`, for both instant and smooth first scrolls. The first end does not consume the next scroll's end. |
| Timer before the next frame; delayed delivery | In `timer-before-next-frame`, a zero-delay JS timer scheduled after an instant jump in rAF ran before its `S E`. In `blocked-event-loop`, the 105 ms timer, overdue after a 140 ms block, ran at 155.6 ms before DOM `S E` at 155.9 ms. In `live-runtime-late-timer`, DOM `S E` arrived at 173.9 ms; the binding ran the overdue runner timer at 174.3 ms before the Contract end handler at 174.5 ms. These are separate traces in `chrome-results.json`; neither proves a universal 100 ms completion deadline. |
| Layout clamp after an ended scroll: offset 700, then child height 160 | Offset became 60 and another `S` arrived; no second `E` in 400 ms. |
| Clamp during a smooth scroll to 600: shrink child to height 100 at the first `S` | First `S` at 32.6 ms, clamp to zero at 32.8 ms, another `S` at 54.5 ms, one `E` at 483.4 ms. The clamp did not finish the animation: its end remained about 450 ms later. |
| Viewport growth after an ended scroll: port height 500 → 900, content height 1,000 | Offset 400 clamped to 100; another `S`, no additional `E` in 400 ms. The same held when delaying growth to the next rAF. Chrome exposes no counterpart to Linux's explicit display acknowledgement. |
| Self or ancestor `display: none` during a smooth scroll, after its first `S` | No `E` while hidden or after showing again. This also held when showing in the very next rAF after forcing hidden layout. Restoring the saved offset did not start another scroll. |
| Instant jump to 600, then self or ancestor `display: none` before events dispatch | **The already queued `S E` still arrived while hidden**, with raw `scrollTop = 0` and no layout box. Showing restored 600 silently: no further `S` or `E` in 400 ms. The collapsed DOM read did not overwrite the retained position. |
| Set `inert`, or open a modal dialog, at the first smooth-scroll sample | The scroll reached 600 and ended once. Blocking new user input did not cancel the existing scroll. |
| Remove the scroller | Removal during smooth scrolling suppressed its future end. Removal immediately after an instant jump still delivered the queued `S E` to the detached DOM element's retained listener. |
| Two instant jumps in one turn | A then B produced `Sₐ Eₐ Sᵦ Eᵦ`; reversing the calls reversed that order. There was no global phase delivering every `S` before every `E`. |
| A's end handler changes B before B's queued events | Moving B preserved its queued `Sᵦ Eᵦ`, reading B's new offset, and added `Sᵦ Eᵦ` next turn. Hiding B preserved its queued events with raw offset zero and no box; showing restored 120 without further events. Removing B also preserved its already queued raw DOM events. |
| A's end jumps A once; B jumps after A's second end | End order `aab`, on successive rendering turns. No 100 ms quiet period was introduced for A's handler command. This establishes the event sequence, not Linux's virtual timestamps. |
| A live end handler alternates offsets 120 and 240 while a flag is true; a runner timer after 500 ms clears it | 31 ends over frame counters 0–30. The timer ran at 519.0 ms, the final end at 519.6 ms, and no further end through 583.3 ms. Handler reentry did not starve the clock. |
| No movement: `scrollBy(0, -100)` at zero | No `S` or `E`. |
| Agent `clock settle` | Chrome has no such API. Exact's current JS agent returned `settled: true` for an instant jump after its end, but also returned it during a smooth `scrollBy` and before all four ends of a handler-driven jump chain. The native scrolling timeline is not a Web Animations timeline that the agent can seek. |

The distinction between finishing and dispatching agrees with CSSOM View's
[completion definition](https://drafts.csswg.org/cssom-view/#scrolling) and
[pending event queue](https://drafts.csswg.org/cssom-view/#scrolling-events).
The draft leaves cross-scroller completion ordering as an open question;
the table records Chrome's observed order for these drives. In particular,
“after its last scroll” does not mean “after every scroll that any handler
can subsequently cause.” Chrome exposes no completion deadline to script;
the delayed-loop probe measures dispatch order, not an inferred compositor
timestamp.

### Apple today

This column of the comparison is from source inspection, not fresh native
device measurements. Both hosts share
[`ScrollEnd.swift`](../host/apple/Sources/ExactKit/ScrollEnd.swift): `moved`
sets an owed bit, `settle` replaces a wall-clock `DispatchWorkItem` due in
100 ms, `hold` cancels it, and `take` clears the debt before dispatch.
[`ScrollEndMac.swift`](../host/apple/Sources/ExactKit/Mac/ScrollEndMac.swift)
and [`ScrollEndIOS.swift`](../host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift)
decide when to call those operations.

| Cases | Current macOS and iOS behavior |
|---|---|
| Wheel, instant jump, key-driven `scrollBy` | A reported changed offset outside a held native gesture/animation schedules the 100 ms quiet check on both hosts. An unchanged offset owes nothing. This can merge notches and delay instant jumps relative to Chrome. |
| Gesture or animated scroll | macOS uses `didEndLiveScrollNotification`, then a main-queue turn so the collection's knob correction runs first; an owned `scrollIntoView` animation calls `endScroll` from its completion. iOS uses `scrollViewDidEndDragging` without deceleration, `scrollViewDidEndDecelerating`, or `scrollViewDidEndScrollingAnimation`; a finger, follow-end correction or offset driver can keep it held. |
| Either handler scrolls again | `endScroll` takes the old debt, calls `sendScrollEvent`, checks that the view still belongs to the same node, then dispatches the end. A move from either handler can arm a new debt. It does not recursively drain the new scroll before sending the old end. Ordinary queued scroll handlers that run before the quiet check can postpone it. |
| Runner timer between quiet deadline and next frame | The fallback is scheduled on real time, not in the runner's timer queue. The session dispatches with `now()` when its callback runs; `Runner::dispatch_at` first runs timers due by that value. A delayed callback can therefore follow a timer later than its intended quiet deadline. Agent time does not advance that `DispatchWorkItem`. |
| Layout clamp | `scrollEndMoved` compares reported offsets; neither implementation classifies a visible layout clamp separately from a scroll. A clamp reported through the native callback can arm another end. The guards exclude hidden-layout resets and unchanged positions, not every clamp. This is a parity gap, not a rule Linux should inherit. |
| Hidden self or ancestor, then shown | `hasScrollLayoutBox` walks ancestors for `display: none`; movement and delivery are suppressed while false. `NodeViewMac.swift` retains the pre-hide position in `hiddenScroll`, excludes the native extent's transient collapse from scroll events and restores the position on show. Hiding does not itself reset `ScrollEnd`. A quiet callback while hidden consumes the debt, but hiding and showing before that callback can retain it. Conversely, an already completed end can be suppressed while hidden. Both completion behaviors differ from Chrome's distinction above. |
| Inert under a modal | The layout-box test does not reject `inert`, and `Presenter.send` checks the view's existence, not input eligibility. An owed end can still dispatch. |
| Removed or pooled scroller | View identity is checked before and after the last scroll handler. iOS also checks its incarnation and resets pooled state. Exact drops delivery to a destroyed node; it cannot call the detached element listener that raw DOM retained. |
| Two scrollers in one turn | Native callbacks/main-queue work dispatch each end separately and apply each handler before continuing. There is no shared deadline-ordered queue across scrollers. |
| `clock settle` | `Agent.nativeInFlight()` knows some native scroll animations, but does not count `ScrollEnd.owed` or its quiet work item. A pending fallback alone need not keep settle busy. This is a gap in settle's fixed point. |

Apple's callback contracts are
[`didEndLiveScrollNotification`](https://developer.apple.com/documentation/appkit/nsscrollview/didendlivescrollnotification),
[`scrollViewDidEndDragging`](https://developer.apple.com/documentation/uikit/uiscrollviewdelegate/scrollviewdidenddragging(_:willdecelerate:)),
[`scrollViewDidEndDecelerating`](https://developer.apple.com/documentation/uikit/uiscrollviewdelegate/scrollviewdidenddecelerating(_:))
and [`scrollViewDidEndScrollingAnimation`](https://developer.apple.com/documentation/uikit/uiscrollviewdelegate/scrollviewdidendscrollinganimation(_:)).
The Mac notification's position relative to a real trackpad coast is still
unqualified; QUEUE's physical-fling check remains. A test that posts that
notification is not evidence about momentum.

### Rule for hosts

1. **Track a scroll, not every change of offset.** A user/API scroll that
   actually moves owes an end. A no-op, boot restoration, resize, inset
   adjustment or layout clamp alone owes none. A clamp can change the last
   reported position of an existing scroll without creating another end.
   Deliver that position's `scroll` in its ordinary event order. A clamp
   neither rearms a fallback nor pulls an in-flight animation's end forward;
   the animation's completion remains authoritative even if the clamped
   offset stops changing.
2. **Finish when the scroll mechanism finishes.** Use the browser's or
   platform's completion for a gesture, coast or animation. A known instant
   command finishes when its move is applied; queue its events for delivery
   after the command, without inventing 100 ms of animation. A discrete
   Linux `wheel_at` notch is such a completed move: queue one `scroll` and
   one `scrollend` at the notch's instant, with no quiet window. This follows
   LLP 1070 §2's rule that a phase-less wheel tick is its own gesture.
   Only the Android reader's offset sample stream, which has no gesture
   phases or completion signal, uses 100 ms after its last actual scrolling
   sample as a fallback. That approximation must not merge discrete wheel
   notches or delay known instant commands and animations.
3. **Keep pending completion distinct from unfinished scrolling.** Queue
   `scroll` before its corresponding end, coalescing repeated pending events
   of the same target/type in a turn. Process a snapshot in enqueue order;
   handler-caused events go to the next turn. Run each handler's commands
   and layout before the next event; scroll payloads read the current
   DOM-equivalent offset, subject to the hidden-layout rule below. A
   handler's new move does not retract an end already in the snapshot.
   Before a quiet fallback has completed, however, a new actual scroll move
   rearms its deadline. Live handler-created events wait for a
   subsequent platform event/rendering turn; time, timers and input keep
   receiving service. Only agent clock drives drain successive virtual
   turns at one instant, within the separate bound below.
4. **Separate layout lifetime from input eligibility.** Losing a layout
   box cancels unfinished scrolling immediately, including loss through an
   ancestor, even if shown before its former deadline. Showing restores
   position, not the cancelled scroll. Inertness and modal coverage alone
   cancel nothing. An end already queued for a still-mounted hidden node
   survives, as Chrome's does. The corresponding queued `scroll` carries
   the DOM read of **zero** while hidden; `scrollend` remains argumentless.
   Keep the retained position separately (600 in the instant-hide probe,
   120 for hidden B). Neither the native extent's collapse nor delivery of
   that queued event is a new move: the host must not feed the temporary
   zero into its position, scroll binding or collection feedback. An authored
   handler recording the event's argument does see zero, as on the web;
   this does not itself change the retained position. An explicit authored
   write is separate from host feedback. Showing restores the retained
   position without another `scroll` or `scrollend`. The existing web
   `glue.js` and `rt.js` already send the live DOM read; Apple retains the
   position but must stop suppressing an already queued event while hidden.
   Check node identity before every dispatch; removal/reuse cancels Exact
   delivery, even for a queued end. Dropping the raw DOM event on a
   destroyed Contract scope is a deliberate lifetime
   deviation, shared with the web binding; retaining dead scopes for this
   event is not admitted.
5. **Order on the clock the host actually owns.** If the host knows a
   completion deadline `d`, service it at `d`: timers due at or before `d`
   retain `dispatch_at`'s precedence, their commands/layout are applied,
   and the completion is revalidated before dispatch. Do not run a timer
   due after `d` first merely because the next display frame is late.
   `performanceNow()` in that completion is `d`. For live reentry, delivery
   eligibility is separate: a handler-created instant completion deferred
   to another live turn uses that turn's time, after its due timers. It
   cannot pin the live clock at the originating handler's time. Agent
   virtual turns instead deliver it at the command instant. When only a
   native callback tells the host that scrolling finished, the current
   session time at delivery is the event time; due runner timers run first
   at that time. Neither Chrome's earlier compositor instant nor an
   unreported UIKit/AppKit instant can be
   reconstructed. This native timing deviation is not permission to stamp
   a known Linux deadline at the next frame.
6. **Settle includes delivery and consequences.** `clock settle` must
   include unfinished scrolls, queued `scroll`/`scrollend`, commands they
   produce and subsequent scrolls. A host-owned deadline is sought; a
   platform-owned animation must finish through its executor within the
   existing settle bound. Recompute after every delivery. Give scroll-event
   turns a separate per-call cap of 16 snapshots, independent of request,
   motion and hatch budgets. Check for quiescence after the last permitted
   snapshot: an exhausted cap with further event work returns
   `settled: false`, retaining that work. A live yield spends none of this
   agent budget.
   Do not manufacture native completion by sending a synthetic DOM event,
   nor claim settlement because paint motion alone is idle. The current
   web/Apple gaps above remain implementation work.

Native scrolling's curve, duration and gesture recognition remain the
platform's (LLP 1115). Apple's unbracketed 100 ms fallback remains a
current implementation difference; where the host knows it applied an
instant command or discrete wheel notch, delaying it is avoidable. Clamp
ends, stale ends revived by showing, and an early `settled: true` are
defects, not additional permitted deviations. None is fixed by this document.

### Linux: one owner of time

Put the advancing loop in `Presenter`, where it can borrow `Host`, apply
commands, arrange and dispatch scrolling before choosing another instant.
Replace the parked `Host::advance_clock` loop and the split work in
`Presenter::{advance, animation_frame, tick, clock, clock_until_request,
clocked}` with one driver and thin mode wrappers. `Presenter::tick` is part
of this replacement: `display.rs` and `android.rs` must route their
`p.tick(wall)` calls through the same owner, including calls after
`advance`. `Host::{advance_effects, frame, advance_until_request, tick}`
become bounded steps under it. Neither `Host::observe_time` nor `Host::tick`
may assign `now_ms` past an unserviced timer or eligible completion; they
only observe/seek the instant already landed by the driver. In particular,
a shell tick at 140 cannot skip an end due at 100 and a timer due at 105.

The driver's next instant is the earliest `Runner::timer_due_ms()`,
host-owned completion deadline, eligible queued event time or requested
destination. Advance to **one due time, not one timer action**. Preserve
`Runner::advance_within`'s batching: it executes every due `then`, `next`
and timer through that time, subject to the shared budget below. Equal-time
actions keep its existing order (`then`, `next`, timers; index order within
ties, with the existing reorder-deadline handling). `Runner::land_then` likewise batches
landed continuations. `advance_until_request` stops only at an action
issuing a request, not after each action.

**Preserve bounded catch-up across steps.** One top-level agent operation
or live wake owns an `AdvanceBudget` of `TIMER_FIRE_LIMIT` (4,096) scheduled
actions, shared across all the driver's internal steps, request resumes,
and wrappers invoked in that wake. Neither reaching a new due time nor
returning from a runner call replenishes it. It is independent of
`scroll_turns`, `rounds` and `hatch_drains`. A later external operation or
live wake gets a fresh allowance.

This needs a runner change, not just a Presenter receipt counter. Add
`&mut AdvanceBudget` to private `Runner::advance_within`, and expose
`advance_timed_with_budget`, `advance_until_request_with_budget`,
`land_then_with_budget` and `frame_with_budget` for the driver. Their
existing standalone entry points create a fresh budget and delegate.
Each scheduled timer, `then` or `next` spends one unit;
virtual frame ticks and reorder-timeout receipts also count. Check the
shared remaining allowance before advancing to or consuming the next
action, including inside a same-time batch. Actual presented frame actions
and direct event dispatch keep their existing semantics; their internal
catch-up calls must use the shared budget. Counting only after a returned
batch can already have exceeded the remaining allowance.

If another scheduled action is due within the requested seek but the
allowance is exhausted, return `TimerFireLimit { limit: 4096 }` with the
runner's **landed clock**, not the candidate next deadline or requested
destination. Apply receipts already returned; keep unconsumed timers,
continuations and queued events pending. Do not dispatch the new input or
present the requested frame after this error. Inside this driver, bounded
`Host::dispatch_at` uses `Runner::dispatch` only after budgeted catch-up
has succeeded at the event's instant. It must not call `Runner::dispatch_at`,
which creates a fresh allowance and dispatches even after a timer refusal.
The standalone runner method's behavior need not change. Host and
Presenter must not seek ahead of the runner's returned time. Report the
error and landed clock in the agent reply (`settled: false` for settle).
If no further scheduled action is due through the destination, using the
last unit alone is not an error. Same-time ordering and request stops stay
unchanged; a budget stop can split a batch, just as the runner's current
local limit can.

Apply the returned receipts in order through `Host::commit_effects`, then
`after_commit`'s layout/clamps and `run_commands`, before selecting a later
time or delivering scroll events. Receipts are not historical kernel
snapshots, and commands come from one `take_commands` queue. Do not promise
host effects between actions of the same runner batch. For example, two
timers due at 100, first issuing `scrollBy`, second reading the offset and
hiding or removing its target, both run before that command is applied.
The second action sees the pre-command offset. Command execution then sees
the final hidden/destroyed tree; if no move was applied, no end is owed.
Any earlier unfinished scroll is cancelled by that loss of layout/lifetime;
an already queued end survives hiding, but not removal. This preserves the
runner's existing semantics; it is not a claim that two arbitrary browser
tasks batch their effects. The budget change does not add single-action
stepping between equal-time actions.

After that batch, revalidate completions and dispatch a snapshot in enqueue
order, applying each event handler's receipts, layout and commands before
the next event. A still-eligible fallback finishes only after the due
runner batch's effects. Consume its old debt before enqueueing its end so
a handler can create new debt. A refusal or request stop leaves undispatched
events and deadlines intact at the landed instant. Frame tasks still run
once at the presented frame, not at every intermediate deadline.

Use different event-turn progress rules for live and agent drives:

- **Live:** events created by a scroll-event handler's commands belong to
  the next platform event/rendering turn, even if the command is instant.
  Preserve them, request another turn and continue servicing the current
  turn's other deadlines, timers and input. They are not overdue work that
  prevents the clock advancing. On the next turn, stamp their eligibility
  with its arrival time, service earlier deadlines and the due runner batch,
  then deliver the retained snapshot. Subsequent reentry waits again.
  Multiple wrappers called in one shell iteration share this boundary;
  `advance` followed by `tick` is not two opportunities to drain the chain.
  A timer at 500 that stops an alternating-offset chain therefore runs even
  though every end until then starts another instant scroll. Do not spin
  another host turn synchronously at the old timestamp.
- **Agent:** `clock` and `clock settle` drain virtual event turns at the
  landed instant before selecting a later deadline. Deferred work inherited
  from a live turn becomes eligible at the current landed clock, never at
  an earlier time. Each nonempty snapshot
  spends one of a new per-call `scroll_turns` allowance of 16. Leave
  `agent.rs::clock_within`'s existing `rounds` (request retries and further
  motion deadlines) and `hatch_drains` unchanged; neither counts scroll
  turns. Share `scroll_turns` across internal driver calls in this one agent
  operation, never across operations or with live turns. Exhaustion with
  queued work returns `settled: false, reason: "scroll"` for settle; an
  explicit clock seek reports an incomplete seek at the landed clock,
  rather than claiming its requested destination. Preserve pending events.
  Ten request rounds followed by six finite handler jumps must leave room
  to deliver their final end and seek the motion deadline. An endless chain
  may exhaust this cap before a future timer; that is a bounded failed
  settle, not permission to freeze the live clock.

Move the parked `scroll_ends` ownership beside the offsets and event queue.
Use node incarnation, completion state and enqueue order, not a hash map's
iteration order or just an owed bit. For equal deadlines, preserve the order
the completions became pending. A completed event in the current snapshot
survives a later scroll; an unfinished fallback is rearmed. Revalidate each
target after preceding handlers: removal drops it, inertness does not.
`dispatch_authored_scroll` must not be a one-pass flush followed by an
advance across eligible queued work. A live continuation explicitly
deferred as above is different from an unserviced deadline.

Route `pump`, live input dispatch, `wheel_at_time`, reader samples, display
acknowledgements and agent clock drives through this owner **before** their
new input can change offsets. Timestamp a live arrival without assigning
it to `Host.now` until earlier known work has run. Raw `Host::dispatch_at`
must be private to this path, called only at the already landed instant;
its internal runner advance cannot provide presenter catch-up. Classify
offset changes at their source: input samples, instant command/binding,
animation, layout clamp or restoration. `clamp_scroll` reports position without calling the
same completion arming path as `scrollBy`; collection feedback does not
redate the move it reports. Linux's discrete `wheel_at`/`wheel_at_time`
notches and known instant commands queue completion at their application
instant. The Android reader sample stream alone uses the 100 ms fallback.
An owned animation supplies its own end.
All bounded Host calls use the budget-taking runner variants; no helper
may create a fresh allowance within the same driver operation.

`Host::timer_due_ms` (or the loop's forwarding wake query) returns the
minimum of runner and eligible completion/event deadlines. Sleeping shells
need that deadline even if no frame is wanted. Deferred live continuations
request the next event/rendering turn, never advertise their old command
time as an overdue wake deadline. `agent.rs`'s `settle`/`clock` uses the
same driver and its pending-work query, rather than a maximum-deadline jump
followed by `run_commands`. For example, reader-stream scroll A due at 100
and B due at 150 must process A's handler at 100; a new reader sample at
100 would owe an end at 200, not 250. A handler's instant `scrollBy`
instead completes in the next **agent** event turn at 100. Stepped and
single-jump agent drives must agree on delivered events and timestamps;
their final clocks differ if one explicitly seeks past the last work.
Live handler reentry follows rendering turns instead. Supply the existing
command executor to the loop; do not add a second runner, clock or general
scheduler.

### Regression reconciliation

The supplied `target/parked/scrollend-order-regressions.rs` and `.log`
contain 14 `round3_` cases and a run with 0 passed, 14 failed. That is the
parked lane's failure evidence, not a run of corrected tests in this
documentation change. Reconcile all 14 below; keep every scenario, change
the false premises rather than dropping coverage. Times are Linux logical
milliseconds. `S(y)@t` names the reported offset and event time; `E@t` names
the completion time. Every discrete wheel below follows the instant rule:
E at its application time, delivered in an event turn, with no 100 ms debt.
The late-wake fixtures instead use an Android reader sample with a known
quiet deadline, or an owned animation end. Those absolute deadlines test
the host's clock discipline; they are not Chrome wheel timings. No claim
that the parked log's wheel deadline of 100 was correct survives this rule.

For rows 5–7, queue the initial binding jump at 0 without dispatching its
events, then drive the first delivery turn to 100. Live continuation uses
an explicitly separate turn at 116; calls within the first shell turn do
not count as that second turn. These fixtures distinguish deferred live
delivery from agent fixed-point delivery.

| # | Supplied test (`round3_` prefix omitted) | Disposition and corrected expectation | Chrome evidence / limit |
|---|---|---|---|
| 1 | `shrink_rearms_a_pending_completion` | **Change.** Wheel to 800 at 0: `S(800)@0 E@0`, then `S(200)@50` from shrink. No pending end remains at 50; neither deadline 100 nor 150 is correct. Two moves, one end, `endedAt = 0`; seeking to 150 returns clock 150 without another end. Use the separate owned-animation clamp regression below for a genuinely unfinished scroll. | `wheel-1` and both bursts: a notch completes by itself. `clamp-after-end`: the later clamp emits only S. |
| 2 | `shrink_after_completion_owes_another` | **Change.** `S(800)@0 E@0 S(200)@150`; no new deadline or end at 250. One total end, `endedAt = 0`, two moves. The logged absence of a post-clamp deadline is valid, but an original end delayed to 100 is not. | `wheel-1` and `clamp-after-end`. |
| 3 | `viewport_growth_rearms_completion` | **Change.** `S(400)@0 E@0`, then growth at 50 clamps to 100 and emits S@50. One end already delivered before growth, `endedAt = 0`; no deadline after growth, rather than the logged 100 or asserted 150. | `wheel-1` plus both `viewport-growth-*` probes: growth changes position without a second end. |
| 4 | `acknowledged_growth_rearms_completion` | **Change.** Wheel's E@0 is delivered by the seek to 50. Keep presented offset 400 until `display_complete`, then 100; acknowledgement at clock 50 creates no end/deadline. Assert one total end at 0. Both logged 100 and asserted 150 are wrong. | Same wheel/clamp evidence as row 3. Chrome has no display-ack API; keeping the prior presented value until acknowledgement is a host presentation assertion. |
| 5 | `clock_drains_recursive_scroll_before_end` | **Change.** Agent seek to 100 drains `S₁ E₁ S₂ E₂` (`"sese"`), both ends at 0, final offset 200, no queued events. Clock finishes at 100; pumping there adds nothing. Drop `"sse"` and the single-end premise. | `instant-scroll-scrolls-again` preserves E₁ before the handler-created S₂ E₂. Agent virtual turns compress Chrome's separate rendering turns. |
| 6 | `advance_drains_recursive_scroll_before_end` | **Change.** Live `advance(100)` gives `"se"`, E₁ at 0, offset already 200 and S₂ E₂ retained. A pump/tick in that same turn must leave `"se"`; a new live turn at 116 gives `"sese"`, E₂ at 116, no pending work. The log's first `"se"` alone is not a failure of this rule. | `instant-scroll-scrolls-again` defers handler-created events to the next rendering turn. Their live delivery time advances. |
| 7 | `frame_drains_recursive_scroll_before_end` | **Change.** Same expectations as row 6 through `animation_frame`: E₁ at 0 in the first late drive to 100, E₂ at the new frame at 116; `"se"` then `"sese"`. Frame tasks run only at 100 and 116, not at the replayed event time 0. | Same reentry probe; frame scheduling must not change snapshot semantics. |
| 8 | `two_scrollers_settle_matches_stepped_clock` | **Change.** A's wheel E@0 starts an instant jump to 40; the seek to 50 drains its S and second E at 0 in the next agent turn. B's wheel E@50 sees 40: `"aab"`, `aEndedAt = 0`, `bSaw = 40`. Settle returns clock 50; the supplied explicit seeks through 200 return 200. Compare event fields, or stop the stepped drive at 50. Neither wheel owes a later quiet deadline; replace `"aba"` and all 100/150/200 end expectations. | Wheel probes plus `instant-scrollend-scrolls-again` and `two-staggered-with-reentry`: each completed move owes its end without another quiet interval. |
| 9 | `pump_keeps_the_deadline_before_a_timer_in_the_gap` | **Change source, keep late-wake assertion.** Replace wheel with an Android reader sample at 1000, no subsequent sample: `S@1000 E@1100 T@1105`, `"set"`, clock 1116. Pump/advance at 1116 must service E@1100. Also run an owned animation ending at 1100, with its final S before E. A wheel version instead requires E@1000 and tests instant delivery, not a delayed deadline. | `smooth` demonstrates a separate animation completion; the reader's quiet deadline is an explicit approximation. The delayed-loop traces do not override ordering on the clock Linux owns. |
| 10 | `queued_scroll_cannot_jump_past_its_deadline` | **Change.** The binding jump is instant: `S@1000 E@1000 T@1105`, `"set"`, final clock 1116. Replace expected E@1100 with E@1000; the logged E@1116 is also wrong. Collection feedback must not redate 1000. | `instant`: next-turn S E about 19 ms after assignment, no 100 ms quiet wait. Exact's agent/owned-clock projection uses the command instant. |
| 11 | `presented_clock_keeps_the_deadline_before_a_timer` | **Change source and entry point.** Use row 9's reader sample or owned animation, not a wheel: E@1100 before T@1105, final clock 1116. For the one-sample fixture the order is `"set"`. Give 1116 to the Presenter driver as an arrival; `Host::observe_time` cannot advance independently. Exercise `Presenter::tick` too. | Same owned-deadline evidence/limit as row 9; Chrome does not justify bypassing the presenter. |
| 12 | `host_dispatch_cannot_jump_past_a_completion` | **Change source and entry point.** Use row 9's reader sample or owned animation. Presenter drains E@1100 and T@1105 before press@1116; the one-sample order is `"setp"`. Raw `Host::dispatch_at` is restricted to the landed instant. A wheel here would end at 1000, not 1100. | Same limitation as row 9. Known earlier work must precede later input on the owned clock. |
| 13 | `visible_inert_ancestor_preserves_completion` | **Change.** The original wheel has S E@0, before inertness at 50; one end, no deadline at 50. To test preservation of unfinished work, add an owned animation ending at 100: make the ancestor inert at 50 and require E@100. Preserve `(input eligible: false, layout box: true)`. The log's absent deadline alone cannot establish whether E was delivered. | Wheel probes for E@0; `inert` for an ongoing animation completing despite blocked input. |
| 14 | `visible_route_below_modal_preserves_completion` | **Change.** The original wheel ends at 0, before the modal opens at 50; one end, no deadline at 50. Add an owned animation ending at 100, open the modal at 50 and require E@100 for the covered but laid-out port. | Wheel probes for instant completion; `modal` for preserving an ongoing animation. The route must retain its layout box. |

The committed parked tests and the new distinctions also require these
regressions when implementation resumes:

- Drive discrete `wheel_at` ticks at 0, 16 and 32: three S E pairs at
  those instants and no quiet deadline at 132. Exercise both live pumps
  and the agent drive. A notch clamped at an edge with no movement owes
  no events. This pins the fast-burst result independently of clamp tests.
- In `scrollend_follows_a_programmatic_scroll_and_drops_a_removed_target`,
  keep the actual live path: `pump(5000)`, tap the binding jump, then
  **`pump(5000)` delivers S E at 5000**. Replace both the post-jump
  `timer_due = 5100` assertion and the post-pump assertion that expects no
  end: after the pump, one end has fired and no deadline remains. There is
  no additional end at 5099 or 5100. Make this same live-pump correction in
  the key-driven `scrollBy` test; an agent-only replacement misses this
  path. Keep removed-target suppression using an unfinished animation or
  reader scroll, since a wheel now completes at its own instant. Preserving
  the move's time means collection feedback cannot redate 5000.
- For an owned smooth animation with known finish A, clamp at C < A and
  assert no end at C or between C and A, then exactly one E@A. Retain the
  clamp's `scroll`. Chrome's 32.8 ms clamp and 483.4 ms end demonstrate why
  an end count alone misses an animation stopped about 450 ms early.
- Hide self/ancestor during unfinished scrolling, including hide/show
  before the old deadline: cancel that end. Hide after an instant end is
  queued: deliver the queued S with zero while hidden, then E; retain 600
  internally (120 for B), restore it on show and emit nothing for the
  collapse/restore itself. A recording handler must see zero, while host
  binding/collection feedback must not overwrite the retained position.
  Include A's end hiding B before B's queued events. Keep removal and
  incarnation checks distinct from hiding.
- At one due time, two timers (and separately two landed `then`s) issue a
  command then hide/remove its target. Assert the second action sees the
  pre-command offset, apply the batch before revalidating ends, and check
  the hidden-versus-removed queued-end distinction. This tests the actual
  runner batch boundary, not nonexistent single-action stepping.
- Exercise `tick` alone and `advance` followed by `tick` at 140 with a
  reader sample at 0 owing an end at 100, and separately an owned animation
  ending at 100, plus a timer at 105: E@100 then T@105, clock 140. Neither
  fixture may use a wheel to manufacture the 100 ms deadline.
  Also test an earlier timer that scrolls/hides/removes and the timer/end
  tie at 100. Runner actions and their batch effects win that tie.
- At Presenter level, boot a repeating 1 ms timer with its first tick at
  1, no scroll events and no other scheduled work. Seek to 1,000,000 in
  one agent call: exactly 4,096 timer actions, `TimerFireLimit`, runner and
  host clock 4096, next timer still due at 4097, requested destination not
  reached. A new call to 4097 fires that tick once. Check that `scroll_turns`
  remains unused. Also exhaust the aggregate allowance inside a same-time
  batch to verify the remaining allowance is enforced before action 4,097,
  rather than counting the batch after it ran. This preserves the bounded
  catch-up in `runner/tests/it/now_screen/timer.rs` across Presenter steps.
- Live reentry alternating two offsets, stopped by a runner timer at 500:
  use successive live turns, assert the timer runs at 500 and the final
  queued end drains without generating another move. A separate input in
  a later live turn must also receive service. Do not assert Chrome's
  measured count of 31 or its frame interval.
- Agent settlement: a finite chain, an endless chain exhausting only
  `scroll_turns`, and ten request rounds followed by six finite handler
  jumps and a motion deadline. The finite case delivers every end and
  reaches that deadline before reporting settlement. Check the independent
  counters and retained work on failure; a live yield consumes none.
- Retain movement/no-op, end-only-handler and input timestamp coverage;
  both handler reentry cases; both two-scroller enqueue orders and A's end
  moving B. A jump interrupting an unfinished animation has one end, unlike
  a jump following an already completed instant scroll. Android reader
  timestamps and physical Apple gesture completion still need native
  qualification.

QUEUE remains open for the host implementation and native qualification.
