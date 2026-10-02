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
| `contract-types` | 1,100 | `contract-syntax`, `exact-plan` (the roster's signatures), `exact-route` (route checks) |
| `contract-analyze` | 354 | `contract-syntax`, `contract-types` |
| `contract-lower` | 953 | `contract-analyze`, `contract-syntax`, `contract-types`, `exact-plan`, `exact-kernel` |
| `contract` (driver + CLI) | 149 | all of the above, `exact-runner` (for the bake) |

Cargo refuses a cycle; layering is a `Cargo.toml` edit a reviewer sees. Every
file is under the 1,500-line cap.

## 2. The language (LLP 1004 D3, scoped to the v1 app)

**Declarations.** `shape Name` with typed fields (`number string bool`, a
shape name, `option<T>`, `list<T>`); `component Name` with sections `props`,
`state`, `derive`, `resource`, `action`, `task`, `view`. A `contract`
section is `syntax-contract-block`, whose message points at `test` blocks in
`app.test.contract` (LLP 1017 P7; LLP 1035.005.000 D8, 2026-10-02: it was
parsed and compiled to nothing, so assertion text promised what nothing
enforced). The first component is the root; only the root holds
`resource`/`mutation`/`task` (`type-child-resource`); a child component is a
view over its `props` that may own `state`, `derive`, and `action` of its own
(LLP 1017 P4c, 2026-08-30 — see **Instances** below), and a prop of type
`action` is an action reference the use site supplies.

**Routes** (LLP 1038 D2/D3, 2026-09-14): one file-scope `routes <slot>`
in the app's root file. Route lines are `[tab] <name> "<pattern>"`;
indentation names the enclosing route as parent. Patterns are absolute paths
with literal segments or whole `:identifier` segments. An optional bare
`notfound` is the fallback. Rows retain declaration order; parents are earlier
row indices; `tab` and `notfound` are table flags. For example:

```
routes nav
  tab home "/"
    post "/post/:post"
  tab prompts "/prompts"
    question "/prompt/:question"
      write "/prompt/:question/write"
  notfound
```

The declaration inserts a root state slot before authored initializers and
instance lifting, with no authored initializer. Its type is `Router`; launch
fills it before initializers run. Actions assign it ordinary values. Only apps with `routes` receive the four compiler shapes,
with these exact positional field orders:

- `Router { tab: string, tabs: list<Tab>, next: number }`
- `Tab { name: string, stack: list<Entry> }`
- `Entry { id: number, name: string, url: string, tab: string, params: Params }`
- `Params { <each distinct :name>: string }`, in first-declaration order
  across the table. Every entry has every parameter field; an unbound field
  is the empty string. An undeclared field is `type-unknown-field`.

The roster resolves those shapes and typed lists: `open`, `push`, `replace`,
`select`, `go` take `(Router, string)` and return `Router`; `back(Router)`
returns `Router`; `stack(Router)` returns `list<Entry>`, `top(Router)` an
`Entry`, `depth(Router)` a number, and `params(Router, string)` a
`list<string>`. `searchParam(Entry, string)` and `encodeURIComponent(string)`
return strings. Thus `derive current = top(nav)`, `nav.tab`,
`each e in stack(nav) key=e.id`, and `` navigationKey=`${e.id}` `` use ordinary
field reads, derives, keyed regions and templates. The `.d.ts` generator
already declares every plan type as `T<id>`, including these four records.

`path("name", args…)` expands at its call site into the pattern's template,
encoding each parameter with `encodeURIComponent`; numeric arguments pass
through `toString` first. The table checks the name and exact argument count;
parameters accept strings or numbers. There is no per-route generated function
or typed parameter record. A location argument to `open`, `push`, `replace`
or `go` is a literal checked against the table (a `notfound` fallback does not
validate an unmatched literal), a `path()` call, or another non-template
expression checked by the runner. A direct template is `route-template`, with message "use `path()`".
`select` takes a tab name, so its string is not a location check.

`Table::path` now distinguishes unknown path-route names from wrong parameter
counts in its refusal text. Unknown names list non-fallback routes in declaration
order; count errors report the expected count and parameter names in pattern
order, including inherited parameters. The compiler forwards that guidance and
separately explains a missing `routes` declaration or nonliteral first argument.
The `route-unknown` ID and existing call span remain; empty/dot-only segment and
argument-type refusals keep their prior text/priority. No choices are built for
valid calls. Five CLI repair fixtures use the reported names or parameter order
and original source locations, with identical repaired plans and unchanged
non-message diagnostic fields; all 18 app/fixture plans also match. The shared
route corpus and compiler integration tests pass. This is repair guidance, not a
runtime performance or general agent-productivity claim.
Evidence: `/tmp/exact-route-guidance-213a2765`.

**Decided (chunk (c), 2026-09-14):** compatible scoped action/action-prop
references keep precedence over the roster. A Router-valued first argument
selects the roster overload when it does not fit the action signature, so
`action back` can assign `nav = back(nav)` and Messages' `press=open(id)`
continues to bind its action. `routes` is refused in any used file: its
component is a child of the using app, never that app's root. The existing
`fn` namespace still precedes compiler-only `path`; roster names themselves
remain `contract-fn-shadows-roster`.

**Resources.** `resource name = source(args) as shape T`: `source` names the
app's data source, `args` are expressions over state, `T` is the declared
shape (LLP 1004 D4). **Files.** `use Name from "./file.contract"` brings a
component, shape, style, or function from another Contract file, resolved by
`contract::compile_path` (LLP 1017 P8, 2026-08-30: the used file's
declarations are merged in after this file's own; a cycle, a missing file, an
unknown name, or a name declared differently in both is refused by name);
anything but a `.contract` path is `contract-no-imports`, as before.
An unknown import keeps `contract-use-unknown` and its original file/span,
and lists the referenced file's resolved exports by kind, in merge order
within each kind. Transitive declarations are included; the importing file's
own declarations and unrelated imports are excluded. Fonts cannot name a
`use` and are not offered. Empty exports say so. Choices are constructed only
on refusal. Four CLI repairs select a component, shape, style, and function
from these choices; all other diagnostic fields and the repaired plans match
the previous compiler, as do 18 app/fixture plans.
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
`contract/corpus/keyboard-bar.contract`, `contract/cli/tests/it/insets.rs`.
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
`send name = source(args)` in an action asks the data source once;
`refresh resource` re-requests a
resource with its current arguments; `pending(x)` is `bool` for a resource
or mutation `x` — a name, not a value, so it is not a roster entry. The name
reads as `option<T>` (`match session { case some(s) => … }`) and may be
assigned (`session = none`), which forgets a reply in flight. `mutation name
as shape T refreshes a, b` (LLP 1054.000.000 D1) names the root's resources a
send changes: each is re-requested, forced, in the commit that sends and in
the one where the reply lands (`type-refreshes-not-resource`,
`type-refreshes-duplicate`). A resource's `else` is a source call over values
(LLP 1048.003 D6) or `empty(field=value, …)`, its type's zero with named fields
replaced by constants (LLP 1054.000.002); without `else`, the zero shows while
it is pending. `failed(resource)` (LLP 1054.000.002, ruled 2026-09-27) reads
whether the latest request for the current arguments failed without an
answer. It takes one resource name (`type-failed-argument` otherwise), not a
mutation or value, and returns `bool`. A failed request keeps the last value
or placeholder with `pending` false and `failed` true; a placeholder remains
a placeholder. Changed arguments or `refresh` clear the failure and allow a
new request; a successful answer clears it too. A failure the source shapes
into an answer is an answer, not `failed`. Like `pending`, this is a compiler
call, not a value-taking roster entry. **Actions.**
`action name(params)` with a body of `slot = expr`
assignments, `send`/`refresh` statements, `name(args)` commands, and — since
2026-08-30, LLP 1017 P2 — `if cond` … `else` … and `match option` with `case
some(x)` / `case none` blocks of statements, nested as deep as wanted, with no
loops (a body still always terminates, LLP 1005 §2; `if` needs a bool,
`type-condition`; the `match` binds its name as a local, as the inline form
does); a parameter's type is written or
inferred from its handler call sites (the handler attributes are `press`,
`change`, `hover`, `focus`, `blur`, `key`, `submit`, `contextmenu`, `dblclick`, `navigate`, LLP 1005 §3 — `submit`
on an `input` is Enter, the web's implicit submission; a `key`'s or
`change`'s payload types the last parameter `string`, a `hover`'s `bool`).
An action's effects are inferred, never declared (LLP 1035.005.000 D1,
2026-10-02): the plan's `actions.writes`, the VM's `StoreSlot`/`Send`
allowlist, is exactly the states and mutations the body assigns or sends
through every branch, in slot order. A `writes` clause after the parameters
is `syntax-writes-clause`, whose message says to delete it; there is no
compatibility before 1.0, and one mechanical rewrite removed every clause.
What the clause once caught is scope and type: an assignment to anything but
the component's own state or mutation is `type-assign-not-state`, a `send` to
anything but its mutation `type-send-not-mutation`. An unintended write to a
valid, in-scope slot is no longer refused; `contract symbols` shows each
action's inferred `writes` on its definition. **Tasks.** `task name mount` with
`every(ms, action)` (fires at boot+ms and every ms after), `every(frame,
action)` (once per presented frame, never caught up; LLP 1073) or `after(ms,
action)` (fires once at boot+ms, then is spent and reports no deadline),
with a whole positive number of milliseconds (`lower-timer-interval`). **View.** Elements `tag positional attr=expr …` with
indented children; component uses `Name(arg=expr, …)`; `when cond … else …`;
`each x in list key=expr`; `match subject` with `case some(x)` and `case
none`. **Expressions.** Numbers, strings, templates with `${…}`, `true`/
`false`, `none`, `some(e)`, `[]` (the empty list, 2026-09-28: its element
type comes from the other arm of a `match` or `?:`, a declared `list<T>`, or
a write into the state it initializes, as `none`'s does; an `[]` nothing
types, in a derive, a source argument, or an `each` list, is
`type-cannot-infer` at the `[]`, and a state only `[]` initializes is
refused as a state nothing writes; a list literal with items stays out,
LLP 1017.003 D4), names, `a.b`, roster calls, `+ - * / %`, `== !=
< <= > >=`, `and`/`or`/`not` (or `&& || !`), `c ? a : b`, and inline `match s
{ case some(x) => a, case none => b }`.

