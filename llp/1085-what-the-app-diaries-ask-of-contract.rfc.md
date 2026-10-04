# LLP 1085: What the app diaries ask of Contract

**Type:** RFC
**Status:** Draft r3, 2026-10-04. Stage 1 is built as of 2026-10-04, without D6's fixed point (§10, "As built"). Reviews so far, both by one family (Astra, `gpt-6-astra`, max; Grok was unavailable): r1 NOT READY (7 MATERIAL, 6 MINOR); r2 NOT READY (10 of r1's findings resolved, 3 partly, 4 new MATERIAL, 3 new MINOR). This is round 2 of the three-round fix-loop limit (`rules/RULES.md`), so r3 descopes rather than grinds: the lists decision moves to a follow-up (§9). §8 lists each revision.
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`, `contract/cli/src/lean.rs`), Plan (`plan/tables/format.json` `stdlib`), Runner (`vm.rs`, `stdlib.rs`, `uses.rs`), JS target (`host/web-js`), web host (`host/web`), Apple hosts (`host/apple`), Linux host, Lean semantics and difftest (`semantics/`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-04, stages 2 and 3 on 2026-10-05 (§5)
**Base:** `gaps/papercuts`, which carries `host/web-js/format.js`, `x_at`/`x_formatDate`/`x_formatNumber` and the `x_` export test (`host/web-js/src/code.rs:595`)
**Amends:** LLP 1006 §2 (the language); LLP 1024 D1 (which known attributes bind to a module tag's box)
**Related:** LLP 1016 D5 (a send forgets the in-flight reply); LLP 1017 P5 and §8; LLP 1017.003; LLP 1035.005.000 D7b; LLP 1047 D2/D6 (linked by use); LLP 1054.000.005 (`trim`); LLP 1071 (the JS target); diaries `~/projects/x2apps/<app>/DIARY.md`; reviews `llp/reviews/1085-r1.astra.md`, `llp/reviews/1085-r2.astra.md`. Web: ECMA-262 `IsLessThan`, `String.prototype.{slice,replaceAll,toLowerCase,toWellFormed}`, `GetSubstitution`, `StringPad`; HTML `tabindex` (focusable areas, sequential focus navigation).

## Summary

On 2026-10-04 several agents each built an app outside the repo with
`exact new` and kept a diary. Calendar moved its validation into the data
module because `"10:00" > "09:30"` does not type, and calc sent Backspace
through a mutation because there is no `slice`. The other agents lost time
to smaller problems: reserved words where a parameter goes, a type error
that printed `?`, a send whose reply vanished, a focus attribute that binds
nothing, and diagnostics that named the fix obliquely. Four apps also
redesigned around the lack of list construction. Admitting that is decided
in principle but deferred to a follow-up (§9), because the JS target cannot
yet bound it the way the runner does.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `<` `<=` `>` `>=` on two strings, in UTF-16 code-unit order | calendar F1 | 3 |
| D2 | `slice`, `replaceAll`, `toLowerCase` on strings, bounded by `MAX_STRING`; numeric parsing deferred | calc, hn-reader F2, calendar F1, flashcards/chat search | 3 |
| D4 | A state initializer's scope, stated by its refusal | kanban F5 | 1 |
| D5 | 16 words reserved at binder positions only; the rest contextual | kanban F13, shop F6, hn-reader F1, ledger F3, calendar F16 | 1 |
| D6 | One fixed point for state, parameter and derive types before strict checks; no `?` in a final message | shop F3, reviews | 1 |
| D7 | Diagnostics that name the fix; module-tag allow-list | pomodoro F1, paint F7, ledger F3, hn-reader F2 | 1 |
| D7.3 | HTML `tabindex`, focusable on every host | calendar F2, paint F7 | 2 |
| D8 | Two sends to one mutation on one path are refused | flashcards F4 | 1 |
| (D3) | List construction: deferred to "The JS target's evaluation budget" | pomodoro F3, kanban, hn-reader F2 | §9 |

Snake F2, where a source named `advance` bricked the app, is not a naming
problem. It was a web build bug, fixed in `gaps/papercuts` `67d6b873`, and
nothing is reserved for it.

## 1. Evidence

Each case was reproduced with `target/debug/contract build --json`, on
`9cd6863d` and then on the rebased base:

- **shop F3** fails only for a child used inside an `each` whose own view
  iterates a derive over its state. Two cases from the reviews also fail.
  `state items = []` written by a typed action, then a nested `each` over
  it, gives "`?` has no fields". `state items = []; state other = items` gives
  "nothing writes a value into `other`" (reproduced).
- **`state none = 1` compiles.** `toString(none)` is then refused as
  `option<?>`.
- **The roster** has nothing that cuts or rewrites a string. `replace` and
  `push` are the router's. The JS target already compares strings with
  `a < b`; only the checker (`types/src/lib.rs:920`) and the runner
  (`vm.rs:696`) refuse.
- **`tabIndex`** is a kernel prop (`schema.json`, id 13) that no authored
  spelling binds (`box tabIndex=0` is `lower-unknown-attr`). No host makes a
  box focusable by it. macOS's `acceptsFirstResponder` ignores it
  (`NodeViewMac.swift:176`), and its presenter leaves a noninteractive box out
  at index 0 (`PresenterMac.swift:1153`). UIKit's `canBecomeFirstResponder`
  ignores it (`NodeViewIOS.swift:256`). Both web renderers synthesize
  `tabindex=0` for a node with `focus`, `blur` or `key` handlers
  (`host/web-js/src/emit.rs:971`, `host/web/src/document.rs:613`).

## 2. Decisions

### D1 — Strings compare in code-unit order

`<`, `<=`, `>` and `>=` type when both operands are `string` and answer as
ECMA-262's `IsLessThan` answers for two Strings: UTF-16 code units in order, a
proper prefix being less, with no locale and no normalization.
`"09:30" < "10:00"`. Mixed or `bool` operands stay `type-operand` ("comparison
needs two numbers or two strings, given …").

- **Runner.** `Lt`/`Le`/`Gt`/`Ge` gain a string arm,
  `a.encode_utf16().cmp(b.encode_utf16())`. Rust's `str` order is code-point
  order and differs.
- **Lean.** The same comparison over a `Str.utf16Units`.
- **JS target.** No change. There is no opcode and no plan change.

### D2 — Three string functions, as JavaScript defines them

| Contract | JavaScript | Type | Consumer |
|---|---|---|---|
| `slice(s, start, end?)` | `s.slice(start, end)` | `string, number, number → string` | calc's Backspace, `slice(expr, 0, -1)` |
| `replaceAll(s, find, with)` | `s.replaceAll(find, with)`, `find` a string | `string, string, string → string` | hn-reader F2; calc's ± |
| `toLowerCase(s)` | `s.toLowerCase()` | `string → string` | search that re-asks the data module on every keystroke (`flashcards/app.ts:151`, `chat/app.ts:72`) |

- **Positions are UTF-16 code units.** `ToIntegerOrInfinity` applies:
  fractions truncate toward zero, `NaN` becomes 0, negative indices count from
  the end, and the result is clamped.
- **The completed result is made well formed.** Each executor computes the
  whole result in code units. Then, once, it replaces every lone surrogate
  with U+FFFD (`toWellFormed`). Fragments are never normalized, so
  `replaceAll("😀", "", "")` keeps the emoji. This is a declared deviation in
  LLP 1006 §2, because the runner, Lean and the native hosts hold Unicode
  scalar values.
- **`replaceAll`** applies `GetSubstitution` (`$$`, `$&`, `` $` ``, `$'`; `$1`
  is literal with a string pattern). An empty `find` inserts at every
  code-unit boundary. There are no regular expressions.
