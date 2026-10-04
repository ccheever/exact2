# LLP 1085: What the app diaries ask of Contract

**Type:** RFC
**Status:** Draft r1, 2026-10-04. Nothing here is built. For review by other model families before implementation.
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`), Plan (`plan/tables/format.json` `stdlib`: six new rows, one retyped), Runner (`vm.rs`, `stdlib.rs`), JS target (`host/web-js`), Lean semantics and difftest (`semantics/`, `contract/cli/src/lean.rs`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** TBD (the orchestrator assigns), 2026-10-04 to 2026-10-05, in the stages of §5
**Amends:** LLP 1006 §2 (the language); LLP 1017.003 "Refused" (`concat`, `slice`, list literals and `includes` on lists are admitted here, the trigger it named having been met); LLP 1024 D1 (which known attributes bind to a module tag's box)
**Related:** LLP 1017 P5 and §8 (the three-mechanism rule: expression → `fn`; list, join, I/O → `resource`; effect → command); LLP 1017.003 D3 (list steps), D4 (`[]`); LLP 1035.005.000 D3 (records), D7b (`then` on a resource); LLP 1054.000.003 D8 (linked by use); LLP 1054.000.005 (`trim`); LLP 1071 (the JS target); the diaries `~/projects/x2apps/<app>/DIARY.md` and their `repros/`. Web: ECMA-262 (`IsLessThan`, `String.prototype.{slice,replaceAll,toLowerCase,toUpperCase,padStart,toWellFormed}`, `GetSubstitution`, `Array.prototype.{concat,slice,includes}`, `SameValueZero`).

## Summary

On 2026-10-04 eight agents each built an app outside the repo with `exact new`
and kept a diary. Four of them redesigned around one missing construct: no
list could be built in Contract, so pomodoro's tasks, kanban's board, shop's
cart and hn-reader's collapsed-comment set all moved into the data module.
Calendar moved its validation there because `"10:00" > "09:30"` does not
type. Calc sent Backspace through a mutation because there is no `slice`.
The rest lost minutes, not designs: reserved words where a parameter goes, a
type error that printed `?`, and diagnostics that named the fix obliquely.

| | Decision | Diaries | Changes |
|---|---|---|---|
| D1 | `<` `<=` `>` `>=` on two strings, in UTF-16 code-unit order | calendar F1 | types, runner, Lean |
| D2 | `slice`, `replaceAll`, `toLowerCase`, `toUpperCase`, `padStart` on strings | calc #5, hn-reader F2, shop log | roster, runner, JS, Lean |
| D3 | List literals `[a, b]`; `concat`, `slice`, and `includes` on lists | pomodoro F3, kanban log, shop log, hn-reader F2 | syntax, types, roster, runner, JS, Lean |
| D4 | A state initializer's scope, stated by its refusal | kanban F5 | types |
| D5 | Reserve only the words that shape expressions; the rest are contextual | kanban F13, shop F6, hn-reader F1, ledger F3, calendar log | syntax |
| D6 | Type row-owned state before the `each` lists that read it; no `?` in a message | shop F3 | types |
| D7 | Four diagnostics that name the fix | pomodoro F1, paint F7, ledger F3, hn-reader F2 | analyze, lower, syntax |

D4–D7 change only the compiler and land first. D1 and D2 add roster rows and
no opcode. D3 adds one syntactic form, over the `List n` opcode that `[]`
already lowers to.

## 1. Evidence

Reproduced on `9cd6863d` with `target/debug/contract build --json`:

- **shop F3.** The diary's 20-line repro (`shop/repros/each-derive-state.contract`)
  compiles because its `Child` is used once. Put the use inside an `each` and
  it fails as the app did: `type-argument: argument 2 of `at` expects `number`,
  given `?``. Remove the child's own `each img in images` and it compiles
  again. Both conditions are needed (§D6).
- **The roster** has `includes`, `startsWith`, `endsWith`, `trim`, `length`,
  `toString`, `join`, `map`, `filter`, `first`, `at`, and nothing that cuts,
  rewrites or builds. `replace` and `push` are the router's.
- **Both executors already build lists of any length**: `[]` lowers to
  `List 0`, and the JS emitter prints `List n` as `[a,b,…]` (`code.rs:330`).
  The JS target already compares strings (it emits `a < b`, `code.rs:339`);
  only the checker (`types/src/lib.rs:920`) and the runner (`vm.rs:696`)
  refuse.
- **Keywords.** `is_keyword` (`parser.rs:1402`) lists 35 words; field names,
  props and named arguments already admit the structural ones (`is_name_word`),
  and `send` and `let` are already contextual (`parser.rs:938`, `:961`). What
  still refuses `key`, `from`, `state`, `view` and `refresh` is `ident()` at
  parameter, action, state and derive names.

The predecessor's reason for no lists (LLP 1017 P5 and §8): `fn` must not grow
into a programming language, so lists, joins and I/O belong to a `resource`.
LLP 1017.003 kept literals, `concat` and `slice` out until "a view-only need
demonstrated in source". The diaries show a different need from the one it
anticipated: not a traversal the view could not do, but session state the
screen owns, such as a draft task list, a set of collapsed ids or per-column
scroll offsets. Moving that state into a data module makes every edit a
mutation round trip and a second copy of the state. D3 admits building lists
and keeps traversal and aggregation (`reduce`, `sort`, `find`) refused. The
three-mechanism rule stands for those.

## 2. Decisions

### D1 — Strings compare in code-unit order

`a < b`, `a <= b`, `a > b` and `a >= b` type when both operands are `string`,
and answer as ECMA-262's `IsLessThan` does for two Strings: compare UTF-16
code units in order; a proper prefix is less. No locale, no normalization.
`"09:30" < "10:00"` and `"2026-01-01T09:00" < "2026-01-01T10:00"` are true,
because ISO 8601 text sorts that way. A number against a string stays
`type-operand`, as does `bool` against anything. JavaScript would coerce, and
no view wants that. The message becomes "comparison needs two numbers or two
strings, given `number` and `string`".

The runner's `Lt`/`Le`/`Gt`/`Ge` gain a string arm,
`a.encode_utf16().cmp(b.encode_utf16())`; Rust's `str` order is code-point
order, which differs where a supplementary character meets U+E000–U+FFFF. Lean
gains the same case over a `Str.utf16Units` beside `utf16Length`
(`Value.lean:89`). The JS target already does this. No opcode or plan change.

### D2 — Five string functions, as JavaScript defines them

| Contract | JavaScript | Type |
|---|---|---|
| `slice(s, start, end?)` | `s.slice(start, end)` | `string, number, number → string` |
| `replaceAll(s, find, with)` | `s.replaceAll(find, with)`, `find` a string | `string, string, string → string` |
| `toLowerCase(s)` | `s.toLowerCase()` | `string → string` |
| `toUpperCase(s)` | `s.toUpperCase()` | `string → string` |
| `padStart(s, length, fill?)` | `s.padStart(length, fill)` | `string, number, string → string` |

Rules shared by all five:

- **Positions and lengths are UTF-16 code units**, as `length` already counts
  them. `start` and `end` follow `ToIntegerOrInfinity`: truncate toward zero,
  `NaN` is 0, a negative index counts from the end, then clamp.
  `slice(expr, 0, -1)` is calc's Backspace.
- **Every result is well formed.** The runner, Lean and the native hosts hold
  strings as Unicode scalar values, so none can hold a lone surrogate. Where
  JavaScript's result would hold one (a cut inside a pair, an empty-pattern
  `replaceAll`, a `fill` truncated mid-pair), it is U+FFFD, as ECMA-262's
  `toWellFormed` makes it. The runner and Lean decode the code units with
  replacement (`char::decode_utf16`, `unwrap_or('\u{FFFD}')`); the JS target
  calls `.toWellFormed()` on these five results. A declared deviation, written
  into LLP 1006 §2.
- **An omitted trailing argument is the web's default, filled in by the
  compiler**: `slice(s, 1)` lowers `end` as the constant `Infinity`, which
  clamps as `undefined` does; `padStart(s, 2)` lowers `fill` as `" "`. Roster
  rows keep a fixed arity, so no executor learns optional arguments.
- **`replaceAll` takes a string pattern and applies `GetSubstitution`**:
  `$$`, `$&`, `` $` `` and `$'` in `with` mean what they mean on the web, and
  `$1` stays literal because a string pattern has no captures. An empty `find`
  inserts `with` at every code-unit boundary, as on the web. There are no
  regular expressions.