`includes(text, substring) -> bool` performs a case-sensitive literal substring
search (the empty substring matches), as the web's `String.prototype.includes`
does; `startsWith(text, prefix)` and `endsWith(text, suffix)` are the web's
too (renamed from `contains` on 2026-09-28: the words are the web's, LLP 1017
§8.1). The Markdown toolbar uses `includes` with spaces around both the token
list and the requested token, so `code` never matches `codeblock`. The declared
roster and the runner provide these operations to every host; they do not
execute app JavaScript.

**Types** come from initializers, shapes, props, and the roster; `none` alone
is `option<?>` and the `?` is filled by the first write that says what it
holds (`state stationId = none` … `stationId = some(id)`); an unfilled `?` is
`type-cannot-infer`, never a guess. Derives are inferred to a fixpoint in any
order; a cycle is `type-derive-cycle`. Every rejection carries a stable id and
a line:column (`CompileError`).

**Every independent refusal in one run** (2026-09-22). `contract build` and
`--json` (an array) report each independent mistake, at most 20;
`compile_path_all` returns them and the other entry points return the first.
The lexer refuses per line and the parser per top-level declaration, resuming
at the next line that starts in column 1; a declaration keyword there also
ends a bracket left open above it, since it can never continue an expression. Expansion records a use it cannot
expand and continues, a missing value reading as `?`. Types records per state,
derive, statement, view attribute and use, analysis per component and action,
lowering per element and attribute. A later refusal that mentions `?` is a
consequence and is dropped, as are repeats. A misspelled or mistyped call site
is reported first, and what it broke inside a child is left out. A pass runs
only on what the one before accepted, except that when types or analysis
refuse, the authored elements are still linted for what needs no types: tags,
attribute names and literal values. Refusals name the author's attribute,
not the kernel row, and suggest the one spelling a slip most plausibly meant,
for names, fields, props, tags and attributes. An expression may nest 64
deep and its tree be 100 deep, which fits a 2 MB thread's stack even
unoptimized; past either is `syntax-expression-depth`, never a stack overflow.
`length` and `isEmpty` take a string or list and `toString` a number, string
or bool; the roster's `any` is the checker's, so the table is unchanged.
`at(list<T>, number)` is `option<T>` (2026-09-28): JavaScript's
`Array.prototype.at`, the index truncated toward zero (NaN is 0), a negative
one counted from the end, `none` where the web answers `undefined`; `xs.at(i)`
is refused with that spelling. It is added under Charlie's standing rule
(2026-09-28: a decision that matches what the SwiftUI benchmark app does
needs no ruling): the xheavy inbox found each message with
`first(filter(pool, …))`, a 997-step scan per row, where SwiftUI's inbox
indexes its array: in the inbox's fastest inner fling on the iPad the VM's
evaluation was 155 of the main thread's 681 ms/s, and 4 with `at`. Other list operations stay refused (LLP 1017.003).
Authored-function and roster arity refusals include the ordered parameter types
as a signature, including empty parameter lists and nested types. Their type
mismatches name the one-based argument position. The existing signature tables
remain the authority; formatting happens only on refusal. Error IDs and spans,
arity-before-argument checking, and scoped-action/router precedence are unchanged.

