# LLP 1006: Contract compiler v1 — what `contract/` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Contract (compiler), Plan, Dev loop
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1004 (the decisions; deviations from its text are named in §7), LLP 1005 (the format this emits and the runner that executes it), LLP 0508 (Contract v1 Edition 1 — the semantics adopted for the enumerated constructs; research)

## Summary

`contract/` turns a `.contract` file into a plan: parse → infer closed types
→ analyze effects → lower to tables and bytecode, byte-identically, in the
kernel's vocabulary. Five crates on a cargo-enforced DAG, one driver, one
CLI, and a build-time bake that boots the runner once so every resource's
boot value ships inside the plan. The v1 app (`apps/caltrain/app.contract`,
129 lines, plus a 282-line Rust data crate) compiles, bakes, boots, lays
out, and ticks, with no JavaScript anywhere. Where this document and the code
disagree, the code and its tests are the authority.

## 1. Crates (LLP 1004 D2, as landed)

| Crate | Lines | Depends on |
| --- | --- | --- |
| `contract-syntax` | 1,825 | nothing |
| `contract-types` | 1,100 | `contract-syntax`, `exact-plan` (the roster's signatures) |
| `contract-analyze` | 354 | `contract-syntax`, `contract-types` |
| `contract-lower` | 953 | `contract-analyze`, `contract-syntax`, `contract-types`, `exact-plan`, `exact-kernel` |
| `contract` (driver + CLI) | 149 | all of the above, `exact-runner` (for the bake) |

Cargo refuses a cycle; layering is a `Cargo.toml` edit a reviewer sees. Every
file is under the 1,500-line cap.

## 2. The language (LLP 1004 D3, scoped to the v1 app)

**Declarations.** `shape Name` with typed fields (`number string bool`, a
shape name, `option<T>`, `list<T>`); `component Name` with sections `props`,
`state`, `derive`, `resource`, `action`, `task`, `contract` (parsed, not
compiled), `view`. The first component is the root; only the root holds
`state`/`derive`/`resource`/`action`/`task` (`analyze-child-state`); a child
component is a view over its `props`, and a prop of type `action` is an
action reference the use site supplies.

**Resources.** `resource name = source(args) as shape T`: `source` names the
app's data source, `args` are expressions over state, `T` is the declared
shape (LLP 1004 D4). There is no `use … from` (`contract-no-imports`).

**Actions.** `action name(params) writes a, b` with a body of `slot = expr`
assignments and `name(args)` commands; a parameter's type is written or
inferred from its handler call sites (the handler attributes are `press`,
`change`, `hover`, `focus`, `blur`, `key`, LLP 1005 §3; a `key`'s or
`change`'s payload types the last parameter `string`, a `hover`'s `bool`);
an assignment to an undeclared slot is
`analyze-write-not-declared`. **Tasks.** `task name mount` with
`every(ms, action)` with a whole positive number of milliseconds
(`lower-timer-interval`). **View.** Elements `tag positional attr=expr …` with
indented children; component uses `Name(arg=expr, …)`; `when cond … else …`;
`each x in list key=expr`; `match subject` with `case some(x)` and `case
none`. **Expressions.** Numbers, strings, templates with `${…}`, `true`/
`false`, `none`, `some(e)`, names, `a.b`, roster calls, `+ - * / %`, `== !=
< <= > >=`, `and`/`or`/`not` (or `&& || !`), `c ? a : b`, and inline `match s
{ case some(x) => a, case none => b }`.

**Types** come from initializers, shapes, props, and the roster; `none` alone
is `option<?>` and the `?` is filled by the first write that says what it
holds (`state stationId = none` … `stationId = some(id)`); an unfilled `?` is
`type-cannot-infer`, never a guess. Derives are inferred to a fixpoint in any
order; a cycle is `type-derive-cycle`. Every rejection carries a stable id and
a line:column (`CompileError`).

## 3. Passes

**Syntax** (`contract-syntax`): an indent-aware lexer (`Indent`/`Dedent`,
bracket-aware line continuation, template strings lexed whole), a
recursive-descent parser with spans on every node, and **inlining**: every
component use becomes the used component's view with props substituted by the
use's argument expressions (a curried `press=prop(args)` becomes
`action(parent-args…, args)`) and the child's bound names renamed apart. Both
later passes run on the same expansion.

**Types** (`contract-types`): `Ty`, `Shapes`, the shared `Scope`/`Ref` (how
a name resolves: slot, derive, resource, action, prop, param, `each` item at
a region depth, `match` binding at a depth, inline-match local), `infer`, and
`check`. The root is checked against its inlined view so a handler's real call
site — behind a child's prop — types the action's parameters; children are
checked standalone. Roster calls are checked against the table's `params`/
`returns`.

**Analyze** (`contract-analyze`): `writes` declared and honored, handler
shape and arity (`change` and `key` supply a string as the last parameter,
`hover` a bool, `press`/`focus`/`blur` nothing — `HANDLERS` and
`handler_payload` in `contract-analyze`), timer
actions exist and take no parameters, component uses name real components
with each argument once, children carry no state.

