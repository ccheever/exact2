# LLP 1088: What the app diaries ask of Contract

**Type:** RFC
**Status:** Accepted (stages 1–3 as descoped), r5, 2026-10-04. Accepted by the orchestrator for Charlie under the three-round rule (`rules/RULES.md`, "Fix loops get 3 rounds") after Astra's r3; D6 deferred. Stage 1 is built as of 2026-10-04, without D6's fixed point, and stages 2 and 3 as of 2026-10-04 (§10, "As built"). All three review rounds were one family (Astra, `gpt-6-astra`, max; Grok was unavailable): r1 NOT READY (7 MATERIAL, 6 MINOR); r2 NOT READY (4 new MATERIAL, 3 new MINOR); r3 NOT READY on one MATERIAL (D6's convergence) and three MINORs. The reviewer stated that only D6 blocked stage 1. r4 is a final edit with no further review: D6 moves to §9 with the requirement a follow-up must meet, and r3's MINORs are folded in. A second family, Grok 4.7 (xhigh), then reviewed r4 (truncated); r5 folds its findings into the decisions with no new round. Renumbered from 1085, then from 1086 (origin/main took 1085, then 1086 and 1087). §8 lists each revision.
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`, `contract/cli/src/lean.rs`), Plan (`plan/tables/format.json` `stdlib`), Runner (`vm.rs`, `stdlib.rs`, `uses.rs`), JS target (`host/web-js`), web host (`host/web`), Apple hosts (`host/apple`), Linux host, Lean semantics and difftest (`semantics/`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3, r4, r5)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-04, stages 2 and 3 on 2026-10-05 (§5)
**Base:** `gaps/papercuts`, which carries `host/web-js/format.js`, `x_at`/`x_formatDate`/`x_formatNumber` and the `x_` export test (`host/web-js/src/code.rs:595`)
**Amends:** LLP 1006 §2 (the language); LLP 1024 D1 (which known attributes bind to a module tag's box)
**Related:** LLP 1016 D5 (a send forgets the in-flight reply); LLP 1017 P5 and §8; LLP 1017.003; LLP 1035.005.000 D7b; LLP 1047 D2/D6 (linked by use); LLP 1054.000.005 (`trim`); LLP 1071 (the JS target); diaries `~/projects/x2apps/<app>/DIARY.md`; reviews `llp/reviews/1088-r1.astra.md`, `llp/reviews/1088-r2.astra.md`, `llp/reviews/1088-r3.astra.md`, `llp/reviews/1088-r4.grok.md`. Web: ECMA-262 `IsLessThan`, `String.prototype.{slice,replaceAll,toLowerCase,toWellFormed}`, `GetSubstitution`, `StringPad`; HTML `tabindex` (focusable areas, sequential focus navigation).

## Summary

On 2026-10-04 several agents each built an app outside the repo with
`exact new` and kept a diary. Calendar moved its validation into the data
module because `"10:00" > "09:30"` does not type, and calc sent Backspace
through a mutation because there is no `slice`. The other agents lost time
to smaller problems: reserved words where a parameter goes, a type error
that printed `?`, a send whose reply vanished, a focus attribute that binds
nothing, and diagnostics that named the fix obliquely. Four apps also
redesigned around the lack of list construction. Admitting that is decided
in principle but deferred to a follow-up (§9.1), because the JS target cannot
yet bound it the way the runner does. The type-inference repair for shop F3
(D6) is also deferred (§9.2), because the repair proposed here cannot be
shown to terminate.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `<` `<=` `>` `>=` on two strings, in UTF-16 code-unit order | calendar F1 | 3 |
| D2 | `slice`, `replaceAll`, `toLowerCase` on strings, bounded by `MAX_STRING`; numeric parsing deferred | calc, hn-reader F2, calendar F1, flashcards/chat search | 3 |
| D4 | A state initializer's scope, stated by its refusal | kanban F5 | 1 |
| D5 | 16 words reserved at binder positions only; the rest contextual | kanban F13, shop F6, hn-reader F1, ledger F3, calendar F16 | 1 |
| (D6) | Inference order for state, parameter and derive types; no `?` in a final message: deferred, with the requirement a follow-up must meet | shop F3, reviews | §9.2 |
| D7 | Diagnostics that name the fix; module-tag allow-list | pomodoro F1, paint F7, ledger F3, hn-reader F2 | 1 |
| D7.3 | HTML `tabindex`, focusable on every host | calendar F2, paint F7 | 2 |
| D8 | Two sends to one mutation on one path are refused | flashcards F4 | 1 |
| (D3) | List construction: deferred to "The JS target's evaluation budget" | pomodoro F3, kanban, hn-reader F2 | §9.1 |

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
  - Each function is one call whose output is built **segment by segment and
    counted as the concatenated output**, not segment by segment. The counter
    is a small UTF-16 → UTF-8 encoder that carries a pending high surrogate
    across segment boundaries:
    - a low surrogate that completes it adds the pair's 4 bytes;
    - anything else first settles the pending half as U+FFFD's 3 bytes;
    - the end of the output settles a pending half the same way.

    Every split pair is reconciled, however many there are, so the count is
    exactly the UTF-8 length of the normalized result. The call traps
    `StringTooLong` the moment that count exceeds `MAX_STRING`, with no slack.
    (Bun: `"😀😀".replaceAll("", "")` is 8 bytes; its four units encoded one
    by one would count 12.)
  - No result over the bound is allocated (the one exception is the JS
    target's `toLowerCase`, below), so a `replaceAll` whose
    `` $` ``/`$'` substitutions grow quadratically traps early.
  - `slice` never grows its input. `toLowerCase` can grow it (`"İ"` → two
    code units).
  - On the JS target, `x_replaceAll` builds with the same carried-surrogate
    counter. `x_toLowerCase` lowercases the whole string first, keeping the
    browser's context rules, and checks the result. That costs one
    intermediate of at most three times the input, which the browser's own
    string limit bounds, and nothing past that is kept. A JS trap throws the
    runner's trap as a `Refusal`.
  - The JS target's other string producers (`+`, templates, `join`) stay
    unbounded today, the same gap as §9.1's.
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
  - **The link gains a bounded entry.** The existing callback returns a
    finished `Cow<str>` from a whole-string `to_lowercase()` (`case.rs:29`,
    `:70`). Calling it and checking afterwards allocates past the bound, and
    calling it on fragments loses context: `"ΟΣ"` must give `"ος"` (final
    sigma), not `"οσ"`.
  - The kernel adds a bounded lowercase beside `apply_after`:
    `lowercase_bounded(text, max_bytes) -> Option<String>`. It walks the
    whole string once, deciding final sigma from the characters on both sides
    as `to_lowercase` does. It appends each mapped character only while the
    output's UTF-8 length stays within `max_bytes`, and returns `None` (the
    trap) as soon as it would not.
  - Expansion is counted as it happens: `"İİ"` grows from 4 bytes to 6.
  - **It is reached only through the link.** On wasm, the case tables stay
    out of a core because `apply_after` is reached only through the function
    pointer that `link()` installs (`case.rs:24–54`; `host/web/src/link.rs:239`).
    If the runner called `lowercase_bounded` directly, every web artifact would
    pay the 3–6 KiB, including apps that use neither `text-transform` nor
    `toLowerCase`. The capability bit refuses an unlinked plan at boot; it
    does not strip a direct call.
  - So `link()` installs `lowercase_bounded` beside `apply_after` through the
    same pointer table. The runner calls the pointer and traps when it is
    unset. Native artifacts and the compiler call the function directly, as
    they map today.
  - The `text-transform` path keeps the unbounded `apply_after`. The JS
    target uses the browser and never reaches the tables.
  - Stage 3's `metrics.mjs` comparison must show no growth for an app that
    uses neither.
  - **Conformance fixtures use stable mappings only**: `"İ"`, `"ΟΣ"` → `"ος"`,
    ASCII. Rust's and the browser's Unicode versions can differ elsewhere, and
    a fixture must not depend on which is newer.
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
- a component's props, injects and **`provide` names** (a provided name is
  read later);
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

