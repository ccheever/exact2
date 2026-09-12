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
`resource`/`mutation`/`task` (`type-child-resource`); a child component is a
view over its `props` that may own `state`, `derive`, and `action` of its own
(LLP 1017 P4c, 2026-08-30 — see **Instances** below), and a prop of type
`action` is an action reference the use site supplies.

**Resources.** `resource name = source(args) as shape T`: `source` names the
app's data source, `args` are expressions over state, `T` is the declared
shape (LLP 1004 D4). **Files.** `use Name from "./file.contract"` brings a
component, shape, or style from another Contract file, resolved by
`contract::compile_path` (LLP 1017 P8, 2026-08-30: the used file's
declarations are merged in after this file's own; a cycle, a missing file, an
unknown name, or a name declared differently in both is refused by name);
anything but a `.contract` path is `contract-no-imports`, as before.
`textarea` supplies `white-space: pre-wrap` and `overflow-wrap: break-word`,
matching the browser control's wrapping defaults. An explicit declaration
uses the ordinary style row and overrides that tag default.

**Line height** (LLP 1035.000.000): `line-height=1.5` is a font-size
ratio, `line-height="24px"` a fixed length, and `line-height="normal"`
restores natural font metrics. Zero is explicit. Dynamic fixed lengths use
existing interpolation, ``line-height=`${height}px` ``. Negative/nonfinite
values, percentages and font-relative units are refused by the kernel row
parser; numeric literals, including unary minus, are checked at compilation.