`image "symbol:<role>"` (LLP 1035.004, 2026-09-10) checks literal roles
against the generated schema vocabulary. Empty, unknown and platform-name
sources are `lower-attr-value` with the available roles; dynamic sources remain
host-checked. `tint-color` names the schema's colour row and accepts `light-dark()`.

**Formatting and continuation** (LLP 1035.005 D1, recovered 2026-09-20):
a view element's attributes may continue on deeper lines beginning with
`name=` before its children. A continuation at the element's own depth is
`syntax-continuation-indent`; repeated attributes are still refused across
lines. Component arguments already span lines inside parentheses.
`contract fmt [--check | --stdout] <file>` explicitly formats one file;
`--check` prints a diff and exits 1 on changes, `--stdout` previews, and bare
`fmt` writes only after parsing the result. Unknown flags and extra paths
are refused. No development loop or blocking check invokes it.

The source-preserving formatter uses the existing lexer's exact byte ranges
and the parser's attribute/argument boundaries. It normalizes structural
indentation to two spaces and token spacing, breaking long headers at
100 columns. Existing line breaks, blank groups, comment attachment, literal
spellings survive; template interiors are kept
verbatim. Long indivisible literals, comments and non-header expressions can
exceed the preferred width; a prefix containing interleaved positional arguments
also stays on the element's head. Re-lexing verifies unchanged ordinary tokens;
fixtures additionally prove unchanged trees, idempotence and byte-identical
plans across the corpus and every app. No checked-in app is reformatted.