**Where the check runs** (Grok, r4). Props, injects and `provide` names are
parsed with the same `field_name` as shape fields and named arguments
(`parser.rs:557`, `:644`, `:667`, `:1346`). Making `field_name` stricter
would refuse `shape` fields and `Flags(in=1)`; widening it would admit a prop
named `in`. So:
- `field_name` accepts all 16 reserved words;
- the reserved-binder predicate is applied **at each binder site**,
  including the prop, inject and `provide` calls of `field_name`;
- `ident()`/`named_ident` (states, resources, actions, `let`) stop refusing
  the 23 contextual words and keep refusing the 16.

The stage-1 lane is implementing it this way.

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

### D6 — Deferred (§9.2)

Shop F3's inference-order bug and the rule against `?` in a final message are
not in stages 1–3. §9.2 keeps the diagnosis, the repros and the requirement a
follow-up must meet.

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
   - an explicit value makes a box a focusable area, subject to the filters
     HTML also applies. A disabled, inert, hidden or `display: none` box is
     never focusable, and the existing disabled/inert/hidden checks on every
     host stay in force;
   - `≥ 0` puts it in sequential navigation, positive values first in
     ascending order, then `0` in tree order;
   - a negative value is focusable by pointer and script but skipped by Tab.

   **"Explicit" means the prop is present.** Today every host reads a
   missing `tabIndex` as `0` (`?? "0"`), so "admit `≥ 0`" on that integer
   would make every plain box a Tab stop. Each host instead distinguishes
   *present* from *absent*. **Focusable** means an explicit value of any
   sign, or what is focusable today. **Tabbable** means an explicit value
   `≥ 0`, or what is tabbable today, and never an explicit negative.

   Each host:
   - **Web.**
     - `props_of` (`host/web/src/document.rs:481`) maps `PropId::TabIndex`
       to the attribute `tabindex`. Today it falls through to `data-tabindex`
       (`element.rs:851`), which the browser ignores. The JS target's
       prop-name tables (`host/web-js/src/rows.rs:470`, `style.rs:422`) do the
       same, so dynamic updates through `P()`/`applyProps` set the real
       attribute.
     - Synthesis of `tabindex=0` for focus, blur and key handlers is skipped
       when the attribute is present, a negative value and a later state
       update included. This applies at `document.rs:618` and `emit.rs:971`.
       The wasm path's `glue.js:534` already checks `hasAttribute("tabindex")`
       and needs only the mapping.
   - **macOS.**
     - AppKit's Tab follows `canBecomeKeyView`, which is
       `acceptsFirstResponder && tabbable` (`NodeViewMac.swift:189`).
     - `acceptsFirstResponder` (`:176`) is true for any explicit value, so a
       click focuses a `tabindex=-1` box.
     - `Presenter.tabbable` (`PresenterMac.swift:1151–1161`) is true for an
       explicit value `≥ 0`, keeps `index < 0` out first, and reads absence as
       absence.
     - The key-view order is HTML's.
     - These stay as they are: dialog scope (`DialogsMac.swift:185` reuses
       `Presenter.tabbable`), the paragraph Tab-start
       (`PresenterMac.swift:1120–1125`), the button key-view override, and
       `drawFocusRingMask`, which draws only for a pressable. The test
       asserts the first responder, not a ring.
   - **iOS.**
     - `canBecomeFirstResponder` (`NodeViewIOS.swift:256`) is true for any
       explicit value, so `becomeFirstResponder` succeeds for a listed node
       and for a tap.
     - The hardware-keyboard Tab walk (`moveFocus`, `PresenterIOS.swift:414`;
       `tabbable`, `:425–429`) uses the same present/absent split, keeps
       `index < 0` out, keeps its zero-size skip, and keeps HTML's order.
   - **tvOS.**
     - `canBecomeFocused` (`RemoteTVOS.swift:13`) is `canBecomeFirstResponder`
       or a press handler. It is **false for an explicit value `< 0`**, so a
       `tabindex=-1` box is not a Siri Remote stop. An explicit `≥ 0` may be
       focused.
     - The remote's order stays UIKit's geometry, not HTML's (tvOS is
       in-tree; `rules/DEFERRED.md:265`).
     - `FocusSearch` keeps leaving `NodeView`s out of UIKit's focus search
       (`FocusSearchIOS.swift:35`).
   - **Linux.** There is no focus walk today. `hardware_key` hands Tab to the
     focused view, and `type_key` neither forwards Tab into a canvas nor moves
     focus (`presenter/typing.rs:146`). Stage 2 adds the walk:
     - **Split** today's single `focusable` predicate
       (`surface_controls.rs:454`, used by tap, `autofocus` and `set_focus`)
       into *focusable* (any explicit value, plus today's controls and
       handler nodes) and *tabbable* (explicit `≥ 0`, plus those controls).
       Hidden, inert and disabled stay out (`route_visibility`).
     - **The walk.** Tab and Shift-Tab move through the tabbable nodes in
       HTML's order and wrap. From no focus, Tab goes to the first and
       Shift-Tab to the last. A canvas or a textarea that consumes Tab keeps
       it.
     - The driver tree's `focused` flag is `p.focus()`.

   Tests, on web and macOS through the driver, Linux headless and iOS in the
   async lane:
   - a `box tabindex=0` with **no** `focus`, `blur` or `key` handler (those
     already grant focusability: `NodeViewMac.swift:183`, `emit.rs:971`)
     takes focus by Tab. The test asserts the focused element itself:
     `document.activeElement` on the web, the window's first responder on
     macOS, as the driver's `tree` reports focus. Key delivery is checked
     separately: a `key` handler on an **ancestor** column observes the
     bubbling key while the box holds focus;
   - positive values order before zero;
   - a `tabindex=-1` box is skipped by Tab but focused by `tap` and
     `autofocus`;
   - `tabindex` bound to state changes eligibility while mounted;
   - `tabindex=-1` on a node with a `key` handler leaves the web's Tab order;
   - a plain box with no `tabindex` is not a Tab stop on any host (the
     absent-is-not-zero rule);
   - a disabled, inert or hidden box with `tabindex=0` is skipped;
   - Linux: Tab, Shift-Tab, wrap, and the start from no focus;
   - tvOS: `canBecomeFocused` is false for `tabindex=-1` (async lane).

   The wider spelling question is a naming RFC of its own and does not block
   this. `scrollLeft` stays the DOM property; `emojiPicker` and
   `navigationKey` are Exact's.