- **Bounded by `MAX_STRING` alone, on both executors.** These functions take
  no list steps and build no list, record or option, so they need no
  evaluation budget, only a string-size bound. `MAX_STRING` (`vm.rs:22`)
  counts UTF-8 bytes of the well-formed result.
  - Each function is one call whose output is built **segment by segment with
    a running UTF-8 byte count**. It traps `StringTooLong` once the count
    passes `MAX_STRING + 2`; the slack covers a surrogate pair split across
    two segments. The completed, normalized result is then checked exactly.
  - No result over the bound is allocated, so a `replaceAll` whose
    `` $` ``/`$'` substitutions grow quadratically traps early.
  - `slice` never grows its input. `toLowerCase` can grow it (`"İ"` → two
    code units).
  - On the JS target, `x_replaceAll` and `x_toLowerCase` count as they build,
    so neither calls the built-in and checks afterwards. A JS trap throws the
    runner's trap as a `Refusal`.
  - The JS target's other string producers (`+`, templates, `join`) stay
    unbounded today, the same gap as §9's.
- **`end?` is an optional roster arity, resolved after names.** A scoped
  action or action prop named `slice` takes precedence over the roster
  (`types/src/lib.rs:790`), so no parser rewrite is used.
  - The roster row declares `"optional": ["Infinity"]`, and `plan/build.rs`
    generates `Stdlib::min_arity()` and the defaults.
  - The checker admits `min_arity..=arity` only for a call it resolved to the
    roster, and records it in `Types.roster_calls` (call span → `Stdlib`).
  - Lowering and `lean.rs` (`:313`) both append the default for a recorded
    call. `Infinity` clamps as `undefined` does, and the JS emitter prints it
    as `Infinity`.
