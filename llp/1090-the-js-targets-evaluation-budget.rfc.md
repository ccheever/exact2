# LLP 1090: The JS target's evaluation budget

**Type:** RFC
**Status:** Draft r1, 2026-10-04
**Systems:** Runner (`runner/src/vm.rs`, `stdlib.rs`), Contract checker (`contract/types`), JS target (`host/web-js`: `src/code.rs`, `src/emit.rs`, `src/regions.rs`, `rt.js`, a new `budget.js`), conformance (`host/web-js/conform.mjs`, `host/web-js/conformance/`), Lean semantics and difftest (`semantics/`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stages 1 and 2 on 2026-10-05, stage 3 on 2026-10-06 (§5)
**Amends:** LLP 1017.003 D3 (what a value's extent counts) and D7 ("no second evaluator": since LLP 1071 there is one); LLP 1088 §9.1 (this LLP meets its precondition)
**Related:** LLP 1005 §6 (a refused commit changes nothing); LLP 1017.003 (map, filter, join and `MAX_LIST_STEPS`); LLP 1071 (the JS target); LLP 1088 D2 (`MAX_STRING` on the new string functions) and §9.1 (list construction); `llp/reviews/1088-r2.astra.md` (the reset points); `QUEUE.md`, "The JS target's evaluation budget"

## Summary

The runner bounds every evaluation (65,536 list steps; 64 MiB strings;
values of 2²⁴ nodes, 64 MiB of string bytes, 64 levels). The JS target, the
web's default build, bounds none of it, so a program the runner refuses runs
in the browser. LLP 1088 deferred list construction until the two executors
refuse the same programs at the same step; this LLP makes that true.

Two choices carry the rest. **The measure changes, not the
representation:** a `Some` stops counting as a node in the runner and depth
becomes a checker rule, so the JS target counts exactly what the runner
counts from the erased values it already has. **Metering is emitted only
where a bound can be reached:** steps and string lengths always, composite
checks where types cannot rule them out, and no budget context in a Code
without a list step.

| | Decision | Stage |
|---|---|---|
| D1 | A budget context per evaluation of a Code that takes list steps; saved and restored, so nested lazy derives start fresh | 2 |
| D2 | A `Some` adds no node and no bytes; a type nests at most 64 deep (`type-too-deep`) | 1 |
| D3 | Checked emission: metered `map`/`filter`/`join`, checked constructors and concatenation, exact UTF-8 bytes, the runner's order and pcs | 2 |
| D4 | Static elision of composite checks, sound by type, tested against the unelided build | 3 |
| D5 | A performance ceiling: no cost outside list steps, at most 10% on a metered `map` | 3 |
| D6 | A trap on the web is the runner's trap: same text, same outcome; two runner gaps closed | 1–2 |
| D7 | Conformance: adversarial plans on wasm, JS and Linux, refusal and trap text compared | 2 |
| D8 | Lean: the semantics stays unbounded; difftest calls a bound trap `OUTSIDE` | 1 |

## 1. What the runner bounds, and where it resets

`vm::eval` (`runner/src/vm.rs:459`) starts a fresh step count and extent cache
on every invocation. Within it:

- **`MAX_LIST_STEPS` = 65,536** (`vm.rs:43`). A step is each run of a
  `Map`/`Filter` body, taken before the body runs (`Callback::begin`,
  `vm.rs:224`), plus one per item `join` prints, taken before it joins
  (`vm.rs:768`). The trap is `IterationLimit` at the `Map`, `Filter` or
  `Call`'s pc.
- **`MAX_STRING` = 2²⁶ UTF-8 bytes** on `Concat` (`vm.rs:717`), `join`, `t`
  and `NativeProps`. The trap is `StringTooLong`.
- **Extents.** These are checked at every `Some`, `Record` and `List` built
  (`vm.rs:554`, `:658`, `:670`) and at each item a `Map`/`Filter` keeps
  (`vm.rs:512`).
  - `MAX_VALUE_NODES` = 2²⁴ and `MAX_VALUE_BYTES` = 2²⁶ trap
    `ValueTooLarge`. A shared part counts once per place it appears.
  - `MAX_VALUE_DEPTH` = 64 traps `ValueTooDeep`.
  - Extents of 64 nodes or more are remembered per evaluation (`vm.rs:320`).

The boundaries are `vm::eval`'s production callers (Astra's r2 table, at
today's lines):

| Evaluation | Runner | JS target today |
|---|---|---|
| Each action body (timers and `then` continuations too) | `runner/commit.rs:469` | `emit.rs:466` (`act`) |
| Each derive evaluation or retry | `runner/settlement.rs:295` | `emit.rs:370` (`memo`) |
| **Each** resource argument | `runner/settlement.rs:367` | `emit.rs:384`, all joined into one callback at `:429` |
| Each root-state initializer | `runner/router.rs:605` → `runner.rs:1352` | `emit.rs:339`, `regions.rs:159` |
| **Each** handler argument, collection edges included | `runner/event.rs:814`, `:971` | `emit.rs:1171` |
| Each binding and surface argument | `instance.rs:810`, `:817`, `:926` | `emit.rs:876`, `:1043`, `:1080`, `:1315` |
| Each region subject, row key, row-state initializer | `instance/region.rs:43`, `:69`, `:95`, `:131`, `:324` | `regions.rs:14`, `:116`, `:77` |
| Virtualized subjects, bindings, keys, initializers | `instance/collection/mod.rs:266`–`:354`, `:456`, `:582`, `:995`; `collection/rekey.rs:72` | `regions.rs:105`, `:116` (`$vl`) |

A `fn` body is inlined at lowering, and a `map` callback is part of its Code.
Both share their caller's budget.

## 2. What the JS target does today

- `code.rs:433` emits a native `.map`/`.filter`, and `x_join` (`rt.js:1293`)
  counts nothing.
- `Concat` is emitted as `+` (`code.rs:334`), so no string is bounded.
- `Some` and `Unwrap` emit nothing (`code.rs:286`). Records and lists are
  arrays (`code.rs:325`, `:330`), so `[some(0)]` and `[0]` are the same value.
- Derives are lazy memos (`rt.js:92`). A read inside another evaluation runs
  the derive there, nested, where the runner settles it in its own `eval`.

## 3. Decisions

### D1 — A budget context per evaluation, only where it can be spent

- **The counter.** `budget.js` holds one step counter, `B`. A Code is
  *metered* when its bytecode holds a `Map`, `Filter` or `Call join`. The
  emitter, which translates each plan Code once (`code::function`,
  `code::expression`), opens a context in a metered Code's emitted body:

  ```js
  const o = enter(); try { … } finally { leave(o); }
  ```

  `enter` saves `B` and sets it to 0. `leave` restores it.
- **The boundaries follow.** Each runner reset point in §1 evaluates one
  Code, so a context per Code reproduces each: every resource argument is its
  own expression (the callback at `emit.rs:429` becomes a list of separately
  metered ones), a row key's context is inside the function called per row,
  an action's inside the function `act` wraps.
- **Nested lazy evaluation.** A derive read mid-`map` runs inside the
  caller's evaluation. A metered derive saves the caller's count and starts
  from 0; an unmetered one never touches `B`. Either way the caller resumes
  intact. `try/finally` restores it even if a future catch site resumes an
  evaluation (none does today: `commit` and `run` rethrow a `Refusal`).
- **Unmetered Codes get nothing** (most bindings, keys and handlers). Steps
  are the only per-evaluation state; string and extent checks need no
  context (D3).
- **Skipped evaluations hide nothing.** An evaluation is a pure function of
  its reads, so where an executor skips one whose reads are unchanged (an
  unchanged memo, a kept row's key) it skips a result already computed,
  trap or not.

### D2 — The measure: a `Some` is transparent; depth is a type rule

- **A `Some` adds no node and no bytes.** In the runner, `some(v)` has `v`'s
  node and byte count, and its depth plus one (`Opcode::Some`, `measure`,
  `Extent::scalar`). Everything else counts as today: a scalar or `none` one
  node, a string one node plus its UTF-8 bytes, a record or list one node
  plus its parts.
- **Why.** Under that measure the erased JS value has exactly the runner's
  count: `null` is one node whether `none` or `unit`, records and lists are
  arrays that count alike, and an erased `Some` was never there to count
  (even `some(none)`, which the JS target collapses to `none`). A `Some` has
  one child and cannot multiply anything; the node bound exists to stop
  doubling (`[x, x]`, again and again), and at most a factor of two on
  option-heavy values gives up nothing it guards.
- **Depth becomes a checker rule.** Shapes cannot recurse (`checks.rs:331`),
  so a value is never deeper than its type. The type's depth is 0 for a
  scalar and 1 + the inner depth for an option, list or record.
  - The checker refuses any declared or inferred type deeper than 64 with
    `type-too-deep`: "a type nests at most 64 deep (the data seam's bound,
    `Value::decode`)".
  - `ValueTooDeep` stays in the VM as the defense of a plan it does not
    trust. For a compiled plan it is unreachable, so the JS target tracks no
    depth.

### D3 — Checked emission, in the runner's order

`host/web-js/budget.js`, linked by use (`QUEUE.md`'s standing rule; `rt.js`
is at 1,498 of 1,500 lines), exports these helpers; the emitter passes each
the instruction's pc as a literal.

- **`xm(list, f, pc)` and `xf(list, pred, pc)`** are loops that take the
  runner's per-item steps in order:
  1. a step (`++B > MAX_LIST_STEPS` traps `IterationLimit` at `pc`);
  2. the body;
  3. the extent of a kept item added to the result's (`ValueTooLarge` at
     `pc`).

  An empty list returns itself; a filter keeping every item returns its
  input (`vm.rs:524`), after accounting for each.
- **`xj(list, sep, pc)`** first takes `list.length` steps at once, then
  joins and checks the result's bytes.
- **`cc(a, b, pc)`** replaces `+` for `Concat`. `Add` stays `+`.
- **`K(arr, pc)`** wraps `Record` and `List`: it sums the parts' extents and
  traps `ValueTooLarge`. The parts are already evaluated, as on the VM's
  stack, so one check after the sum is the same trap at the same pc. `Some`
  stays unemitted (D2); `NP` and `x_t` check their result's bytes.
- **Extents.** `ext(v)` measures as D2 does. An array of 64 nodes or more
  is remembered in a module-wide `WeakMap` (the runner's `REMEMBERED` rule,
  kept across evaluations).
  - That is sound because a value is immutable once compiled code holds it:
    compiled code never mutates, the router verbs copy first, and a data
    answer belongs to the runtime once taken (memo equality already assumes
    it).
- **Bytes are exact UTF-8, without encoding on the fast path.** A string of
  `u` UTF-16 units encodes to between `u` and `3u` bytes.
  - Each check compares the cheap upper bound `3u` first, and only when that
    passes the limit counts exactly. A unit counts 1, 2 or 3 bytes, a
    surrogate pair 4, and a lone surrogate 3 (as U+FFFD, LLP 1088 D2).
  - A running total that once needed the exact count keeps counting
    exactly, so the trap falls on the runner's item or construction. LLP
    1088 D2's string functions use the same counter.
- **The order is the runner's.** The translator keeps the stack's
  left-to-right order (`code.rs`, `flush`) and each helper checks where the
  VM does, so an evaluation that would pass two bounds reports the same
  first one on both executors.

### D4 — Static elision, decided at compile time

Stage 3 has the translator carry a static type with each symbolic stack
entry:

- loads take the plan's types: slots, derives, resources, action params, a
  record's `TypesId` fields;
- a frame takes its subject's element type;
- calls take the roster's return type, with `first`/`at` giving
  `option<elem>`;
- a `map` takes its body's type.

With `maxNodes(T)` infinite when T holds a list, and `strings(T)` the count
of string leaves (infinite under a list):

- **`K` is skipped** when `1 + Σ maxNodes(partᵢ) ≤ 2²⁴` and
  `Σ strings(partᵢ) ≤ 1`. One string is bounded by `MAX_STRING`, which
  equals `MAX_VALUE_BYTES`. With two string parts it emits the length sum
  only.
- **In `xm`/`xf`, node accounting is skipped** when the item type has
  `maxNodes ≤ 255` (65,536 × 255 + 1 < 2²⁴). Byte accounting is
  skipped when the item holds no string, and is otherwise specialized to the
  item's string fields.
- **Steps and `cc` are never skipped.** They cost about nothing (D5).
- **Unknowns fall back.** An unknown type (`?`, a `none` or `[]` literal not
  yet joined) is treated as unbounded, so elision can only remove a check
  that a type proves unreachable.

Each rule is sound locally, and every D7 case runs with elision off (an
emitter option for tests) and on, with identical results. Correctness never
depends on D4: D2 made the generic helpers exact.

### D5 — The performance ceiling

Microbenchmarks of the emitted shapes on this Mac (Apple M5 Ultra; Chrome
154, V8; Bun 1.4.2, JavaScriptCore), in ns per operation, unmetered → metered:

| Shape | V8 | JSC |
|---|---|---|
| A binding with no list step | 3.0 → 3.0 (no context) | 2.2 → 2.2 |
| The same, inside a context (D1) | 3.0 → 3.4 | 2.2 → 3.7 |
| `map` of 48 numbers (Crypto's row), steps only | 89 → 69 | 107 → 95 |
| The same, generic extent per item (no D4) | 89 → 143 | 107 → 161 |
| `map` of 1,000 strings → records, steps and specialized bytes (D4) | 5,860 → 3,820 | 6,160 → 6,770 |
| The same, generic `xm` + `K` (no D4) | 5,500 → 12,180 | 5,240 → 15,280 |
| Two concatenations, checked | 11.5 → 12.1 | 4.0 → 5.8 |

A loop into a presized array matches `.map`; the cost is the generic extent
walks, which D4 removes for list-free items.

**The ceiling, held at stage 3:**

- No measurable cost in a Code with no list step.
- At most 10% per metered `map`/`filter` over list-free items in both
  engines.
- At most 0.5 KiB brotli on the `app.js` of an app that uses `map`, and
  nothing on one that does not (`metrics.mjs --long` budgets).
- Caltrain, RealWorld (`host/web-js/bench.mjs`) and the Bluesky feed within
  noise of before.

Measured with a scratch microbenchmark outside the repo and the existing
benches; no script is added. Stage 2 (unelided) may exceed the ceiling,
stage 3 may not; a lane that cannot meet it stops after three rounds and
reports. Parity is not traded for speed silently.

### D6 — A trap on the web is the runner's trap

- **The trap class.** `budget.js` throws a `Trap extends Refusal` whose
  message is the runner's `Debug` text of the same trap, for example
  `Trap(IterationLimit { pc: 17 })`, and which carries `kind` and `pc`. The
  pc is the Code's own, so the two executors print the same text.
- **The outcome follows from where the evaluation ran**, as LLP 1017.003 D3
  states for the runner:

  | Where | Runner | JS target |
  |---|---|---|
  | An action body, derive, resource argument or handler argument | The commit is refused and rolled back; journal `… refused: Trap(…)` | `commit`'s catch rolls back; journal `refused …: Trap(…)` |
  | A binding, subject, key or row initializer while the tree updates | The runner is poisoned | `flush` sets `Poisoned`; journal `poisoned: Trap(…)` |
  | A root initializer, or a first frame at boot or bake | Boot or bake fails with the trap | Boot throws the trap |

  Line prefixes stay each host's own; the trap text must match.
- **Two gaps close, so there is one rule to match:**
  - `encodeURIComponent` and `encodeRouteSegment` can triple a string
    unchecked (`stdlib.rs:89`). They trap `StringTooLong` past `MAX_STRING`.
  - The JS target checks an action argument's string bytes in `act`, as the
    runner refuses `StringTooLong { name }`.

### D7 — Conformance proves parity

- **The plans.** `host/web-js/conformance/budget.contract` and its `.steps`
  run on Carousel's sources (`// data: carousel`, whose `cards(n)` answers n
  records). The async lane's `conform.mjs --synthetic --linux --strict` runs
  them on the wasm runner, the JS target and the Linux runner; it already
  fails a step one target refuses and another does not (`conform.mjs:378`),
  and gains a comparison of the refused step's trap text.
- **Each case is a step that must succeed or be refused identically:**
  1. A `map` over `cards(65536)` succeeds; over `cards(65537)` it traps
     `IterationLimit`.
  2. Nested `map` over `cards(300)` traps (90,300 steps).
  3. Two arguments of one resource, 40,000 steps each, succeed (`cards`
     gains an ignored second argument for this case).
  4. Three row keys, and two handler arguments, of 40,000 steps each
     succeed.
  5. Derive A maps 40,000 items, then reads derive B, which maps 40,000:
     both succeed; raised to 70,000, B's pc traps on every target.
  6. `join` over `cards(65537)` traps at the `Call`.
  7. Doubling `s = s + s` from `"é"`: the 25th click reaches exactly
     `MAX_STRING` (2²⁵ units, 2²⁶ bytes, between `u` and `3u`) and
     succeeds; the 26th traps `StringTooLong`. A lone surrogate typed into
     an input counts as U+FFFD.
  8. A `map` of `cards(2000)` to `Two(a=cards, b=cards)` traps
     `ValueTooLarge` partway (about 24,000 nodes an item).
  9. 40,000 items of a 2 KiB string trap on bytes; 30,000 do not.
  10. `map(cards, c => some(c))` and `map(cards, c => c)` measure alike.
  11. After every refusal, the next step's state is identical on all three
      targets (LLP 1005 §6).
- Each case also runs with D4's elision off. The blocking `cargo test`
  gains only the runner's, checker's and emitter's tests (§4); no browser.

### D8 — Lean

- **Eval and Runtime are unchanged.** The semantics stays unbounded
  (`Eval.lean`'s `fuel` is a recursion bound, not one of the runner's). The
  bounds are executor refusals, and their parity is shown by running the two
  executors (D7), not by proof.
- **The checker.** `Contract.check` gains `type-too-deep` as a clause, so the
  two checkers stay equal for `difftest types`. A refusal adds no
  derivation, so `TypeCheck`'s soundness proof is unaffected.
- **difftest.** A runner refusal whose reason is `IterationLimit`,
  `StringTooLong`, `ValueTooLarge` or `ValueTooDeep` is reported `OUTSIDE`,
  not as a divergence. The generator already stays below the bounds
  (`gen/action.rs`, `GROWTH_CAP`).
- **The claim**, in two parts:
  - where no evaluation passes a bound, both executors agree with the
    semantics (LowerCorrect and difftest);
  - where one does, both refuse the same step with the same trap (D7).

## 4. Tests

- **Runner** (`vm.rs` tests, counted, not timed): `some(x)` has `x`'s nodes
  and bytes; `[some(0)]` and `[0]` measure alike; doubling through a local
  still traps; each encoder traps one byte past `MAX_STRING`.
- **Checker:** a shape nested 64 deep compiles; 65, declared or an inferred
  `some(…)` chain, is refused; every in-repo app still compiles.
- **Emitter**, in `host/web-js`'s Rust tests:
  - a Code with no list step gets no context, and one with a step gets
    exactly one;
  - each resource argument and each handler argument is its own context;
  - each emitted pc equals the `Instruction.pc` it came from;
  - D4's decisions on a table of types: a list-free record with one string
    is skipped, two strings check bytes only, a list part checks nodes, `?`
    falls back.
- **Conformance.** D7's eleven cases, with elision on and off.

## 5. Stages

1. **Runner, checker, Lean (2026-10-05).** D2, D6's encoder bound and D8 in
   one change, with their tests and `docs/reference.md`'s limits (the four
   bounds and `type-too-deep`). No JS changes.