4. **The web's methods name their Contract form.**
   - CSS's `background-color: …` in a style gets the rewritten row
     (ledger F3).
   - `idioms.rs` gains `split`, `indexOf`, `substring`, `padStart`,
     `padEnd`, `toUpperCase`, `Number`, `parseInt` and a three-argument
     `replace`, which points to `replaceAll`.
   - `concat`, list `push`, `slice` on a list and list literals name §9.1's
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

**It runs on the post-inline body** (Grok, r4). Tail inlining
(`inline/tail.rs`, run by `inline.rs:223` during expand) splices a callee's
statements into the caller before analysis. A send in the caller and a send in
the inlined callee are one commit at runtime, and they are invisible on the
authored body that `check_all` walks (`analyze/src/lib.rs:198`). The walk
therefore runs on the statements after `tail::resolve`: `expanded.root` for
the root, and each component's resolved actions. `@check:` leftovers are
`Stmt::Command` and are ignored. The stage-1 lane is implementing it this
way.

**Rollout.** Flashcards (`~/projects/x2apps/flashcards/app.contract:192`) sends
`saveCard` and then `setImage`, and stage 1 will refuse it. The app's owner
changes it to one combined request (`edit("saveCardWithImage", …)`) or to two
mutations, and drives save-with-image. It is outside this repo. The in-repo
apps all compile in stage 1.

