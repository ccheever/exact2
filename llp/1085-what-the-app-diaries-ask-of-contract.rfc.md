# LLP 1085: What the app diaries ask of Contract

**Type:** RFC
**Status:** Draft r2, 2026-10-04. Nothing here is built. r1 was reviewed by one family, Astra (`gpt-6-astra`, max): NOT READY, 7 MATERIAL and 6 MINOR findings. Grok was unavailable (not signed in), so no second family reviewed r1. §8 lists what r2 changed.
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`, `contract/cli/src/lean.rs`), Plan (`plan/tables/format.json` `stdlib`), Runner (`vm.rs`, `stdlib.rs`, `uses.rs`), JS target (`host/web-js`), Lean semantics and difftest (`semantics/`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-04, stages 2 and 3 on 2026-10-05 (§5)
**Base:** `gaps/papercuts`. It carries `host/web-js/format.js`, `x_at`/`x_formatDate`/`x_formatNumber` on the JS target, and the test that every roster entry has an `x_` export (`host/web-js/src/code.rs:595`).
**Amends:** LLP 1006 §2 (the language); LLP 1017.003 "Refused" (`concat`, `slice`, list literals, `includes` on lists); LLP 1024 D1 (which known attributes bind to a module tag's box)
**Related:** LLP 1017 P5 and §8 (the three-mechanism rule); LLP 1017.003 D3 (list steps), D4 (`[]`); LLP 1016 D5 (a send forgets the in-flight reply); LLP 1035.005.000 D3, D7b; LLP 1047 D2/D6 (linked by use); LLP 1054.000.005 (`trim`); LLP 1071 (the JS target); diaries `~/projects/x2apps/<app>/DIARY.md`; review `llp/reviews/1085-r1.astra.md`. Web: ECMA-262 `IsLessThan`, `String.prototype.{slice,replaceAll,toLowerCase,toWellFormed}`, `GetSubstitution`, `StringPad`, `Array.prototype.{concat,slice,includes}`, `SameValueZero`; HTML `tabindex`.

## Summary

On 2026-10-04 several agents each built an app outside the repo with
`exact new` and kept a diary. Four of them redesigned around one missing
construct: no list can be built in Contract. HN's collapsed-comment ids and
kanban's per-column scroll offsets are session state, and both moved into the
data module anyway. Calendar moved its validation there because
`"10:00" > "09:30"` does not type. Calc sent Backspace through a mutation
because there is no `slice`. The rest lost minutes, not designs: reserved
words where a parameter goes, an error that printed `?`, a send whose reply
vanished, and diagnostics that named the fix obliquely.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `<` `<=` `>` `>=` on two strings, in UTF-16 code-unit order | calendar F1 | 2 |
| D2 | `slice`, `replaceAll`, `toLowerCase` on strings; numeric parsing deferred, explicitly | calc, hn-reader F2, calendar F1, flashcards/chat search | 2 |
| D3 | List literals; `concat`, `slice`, `includes` on lists; one evaluation budget on both executors | hn-reader F2, kanban, pomodoro F3 | 3 |
| D4 | A state initializer's scope, stated by its refusal | kanban F5 | 1 |
| D5 | Reserve 16 words at binder positions only; the rest are contextual | kanban F13, shop F6, hn-reader F1, ledger F3, calendar F16 | 1 |
| D6 | Type state from initializers *and* writes before the `each` lists that read it; no `?` in a final message | shop F3, review finding 4 | 1 |
| D7 | Four diagnostics that name the fix; HTML `tabindex` bound for real | pomodoro F1, paint F7, calendar F2, ledger F3, hn-reader F2 | 1 |
| D8 | Two sends to one mutation on one path are refused | flashcards F4 | 1 |

Snake F2 (a source named `advance` bricked the app) is not a naming
problem. It was a web build bug, fixed in `gaps/papercuts` `67d6b873`, so
nothing is reserved for it.

## 1. Evidence

Reproduced on `9cd6863d` and re-checked on the rebased base with
`target/debug/contract build --json`:

- **shop F3.** It fails only for a child used inside an `each` whose own
  view iterates a derive over its state; either condition alone compiles.
  Review finding 4 adds a second case that fails even at the root:
  `state items = []`, an action assigning a `list<Item>`, and a nested
  `each i in items` / `each image in i.images` give "`?` has no fields".
- **`state none = 1` compiles.** `toString(none)` is then refused as
  `option<?>`: the literal shadows the state.
- **The roster** has nothing that cuts, rewrites or builds. `replace` and
  `push` are the router's.
- **`[]` lowers to `List 0`**, and both executors build `List n`
  (`code.rs:330`). The JS target already compares strings, because it emits
  `a < b`; only the checker (`types/src/lib.rs:920`) and the runner
  (`vm.rs:696`) refuse.
- **Budgets.** Only the runner enforces `MAX_LIST_STEPS` and composite
  accounting, and only for `Map`/`Filter`/`Join` (`vm.rs:763`); other roster
  calls get neither. The JS target enforces neither: it emits native
  `.map`/`.filter` (`code.rs:433`), and `x_join`/`x_includes` are unmetered.
- **`tabIndex`** is a kernel prop (`schema.json`, id 13) that the macOS
  presenter orders focus by. No authored spelling binds it: `box tabIndex=0`
  is `lower-unknown-attr` (calendar F2), and on a module tag it was a module
  prop (paint F7).

The predecessor kept lists out (LLP 1017 P5, §8) so that `fn` would not grow
into a programming language. LLP 1017.003 already admits bounded traversal
(`map`, `filter`, `join`). The boundary this RFC draws, taking the reviewer's
framing, is **bounded, pure value operations** (admitted) against **durable
ownership, I/O and domain algorithms** (the data module's). A list the app
keeps across launches, such as shop's persisted cart and orders, stays in the
data module.

## 2. Decisions

### D1 — Strings compare in code-unit order

`<`, `<=`, `>` and `>=` type when both operands are `string`, and answer as
ECMA-262's `IsLessThan` does for two Strings: UTF-16 code units in order, a
proper prefix being less, with no locale and no normalization. ISO 8601 text
sorts correctly: `"09:30" < "10:00"`. Mixed or `bool` operands stay
`type-operand` ("comparison needs two numbers or two strings, given …").

In the runner, `Lt`/`Le`/`Gt`/`Ge` gain a string arm,
`a.encode_utf16().cmp(b.encode_utf16())`. Rust's `str` order is code-point
order, which is different. Lean gains the same case over a `Str.utf16Units`.
The JS target needs no change, and there is no opcode or plan change.

### D2 — Three string functions, as JavaScript defines them

| Contract | JavaScript | Type | Consumer |
|---|---|---|---|
| `slice(s, start, end?)` | `s.slice(start, end)` | `string, number, number → string` | calc Backspace, `slice(expr, 0, -1)` |
| `replaceAll(s, find, with)` | `s.replaceAll(find, with)`, `find` a string | `string, string, string → string` | hn-reader F2; calc's ± |
| `toLowerCase(s)` | `s.toLowerCase()` | `string → string` | case-insensitive search that re-asks the data module per keystroke (`flashcards/app.ts:151`, `chat/app.ts:72`) |

- **Positions are UTF-16 code units**, as `length` counts them.
  `ToIntegerOrInfinity` applies: truncation toward zero, `NaN` becomes 0,
  negative indices count from the end, and the result is clamped.
- **The completed result is made well formed.** The runner, Lean and the
  native hosts hold strings as Unicode scalar values. Each executor computes
  the whole result in code units. Then, once, every lone surrogate becomes
  U+FFFD, which is ECMA-262's `toWellFormed`. Fragments are never normalized,
  so `replaceAll("😀", "", "")` keeps the emoji. The runner and Lean decode
  with `char::decode_utf16` and replacement; the JS target calls
  `.toWellFormed()` on the result. This is a declared deviation in
  LLP 1006 §2.
- **`replaceAll`** applies `GetSubstitution` (`$$`, `$&`, `` $` ``, `$'`; `$1`
  is literal with a string pattern). An empty `find` inserts at every
  code-unit boundary. No regular expressions.