**Lower** (`contract-lower`): shapes to `types`; declarations to `slots`,
`derives`, `resources`, `actions`, `timers` in source order; the inlined view
to `nodes`/`regions`/`arms`/`bindings`/`handlers` through the tag/attribute
table (`tags.rs`: `column`/`row`/`main`/`scroll`/`text`/`button`/`link`/
`input`/`image` onto kernel node types plus fixed rows; attributes onto style
rows by CSS name — `size`→`font_size`, `gap`→`row_gap`+`column_gap`,
`padding`→four rows, `radius`→four rows, `flex=n`→CSS `flex: n` = grow n,
shrink 1, basis 0% … — or props or handlers; anything else is
`lower-unknown-attr`; a leaf tag with children, or a `text` holding anything
but `text` runs, is `lower-leaf-children`); every expression through one assembler
(`expr.rs`: `and`/`or` short-circuit through a local; inline `match` binds a
local; a non-string template part gets `toString`). Row order is source
order, so compilation is byte-identical (`the_app_compiles_deterministically…`).

**Driver** (`contract`): `compile(src) → Plan`; `bake(plan, data) → Plan`
boots the runner once against the app's data source and writes every
resource's boot value into the data pool, so the first frame on a device
queries nothing (`the_first_frame_needs_no_data_source`). The CLI: `contract
build <file> [-o <plan>]` prints a one-line summary or a rejection as
`file:line:col [id] message`, exit 1.

## 4. The corpus (LLP 1004 D6)

`contract/corpus/now-screen.contract` is the runner's hand-built plan as
Contract; the corpus test compiles it, round-trips the bytes, bakes, boots,
and asserts the same behavior the hand-built test asserts (keyed reorder,
`when` flip, timers, commands) — proven at both ends. `rejects.txt` holds one
fixture per diagnostic id (26 today), each refused with exactly its id. The
app itself is the integration fixture (`apps/caltrain/tests/app.rs`).

## 5. The v1 app (`apps/caltrain`)

`app.contract`: three shapes, a root with 4 slots, 1 derive, 7 resources, 6
actions, 1 timer, and two child components; compiles to 57 nodes and 11
regions, 9,893 bytes. `caltrain-data` implements `DataSource` for
`defaultLocation`, `stations`, `nearest`, `station`, `board`, `search` over a
seeded nine-station corridor with clock-face departures — exact1's `data.ts`,
in Rust, one implementation for every host and for the bake.

## 6. Identity and refusal (LLP 1004 D3)

A plan carries `FORMAT_DIGEST`, the kernel's `SCHEMA_DIGEST`, and
`compiler_identity()` (an FNV fold of the lowering crate's version; there is
no configuration yet). The decoder refuses a format mismatch and the runner a
kernel schema mismatch; the compiler identity is carried for a host to compare
against the compiler it expects — the runner has no expected identity of its
own (a 2026-08-28 review correction to an earlier overclaim here).

## 7. Deviations from LLP 1004's text

- **Inlining lives in `contract-syntax`, not `contract-lower`**: type
  inference needs it (§3). Purely syntactic, so it belongs where the AST is.
- **The driver bakes; `contract-lower` does not link the app's data crate.**
  1004 D2/D4 had lowering evaluate constant resources at build time; what
  landed is simpler and more general — the driver boots the runner once, so
  *every* resource's boot value (not only constant-argument ones) is compiled
  data. The `contract` driver depends on `exact-runner` for this.
- **`exact-plan` was created by this lane**, not by a separate runner lane —
  the format, runner, and compiler landed together, which is what made the
  format real (LLP 1000's lane order, revised again).
- **Only the root holds state** (§2) — a scope narrower than 1004 D3's
  construct list implies; child components with state are a later addition.
- **`analyze-derive-cycle` became `type-derive-cycle`**: the fixpoint that
  finds it is in the type pass.

## 8. Not in v1 (and where each is declared)

~~An incremental/resident driver and the ≤20 ms slice~~ — the resident
driver landed with the web host the same day (LLP 1007 §6,
`host/web/src/dev.rs`): a save is observed, compiled, and baked in 8–13 ms
including a 10 ms poll, so the ≤20 ms slice (1004 D5) holds with no
incremental compilation at this size; the CLI stays a one-shot.
**compile-time checking of attribute
values against their kernel rows** (`width=true` compiles and fails at first
frame — a typed refusal, but late; the 2026-08-28 review's circle-back);
**handler arity through a bare `action` prop** (checked at dispatch, not
compile time — the other circle-back); a total inlining budget beyond the
depth guard; `@keyframes`, the `contract` block as
executable assertions, `cursor`, per-instance state, LSP/formatter, `linear()`
and transition rows from Contract (the kernel has the row; the tag table does
not yet expose `transition`). Each is a fixture away, never a speculation.

## 9. Checks that hold this

`contract/syntax/tests/parse.rs`, `contract/cli/tests/corpus.rs`,
`apps/caltrain/tests/app.rs`, and every crate's unit tests, all under
`cargo test --workspace` (135 tests across the workspace on 2026-08-28);
clippy `-D warnings`, fmt, and `caps` green.