## 3. Effect on each implementation

| | Stage 1 (D4, D5, D7, D8; not D7.3) | Stage 2 (D7.3) | Stage 3 (D1, D2) |
|---|---|---|---|
| syntax | D5 in a new `names.rs` (`parser.rs` is at 1,469 lines); `refresh` contextual; colon recovery; idioms | — | — |
| types | D4 | — | string order; `optional` arity; `roster_calls` |
| kernel | — | — | `kernel/src/text/case.rs`: `lowercase_bounded(text, max_bytes)` beside `apply_after`, whole-string context (final sigma), counted expansion, installed by `link()` through the same pointer |
| analyze, lower | D7.1; D7.2 allow-list; D8 walk | `tabindex` binding, case-variant refusal | defaults for recorded calls |
| `format.json` | — | — | `slice`, `replaceAll`, `toLowerCase`; `optional`; min arity |
| runner | — | — | string compare; bounded construction with the carried-surrogate counter; `toLowerCase` through the linked `lowercase_bounded` pointer, trapping when unset; `TextTransform` use |
| JS target | — | `tabindex` in the prop-name tables (`rows.rs:470`, `style.rs:422`); synthesis skipped when present (`emit.rs:971`) | `x_slice`, `x_replaceAll`, `x_toLowerCase` in `format.js`, re-exported on `rt.js`'s existing `format.js` line (`rt.js` is at 1,498 lines) |
| web host (wasm) | — | `props_of` maps `TabIndex` → `tabindex`; synthesis skipped when present (`document.rs:618`; `glue.js:534` already checks) | — |
| Apple | — | present/absent split; macOS `acceptsFirstResponder` and `tabbable`/`canBecomeKeyView`; iOS `canBecomeFirstResponder` and `moveFocus`; tvOS `canBecomeFocused` false for `< 0` | — |
| Linux | — | focusable/tabbable split; a Tab/Shift-Tab walk | — |
| Lean, difftest | — | — | `utf16Units`, string order, `slice`, `GetSubstitution`, `lean.rs` defaults; generator (not `toLowerCase`), astral strings |
| docs | keyword sets; initializers; D8 | `tabindex` | roster; comparison; the deviation in LLP 1006 §2 |