**Styles.** `style Name` with lines of `attr=literal` (style rows only), applied
by `class=Name` on a node, the node's own attribute winning (LLP 1017 P6).
**Editor hints** (2026-09-09, Messages). `autocapitalize` and `autocorrect`
lower to same-named string props on `input` and `textarea`. Their values use
[HTML's vocabulary](https://html.spec.whatwg.org/multipage/interaction.html#autocapitalization):
`none`/`off`, `sentences`/`on`, `words`, `characters` for capitalization, and
`on`/`off` for correction. They are input-method hints, not text transforms.
Contract has no form-owner or contenteditable model; these hints are applied
directly to the declared editors.
`spellcheck` is a same-named string prop on any element: `"true"` and `"false"`
(case-insensitive), with empty meaning true and invalid/missing values deferring
to ancestors and then the editor default. The browser receives the authored HTML
attribute; native editors receive the nearest explicit logical-tree hint.
`swipeContent`, `swipeLeading` and `swipeTrailing` lower to string props;
`destructive` lowers to a boolean. Their id references are resolved by the
native presenter after keyed nodes exist, not inferred from test ids. The
kernel declaration and native boundary are in LLP 1001 §1 and LLP 1008 §9.

`contextMagnify` lowers to a boolean prop controlling the `contextTarget`
preview's host enlargement (LLP 1001 §1); expressions may change it as the
presentation mode changes. It does not lower to a CSS transform.
`emojiPicker` lowers to a boolean prop for the selection-input policy in LLP
1001 §1. It uses the ordinary `change` action and an empty authored input value;
it does not introduce an `inputmode` value or a new event.

**The environment** (2026-08-30): a style attribute's text may be an `env()`
length — `padding-top="env(safe-area-inset-top)"`,
`margin-bottom="calc(env(safe-area-inset-bottom) + 12px)"` — passed as text by
the runner's bridge and parsed by the kernel (LLP 1001 §2; any other text on a
dimension row is refused at boot, so at bake). `viewport-fit="cover"` and
`interactive-widget="resizes-content"` are attributes (the viewport meta's
keys, spelled as the web spells them; `viewportFit`, `safeArea`,
`keyboardAvoidingView` in the did-you-mean) lowering to the `viewportFit` and
`interactiveWidget` props, which a host reads from the first root (LLP 1008
§9, LLP 1007 §4). `contract/corpus/insets.contract`,
`contract/corpus/keyboard-bar.contract`, `tests/insets.rs`.
**Functions** (LLP 1017 P5, 2026-08-30): `fn name(param: type, …): type =
**Tests** (LLP 1017 P7, 2026-08-30): `test "name"` blocks — normally in
`app.test.contract` beside the app — whose steps are the agent API's
operations (`tap`, `type`, `clock`, `screenshot`) and `expect tree|text|state`
lines over their replies; parsed by `contract test <file>` into JSON and run by
`scripts/agent.mjs <host> --test <file>`; never compiled into the plan, never a
second evaluator. **Functions** (LLP 1017 P5, 2026-08-30): `fn name(param: type, …): type =
expr` at file scope — one expression over its parameters and the roster only,
typed like a roster call, expanded inline at each call (no opcode, no table);
a cycle is `type-fn-recursive`, a roster name `contract-fn-shadows-roster`.
**Instances** (LLP 1017 P4c, 2026-08-30): a child may own `state`, `derive`,
and `action` (never a resource, mutation, or task — `type-child-resource`);
`expand` lifts them into the root per use, renamed apart, a derive as a
substituted expression, and a use under an `each` makes its states row slots
(`slots.owner`), one value per keyed row on the runner. The "only the root
holds state" rule of §2 and §7 is gone. **Composition** (LLP 1017 P4a/b, 2026-08-30): a component may declare `inject`
(typed names, like `props`) that a use site does not pass — the nearest
enclosing `provide name = expr` fills them at inlining, the innermost winning,
none on the path `syntax-missing-provide`; and `slot`, so that the nodes
indented under a use of it replace its `children` node, inlined in the use
site's scope (`syntax-no-slot`, `syntax-children-without-slot`). Both are the
inliner's; nothing reaches the plan.

**Mutations (LLP 1016, decided A, 2026-08-30).** `mutation name as shape
T` declares an `option<T>` slot, `none` at boot, that only a `send` fills:
`send name = source(args)` in an action asks the data source once (the
mutation must be in the action's `writes`); `refresh resource` re-requests a
resource with its current arguments; `pending(x)` is `bool` for a resource
or mutation `x` — a name, not a value, so it is not a roster entry. The name
reads as `option<T>` (`match session { case some(s) => … }`) and may be
assigned (`session = none`), which forgets a reply in flight. **Actions.**
`action name(params) writes a, b` with a body of `slot = expr`
assignments, `send`/`refresh` statements, `name(args)` commands, and — since
2026-08-30, LLP 1017 P2 — `if cond` … `else` … and `match option` with `case
some(x)` / `case none` blocks of statements, nested as deep as wanted, with no
loops (a body still always terminates, LLP 1005 §2; `writes` covers every
branch; `if` needs a bool, `type-condition`; the `match` binds its name as a
local, as the inline form does); a parameter's type is written or
inferred from its handler call sites (the handler attributes are `press`,
`change`, `hover`, `focus`, `blur`, `key`, `submit`, `contextmenu`, `dblclick`, LLP 1005 §3 — `submit`
on an `input` is Enter, the web's implicit submission; a `key`'s or
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

`image "symbol:<role>"` (LLP 1035.004, 2026-09-10) checks literal roles
against the generated schema vocabulary. Empty, unknown and platform-name
sources are `lower-attr-value` with the available roles; dynamic sources remain
host-checked. `tint-color` names the schema's colour row and accepts `light-dark()`.

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

HTML `dialog` lowers to a View with `semanticTag="dialog"` and the absolute
position default; `commandfor` and `command` are schema props, passed by their
HTML names. Their presentation belongs to the host (LLP 1021 D2), with no
compiler-created open-state slot.

**Analyze** (`contract-analyze`): `writes` declared and honored, handler
shape and arity (`change` and `key` supply a string as the last parameter,
`hover` a bool, `press`/`focus`/`blur`/`submit` nothing — `HANDLERS` and
`handler_payload` in `contract-analyze`), timer
actions exist and take no parameters, component uses name real components
with each argument once, children carry no state.

**Lower** (`contract-lower`): shapes to `types`; declarations to `slots`,
`derives`, `resources`, `actions`, `timers` in source order; the inlined view
to `nodes`/`regions`/`arms`/`bindings`/`handlers` through the tag/attribute
table (`tags.rs`: `column`/`row`/`main`/`scroll`/`text`/`button`/`link`/
`input`/`image` onto kernel node types plus fixed rows; attributes onto style
rows by their **literal CSS names** (LLP 1017 §8.1, 2026-08-30 — `font-size`,
`background-color`, `border-radius`→four rows, `gap`→`row_gap`+`column_gap`,
`padding`→four rows, `flex=n`→CSS `flex: n` = grow n, shrink 1, basis 0%;
hyphens are grammar, and the lexer reads `a-b` as one name as CSS's `calc()`
does, so subtraction between names is `a - b`) or onto props by their HTML
and ARIA names (`aria-label`, `aria-description`, `aria-level`, `role`,
`placeholder`, `value`, `href`, `disabled`, `inert`, `lang`; `testId` is the one
Exact-named attribute) or handlers; there are no aliases — an old spelling
(`size`, `fontSize`, `radius`, `label`) is `lower-unknown-attr` naming the
CSS name it became; **a literal value is checked against its rows at
compile time by the kernel's own parser** (`StyleProps::set_dynamic` on a
probe: `width=true`, `align-items="middle"`, `background-color="red"` are
`lower-attr-value`; a computed value that is not a number or a string, or a
prop of the wrong type, is `lower-attr-type`); a handler behind a child's
`action` prop names the real action after inlining and its arity is
`lower-handler-arity`; a `when`/`each`/`match` at the root is
`lower-root-region`; a `scroll` with no `height`, `max-height`, or `flex`
under a parent that stacks is `lower-scroll-unbounded`, and a childless
`button`/`link` with no size is `lower-zero-size` (the two conservative
layout refusals; the measured ones are bake's, §3 Driver); a leaf tag with
children, or a `text` holding anything
but `text` runs, is `lower-leaf-children`); every expression through one assembler
(`expr.rs`: `and`/`or` short-circuit through a local; inline `match` binds a
local; a non-string template part gets `toString`). Row order is source
order, so compilation is byte-identical (`the_app_compiles_deterministically…`).

**Driver** (`contract`): `compile(src) → Plan`; `bake(plan, data) → Plan`
boots the runner once against the app's data source and writes every
resource's boot value into the data pool, so the first frame on a device
queries nothing (`the_first_frame_needs_no_data_source`). Since 2026-08-30
(LLP 1017 P1d) bake is also **the layout lint**: the first frame is laid out
at `LINT_VIEWPORT` (390×844, a phone) on the monospace measurer, and a
`scroll` that is exactly as tall as its children with nothing bounding it
(`height`, `max-height`, and `flex_grow` all unset) is `bake-scroll-unbounded`,
a pressable with zero area is `bake-zero-size` (one holding an image or a
canvas is exempt — their size is the host's), each named by the node's
`testId`; `bake` returns `BakeError` — the runner's refusal or the lint's —
and every host's `build.rs` fails on either (`contract/cli/tests/lint.rs`).
The compiler cannot see layout; bake can, and it already had the kernel. The CLI: `contract
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
- ~~**Only the root holds state**~~ — held from 2026-08-28 to 2026-08-30;
  LLP 1017 P4c gave children `state`, `derive`, and `action` (§2 Instances).