## 3. Passes

LLP 1039 adds `aria-orientation` as the string prop `accessibilityOrientation`, emitted as ARIA on the web. Bake answers `exactViewport` at 390 × 844 and refuses unknown fields as `bake-viewport-field`; TypeScript source declarations and executor requirements skip this reserved source as they skip `exactDelivery`.

**Syntax** (`contract-syntax`): an indent-aware lexer (`Indent`/`Dedent`,
bracket-aware line continuation, template strings lexed whole), a
recursive-descent parser with spans on every node, and **inlining**: every
component use becomes the used component's view with props substituted by the
use's argument expressions (a curried `press=prop(args)` becomes
`action(parent-args…, args)`) and the child's bound names renamed apart. The
type pass expands once and returns `Checked`, pairing the authored file,
its inferred types, and that expansion. Analysis, lowering, and component
symbol navigation reuse it without cloning or re-expanding the root.

**Types** (`contract-types`): `Ty`, `Shapes`, the shared `Scope`/`Ref` (how
a name resolves: slot, derive, resource, action, prop, param, `each` item at
a region depth, `match` binding at a depth, inline-match local), `infer`, and
`check`. The root is checked against its inlined view so a handler's real call
site — behind a child's prop — types the action's parameters; children are
checked standalone. Roster calls are checked against the table's `params`/
`returns`. Compatibility-only checks evaluate the unifier's success conditions
without constructing a merged type. Type inference uses `Ty::unify` wherever
it needs the merged value. Function body checking borrows the resolved signature;
only the parameter types entering the body's owned scope are cloned.