- **Bounds, in UTF-8 bytes.** `MAX_STRING` (`vm.rs:22`) counts the UTF-8
  bytes of the well-formed result. Both executors build by **bounded
  construction**: segment by segment, with a running byte count. They trap
  `StringTooLong` as soon as the count exceeds `MAX_STRING + 2`, where the
  slack covers a pair split across two segments. They then check the
  completed, normalized result exactly. No result larger than the bound is
  ever allocated, so a `replaceAll` whose `` $` ``/`$'` substitutions grow
  quadratically traps after at most `MAX_STRING + 2` bytes. On the JS target,
  `x_replaceAll` counts as it builds rather than calling the built-in and
  checking afterwards. `toLowerCase` can expand (`"İ"` becomes two code units)
  and is bounded the same way. `slice` never grows its input.
- **`end?` is an optional roster arity, resolved after names.** A scoped
  action or action prop named `slice` takes precedence over the roster
  (`types/src/lib.rs:790`). A parser rewrite would therefore change a user
  action's arity, so there is none. Instead, the roster row declares
  `"optional": ["Infinity"]`. `plan/build.rs` generates `Stdlib::min_arity()`
  and the defaults. The checker admits `min_arity..=arity` only for a call it
  resolved to the roster, and records it in `Types.roster_calls` (call span →
  `Stdlib`). Lowering appends the default for a recorded call; the default
  `Infinity` clamps as `undefined` does. `lean.rs` (which today emits the
  parsed arguments, `:313`) appends the same default from the same record.
  The JS emitter's `number()` prints `Infinity`.