- **`toLowerCase` is linked by use.** It joins
  `Capability::TextTransform`'s use-set and maps through the kernel's case
  link (`kernel/src/text/case.rs:31`), which is about 3–6 KiB on a web core.
  An artifact that lacks the link refuses the plan at boot (LLP 1047 D6).
  - Mapping is Unicode default, locale-independent case conversion with final
    sigma. A code point whose mapping changed between Rust's and the browser's
    Unicode versions may map differently; that is declared.
  - Lean leaves it out, as it does `format*`, and the difftest generator never
    emits it.

**Deferred, each with its trigger.**

- **Numeric parsing** (`Number`, `parseInt`; calendar asked for
  `toNumber`). D1 covers calendar's validation. The remaining use, keeping a
  duration while the start moves, is a calendar algorithm with zones and
  rollover, and the data module already owns those. In a view, parsing is a
  footgun: `Number("")` is 0, and `NaN` spreads. Trigger: a view that needs a
  number from text on each keystroke, with no domain meaning.
- **`toUpperCase`**: display casing is CSS `text-transform`.
- **`padStart`**. When admitted, it follows `StringPad`'s order: ToLength
  first; return `s` when the target length is at most `length(s)`, or when
  `fill` is empty (`"abc".padStart(Infinity, "") === "abc"`); only then apply
  the byte bound.
- **`replace` (first match), `split`, `indexOf`, `substring`, `padEnd`,
  regular expressions.** `contract_syntax::idioms` names the alternative for
  each.

### D4 — A state initializer reads props, injects and earlier states

Kanban F5's `state scrolls = map(board.cols, …)` got "unknown name `board`;
did you mean `boardX`?". The rule is right, because an initializer runs
before any resource answers. The message is wrong. The new id is
`type-initializer-scope`:

> `board` is a resource; a state's initializer runs before any resource
> answers, and reads only props, injects and earlier states. Derive it from
> `board`, or keep per-row state in a component used inside
> `each board.cols`.

A derive, an action or a later state named in an initializer gets the same
refusal. A near-name suggestion appears only when no declaration of that name
exists. A state seeded from a resource's first answer is LLP 1035.005.000 D7b.

### D5 — Sixteen words are reserved, and only where a name is bound

**Binder positions** introduce a name that a later expression reads or calls.
They are:
- a component's props and injects;
- states, derives, resources, mutations and actions;
- action and `fn` parameters, and `fn` names;
- `let`, `each` item and index, `case some(x)`, and arrow parameters;
- **shape names**, because a shape name is a constructor, an expression head
  (`records.rs:9`).