Unknown written types retain `type-unknown` and their original token span. The
message lists known named types: primitives and bare `action`, then declared
shapes in sorted order without duplicate spellings. Forward/imported shapes are
included; router shapes appear only when declared by `routes`. These are names,
not a promise that every use is legal (shape cycles and other checks still apply).
Valid resolution does no choice-list work. Four scripted CLI repairs use the
reported choices and source ranges; all non-message diagnostic fields and repaired
plans remain identical, as do the 18 app/fixture plans.

Unknown component uses list the merged component declarations in source order,
including imported declarations. They retain the refusing pass's existing id
and original use span; the list describes declarations, not a promise that every
component can be used without recursion at that position. Valid uses do not
construct the diagnostic list.

An unknown component prop retains `type-unknown-prop` at the first offending
argument. Its message lists all distinct unknown props in call order and the
declared props in declaration order, so one repair can remove every invalid
argument. Injected values are not call props and are excluded from those choices;
a component with no props says so. Missing-required-prop and prop-type checks
keep their existing precedence. Valid calls do not build these diagnostic lists.

An unknown function call retains `type-unknown-function` and its original span.
For ASCII misspellings of 3–64 bytes, it suggests a single insertion, deletion,
substitution or adjacent transposition only when one available global name fits:
a declared `fn`, an admitted roster call, or the compiler calls `pending`, `failed` and
`path`. Router-only calls, including intrinsic `path`, require `routes`. Scoped action names, including conservative stems of lifted instance
names, veto ambiguous suggestions; action names themselves are not offered, and
a generated name (spelled with `#`, which no author can write) never is. This avoids exposing generated names or treating the
expanded root as the child's authored scope. Existing arity/type refusals and
valid calls do no suggestion work. Suggested replacements still undergo normal
compilation; no typo is accepted as an alias.

The driver now enriches action-valued refusals from the original authored tree:
Call hints exclude intrinsic `pending`/`failed`/`path` and global-function collisions; bare
action references and timers retain those legal action spellings. This follows the
refused expression kind even when a bare prop is called downstream.
Handlers, action props and timers offer one unambiguous action spelling under the
same ASCII edit rule. Candidates come from that component's actions and action
props/injections, excluding local `each`/`match` shadows.
A global function is not a handler correction; timers offer declared actions only.
When prop/provider substitution obscures the caller, an error-only mapped expansion
traces the argument through instance parents and adds the original supplied
expression as a related location. All matching instances must agree on the hint;
ambiguous or untraceable origins get none. The primary diagnostic id and span stay
unchanged, as do normal type/arity checks. Four CLI repair cases use the diagnostic
location (related for forwarded arguments) to produce valid, byte-identical plans.
The 18-app plan comparison is unchanged. Successful builds do not walk authored
scopes or construct this diagnostic provenance; no compiled-plan metadata is added.

Component expansion copies the root declarations and constructs its expanded view
directly, avoiding a discarded copy of the original view. Each child use renames
only its view; its declarations are already lifted separately. The same rename
and substitution walks preserve capture avoidance, instance order, and source sites.
Renaming borrows names from its existing string map through the same expression
substitution walk, avoiding reconstruction of the map for every expression.
`each` and `match` retain their lexical scopes; references keep their authored spans.
An empty substitution map copies the expression without performing name lookups.