The plan's digest changes and `formatVersion` does not.

## 4. Tests

- **Compiler** (`contract/cli/tests/it/`):
  - `names.rs`: every contextual word at every binder. Reserved words are
    refused at binders, shape names included (both the declaration and the
    construction), and admitted as fields, named arguments, members and SVG
    `in`.
  - `search.rs`: string order; the three functions; a user action named
    `slice` keeps its arity.
  - `diagnostics.rs`: D4, D7 and D8, each message asserted whole. For D8,
    exclusive arms are accepted, and a send in an arm plus a send in the
    common suffix is refused.
  - `corpus.rs`: `ghostty-terminal`; module-tag `inert` and `disabled` stay on the box; a
    `tabindex` lowering case.
- **Runner** (`stdlib.rs` tests, expected values from `bun -e`): negative and
  fractional indices, `NaN`, `±Infinity`, surrogate cuts, `"😀"` with an empty
  pattern, `$&`/`$$`/`` $` ``/`$'`/`$1`, a quadratic substitution trapping
  before allocation, and order across U+E000/U+10000.
  - **Split pairs at the byte boundary:** with the limit lowered to a test
    value, `"😀😀".replaceAll("", "")` (8 bytes) succeeds at a limit of 8 and
    traps at 7. A string of many astral characters, each split by an
    empty-pattern insertion, lands exactly on the limit. A lone half settled
    at the end counts 3 bytes.
  - **Kernel (`case.rs` tests):** `lowercase_bounded("ΟΣ")` is `"ος"`;
    `"İİ"` is 6 bytes, fits a limit of 6 and returns `None` at 5; the result
    always equals `to_lowercase` when it fits.
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

1. **Stage 1, 2026-10-04: the compiler (D4, D5, D7 except D7.3, D8).** No
   runtime change. Exit: the diaries' repros for these decisions compile or
   are refused with the new text, and `native-fixture` is driven on web and
   macOS.
2. **Stage 2, 2026-10-05: `tabindex` (D7.3)** across the compiler, both web
   renderers, macOS, iOS, tvOS and Linux. Exit: D7.3's tests are green on web
   and macOS through the driver and on **Linux headless** (the new Tab walk is
   not shipped untested). The iOS and tvOS cases run in the async lane.
3. **Stage 3, 2026-10-05: strings (D1, D2)**, including the kernel's
   `lowercase_bounded`. Exit: the runner and kernel tables, Lean,
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
  two executors would refuse different programs (§9.1).

## 7. Questions decided (the orchestrator, for Charlie, 2026-10-04)

1. **Session-owned lists are admitted in principle.** The line runs between
   bounded, pure value operations and durable ownership, I/O and domain
   algorithms. Shipping waits for §9.1's precondition.
2. **U+FFFD normalization of the completed result** (D2).
3. **The case tables are linked by use** (`TextTransform`, D2).
4. **HTML `tabindex`, with no aliases.** A naming RFC for the remaining props
   does not block it (D7.3).

## 8. Revisions