- **`toLowerCase` is linked by use.** It joins `Capability::TextTransform`'s
  use-set (`uses.rs`). The runner maps through the kernel's case link
  (`kernel/src/text/case.rs:31`, about 3–6 KiB on a web core). An artifact
  that does not link it refuses the plan at boot (LLP 1047 D6). There is one
  table and one link. Mapping is Unicode default, locale-independent case
  conversion with final sigma. Code points whose mapping changed between
  Rust's and the browser's Unicode versions may differ; this is declared.
  Lean leaves `toLowerCase` out, as it leaves out `format*`, and the
  difftest generator never emits it.

**Deferred, each with its trigger.**

- **Numeric parsing** (`Number`, `parseInt`, `parseFloat`; calendar asked
  for `toNumber`). Calendar's validation is D1. Its remaining use is
  time-of-day arithmetic: keeping a duration while the start time moves.
  That is a calendar algorithm with time zones and day rollover, which
  belongs with the data module that already builds the month and the
  overlaps. `Number("")` is 0 and `NaN` spreads, which makes parsing a
  footgun inside a view. Trigger: a view that needs a number from text per
  keystroke with no domain meaning, such as a numeric field's live total.
- **`toUpperCase`.** Display casing is CSS `text-transform`.
- **`padStart`.** Its only consumer would be formatting the arithmetic above.
  When it is admitted, it follows `StringPad`'s order exactly: ToLength
  first; return `s` when the target length is at most `length(s)`; return
  `s` when `fill` is empty (Bun: `"abc".padStart(Infinity, "") === "abc"`);
  only then the byte bound.
- **`replace` (first match), `split`, `indexOf`, `substring`, `padEnd`, regular
  expressions.** `contract_syntax::idioms` names the alternative for each.

### D3 — Lists the screen builds, under one budget

**Literals.** `[a, b, c]` is a list whose items unify as the arms of `?:`
do. `[]` keeps its current rules. Items may span lines, because the lexer
counts bracket depth. A trailing comma is accepted and **kept**: the
formatter preserves tokens (`fmt.rs:24`). Mismatched items are
`type-list-item` ("item 2 is `number`, item 1 `string`"). A spread such as
`[...xs, x]` is `syntax-refused-idiom` ("write `concat(xs, [x])`"). The AST's
`Expr::EmptyList` becomes `Expr::List(items, span)`, which lowers to the items
and `List n`.

**Functions.**