2. **The JS target, unelided (2026-10-05).**
   - `budget.js`, D1, D3, D6's trap class and `act` check;
   - D7's plans and the harness's trap-text comparison;
   - the overhead measured and recorded (it may exceed D5).
   - The `QUEUE.md` entry is removed. LLP 1088 §9.1's precondition is met.
3. **Elision (2026-10-06).** D4, held to D5's ceiling, with D7 run with
   elision on and off.

LLP 1088 §9.1, list construction, then lands as an amendment to that LLP.
Each new list function (literals, `concat`, `slice`, `includes`) states its
steps and extent on both executors in terms of D3's helpers.

## 6. Considered and not taken

- **Boxing `Some` on the JS target.** Exact without touching the runner,
  but it changes every value crossing `rt.js`, the DOM bindings, `eq`,
  `conforms` and the data seam, and allocates per optional. D2 changes one
  measure instead.
- **Type metadata from the lowering** (opcode operands or a side table):
  bytes in every target's plan and a new `Vm.lean` decoder, for one
  executor. D4 reconstructs types only for speed, inside `host/web-js`.
- **One counter reset per commit** (LLP 1088 r2's D3): it merges budgets the
  runner keeps apart; Astra's two 40,000-step arguments would fail.
- **Charging a whole `map` at entry.** It is cheaper, but it reports
  `IterationLimit` where the runner reports a `ValueTooLarge` or
  `StringTooLong` from an earlier item.
- **Counting UTF-16 units in both executors.** The runner would pay O(n) per
  string per construction, and `MAX_STRING` is a UTF-8 bound everywhere else.
- **Bounds in the Lean semantics:** a step count through `Vm.lean` and every
  lowering proof, for what running the executors already shows.
- **A declared deviation instead.** List construction stays deferred, and a
  tab can hang on a plan native refuses in microseconds.

## 7. Open questions

1. **Is D2's runner change acceptable?** It refuses fewer programs (an
   option-heavy value gets up to twice the nodes). The alternatives are
   boxing or type metadata (§6). The author recommends D2.
2. **Should `type-too-deep` be a Contract rule or a JS-build refusal?** As a
   Contract rule (proposed), every target refuses the same programs. As a
   build refusal, only the web build refuses; nothing in the repo nests
   anywhere near 64.
3. **Is D5's ceiling right?** It is set from microbenchmarks, not an app's
   frame; the alternative is the Crypto and Bluesky fling numbers on the
   web, which stage 3 would then measure.
4. **The JS target's `some(none)` collapse** (`code.rs:13`) is a value bug,
   not a budget one. D2 makes it harmless to the bounds. Should the JS build
   refuse `option<option<T>>` and `option<unit>`, as a separate `QUEUE.md`
   line?