- **r2** (Astra's r1 review): every finding was checked against the code and
  each held. Dispositions are in `llp/reviews/1088-r1.astra.md`.
- **r3** (Astra's r2 delta review, round 2 of 3; dispositions in
  `llp/reviews/1088-r2.astra.md`):
  - D3 moved out to §9. Both of its MATERIAL findings concern the JS target's
    budget, and that budget is a design of its own.
  - D2 is restated as string-bounded only.
  - D6's fixed point now covers handler parameters and every initializer,
    merging what it learns.
  - D7.3 covers each host's focus eligibility, explicit values that override
    the web's synthesis, and a test list. It is its own stage.
  - Shape names are binders. `inert` is allow-listed.
  - D8 has a path-sensitive walk and a rollout.
- **r4** (Astra's r3 review, the third and last round; dispositions in
  `llp/reviews/1088-r3.astra.md`). This is a final edit with no new review
  round, and the RFC is accepted as descoped.
  - D6 moves to §9.2, with the requirement a follow-up must meet.
  - D2's counter carries a pending high surrogate across segments and counts
    the concatenated output.
  - The kernel's case link gains `lowercase_bounded`.
  - D7.3's first test drops the handler that granted focus and asserts the
    focused element.
  - Stage 1 is D4, D5, D7 (except D7.3) and D8.
- **r5** (Grok 4.7, xhigh, the second family, on r4; its run was cut off
  mid-way by a process kill. Dispositions in `llp/reviews/1088-r4.grok.md`.)
  This is an edit to the decisions with no new review round. The document is
  renumbered from 1085 to 1086, because origin/main took 1085.
  - D7.3 now specifies the real web binding: `props_of` maps the prop to
    `tabindex`, and synthesis is skipped when the attribute is present.
  - D7.3 adds the present/absent split, so focusable and tabbable are
    separate predicates on every host. On macOS that means
    `canBecomeKeyView`; on iOS, `moveFocus`.
  - D7.3 adds tvOS (`canBecomeFocused` is false for `< 0`, and the order stays
    geometric) and a real Linux Tab walk. Linux headless joins the stage 2
    exit.
  - D7.3 keeps HTML's disabled, inert and hidden filters.
  - D2's `lowercase_bounded` is reached only through `link()`'s pointer, and
    conformance fixtures use stable mappings only.
  - Two items are recorded for the stage-1 lane, which is already building
    them: D5's binder check runs at each binder site, `provide` included; and
    D8 walks the post-`tail::resolve` body.

- **Renumbered 1088** (2026-10-04, merging origin/main into batch 2): origin/main
  took 1086 (`llp/1086-building-with-exact2.rfc.md`) and 1087
  (`llp/1087-authoring-bench.plan.md`), so this document, its reviews
  (`llp/reviews/1088-*.md`) and every reference stage 1 added say 1088. No
  decision changed.

## 9. Deferred to follow-ups

### 9.1 List construction (r2's D3)

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

### 9.2 Inference order and `?` in messages (r3's D6)

**The bug, kept with its repros.** `check_component` runs its inference
phases once each, in this order:

1. derives;
2. owned (row) initializers (`checks.rs:409`), whose owner scopes strictly
   infer every `each` list;
3. handler-parameter refinement (`component.rs:207`);
4. action-write refinement (`:213`).

A type learned in a later phase never reaches an earlier one. Each of these
fails today with `target/debug/contract build --json`, and each is a test the
follow-up must make pass:

- **Shop F3.** A child used inside an `each`, with
  `state pick = 0`, `derive chosen = at(b.items, pick)`, and its own view
  `each img in images` over a derive reading `chosen`, gives "argument 2 of
  `at` expects `number`, given `?`". This is the diary's
  `shop/repros/each-derive-state.contract` with the use wrapped in an `each`.
- A root `state items = []`, written by a typed action and iterated two deep,
  gives "`?` has no fields".
- `items = v` in `action replaceItems(v)`, typed only through
  `press=replaceItems(sourceItems)`.
- `state items = []; state other = items` gives "nothing writes a value into
  `other`".

**Why r3's repair was not taken.** r3 iterated all four phases, merging by
unification, until nothing changed, within `slots + derives + parameters + 2`
rounds. Unification accepts `? → option<?>` (`Ty::unify`,
`types/src/lib.rs:102`), so two programs grow under repeated joins:

- `derive d = some(d)` grows without end;
- `action a(p, q)` used as `press=a(a, a)` grows `Action` types
  exponentially through parameter refinement (`component.rs:460`).

A round cutoff alone bounds neither the time nor the memory of a round. The
current single-pass phases refuse both programs.

**What a follow-up must meet.**

1. **Recursive-type detection** (an occurs check: a type may not contain the
   variable being solved for) **or an explicit type-growth guard** (a bound
   on a type's size or depth, checked at every join).
2. **Deterministic refusal on nonconvergence.** One diagnostic, naming the
   binding whose type would not settle. It is the same on every run and
   independent of declaration order.
3. **Both growth regressions** (`derive d = some(d)`; `press=a(a, a)`) are
   refused promptly, beside the four repros above, which must compile.
4. **No `?` in a final message, without changing inference.** `uses.rs:138`
   and `lib.rs:236` read message text to decide what is provisional:
   - a structured `provisional` flag on `TypeError` replaces that;
   - a surviving provisional error is reported as `type-cannot-infer`,
     naming the binding;
   - a test asserts that no final message contains `?` in any form
     (`` `?` ``, `option<?>`, `list<?>`).

Until then, the diary's workaround stands: move the `at()` into a file-level
`fn` taking the state as an argument. `docs/agent-pitfalls.md` gains that
pitfall in stage 1.

## 10. As built

### Stage 1, 2026-10-04 (branch `impl/1085-s1`, named for this LLP's first number)

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

### Stage 2, 2026-10-04 (branch `fix/impl1088`)

D7.3 as specified, on every host:

- **Compiler.** `tabindex` binds `PropId::TabIndex` on every element and a
  module tag's box (`BOX_PROPS`); `tabIndex` is a renamed spelling, refused
  as "`tabIndex` is spelled `tabindex` here", on a module tag too. `tabindex`
  leaves the host's `data-` words, since no host writes `data-tabindex` now.
- **Web.** `props_of` names the prop `tabindex`, which the JS target's
  prop-name discovery inherits, so `P()` sets the real attribute. Both
  renderers skip the synthesized `tabindex="0"` when the attribute is
  present (`document.rs`, `emit.rs`); `glue.js` already checked.
- **macOS.** `explicitTabIndex` (absent is nil) is shared by the Apple hosts.
  `acceptsFirstResponder` admits any explicit value; `tabbable`, and so
  `canBecomeKeyView` and `Presenter.tabbable`, an explicit value ≥ 0 or
  what is a stop by kind, never an explicit negative. Driving it found that
  a click never kept the focus on a non-pressable focusable node (a `key`
  handler's included): NSView forwarded the `mouseDown` up the chain, so an
  ancestor took the focus and `PageScrollView` then blurred it. The
  innermost node the web would focus now keeps it, and the page blurs only
  when what was clicked is not inside the focus — HTML's "nearest focusable
  ancestor".
- **iOS.** `canBecomeFirstResponder` admits any explicit value; `moveFocus`
  skips an explicit negative and orders by HTML's rule. **tvOS:**
  `canBecomeFocused` is false for an explicit negative; there is no tvOS
  test target in the tree, so that line is compiled (`build.mjs --tvos`), not tested.
- **Linux.** `focusable` (tap, `autofocus`, `focus()`) admits any explicit
  value; the new `tabbable` (in `presenter/typing.rs`) is an explicit value
  ≥ 0 or what is focusable by kind, shown, not inert, with a box. Tab is the
  key's default action after the `key` handlers (a `preventDefault()` keeps
  it): HTML's order, wrapping, Shift-Tab back, from no focus the first or
  the last. Nothing consumes Tab, as on the web, where neither a canvas nor
  a textarea does. Tab's release does not refocus its target.
- **Tests.** `aria.rs` (binding, state, the case refusal, a module tag);
  `host/web/tests/it/document.rs` (the attribute wins, a negative one
  included; absent writes none); `AccessibilityTests` and
  `KeyboardFocusIOSTests` (order, `-1` by click or tap, absent, disabled,
  inert, hidden, a change while mounted); `events_tests.rs` on Linux (the
  whole list, the ancestor's `key` included). Driven on the web and macOS
  with a scratch app: Tab from a `tabindex=0` box with no handler, `-1`
  skipped and focused by tap, the bound value joining the order, Shift-Tab.
  `host/web-js/conformance/tabindex.{contract,steps}` holds the two web
  renderers to the same focus (10 steps equal in Chrome).

### Stage 3, 2026-10-04 (branch `fix/impl1088`)

D1 and D2 as specified, with these differences:

- **The omitted `end` is `Number.MAX_VALUE`, not `Infinity`.** A plan's
  number constants are finite (the code validator's `NonFinite`), and an
  index past any end clamps exactly as `undefined` and `Infinity` do. The
  roster row says `"optional": ["Number.MAX_VALUE"]`; `plan/build.rs`
  generates `Stdlib::min_arity()` and `Stdlib::defaults()`.
- **No `Types.roster_calls`.** `Stdlib::omitted(given)` names the defaults
  a call leaves out. The checker admits `min_arity..=arity` only in
  its roster arm, after scoped actions and props (so `action slice(by)`
  keeps its arity), and lowering's roster arm, which every expression call
  that is not a `fn` or a record reaches, fills the defaults. `lean.rs`
  fills them for a roster-named call short of its arity: no `fn` takes a
  roster name, and the semantics evaluates no action in an expression, so
  the name decides. The arity refusal reads "`slice` takes 2 to 3
  argument(s), given 1; expected `slice(string, number, number?)`", and
  `slice` over a list is `type-refused-idiom` naming §9.
- **`lowercase_bounded` is a preflight.** It counts each character's mapped
  UTF-8 length first, allocating nothing and returning `None` the moment the
  count passes the bound, and only then maps the whole string with
  `to_lowercase`. Final sigma never changes a length (`ς` and `σ` are both 2
  bytes), so the count is exact, no allocation passes the bound, and the
  result is `to_lowercase`'s, context included, without reimplementing
  `Final_Sigma`'s case-ignorable scan. `link()` installs it beside
  `apply_after`; `linked_lowercase()` is that pointer on wasm (`None`
  unlinked: the runner traps, after boot already refused the plan) and the
  function natively.
- **Runner.** `runner/src/strings.rs`: `order`, `slice`, and `replace_all`
  through one counter that carries a pending high surrogate across pieces,
  trapping `StringTooLong` before a byte past `MAX_STRING` is kept. A
  non-empty pattern is matched in UTF-8 (a well-formed pattern's matches
  begin and end on character boundaries); only an empty one walks code
  units. `uses` adds `text_transform` for a `toLowerCase` call.
- **JS target.** `x_slice`, `x_replaceAll` and `x_toLowerCase` live in
  `roster.js`, which imports `Refusal` from `rt.js` as `router.js` does.
- **Lean.** `Str.utf16Units`, `wellFormed`, `lt`, `slice`, `substitute`
  (`GetSubstitution`) and `replaceAll` in `Value.lean`; the string arms of
  `binop` (`Eval.lean`) and the VM model's `binary` (`Vm.lean`); `rosterTy`
  and `binTy` (`Types.lean`), with `toLowerCase` typed and left out as the
  formats are; the soundness proofs extended (`lake build` green). The
  `MAX_STRING` bound is not modelled, as no string bound is.
- **Difftest.** `semantics/corpus/text/{compare,slice,replace-all}`; the
  generator writes string comparisons, `slice` (two and three arguments)
  and `replaceAll`, and U+E000 joins its strings. The text corpus agrees on
  the runner, Lean and (`--js`) the JS target, and the lowering corpus
  matches `Contract.Lower` byte for byte. `--js` had failed to bundle on
  origin/main (its runtime copy lacked `media-glue.js`); that list is fixed.
  A random sweep (seed 1088, 300) agrees; its 20 compiler refusals are all
  stage 1's `analyze-send-twice` on generated double sends.
- **Conformance.** `host/web-js/conformance/text.{contract,steps}`: the three
  functions, both comparisons, the trap (a refused action keeps its state),
  `at`, `join`, `includes` and `formatDate`, 13 steps equal in Chrome. The
  Firefox and WebKit runs are the async lane's.
- **Metrics** (`host/web/build.mjs --wasm`, brotli-11 `app.wasm`, against
  origin/main 423e4c4bc). Caltrain, which uses neither: 335.59 → 336.92 KiB,
  the always-linked roster code (`exact_runner::strings`' `order`, `slice`,
  `replace_all`, as `trim` is core) and stage 2's and the diary's rows; the
  case tables are not newly linked (a names build of each shows the same
  case-mapping symbols, and neither `lowercase_bounded` nor
  `linked_lowercase`). A scratch app calling `toLowerCase`: 308.48 KiB,
  against 302.15 KiB for the same app without the call (+6.33 KiB, the
  tables `text_transform` links), and its wasm build lowercases. The JS
  target never reaches the tables.
- **Tests.** `strings.rs` (runner: indices, NaN, ±∞, cuts, `$` forms, the
  split-pair byte boundary at 8/7, 400/399, 11/10, the quadratic trap,
  order across U+E000); `case.rs` (kernel); `search.rs` and
  `corpus/strings.contract` (calendar's end-after-start and calc's
  Backspace in Contract); `uses.rs`; `js-runtime.test.mjs` (the JS entries
  against the browser's methods and the trap). Driven with a scratch app on
  the web (JS and wasm targets) and macOS: an end before the start refused,
  Backspace, a sign flip, a search lowercased on both sides.