**Expansion identity** (2026-09-22). A child's lifted states, actions and view
binders are `name#N` for use `N`. `#` cannot appear in an authored identifier,
so a root `count__1` and a child's lifted `count` are two slots. Substitution
avoids capture: a binder a replacement would capture is renamed `x@k`, and a
property test checks it against an evaluator. A child derive the view, a state
or an action reads resolves once per use to its body with its dependencies in
scope. A dependency read more than once is bound once, as the compiler-only
`Expr::Let` (lowered with BindLocal/LoadLocal/DropLocal, like an inline
`match`), at the smallest part of the body that evaluates it on every path,
counting reads through other derives. It is never bound ahead of a condition,
`match` arm or `and`/`or` that would skip it. One no path always evaluates is
written where it is read. A `fn` body's repeated identical calls are bound the
same way. A chain of 64 derives, each reading the one before twice, expands to
257 expression nodes (was 2^n); a 32-deep `fn` chain is 897 plan bytes.
`corpus/lazy-derive.contract` and a generated property test hold that each
value is computed exactly where its readers computed it. Lowering takes each
expression's type from what it compiles (no re-inference), and the plan
builder shares identical code bodies.

Scope clones share immutable name/type frames while retaining independent frame
stacks. Entering or leaving a branch changes only its own stack; shadowing and
`Item`/`Bound` region depths follow the same innermost-first walk. Atomic shared
ownership preserves the public scope's ability to cross threads. Frames live only
as long as the scopes that use them; there is no cross-compilation cache.
Name and field lookups borrow their stored types. Inference copies a type when
it needs an owned result; lowering and handler checks inspect only what they need.
Ordinary state initializers grow one non-region frame after each initializer is
inferred. Earlier bindings are retained without recopying the whole prefix; later
states remain unavailable, and duplicate declarations are refused beforehand.
Growth uses copy-on-write so any shared snapshot stays unchanged, retaining the
same within-frame name lookup order. Row-owned initializers are still resolved in
their owning region scopes in the later pass.
Before constructing a component scope, inference has allocated every declaration's
type entry, including unresolved derive and action parameter types. Scope construction
copies bindings directly from that table into its owned snapshot, without first
cloning the whole component type table and its unrelated data-source signatures.

HTML `dialog` lowers to a View with `semanticTag="dialog"` and the absolute
position default; `commandfor` and `command` are schema props, passed by their
HTML names. Their presentation belongs to the host (LLP 1021 D2), with no
compiler-created open-state slot.

**Analyze** (`contract-analyze`): handler shape and arity (`change` and
`key` supply a string as the last parameter, `hover` a bool,
`press`/`focus`/`blur`/`submit` nothing — `HANDLERS` and `handler_payload` in
`contract-analyze`), timer actions exist and take no parameters, a mutation's
`then` action takes none and never sends that mutation, in any branch
(`analyze-then-self-send`, read from the body's effects), component uses name
real components with each argument once. It checks no effect declarations:
there are none (§2, LLP 1035.005.000 D1). `analyze-write-not-declared`,
`analyze-writes-unknown-state` and `analyze-writes-duplicate` retired with the
clause on 2026-10-02, with their fixtures and their repair-message work.
Lowering computes each action's `writes` from `Action::effects` (the body's
assignments and sends, every branch) after expansion, so a lifted child
instance's action names its own renamed slots and a row-owned slot is written
to the row in force, as before. Proof at the change: every one of the 117
`.contract` roots in the repository (apps, examples, game fixtures, corpus,
conformance) compiled before and after, and the decoded plans compared with
`actions.writes` set aside are identical. 99 kept the same allowlists in the
same order, 16 the same sets now in slot order, and two shrank where an app
listed a slot its body never writes: Completion Storm's `sample` (`clicks`)
and the iOS Calendar example's `animateDrag` (`editorMorphAt`) and
`chooseType` (`pickerClosing`, `pickerUntil`).