- **`toLowerCase` and `toUpperCase` use Unicode default case conversion**:
  locale-independent, with SpecialCasing (`"ß"` → `"SS"`, final sigma), which
  is what Rust's `str::to_lowercase`/`to_uppercase` implement. A code point
  whose mapping changed between Rust's and the browser's Unicode versions may
  differ; that is declared, not fixed. Both are core: the kernel's
  `text-transform` (`kernel/src/text/case.rs`) already links the same tables on
  native hosts, and the JS target uses the browser's. Stage 2 measures the
  wasm target's `app.wasm`. Over 2 KB compressed, both move behind `format`'s
  link (LLP 1054.000.003 D8, one `uses()` arm).
- **Bounds.** `MAX_STRING` traps an oversized result, as `+` does. `padStart`,
  whose output size an argument chooses, checks `length` before allocating,
  on the runner and the JS target alike.

**Not admitted:** `replace` (the router's; no diary asked for first-only
replacement), `split`, `indexOf`, `substring`, `padEnd`, `trimStart`/`trimEnd`,
`Number`/`parseInt`/`parseFloat`, `toFixed`, regular expressions. Each waits
for an app's source to need it. `contract_syntax::idioms` names the
alternative for each; a three-argument `replace(s, a, b)` names `replaceAll`.