These positions refuse `when`, `if`, `else`, `each`, `in`, `match`, `case`,
`as`, `fn`, `and`, `or`, `not`, `true`, `false`, `none` and `some`. Today
`shape none` is accepted, and then `none(value=1)` parses as the literal.

**Unrestricted positions** admit every word, because none of them can head an
expression. They are, exhaustively:
- shape fields;
- named arguments (`Flags(none=1)`);
- members after `.`;
- attribute names (SVG's `in`, unchanged).

The other 23 words of `is_keyword` are contextual: each is a keyword only
where its construct starts, and a name at every binder. They are
`component`, `font`, `shape`, `style`, `from`, `state`, `derive`,
`resource`, `mutation`, `action`, `task`, `mount`, `view`, `props`,
`provide`, `inject`, `slot`, `children`, `key`, `refresh`, `writes`, `test`
and `expect`.
- `refresh` begins a statement only before a name, as `send` and `let`
  already do. So `action refresh`, `press=refresh` and `refresh feed` each
  have one parse.
- No backend uses an authored name as a JavaScript identifier.

The refusal reads "`in` is reserved in Contract (it shapes an expression);
choose another name", without the `ins`/`myIn` suggestion. The grammar doc
lists both sets.

### D6 — One fixed point before any strict check

**Root cause.** `check_component` runs its inference phases once each, in
this order:
1. derives;
2. owned (row) initializers (`checks.rs:409`), whose owner scopes strictly
   infer every `each` list;
3. handler-parameter refinement (`refine_params_from_view`,
   `component.rs:207`);
4. write refinement from action bodies (`:213`).

A type learned in a later phase never reaches an earlier one. So:
- shop's inner list reads its row's `pick#N` while that state is still `?`;
- a root `items = []` is still `list<?>` when the nested `each` is checked;
- `items = v` cannot type `items` until `v` has its type, which
  `press=replaceItems(sourceItems)` gives it;
- root initializers are never revisited, so `state other = items` keeps
  `list<?>`.

**Fix.** Iterate these phases, all lenient, until nothing changes:
1. Derives.
2. **Every** state initializer, root and owned. Owned initializers go outer
   to inner, each in its `each`'s scope, which is never wider than D4's.
3. Handler-parameter refinement.
4. Action-write refinement, as the existing scratch round.

Merging is a join: each phase **unifies** the type it learns with the type
already learned, and never replaces it. An initializer's `list<?>` must not
overwrite a `list<Item>` that a write already taught; today that overwrite is
at `checks.rs:447`.

Convergence is reached when no slot, derive or action-parameter type changes.
The number of rounds is bounded by `slots + derives + parameters + 2`, because
each change completes at least one `?`. A phase that still meets a `?` waits
for the next round. After the last round, the strict passes run unchanged.

**No `?` in a final message, without changing inference.** `uses.rs:138` and
`lib.rs:236` decide what is provisional by searching message text.
- `TypeError` gains a structured `provisional` flag, set when an operand's
  type is incomplete, and those two sites read the flag.
- A provisional error that reaches the final sink is reported as
  `type-cannot-infer`, naming the binding when there is one.
- A test asserts that no final message contains `?` in any form
  (`` `?` ``, `option<?>`, `list<?>`).

### D7 — Diagnostics that name the fix

1. **Handler arity spells the declaration** (pomodoro F1).
   - The payload is named after the DOM property it carries (`checked`,
     `value`, `key`, `data`) and typed by the element. For example:
     "`change=flipTask(t.id)` calls `flipTask` with `t.id` and then the
     checkbox's new `checked` (bool); declare `action flipTask(id: string,
     checked: bool)`".
   - Analysis (`lib.rs:478`) and lowering (`lower/src/lib.rs:1414`) print it
     through one function.
2. **On a module tag, a known attribute binds to the box only if the box uses
   it** (paint F7).
   - The deny-list (`native.rs:78`) becomes an allow-list. The box takes
     layout, box and paint rows, handlers, `testId`, `id`, `class`,
     `data-*`, `role`, `aria-*`, `disabled`, **`inert`** (the module's
     interaction suppression reads it, `NativeModule.swift:706`) and
     `tabindex`.
   - **SVG-only props stay module props** (`svg_only_prop`, `native.rs:61`).
     The native fixture's `mode` and the `ghostty-terminal` corpus test
     (`corpus.rs:595`) depend on them.
   - Every other known attribute is `lower-native-attr` ("give the module prop
     another name"). That covers `command`, `commandfor`, `href`, `src`,
     `value` and the rest.
   - A case variant of a known attribute (`tabIndex`) is refused naming the
     known spelling. This runs before the leftover classification.
   - **Census** of every module tag in `apps/*` and `x2apps`: `native-fixture`
     keeps `mode` and `disabled`; `map-demo`, `photo-editor` and `recorder`
     are unaffected; paint's `tabIndex` becomes `tabindex`, outside the repo.
     No in-repo app changes.
3. **`tabindex`, focusable on every host** (calendar F2, paint F7). Stage 2.
   HTML's `tabindex` is an authored attribute on every element and module
   tag, bound to `PropId::TabIndex`, with no `tabIndex` alias. Its meaning is
   HTML's:
   - an explicit value makes any box a focusable area;
   - `≥ 0` puts it in sequential navigation, positive values first in
     ascending order, then `0` in tree order;
   - a negative value is focusable by pointer and script but skipped by Tab.

   Each host:
   - **Web** (`emit.rs:971`, `document.rs:613`). An explicit value is emitted
     and **overrides** the `tabindex=0` synthesized for focus, blur and key
     handlers. A negative value on such a node takes it out of the Tab order.
   - **macOS.** `acceptsFirstResponder` (`NodeViewMac.swift:176`) returns
     true for a box with an explicit `tabindex`. `tabbable`, and the
     presenter's index-0 exclusion of noninteractive boxes
     (`PresenterMac.swift:1153`), admit an explicit value `≥ 0`. The key-view
     order follows HTML's.
   - **iOS.** `canBecomeFirstResponder` (`NodeViewIOS.swift:256`) is true for
     an explicit `tabindex`. A hardware keyboard's Tab follows the same order.
   - **Linux.** The focus walk applies the same eligibility and order.

   Tests, on web and macOS through the driver, Linux headless and iOS in the
   async lane:
   - an otherwise noninteractive `box tabindex=0` takes focus by Tab and then
     its `key` handler fires. The test has no handler-granted focusability,
     unlike r2's `main … key=` test, which proved nothing;
   - positive values order before zero;
   - a `tabindex=-1` box is skipped by Tab but focused by `tap` and
     `autofocus`;
   - `tabindex` bound to state changes eligibility while mounted;
   - `tabindex=-1` on a node with a `key` handler leaves the web's Tab order.

   The wider spelling question is a naming RFC of its own and does not block
   this. `scrollLeft` stays the DOM property; `emojiPicker` and
   `navigationKey` are Exact's.
4. **The web's methods name their Contract form.**
   - CSS's `background-color: …` in a style gets the rewritten row
     (ledger F3).
   - `idioms.rs` gains `split`, `indexOf`, `substring`, `padStart`,
     `padEnd`, `toUpperCase`, `Number`, `parseInt` and a three-argument
     `replace`, which points to `replaceAll`.
   - `concat`, list `push`, `slice` on a list and list literals name §9's
     follow-up and the data module for now.
   - `len` gets an explicit hint. The one-edit suggestions (`checks.rs:281`)
     cannot reach it.

### D8 — Two sends to one mutation on one path are refused

Flashcards F4's `commitEdit` sends `edited` twice, and its `then` sees only
the second reply (LLP 1016 D5). The new refusal is `analyze-send-twice`:

> `commitEdit` sends `edited` twice; only the last send's reply reaches
> `then afterEdit` (LLP 1016 D5). Send once, or use a mutation per request.

**The check is path-sensitive and walks `Stmt` itself.** `Action::effects()`
(`ast.rs:502`) flattens branches, so it cannot be used. The walk works like
this:
- It carries the set of mutations that some path may already have sent.
- Each `if`/`match` arm starts from the incoming set.
- After the branch, the set is the union of the arms' sets.
- A send whose mutation is already in the set is refused.

So exclusive arms pass, which keeps Messages and Fieldnotes accepted, while a
send in an arm plus another after the branch, in the common suffix, is
refused. It is a refusal, not a warning, because warnings go unread.

**Rollout.** Flashcards (`~/projects/x2apps/flashcards/app.contract:192`) sends
`saveCard` and then `setImage`, and stage 1 will refuse it. The app's owner
changes it to one combined request (`edit("saveCardWithImage", …)`) or to two
mutations, and drives save-with-image. It is outside this repo. The in-repo
apps all compile in stage 1.

## 3. Effect on each implementation

| | Stage 1 (D4–D8, not D7.3) | Stage 2 (D7.3) | Stage 3 (D1, D2) |
|---|---|---|---|
| syntax | D5 in a new `names.rs` (`parser.rs` is at 1,469 lines); `refresh` contextual; colon recovery; idioms | — | — |
| types | D6 fixed point, `provisional` flag; D4 | — | string order; `optional` arity; `roster_calls` |
| analyze, lower | D7.1; D7.2 allow-list; D8 walk | `tabindex` binding, case-variant refusal | defaults for recorded calls |
| `format.json` | — | — | `slice`, `replaceAll`, `toLowerCase`; `optional`; min arity |
| runner | — | — | string compare; bounded construction; `TextTransform` use |
| JS target | — | explicit `tabindex` emitted, overriding synthesis | `x_slice`, `x_replaceAll`, `x_toLowerCase` in `format.js`, re-exported on `rt.js`'s existing `format.js` line (`rt.js` is at 1,498 lines) |
| web host, Apple, Linux | — | focus eligibility and order | — |
| Lean, difftest | — | — | `utf16Units`, string order, `slice`, `GetSubstitution`, `lean.rs` defaults; generator (not `toLowerCase`), astral strings |
| docs | keyword sets; initializers; D8 | `tabindex` | roster; comparison; the deviation in LLP 1006 §2 |

The plan's digest changes and `formatVersion` does not.

## 4. Tests

- **Compiler** (`contract/cli/tests/it/`):
  - `names.rs`: every contextual word at every binder. Reserved words are
    refused at binders, shape names included (both the declaration and the
    construction), and admitted as fields, named arguments, members and SVG
    `in`.
  - `instance.rs`: shop's shape; a root `items = []` with a nested `each`;
    `replaceItems(v)` typed through `press=replaceItems(sourceItems)`;
    `state other = items` after a typed write.
  - `search.rs`: string order; the three functions; a user action named
    `slice` keeps its arity.
  - `diagnostics.rs`: D4, D7 and D8, each message asserted whole. For D8,
    exclusive arms are accepted, and a send in an arm plus a send in the
    common suffix is refused.
  - `corpus.rs`: no final message contains `?` in any form;
    `ghostty-terminal`; module-tag `inert` and `disabled` stay on the box; a
    `tabindex` lowering case.
- **Runner** (`stdlib.rs` tests, expected values from `bun -e`): negative and
  fractional indices, `NaN`, `±Infinity`, surrogate cuts, `"😀"` with an empty
  pattern, `$&`/`$$`/`` $` ``/`$'`/`$1`, a quadratic substitution trapping
  before allocation, `"İ"`, final sigma, and order across U+E000/U+10000.
- **Lean differential:** `semantics/corpus/text/{compare,slice,replace-all}`,
  the corpus run and a random sweep.
- **JS against the runner:** `host/web-js/conformance/text.contract` runs the
  three functions, their traps and the existing roster entries (`at`, `join`,
  `includes`, `formatDate`) in Chrome, Firefox and WebKit (async lane).
- **Hosts:** the D7.3 list. For `native-fixture`, the module's `inert`
  suppresses interaction on web and macOS.
- **Driven:** `native-fixture` (stage 1); `tabindex` focus (stage 2);
  calendar's end-after-start and calc's Backspace written in Contract
  (stage 3).
- The five checks after each stage.

## 5. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, on
`gaps/papercuts` or its successor on main. Each commit passes the five checks.

1. **Stage 1, 2026-10-04: the compiler (D4, D5, D6, D7.1, D7.2, D7.4, D8).**
   No runtime change. Exit: the diaries' and reviews' repros compile or are
   refused with the new text, and `native-fixture` is driven on web and macOS.
2. **Stage 2, 2026-10-05: `tabindex` (D7.3)** across the compiler, both web
   renderers, macOS, iOS and Linux. Exit: D7.3's tests are green on web and
   macOS through the driver.
3. **Stage 3, 2026-10-05: strings (D1, D2).** Exit: the runner tables, Lean,
   difftest and conformance are green. The `TextTransform` link is measured by
   `metrics.mjs` on an unused app, a formatting-only app and a case-using app.

## 6. Considered and not taken

- `compare(a, b)`: a second spelling of an operator JavaScript has.
- Overloading roster names by type, so string `replace` could sit beside the
  router's: a new resolution rule.
- Removing the SVG-only module-prop exception: it renames consumers for no
  gain.
- A warning instead of a refusal in D8: warnings go unread.
- Shipping list construction on the runner first and the JS target later: the
  two executors would refuse different programs (§9).

## 7. Questions decided (the orchestrator, for Charlie, 2026-10-04)

1. **Session-owned lists are admitted in principle.** The line runs between
   bounded, pure value operations and durable ownership, I/O and domain
   algorithms. Shipping waits for §9's precondition.
2. **U+FFFD normalization of the completed result** (D2).
3. **The case tables are linked by use** (`TextTransform`, D2).
4. **HTML `tabindex`, with no aliases.** A naming RFC for the remaining props
   does not block it (D7.3).

## 8. Revisions

- **r2** (Astra's r1 review): every finding was checked against the code and
  each held. Dispositions are in `llp/reviews/1085-r1.astra.md`.
- **r3** (Astra's r2 delta review, round 2 of 3; dispositions in
  `llp/reviews/1085-r2.astra.md`):
  - D3 moved out to §9. Both of its MATERIAL findings concern the JS target's
    budget, and that budget is a design of its own.
  - D2 is restated as string-bounded only.
  - D6's fixed point now covers handler parameters and every initializer,
    merging what it learns.
  - D7.3 covers each host's focus eligibility, explicit values that override
    the web's synthesis, and a test list. It is its own stage.
  - Shape names are binders. `inert` is allow-listed.
  - D8 has a path-sensitive walk and a rollout.

## 9. Deferred to a follow-up: list construction (r2's D3)

**Decided in principle (§7.1).** List literals and `concat`, `slice` and
`includes` on lists are admitted for session state. HN's collapsed ids and
kanban's per-column offsets are the consumers. Durable lists stay in the data
module.

**Precondition, established by the r2 review.** The two executors must refuse
the same programs at the same step. Today they cannot:

- **The JS target has no evaluation budget matching the runner's.** The
  runner starts fresh `steps` and `Extents` on every `vm::eval` invocation
  (`vm.rs:457`). It does so separately for:
  - each action body;
  - each derive evaluation;
  - **each** resource argument;
  - each root initializer;
  - **each** curried handler argument;
  - each binding and surface argument;
  - each region subject, row key and row initializer, virtualized ones
    included.

  The review maps every call site. The JS target emits all of a resource's
  arguments in one callback (`emit.rs:438`), and its lazy memo evaluation can
  nest (`rt.js:28`). A budget there needs contexts at compiled `Code`
  boundaries, saved and restored with `try/finally`.
- **Option and record erasure.** The JS target erases `Some` and emits
  records and lists as plain arrays (`code.rs:285`, `:325`), so `[some(0)]` and
  `[0]` look alike. The runner counts nodes, depth (`MAX_VALUE_DEPTH`) and
  aggregate string bytes (`MAX_VALUE_BYTES`) at every `Some`, `Record` and
  `List` construction, intermediate ones included (`vm.rs:552`, `:655`,
  `:667`). The JS target cannot reproduce these bounds without checked
  constructor emission or a representation change.
- **The existing gap.** The JS target's `map`, `filter` and `join` are
  unmetered today: `code.rs:433` emits native `.map`/`.filter`, and `x_join`
  counts nothing. A program the runner traps on can run to completion on the
  JS target. This is a pre-existing gap, recorded in `QUEUE.md`.

**The follow-up** is an LLP of its own, "The JS target's evaluation budget".
It covers per-invocation budget contexts at the runner's reset points, checked
constructor emission, and the bounds above, with conformance cases for
separate arguments, repeated row keys and nested lazy derives. List
construction (r2's D3: literals with kept trailing commas,
`concat`/`slice`/`includes`, `type-list-item`, spread refused, the Lean
`.list` with its proofs) lands after it, as an amendment to that LLP or to
this one.

## 10. As built

### Stage 1, 2026-10-04 (branch `impl/1085-s1`)

Astra's r3 review, the last round, found D6's convergence unsound: a join
grows `derive d = some(d)` from `?` to `option<?>` without bound, and
`press=a(a, a)` grows an action type exponentially. **D6's fixed point and the
structured `provisional` flag are deferred**; the message-text checks at
`uses.rs:138` and `lib.rs:236` remain. Its two repros are met by one ordered
pass that cannot loop instead.

- **D4** as specified (`type-initializer-scope`, `component.rs`
  `initializer_scope`), for root and row state. Each kind of name has its own
  tail: a resource names `each … in board.cols`, a derive says to make the
  state a derive, a later state says to declare it first.
- **D5** as specified, in `contract/syntax/src/parser/names.rs`. A name that
  refers to a binder (a type name, a source, a `then` action) follows the
  binder rule too, and `provide` names are binders (Grok's r4 review), so every contextual word is a name everywhere a name goes.
  `none(value=1)` is refused as "`none` is reserved in Contract (it is a
  literal), so it names no shape or function".
- **D6, descoped.** Row state is typed from its initializer once before the
  derive fixpoint (shop F3), and the row scopes skip an `each` whose list a
  later write types rather than refusing (the nested `items = []` and
  `replaceItems(sourceItems)` repros). `state other = items` after a typed
  write is still `type-cannot-infer`, without a `?`. `derive d = some(d)`
  and `press=a(a, a)` are refused as before (tests in `instance.rs`).
- **D7.1** as specified, `contract/analyze/src/payload.rs`, shared by
  analysis and lowering. Payloads without a DOM property are named as the
  in-repo apps name them (`scrollLeft, scrollTop`, `dx, dy`, `vx, vy`,
  `height, velocity`, `item, before`).
- **D7.2** as specified, plus `hook` on a module tag keeping its own
  refusal (`lower-hook-module`). No in-repo or x2apps module tag changed.
- **D7.4** as specified. Until stage 3, `replace(s, a, b)` points to
  `replaceAll`, which stage 3 adds.
- **D8** as specified, in `contract/analyze/src/sends.rs`, with one
  widening the rollout needed: sequential `if`s that test one name against
  different literals, with no assignment or send of that name between them,
  are exclusive (calendar's, minesweeper's and flashcards' key handlers
  send once per key). The refusal names the first send as related. The
  walk reads the expanded root, after tail calls are inlined (Grok's r4
  review); since only the root sends and a root action makes no tail call,
  a caller's send plus its callee's cannot arise today, and an inlined
  callee's own refusal is reported once.
  **Rollout:** `apps/messages-legacy` sent `change` twice on one path (a
  draft saved on the way to a new message); its draft send now has its own
  mutation, `draftSaved`. Outside the repo, flashcards' `commitEdit` and
  spreadsheet's `down` and `openSheet` are refused.