| Contract | JavaScript | Type |
|---|---|---|
| `concat(xs, ys)` | `xs.concat(ys)`, `ys` an array | `list<T>, list<T> → list<T>` |
| `slice(xs, start, end?)` | `xs.slice(start, end)` | `list<T>, number, number → list<T>` (D2's row) |
| `includes(xs, x)` | `xs.includes(x)` (`SameValueZero`) | `list<T>, T → bool`; `T` must be string, number or bool |

`concat` takes exactly two lists. Records and options in `includes` are
refused, as `join` refuses them, because the web compares objects by
identity.

**One evaluation budget on both executors.** One evaluation means one run of
a code body: a derive, a binding or an action.

- **Runner.** The VM's `Call` arm passes its `steps` counter and its
  `Extents` to every list-touching entry through one internal signature,
  where `join` is special-cased today. Steps are charged as follows: `concat`
  pays `len(xs) + len(ys)`, `slice` pays the result's length, and `includes`
  pays the items scanned up to the match, so an early exit pays less. Every
  constructed list (a literal, `concat`, `slice`) goes through
  `extents.built`, so `MAX_VALUE_NODES` and `MAX_VALUE_BYTES` count shared
  elements once per place. A short `concat` of two huge shared elements traps
  even though its step count is small.
- **JS target.** The roster's value functions (`x_length` … `x_join`,
  `rt.js:1283–1293`) move into a new `host/web-js/stdlib.js` with the new
  entries. `rt.js` re-exports them with one `export * from "./stdlib.js"`,
  since it is at 1,498 lines, and `build.mjs:232`'s copy list gains the
  file. `stdlib.js` holds the same budget: a step counter, plus node and
  byte totals cached per array in a `WeakMap`. A string's UTF-8 length is
  counted only when `3 × length` could exceed the bound. Where the runner's
  `eval` starts, the budget resets: in the derive/binding wrapper
  (`rt.js:94`) and at an action's commit (`rt.js:138`). The emitter calls
  `x_map`/`x_filter` instead of native `.map`/`.filter` (`code.rs:433`). An
  exceeded budget throws a `Refusal` named as the runner's trap, so
  conformance compares the two refusals. This closes the JS target's existing
  gap for `map`, `filter` and `join` as well.