### D3 — Lists the screen builds: literals, `concat`, `slice`, `includes`

**Literals.** `[a, b, c]` is a list. Its items unify to one type the way the
arms of `?:` do, so `[some(x), none]` is `list<option<T>>`. `[]` keeps
today's inference rules unchanged. A trailing comma is accepted, and
`contract fmt` drops it. Items span lines, because the lexer already counts
bracket depth. Mismatched items are `type-list-item`: "list items share one
type: item 2 is `number`, item 1 `string`". Spread does not parse. `[...xs, x]`
is `syntax-refused-idiom` with "write `concat(xs, [x])`". The AST's
`Expr::EmptyList` becomes `Expr::List(items, span)`. It lowers to the items
and `List n`, which both executors already run.

**Functions** (`list<T>` in, as `first` and `at` are typed by the checker
over `any` roster rows):

| Contract | JavaScript | Type | List steps |
|---|---|---|---|
| `concat(xs, ys)` | `xs.concat(ys)`, `ys` an array | `list<T>, list<T> → list<T>` | `len(xs) + len(ys)` |
| `slice(xs, start, end?)` | `xs.slice(start, end)` | `list<T>, number, number → list<T>` | result length |
| `includes(xs, x)` | `xs.includes(x)` | `list<T>, T → bool`, `T` string, number or bool | items scanned |

- `concat` takes exactly two lists of one element type. A non-list second
  argument is `type-argument` with "wrap it: `concat(xs, [x])`". JavaScript's
  variadic form and its quiet flattening are not taken.
- `slice` is one roster row over `any`. It cuts a string (D2) or a list,
  as `length` measures either.
- `includes` on a list compares with `SameValueZero`: `NaN` is found, and
  `0` matches `-0`. Lists of records or options are refused, as `join`
  refuses them, because the web compares objects by identity and Contract's
  records are values. That disagreement should not be hidden behind the web's
  name. `includes(s, t)` on strings is unchanged.
- Steps count against `MAX_LIST_STEPS` (LLP 1017.003 D3), like `map` and
  `filter`.

With `map`, `filter` and record copies (LLP 1035.005.000 D3), this is the
whole set for client-side list state:

```contract
state tasks = [Task(id="t0", title="Plan the day", done=false)]
state nextId = 1
action add(title: string)
  tasks = concat(tasks, [Task(id=`t${nextId}`, title=title, done=false)])
  nextId = nextId + 1
action toggle(id: string, checked: bool)
  tasks = map(tasks, (t) => t.id == id ? Task(t, done=checked) : t)
action remove(id: string)
  tasks = filter(tasks, (t) => t.id != id)
```