**Lower** (`contract-lower`): shapes to `types`; declarations to `slots`,
`derives`, `resources`, `actions`, `timers` in source order; the inlined view
to `nodes`/`regions`/`arms`/`bindings`/`handlers` through the tag/attribute
table (`tags.rs`: `column`/`row`/`main`/`scroll`/`text`/`button`/`link`/
`input`/`image` onto kernel node types plus fixed rows — `button` is a
pressable `column`, role button with `display: flex; flex-direction: column`
(Charlie, 2026-09-23: "One native button, flex column"), so the web's
`<button>` lays out as the kernel does (LLP 1007 §1); attributes onto style
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
local; a non-string template part gets `toString`). A template whose parts are
all literal strings after component expansion emits one interned string, so a
literal prefix passed to a component adds no runtime concatenation. Dynamic
parts and non-string conversions retain their ordinary evaluation. Row order is source
order, so compilation is byte-identical (`the_app_compiles_deterministically…`).
Tag defaults and attribute style targets borrow static slices of kernel-generated
identifiers; lookup constructs no temporary row vectors.
Nodes without a class borrow their existing attributes; only class expansion
builds a merged attribute list.
Duplicate node bindings compact their existing buffers in place, preserving
the first row position and the last value and source origin. Nodes with fewer
than two bindings skip duplicate detection.

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
and every host's `build.rs` fails on either (`contract/cli/tests/it/lint.rs`).
The compiler cannot see layout; bake can, and it already had the kernel. The CLI: `contract
build <file> [-o <plan>]` prints a one-line summary or a rejection as
`file:line:col [id] message`, exit 1.

## 4. The corpus (LLP 1004 D6)

`contract/corpus/now-screen.contract` is the runner's hand-built plan as
Contract; the corpus test compiles it, round-trips the bytes, bakes, boots,
and asserts the same behavior the hand-built test asserts (keyed reorder,
`when` flip, timers, commands) — proven at both ends. `rejects.txt` holds one
fixture per diagnostic id, each refused with exactly its id. The
app itself is the integration fixture (`apps/caltrain/tests/app.rs`).

Router rejects (LLP 1038 D2/D3) each have a same-named fixture in
`rejects.txt`: `route-duplicate`, `route-shadowed`, `route-parent-param`,
`route-pattern` (the four ids and messages come from `Table::check`),
`route-no-match`, `route-unknown`, `route-template`,
`analyze-routes-not-root`, and `type-shape-reserved`. A second `routes`
declaration also uses `route-duplicate`; a state that redeclares its slot
uses the ordinary `type-duplicate-name`.

`routes.contract` is Interview's table with every verb and D6's stack rows.
`contract/cli/tests/it/routes.rs` holds declaration order, positional shapes, byte-identical
compilation and encode/decode, `/` and `/prompt/5/write` launch fill, the
baked-argument rule, encoded string/number paths, query reads, retained tabs,
entry-id row identity, imported-file refusals and instance lifting. It replays
all 66 steps from `route/tests/corpus.json` through compiled actions, without
copying expectations. `contract/cli/tests/it/typescript.rs` checks the four generated types.
`navigate=action` (LLP 1038 D8/D11) belongs only to the first navigation
root, carrying both `navigationKey` and `navigationBack`; after inlining, any
other placement is `lower-navigate-root` (corpus reject). Its action takes one
string location or no parameters, with no captured arguments. The type checker
infers the payload or refuses a non-string declaration; analyze and lower check
arity, including an action passed through a component prop.

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
depth guard; `@keyframes`, `cursor`, per-instance state, LSP (the formatter is implemented above), `linear()`
and transition rows from Contract (the kernel has the row; the tag table does
not yet expose `transition`). Each is a fixture away, never a speculation.
Executable `contract` assertions are not deferred but refused (§2): assertions
are `test` blocks beside the app.

## 9. Checks that hold this

`contract/syntax/tests/parse.rs`, `contract/cli/tests/it/corpus.rs`,
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