**Where a list lives** (in `contract-for-agents.md`, replacing "There are no
nonempty list literals"): a list the screen keeps for the session is `state`.
Examples are a selection, open or collapsed ids, and per-row UI offsets. A
list the app keeps across launches or a server owns belongs to the data
module. So do sorting, grouping and aggregates. HN's collapsed set, written
in Contract:

```contract
state collapsed = []
action toggle(id: string)
  collapsed = includes(collapsed, id) ? filter(collapsed, (c) => c != id) : concat(collapsed, [id])
```

### D4 — A state initializer reads props, injects and earlier states

Kanban F5's `state scrolls = map(board.cols, …)` got "unknown name `board`;
did you mean `boardX`?". The rule is right, because an initializer runs
before any resource answers. The message is wrong. The new id,
`type-initializer-scope`, says what the name is and where the value belongs:

> `board` is a resource; a state's initializer runs before any resource
> answers, and reads only props, injects and earlier states. Derive it from
> `board`, or keep per-row state in a component used inside
> `each board.cols`.

A derive, an action or a later state named in an initializer gets the same
refusal. A near-name suggestion appears only when no declaration of that name
exists. Initializing a state from a resource's first answer needs a
lifecycle event, which is LLP 1035.005.000 D7b.

### D5 — Sixteen words are reserved, and only where a name is bound

**Binder positions** introduce a name a later expression reads: a component's
props and injects, states, derives, resources, mutations, actions, action and
`fn` parameters, `fn` names, `let`, `each` item and index, `case some(x)`, and
arrow parameters. These positions refuse `when`, `if`, `else`, `each`, `in`,
`match`, `case`, `as`, `fn`, `and`, `or` and `not`, and the literals `true`,
`false`, `none` and `some`. JavaScript reserves its literals the same way.

**Every other name position admits every word**: shape fields, named
arguments (`Flags(none=1)`), members after `.`, and attribute names (SVG's
`in`, unchanged). None of these can be read as an expression head.

The other 23 words of `is_keyword` are contextual. They are a keyword only
where their construct starts, and a name at every binder: `component`,
`font`, `shape`, `style`, `from`, `state`, `derive`, `resource`, `mutation`,
`action`, `task`, `mount`, `view`, `props`, `provide`, `inject`, `slot`,
`children`, `key`, `refresh`, `writes`, `test`, `expect`. `refresh` begins a
statement only before a name, as `send` and `let` already do. So
`action refresh`, `press=refresh` and `refresh feed` each have one parse.
`state view = "month"` is assigned as `view = "week"`. No backend uses an
authored name as a JavaScript identifier (the JS target emits `p0`, and
`typescript.rs` quotes property names).

The refusal drops the generated `ins`/`myIn` suggestion: "`in` is reserved
in Contract (it shapes an expression); choose another name". The grammar doc
lists both sets.

### D6 — State is typed from initializers and writes before the lists that read it

**Root cause.** `check_component` types row-owned slots
(`infer_owned_state_initializers`, `checks.rs:409`) only after
`collect_owner_scopes` has strictly inferred every `each` list, nested ones
included. It types writes (action bodies, `component.rs:202` onward) later
still. Shop's inner list reads its row's `pick#N` while that slot is `?`.
Review finding 4's root list `items = []` is `list<?>` until an action's write
types it.

**Fix.** Iterate to a fixpoint before any strict check. Each round types the
derives, then the owned initializers (outer to inner, each in its `each`'s
scope), then the slot refinements from action writes (the existing scratch
round). Each phase is lenient: a list or an initializer that is still `?`
waits for the next round. The rounds stop when no slot or derive type changes,
or after `slots + derives + 2` rounds. The strict passes then run unchanged.
Both repros compile, and so does an empty record-list state iterated twice
deep.

**No `?` in a final message, without changing inference.** Today
`uses.rs:138` and `lib.rs:236` decide what is provisional by searching
message text for `` `?` ``. `TypeError` gains a structured `provisional`
flag, set wherever an operand's type is incomplete, and those two sites read
the flag. A provisional error that survives to the final sink is reported as
`type-cannot-infer`, naming the unknown binding when there is one ("cannot
infer the type of `pick` here"). A test asserts that no final message
matches `?` in any form (`` `?` ``, `option<?>`, `list<?>`), and the existing
tests where provisional errors resolve stay green.

### D7 — Diagnostics that name the fix

1. **Handler arity spells the declaration** (pomodoro F1). The payload is
   named after the DOM property it carries (`checked`, `value`, `key`,
   `data`) and typed by the element: "`change=flipTask(t.id)` calls `flipTask`
   with `t.id` and then the checkbox's new `checked` (bool); declare
   `action flipTask(id: string, checked: bool)`". Analysis (`lib.rs:478`) and
   lowering (`lower/src/lib.rs:1414`) print it through one function.
2. **Module tags: a known attribute binds to the box only if the box uses it**
   (paint F7). The deny-list (`native.rs:78`) becomes an allow-list. The box
   takes layout, box and paint rows, handlers, `testId`, `id`, `class`,
   `data-*`, `role`, `aria-*`, `disabled` and `tabindex`. **SVG-only props
   stay module props** (`svg_only_prop`, `native.rs:61`): a leaf box never
   reads them, and the native fixture's `mode` and the `ghostty-terminal`
   corpus test (`corpus.rs:595`) depend on it. Removing the exception would
   rename consumers for no gain. Every other known attribute (`command`,
   `commandfor`, `href`, `src`, `value`, …) is `lower-native-attr` ("give the
   module prop another name").
   A case variant of a known attribute (`tabIndex` for `tabindex`) is refused
   naming the known spelling. This check runs **before** the leftover
   classification.

   **Consumer census** (every module tag in `apps/*` and `x2apps`):
   - `native-fixture`: `mode` stays (SVG-only), and `disabled` is
     allow-listed.
   - `map-demo`, `photo-editor`, `recorder`: no known attributes besides
     handlers and box rows.
   - paint: its `tabIndex` becomes `tabindex`, outside the repo.

   No in-repo app changes. Stage 1 drives `native-fixture` on web and macOS.
3. **`tabindex` binds for real** (calendar F2, paint F7). HTML's `tabindex`
   is an authored attribute on every element and module tag, bound to
   `PropId::TabIndex`, with no `tabIndex` alias. Today the web hosts never
   emit it. The JS target and the wasm web host write the `tabindex` DOM
   attribute from the prop. The Apple presenters already order by it
   (`PresenterMac.swift:1102`). Stage 1 verifies focus by driving `key Tab`
   on web and macOS over a root `main tabindex=0` with a `key=` handler
   (calendar's case) and over a module tag. The broader spelling question
   (`scrollLeft` stays the DOM property; `emojiPicker` and `navigationKey`
   are Exact's) is a naming RFC of its own and does not block this.
4. **The web's methods name their Contract form.** CSS `background-color: …`
   in a style gets the rewritten row (ledger F3). `idioms.rs` gains `split`,
   `indexOf`, `substring`, `padStart`, `padEnd`, `toUpperCase`, `Number`,
   `parseInt`, a three-argument `replace` (pointing to `replaceAll`) and a
   list `push` (pointing to `concat(xs, [x])`). `len` gets an explicit hint
   naming `length`, with no alias. The one-edit suggestions
   (`checks.rs:281`) reach `lengh` and `lenght`, but not `len`.

### D8 — Two sends to one mutation on one path are refused

Flashcards F4's `commitEdit` sent `saveCard`, then `setImage`, to the same
`edited` mutation. Its `then` saw only the second reply, because a send
forgets the in-flight reply (LLP 1016 D5). The behavior is documented and
was still invisible. Analysis refuses an action body that sends one mutation
twice on any path, with the new id `analyze-send-twice`; exclusive `if` and
`match` arms are separate paths:

> `commitEdit` sends `edited` twice; only the last send's reply reaches
> `then afterEdit` (LLP 1016 D5). Send once, or use a mutation per request.

A refusal, not a warning: what an agent ignores is a warning, and assigning a
mutation twice has no intended use that two mutations do not serve.

## 3. Effect on each implementation

| | Stage 1 (D4–D8) | Stage 2 (D1, D2) | Stage 3 (D3) |
|---|---|---|---|
| syntax | D5 in a new `names.rs` (`parser.rs` is at 1,469 lines); `refresh` contextual; colon recovery; idioms | — | `Expr::List`; spread refusal; `fmt` keeps commas |
| types | D6 fixpoint and `provisional` flag; D4 | string order; `optional` arity after resolution; `roster_calls` | literal unification; list `concat`/`slice`/`includes` |
| analyze, lower | D7.1; D7.2 allow-list; `tabindex` binding; D8 | append defaults for recorded calls | `List n` |
| `format.json` | — | `slice` (`any, number, number → any`, `optional`), `replaceAll`, `toLowerCase`; `plan/build.rs` min arity | `concat`; `includes` → `any, any → bool` |
| runner | — | string compare; bounded `replaceAll`/`toLowerCase`; `TextTransform` use | budget through `Call`; `extents.built` |
| JS target | `tabindex` attribute | `x_slice`, `x_replaceAll`, `x_toLowerCase` | `stdlib.js` extraction with the budget; `x_map`/`x_filter`; `build.mjs` copy list |
| web host (wasm) | `tabindex` attribute | — | — |
| Lean | — | `utf16Units`, string order, `slice`, `GetSubstitution`; `lean.rs` defaults | `.list` replaces `.emptyList` in `Syntax`, `Eval` and `Big` (relations, soundness, completeness, determinism) |
| difftest | — | generator emits D1/D2 (not `toLowerCase`), astral strings | literals, list calls |
| docs | keyword sets; initializers; `tabindex`; D8 | roster; comparison; deviation in LLP 1006 §2 | "Where a list lives" |

The plan's digest changes and `formatVersion` does not, as when
`startsWith` landed.

## 4. Tests

- **Compiler** (`contract/cli/tests/it/`):
  - `names.rs`: every contextual word at every binder; reserved words refused
    at binders and admitted as fields, named arguments, members and SVG `in`.
  - `instance.rs`: shop's shape; the root `items = []` with a nested `each`.
  - `search.rs`: string order; the three functions; a user action named
    `slice` keeps its arity.
  - `lists.rs`: literals with trailing comma kept by `fmt`, `type-list-item`,
    spread, `concat`, `slice`, `includes`.
  - `diagnostics.rs`: D4, D7 and D8, each message asserted whole; the
    no-`?` assertion over `rejects.txt`.
  - `corpus.rs` (`ghostty-terminal`) and a new `tabindex` lowering case.
- **Runner** (`stdlib.rs` and `vm.rs` tests, with expected values from
  `bun -e`): negative and fractional indices, `NaN`, `±Infinity`, surrogate
  cuts, `"😀"` with an empty pattern, `$&`/`$$`/`` $` ``/`$'`/`$1`, quadratic
  substitution trapping before allocation, `"İ"`, final sigma, code-unit order
  across U+E000/U+10000, early-exit `includes` steps, nested
  `map(concat(…))` steps, and a `concat` over `MAX_VALUE_BYTES`.
- **Lean differential:** `semantics/corpus/text/{compare,slice,replace-all}`
  and `semantics/corpus/lists/{literals,concat,slice,includes}`; the corpus run
  and a random sweep; `lake build` checks the updated proofs.
- **JS against the runner:** `host/web-js/conformance/text-and-lists.contract`
  exercises the existing roster functions (`at`, `join`, `includes`,
  `formatDate`) beside the new ones, plus the budget traps, in Chrome, Firefox
  and WebKit (async lane). The `x_` export test covers the new entries.
- **Driven:**
  - Stage 1: `native-fixture` on web and macOS; `key Tab` focus as in D7.3.
  - Stage 2: calendar's end-after-start and calc's Backspace, in Contract.
  - Stage 3: a scratch `exact new` app with HN's collapsed set and kanban's
    per-column scroll offsets in `state`, driven on web and macOS; shop's
    first form of `product.contract` builds.
- The five checks after each stage.

## 5. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, on
`gaps/papercuts` or its successor on main. Each commit passes the five checks.

1. **Stage 1, 2026-10-04: the compiler, plus `tabindex` on the web hosts
   (D4–D8).** Exit: the diaries' repros and review finding 4's case compile or
   are refused with the new text, `native-fixture` is driven, and Tab focus is
   verified.
2. **Stage 2, 2026-10-05: strings (D1, D2).** Exit: the runner tables, Lean,
   difftest and conformance are green; the `TextTransform` link is measured
   with `metrics.mjs` for an unused, a formatting-only and a case-using app.
3. **Stage 3, 2026-10-05: lists and the budget (D3).** Exit: the budget trap
   tables are green on both executors, the scratch app is driven, and
   LLP 1006 §2 and LLP 1017.003 are amended in the same commit.

## 6. Considered and not taken

- `compare(a, b)`: a second spelling of an operator JavaScript already has.
- Overloading roster names by type, so string `replace` could sit beside the
  router's: a new resolution rule.
- Spread syntax: a second construction form beside `concat`. Revisit if
  agents keep writing it after the refusal ships.
- Removing the SVG-only module-prop exception: it would rename consumers for
  no gain.
- Warning instead of refusing in D8: warnings go unread.

## 7. Questions decided for r2 (the orchestrator, for Charlie, 2026-10-04)

1. **Admit session-owned lists**, on the line between bounded, pure value
   operations and durable ownership, I/O and domain algorithms (D3).
2. **U+FFFD normalization of the completed result** (D2).
3. **Link the case tables by use** (`TextTransform`, D2).
4. **HTML `tabindex`, no aliases.** The naming RFC for the remaining props
   does not block it (D7.3).

Still open for a reviewer: whether the JS target's budget reset points (D3)
match the runner's `eval` boundaries closely enough that a trap at the edge of
the budget falls on the same step. The stage 3 conformance plan tests this,
and any mismatch is fixed toward the runner.

## 8. Revisions

**r2** (2026-10-04, Astra's r1 review; one family). Each finding was checked
against the code, and §1 records the re-checks.

- D3 gains the shared budget on both executors (finding 1).
- D2 bounds are UTF-8 bytes with bounded construction, and the `padStart`
  order is recorded for when it is admitted (2).
- Optional arity resolves after names, including in Lean (3).
- D6 becomes a fixpoint over writes, with a structured `provisional` flag
  (4, 8).
- The SVG exception is kept, with a consumer census (5).
- `tabindex` is really bound and focus is verified (6).
- The base is `gaps/papercuts`, and `stdlib.js` is extracted with its
  packaging (7).
- Trailing commas are kept (9).
- D5 applies at binder positions only (10).
- D2 is narrowed to entries with consumers, `toUpperCase` and `padStart` are
  deferred, and numeric parsing is decided (11).
- `len` gets an explicit hint (12).
- The implementer is named (13).
- D8 (flashcards F4) and the snake F2 note are added.