**Where a list lives** (stated in `contract-for-agents.md`, replacing "There
are no nonempty list literals"): a list the screen keeps for the session, such
as a draft, a selection, a set of open or collapsed ids or per-row UI state, is
`state`. A list the app keeps across launches, or that a server owns, is the
data module's, read through a `resource` and changed with a `mutation`
(Fieldnotes). Sorting, grouping and aggregates stay in the data module: the
three-mechanism rule, narrowed to traversal.

### D4 — A state initializer reads props, injects and earlier states

Kanban F5 wrote `state scrolls = map(board.cols, …)` and got "unknown name
`board`; did you mean `boardX`?". The rule is right and the message is wrong.
An initializer runs when its instance is created, before any resource has
answered, and a resource may stay pending. "Initial value from the first
answer" would need a lifecycle event Contract does not have; that is
LLP 1035.005.000 D7b's `then` on a resource, still unselected.

Decision: keep the rule and refuse with `type-initializer-scope`, naming what
the name is and where the value belongs:

> `board` is a resource; a state's initializer runs before any resource
> answers, and reads only props, injects and earlier states. Derive the value
> from `board` (`derive …`), or keep per-row state in a component used inside
> `each board.cols` (`state top = 0`).

The same refusal covers a derive, an action or a later state named in an
initializer. Near-name suggestions apply only when no declaration of that
name exists in the component.

### D5 — Reserved words are the words that shape an expression

**Reserved everywhere** (`syntax-expected-name` where a name goes): `when`,
`if`, `else`, `each`, `in`, `match`, `case`, `as`, `fn`, `and`, `or`, `not`,
and the literals `true`, `false`, `none`, `some`. The literals are not in
`is_keyword` today: `state none = 1` compiles, and `toString(none)` is then
refused as `option<?>`.

**Contextual** (a keyword only where its construct starts, a name
everywhere else): `component`, `font`, `shape`, `style`, `use`, `from`,
`state`, `derive`, `resource`, `mutation`, `action`, `task`, `mount`,
`view`, `props`, `provide`, `inject`, `slot`, `children`, `key`, `refresh`,
`writes`, `test`, `expect`. Each is recognized only at its own position: a
line start in its block (`state`, `view`, `children` in a view), after `use X`
(`from`), after `each … in xs` (`key`), or after `task name` (`mount`).
`refresh` becomes a statement only when a name follows it, as `send` and
`let` already work, so `action refresh` and `press=refresh` mean the
action and `refresh feed` means the statement. A state named `view` is
`state view = "month"`, assigned as `view = "week"`. Neither position is
the view section's.

`ident()` admits the contextual words, and `is_name_word` stops excluding
`refresh`. No backend uses an authored name as a JavaScript identifier: the
JS target emits slots and parameters by index (`p0`), `typescript.rs` quotes
property names, and the Lean backend keeps names as strings.

The refusal for the 16 reserved words drops the generated `ins`/`myIn`
suggestion, which hn-reader found awkward:

> `in` is reserved in Contract (it shapes an expression, like JavaScript's
> `in`); choose another name.

`docs/contract-grammar.md` lists both sets, replacing "see `is_keyword`".

### D6 — Row-owned state is typed before the `each` lists that read it

**Root cause.** A child used inside an `each` has its states lifted into the
root as row-owned slots, typed `Ty::Unknown` (`component.rs:111`) until
`infer_owned_state_initializers` (`checks.rs:409`) types them in their owning
`each`'s scope. `collect_owner_scopes` builds those scopes by inferring every
`each` list in the view, the lists nested inside the row included. After
inlining, shop's inner `each img in images` iterates
`match at(product.swatches, pick#N) …`, which reads the row's own slot while
it is still `?`. `at` refuses, and the error escapes. Without an inner `each`
over such a derive nothing asks, which is why the 20-line repro compiled.

**Fix.** One walk, outer to inner: on entering an `each` whose tag owns slots,
`collect_owner_scopes` types those initializers in the `each`'s scope and
retypes them there before descending. `infer_owned_state_initializers` becomes
that walk. An initializer reads only earlier slots and the frames above it
(D4), so the order is total.

**No `?` in a message.** With no earlier error to explain it, a `?` in a
message is a compiler bug the author cannot act on. Where a type stays
unknown, the refusal is `type-cannot-infer` naming the binding ("cannot infer
the type of `pick` here"), and a test over the reject fixtures holds it (§4).

### D7 — Four diagnostics that name the fix

1. **Handler arity spells the declaration** (pomodoro F1). Today: "`flipTask`
   takes 1 parameter(s); `change=` supplies 1 plus the new value". Instead,
   with the payload named after the DOM property it carries (`checked`,
   `value`, `key`, `data`) and typed by the element (a checkbox's `change`
   is `bool`):
   > `change=flipTask(t.id)` calls `flipTask` with `t.id` and then the
   > checkbox's new `checked` (bool); declare `action flipTask(id: string,
   > checked: bool)`.

   Analysis (`analyze/src/lib.rs:478`) and lowering (`lower/src/lib.rs:1414`)
   print the same message through one function, `analyze::handler_shape`.
2. **A known attribute on a module tag binds to the box only if the box uses
   it** (paint F7). LLP 1024 D1's deny-list (`native.rs:78`) refuses text rows
   and control props but missed `command`, `commandfor`, `href`, `src` and
   the rest, so `command` silently became the button-invoker prop. It becomes
   an allow-list: layout, box and paint rows, handlers, `testId`, `id`,
   `class`, `data-*`, `role`, `aria-*`, `tabIndex`. Any other known attribute
   is `lower-native-attr` ("give the module prop another name"), and a case
   variant of a known name (`tabindex`) is refused naming it (`tabIndex`).
   Passing such props through to the module is not taken: the word would
   change meaning the day the box gains that prop.
3. **CSS declaration syntax in a style** (ledger F3). A row written
   `background-color: "…"` in a `style` block or an element's attributes is
   refused with the rewritten row: "a Contract style row is
   `background-color=\"…\"`, not CSS's `background-color: …`".
4. **The web's string and array methods name their Contract form or say why
   not** (hn-reader F2's `replaceAll`, its `len`). `idioms.rs`'s refusal
   table moves `slice` and `concat` to admitted (D2, D3). It gains `split`,
   `indexOf`, `substring`, `padEnd`, `replace` (three arguments) and `push`
   (two arguments, not a router), each naming the admitted alternative or the
   data source. An unknown function one edit away from a roster name suggests
   it (`len` → `length`).

## 3. Effect on each implementation

| | Stage 1 (D4–D7) | Stage 2 (D1, D2) | Stage 3 (D3) |
|---|---|---|---|
| `contract/syntax` | D5 keyword sets in a new `names.rs` (`parser.rs` is at 1,469 lines); `refresh` contextual; D7.3 colon recovery; D7.4 idioms | omitted-argument fill for `slice`/`padStart` | `Expr::List`; spread refusal; `fmt.rs` prints literals |
| `contract/types` | D6 walk (`checks.rs`); D4 refusal; no-`?` rule | string comparison; checker types for `slice`, `padStart` | literal unification, `type-list-item`; `concat`/`slice`/`includes` over lists |
| `contract/analyze`, `lower` | D7.1 one message; D7.2 allow-list (`native.rs`) | constant `Infinity`/`" "` for omitted arguments | lower `Expr::List` to items + `List n` |
| `format.json` `stdlib` | — | rows `slice` (`any, number, number → any`), `replaceAll`, `toLowerCase`, `toUpperCase`, `padStart` | rows `concat` (`any, any → any`); `includes` retyped `any, any → bool` |
| `runner` | — | `vm.rs` string comparison; `stdlib.rs` five arms (or `runner/src/text.rs` if `stdlib.rs` passes 1,000 lines) | `concat`, list `slice`, list `includes` with step counting |
| JS target | — | `x_slice`, `x_replaceAll`, `x_toLowerCase`, `x_toUpperCase`, `x_padStart`, each `.toWellFormed()` where D2 says; not in `rt.js`, which is at 1,499 lines, but in the module holding `x_at`/`x_format*` | `x_concat`, list arms of `x_slice`/`x_includes`; `List n` already emitted |
| Lean | — | `Value.lean` `utf16Units`, slice and pad over code units, `GetSubstitution`, string `<`; `toLowerCase`/`toUpperCase` listed under "What the semantics leaves out", like `format*` | `Syntax.lean` `.list (items)` replaces `.emptyList`; `concat`/`slice`/`includes`; `lean.rs` emits it |
| difftest | — | generator (`gen/expr.rs`) emits the new calls, with strings drawn to include astral characters | generator emits literals and list calls |
| docs | grammar's two keyword sets; agents guide on initializers | roster table; comparison semantics; the deviation in LLP 1006 §2 | "Where a list lives"; delete "no nonempty list literals" in three docs |
| apps | none required | none required | none required |

The plan format gains roster rows only. The digest changes, `formatVersion`
does not, as when `startsWith` landed (`f88bd2b6`).

## 4. Tests

Each test extends the nearest existing one.

- **Compiler** (`contract/cli/tests/it/`). `names.rs`: every contextual word
  as a parameter, action, state and derive name (`action refresh` beside
  `refresh feed`), every reserved word refused with the new text.
  `instance.rs`: the shop shape compiles, as does an initializer reading its
  row's item. `search.rs`: string order, the five functions, omitted
  arguments. `lists.rs`: literals, `type-list-item`, spread, `concat`,
  `slice`, `includes`. `diagnostics.rs`: D4 and D7, each message asserted
  whole. `corpus.rs`: no message over `rejects.txt` contains `` `?` ``.
  Corpus fixtures `search.contract`, `lists.contract` and `rejects.txt` grow.
- **Runner** (`stdlib.rs` tests, as
  `includes_starts_with_and_ends_with_answer_as_javascript_does`): case tables
  whose expected values come from `bun -e`. They cover negative and fractional
  indices, `NaN`, `±Infinity`, cuts inside a surrogate pair, the empty pattern,
  `$&`/`$$`/`` $` ``/`$'`/`$1`, `ß`, final sigma, a two-unit `fill`,
  `MAX_STRING` and step traps, `SameValueZero`, and order across U+E000 and
  U+10000.
- **Lean differential.** `semantics/corpus/text/{compare,slice,replace-all,pad}`
  and `semantics/corpus/lists/{literals,concat,slice,includes}`; `difftest --
  corpus` and a random sweep with the generator extended.
- **JS against the runner.** `host/web-js/conformance/text-and-lists.contract`
  and `.steps` under `conform.mjs --synthetic`, in Chrome, Firefox and WebKit
  (async lane).
- **Driven.** A scratch `exact new` app keeps pomodoro's task list in `state`,
  with a `slice` Backspace and an end-after-start check. It is driven with
  `agent web`/`test web` and `mac`/`test macos`. Shop's `product.contract`,
  returned to its first form, builds.
- The five checks after each stage.

## 5. Implementation plan

Implementer TBD by the orchestrator, one lane, three commits or more per stage,
each passing the five checks.

1. **Stage 1, 2026-10-04: compiler only (D4, D5, D6, D7).** No plan or
   runtime change, so no conformance run is needed. Exit: the diaries'
   repros compile or are refused with the new text; `names.rs`,
   `instance.rs`, `diagnostics.rs` and `corpus.rs` are green.
2. **Stage 2, 2026-10-05: strings (D1, D2).** Roster rows, runner, JS
   exports, Lean and difftest, conformance plan, docs, and the `app.wasm`
   measurement for the case functions. Exit: runner tables, difftest corpus
   and a random sweep, and conformance on three browsers are green; calendar's
   `end > start` and calc's Backspace written in Contract and driven.
3. **Stage 3, 2026-10-05: lists (D3).** Syntax, types, lowering, `fmt`, roster
   rows, runner, JS, Lean `.list`, difftest, conformance, docs. Exit: the
   scratch task-list app driven on web and macOS; LLP 1006 §2 and LLP 1017.003
   amended in the same commit.

## 6. Considered and not taken

- **`compare(a, b)`** (calendar's alternative): a second spelling of an
  operator JavaScript already has.
- **Overloading roster names by argument type**, so string `replace` could
  sit beside the router's: a new resolution rule for a function nobody asked
  for.
- **Spread**, `[...xs, x]`: what a React author writes first, but a second
  construction syntax beside `concat`. The refusal spells the rewrite;
  revisit if agents keep writing it.
- **A state initialized from a resource's first answer**: needs a lifecycle
  event (D4).
- **Making every keyword contextual.** Expression words must stay reserved,
  or `each x in in` and `when not` lose their single parse.
- **`tabIndex` → `tabindex`.** Contract mixes HTML attribute spellings
  (`readonly`, `inputmode`) with DOM property spellings (`tabIndex`,
  `scrollLeft`). Settling that is a naming RFC of its own (§7).

## 7. Open questions

1. **Lists as session state against the three-mechanism rule.** D3 holds that
   building a list is an expression and traversing one is a data source. A
   reviewer who reads LLP 1017 §8 as "every dynamic list is the data module's"
   should say so. The alternative is pomodoro's other fix: a doc paragraph and
   no language change.
2. **The well-formed-string deviation (D2).** Is U+FFFD for a lone surrogate
   acceptable on every host, or should the cutting functions refuse at run time
   instead (a trap)? A trap would be stricter, and less like the web.
3. **The case functions' cost on the wasm target**: core, or behind `format`'s
   link. D2 decides by measurement at a 2 KB threshold; Charlie may prefer
   linked by default.
4. **HTML attribute or DOM property spelling** for `tabIndex`, `scrollLeft`,
   `emojiPicker` and `navigationKey`. That is outside this RFC, and paint F7's
   `tabindex` refusal is a stopgap.