- **`analyze-derive-cycle` became `type-derive-cycle`**: the fixpoint that
  finds it is in the type pass.

## 8. Not in v1 (and where each is declared)

~~An incremental/resident driver and the ≤20 ms slice~~ — the resident
driver landed with the web host the same day (LLP 1007 §6,
`host/web/src/dev.rs`): a save is observed, compiled, and baked in 8–13 ms
including a 10 ms poll, so the ≤20 ms slice (1004 D5) holds with no
incremental compilation at this size; the CLI stays a one-shot.
~~compile-time checking of attribute
values against their kernel rows~~ and ~~handler arity through a bare
`action` prop~~ — both landed 2026-08-30 under LLP 1017 P1 (§3 Lower), with
the root-region and the two layout refusals, and bake's layout lint (§3
Driver); a total inlining budget beyond the
depth guard; `@keyframes`, the `contract` block as
executable assertions, `cursor`, per-instance state, LSP/formatter, `linear()`
and transition rows from Contract (the kernel has the row; the tag table does
not yet expose `transition`). Each is a fixture away, never a speculation.

## 9. Checks that hold this

`contract/syntax/tests/parse.rs`, `contract/cli/tests/corpus.rs`,
`apps/caltrain/tests/app.rs`, and every crate's unit tests, all under
`cargo test --workspace` (135 tests across the workspace on 2026-08-28);
clippy `-D warnings`, fmt, and `caps` green.

The `swiperight` handler accepts an action and its captured arguments, like
`press`; the compiler emits its distinct EventKind. `touch-action` lowers to
the schema's CSS keyword row. Messages uses the pair to open an inline reply
without replacing vertical scrolling or leftward timestamp reveal.

`scroll=action(args)` (2026-09-09, Messages) appends two numeric arguments,
`scrollLeft` then `scrollTop`. Analysis and lowering check both arguments;
type inference refines both untyped parameters and rejects incompatible explicit
types. The corpus checks currying, round-trip dispatch, arity and both types.
