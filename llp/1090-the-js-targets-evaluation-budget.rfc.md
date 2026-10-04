# LLP 1090: The JS target's evaluation budget

**Type:** RFC
**Status:** Accepted (r3, by the orchestrator under Charlie's delegation after three rounds; Grok 4.7 only — Codex budget exhausted), 2026-10-04.
- r1 was reviewed twice by Grok 4.7 (xhigh), with two scopes: semantic parity (`llp/reviews/1090-r1.grok-a.md`) and performance and implementation (`llp/reviews/1090-r1.grok-b.md`). Both NOT READY.
- r2 was delta-reviewed (`llp/reviews/1090-r2.grok.md`). NOT READY, with two MATERIAL, four MINOR and one NIT finding.
- r3 resolves all seven, so nothing is descoped (§8). r3 had no further review.
**Systems:** Runner (`runner/src/vm.rs`, `stdlib.rs`, `instance/collection/mod.rs`), Contract checker (`contract/types`), JS target (`host/web-js`: `src/code.rs`, `src/emit.rs`, `src/regions.rs`, `rt.js`, a new `budget.js`), conformance (`host/web-js/conform.mjs`, `host/web-js/conformance/`), Lean semantics and difftest (`semantics/`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stages 1 and 2 on 2026-10-05, stage 3 on 2026-10-06 (§5)
**Built:** stages 1 and 2, 2026-10-04 (`fix/impl1090`; §5, "As built"). Stage 3 is not.
**Amends:** LLP 1017.003 D3 (what a value's extent counts) and D7 ("no second evaluator": since LLP 1071 there is one); LLP 1006 §2 (two type refusals); LLP 1088 §9.1 (this LLP meets its precondition)
**Related:** LLP 1005 §6; LLP 1017.003; LLP 1071; LLP 1088 D2 and §9.1; `llp/reviews/1088-r2.astra.md` (the reset points); `QUEUE.md`, "The JS target's evaluation budget". Benchmark harness, kept outside the repository: `~/projects/x2apps/_reviews/llp1090-bench/`.

## Summary

The runner bounds every evaluation:

- 65,536 list steps;
- strings of 64 MiB;
- values of 2²⁴ nodes, 64 MiB of string bytes and 64 levels.

The JS target bounds none of it, so a program the runner refuses runs in the
browser. LLP 1088 deferred list construction until the two refuse the same
programs at the same step. Three choices make that true:

- **The measure changes, not the representation.** A `Some` stops counting
  as a node, and the checker refuses an option directly inside an option. The
  JS target then counts exactly what the runner counts from the erased values
  it already has, and the runner's walk bound loses at most a factor of two.
  Depth becomes a checker rule.
- **The step counter is a local variable of each metered evaluation.** There
  is no global counter, no save and restore, and no context in a Code without
  a list step. A nested lazy derive is a separate function, so it starts at 0
  by construction.
- **Checks are emitted inline, not elided.** Loops replace `.map`/`.filter`.
  Measured in both engines, they cost no more than the unmetered native calls
  they replace, so no check is skipped and no soundness argument is needed
  for skipping one.

| | Decision | Stage |
|---|---|---|
| D1 | A local step counter per metered evaluation, at the runner's reset points; handler arguments evaluated inside the commit | 2 |
| D2 | A `Some` adds no node or bytes; `type-option-option` and `type-too-deep` | 1 |
| D3 | Checked emission: inline loops, checked constructors, concatenation and `join`; exact UTF-8 bytes; the runner's order and pcs | 2, 3 |
| D4 | No compile-time elision | — |
| D5 | Performance: a published harness and a ceiling held in both engines | 2, 3 |
| D6 | A trap on the web is the runner's refusal, with the same `Debug` text at each site; three runner gaps closed | 1, 2 |
| D7 | Conformance: adversarial plans on wasm, JS and Linux, with pinned oracles and trap text compared | 2 |
| D8 | Lean: the semantics stays unbounded; both checkers gain the two refusals; difftest calls a bound refusal `OUTSIDE` | 1 |

## 1. What the runner bounds, and where it resets

`vm::eval` (`runner/src/vm.rs:459`) starts a fresh step count and extent cache
on every invocation.

- **`MAX_LIST_STEPS` = 65,536** (`vm.rs:43`). There is one step per
  `Map`/`Filter` body run, taken before the body (`vm.rs:224`), and one per
  item `join` prints, taken before it joins (`vm.rs:768`). The trap is
  `IterationLimit` at that pc. An empty list takes no step
  (`vm.rs:806–809`).
- **`MAX_STRING` = 2²⁶ UTF-8 bytes.**
  - `Concat` (`vm.rs:717`), `join`, `t` and `NativeProps` trap
    `StringTooLong`. A one-element string list joins unchecked
    (`stdlib.rs:191–195`).
  - An action argument or a slot write over the limit is
    `RunnerError::StringTooLong { name }` (`commit.rs:432–436`, `:519–527`).
  - Nothing bounds a resource or literal string.
- **Extents** are checked at every `Some`, `Record` and `List` built
  (`vm.rs:554`, `:658`, `:670`) and at each item a `Map`/`Filter` keeps
  (`vm.rs:512`), with loaded parts measured where used.
  - `MAX_VALUE_NODES` = 2²⁴ and `MAX_VALUE_BYTES` = 2²⁶ trap `ValueTooLarge`,
    with a shared part counted per place.
  - `MAX_VALUE_DEPTH` = 64 traps `ValueTooDeep`.
  - A filter that keeps everything returns its input (`vm.rs:525`).

The reset points are the production callers of `vm::eval`. This is Astra's
r2 table at today's lines, with the JS sites that must match:

| Evaluation | Runner | JS target |
|---|---|---|
| Each action body (timers and `then` too) | `runner/commit.rs:469` | `emit.rs:421`, wrapped by `act` at `:433` |
| Each derive evaluation or retry | `runner/settlement.rs:295` | `emit.rs:325`, `memo` at `:327` |
| **Each** resource argument | `runner/settlement.rs:367` | `emit.rs:339`, inside one `args()` callback (`:384`) |
| Each root-state initializer | `runner/router.rs:605` | `regions.rs:159` (`root_slot`; `emit.rs:294` is the locale slot) |
| **Each** handler argument, collection edges included | `runner/event.rs:814`, `:971` | `emit.rs:1127`, wrapped at `:1183–1189` |
| Each binding and surface argument | `instance.rs:810`, `:817`, `:926` | `rows.rs:63`; `emit.rs:999`, `:1036`, `:1271` |
| Each region subject, row key, row-state initializer | `instance/region.rs:43`, `:69`, `:95`, `:131`, `:324` | `regions.rs:105` (`each` and `$vl` alike), `:116`, `:77` |
| Virtualized bindings, subjects, keys, initializers | `instance/collection/mod.rs:266`–`:309`, `:456`, `:582`, `:995`; `collection/rekey.rs:72` | the same `regions.rs` sites |

`collection/mod.rs:354` evaluates `scroll-restoration` and discards a trap
with `.ok()`. D6 removes that exception. A `fn` body is inlined at lowering
and a `map` body belongs to its Code: both share their caller's budget.

## 2. What the JS target does today

- `.map`/`.filter` are native (`code.rs:433`), and `x_join` (`rt.js:1293`)
  and `Concat` (`+`, `code.rs:334`) count nothing.
- `Some` is erased, and records and lists are arrays (`code.rs:286`, `:325`,
  `:330`).
- Handler arguments are evaluated before `act` opens the commit
  (`emit.rs:1189`, `rt.js:206`). A row action's handler always has
  arguments, because the emitter inserts `$r` first (`emit.rs:1183`).
- A tree-update failure is journaled `poisoned: <message>` (`rt.js:175`).

## 3. Decisions

### D1 — A local step counter per metered evaluation

- **Metered Codes.** A Code is *metered* when its bytecode holds a `Map`, a
  `Filter` or a `Call join`. The emitter translates each plan Code once
  (`code::function`, `code::expression`). A metered Code's function declares
  `let $s=0`. Its loops (D3) increment `$s`, including loops inside a map
  body, which captures `$s`. A `join` is emitted as `$s+=l.length` with its
  trap test at the `Call`'s pc, then `x_join(l, sep, pc)`, which joins and
  checks bytes and has no counter of its own.
  - There is no module-level counter, so nothing is saved or restored.
  - A derive read mid-loop is another function's invocation (its memo), with
    its own `$s` from 0, and the caller's `$s` is untouched.
  - **An unmetered Code gets no `$s` and nothing else changes in its
    shape.** It still gets D3's checks wherever it constructs, concatenates
    or calls a checked function (`K`, `cc`, `x_t`, `NP`, the encoders). A
    Code with none of these is emitted exactly as today.
- **One invocation per reset point.** That holds for a metered
  `code::expression` because it is always an immediately called function.
  The strip in `code.rs:99–101` applies to unmetered straight-line
  expressions only. Each of the following is its own expression, never the
  enclosing callback, which also builds DOM:
  - each row-state initializer (`regions.rs:77`);
  - each surface argument;
  - each list-option binding;
  - each resource argument (an IIFE inside the existing `args()` callback;
    the memo and its subscriptions are unchanged).
- **Handler and edge arguments run inside the commit, through one entry.**
  The emitter writes `(...v)=>a_N.t(()=>[args], v)` for a handler with
  arguments, row actions included, where `args` begins with `$r`.
  - `act(fn, names, rowed)` on `rt.js:206` builds both entry points over one
    function `go(a)`:
    1. it checks the string arguments in parameter order, zipped to the
       contract's parameters with `$r` skipped when `rowed`, against
       `names` (the string parameters' names, emitted only when there are
       any);
    2. it calls `fn(...a)`.
  - The plain entry is `(...a) => commit(() => go(a), "action")`.
  - `.t(f, v)` first tests `Poisoned`. On a poisoned runner it forces `f()`
    alone, inside a `try`, so a trapping argument journals its `Trap(…)` and
    otherwise the poisoned refusal stands. This matches the runner, which
    evaluates handler arguments (`event.rs:969–971`) before `run_action`'s
    poisoned check (`commit.rs:412–414`).
  - Otherwise `.t` is `commit(() => go([...f(), ...v]), "action")`.
  - A trap in an argument is caught by `commit` and journaled as a refusal
    with nothing changed. Edges (`list.js:755`) call the same closures.
- **A skipped evaluation hides nothing.** An evaluation is a pure function of
  its reads, so skipping one whose reads are unchanged (a memo, a kept row's
  key) skips a result already computed, trap or not.

### D2 — The measure: a `Some` is transparent, nesting is a type rule

- **A `Some` adds no node and no bytes, and checks nothing.** In the runner,
  `some(v)` has `v`'s nodes and bytes and its depth plus one (`Opcode::Some`,
  `measure`, `Extent::scalar`). Everything else counts as today. Under this
  measure the erased JS value has exactly the runner's count:
  `null` is one node, records and lists are arrays that count alike, and an
  erased `Some` was never there to count.
- **`type-option-option`: an option directly inside an option is refused.**
  Without this rule, a tower of `some`s between two counted nodes makes the
  runner walk up to 64 uncounted wrappers per counted node (r1-a, finding
  1). With it, at most one `Some` sits above each counted node. So the
  expanded tree that `compare::equal`, conversion and dropping walk is at
  most twice `MAX_VALUE_NODES`, as r1 claimed.
  - The JS target already cannot represent such a value (`some(none)`
    erases to `none`, `code.rs:13`), and no `.contract` in this repository,
    x2apps, weird-castle or the Bluesky client declares one.
  - The rule applies to inferred types too, such as `first` of a
    `list<option<T>>`. The message names the type and suggests a record
    field.
- **`type-too-deep`: a type nests at most 64 deep.** Depth is 0 for a scalar
  and 1 + the inner depth for an option, list or record.
  - It is computed on value types after shape expansion. `fn` and action
    types are left out, so the walk cannot loop.
  - `?` counts 0. The semantics proves no value has type `?`
    (`ValTy.unknown`, `semantics/Contract/ValTy.lean:144`), so with type
    soundness a value is never deeper than its type.
  - Shapes cannot recurse (`checks.rs:331`).
  - The cutoff is `depth > 64`, which matches `vm.rs:258` and
    `Value::decode`.
  - `ValueTooDeep` stays in the VM to defend a plan the runner did not
    compile. For a compiled plan it cannot fire, so the JS target tracks no
    depth.
  - It is a Contract rule, so every target refuses the same programs.

### D3 — Checked emission, inline, in the runner's order

- **Loops.**
  - **Map.** `map` becomes `for` into a presized array. Each item:
    1. a step (`++$s>65536` traps `IterationLimit` at the `Map`'s pc);
    2. the body, an arrow defined once and called per item;
    3. the kept item's extent added inline (`ValueTooLarge` at the same pc).
  - **Filter** does the same, and returns its input when it kept everything.
  - **Empty lists** return themselves without taking a step.
- **Extent of a part, inline.** A string is 1 node plus `3·length` toward
  the byte bound. A number, bool or `null` is 1. An array goes to
  `nodesOf(v)`, which returns the cached or walked extent.
- **Constructions.** A `Record` or `List` built inside a loop body sums its
  parts inline, since the part count is in the opcode, and the loop adds the
  sum. Elsewhere it calls `K(arr, pc)` in `budget.js` (stage 2: `K`
  everywhere; stage 3 inlines the loop bodies, D5).
- **Concatenation.** `Concat` becomes `cc(a, b, pc)`: it builds the
  string, then checks the length when `length > ⌊MAX/3⌋`. `Add` stays `+`.
- **`join`.** The steps are the caller's `$s` (D1), taken before joining,
  as `vm.rs:768` does. `x_join` then returns a one-element list's string
  unchecked (`stdlib.rs:191–195`) and otherwise checks the result's
  bytes.
- **`NP`, `x_t` and the two encoders** check their result's bytes. The
  encoders check after `encode_route_segment`'s empty and dot refusal, at
  the `Call`'s pc.
- **Exact UTF-8 bytes, cheap until near the bound.**
  - A string of `u` UTF-16 units encodes to between `u` and `3u` bytes: a
    unit counts 1, 2 or 3, a surrogate pair 4, and a lone surrogate 3 (as
    U+FFFD, LLP 1088 D2).
  - An accumulator (a loop's sum, `K`'s sum, a cached extent) holds the
    upper bound `3u` until that bound passes `MAX_VALUE_BYTES`.
  - Then the exact count of everything accumulated so far **replaces** it,
    and every later addition is exact. A trap fires only on the exact
    total, `>` as in Rust. So the trap falls on the runner's item or
    construction, never earlier and never later.
  - In an exact recount, a cached part's bytes count only if its cache
    entry is marked exact. Otherwise the part is recounted in UTF-8, and its
    entry is updated to that exact total and marked exact.
  - A one-shot check (`cc`, `join`, `t`, `NP`) counts exactly once its upper
    bound passes.
- **The extent cache.** An array of 64 nodes or more is remembered in a
  module-wide `WeakMap` with its nodes, its bytes and whether the bytes are
  exact (the runner's `REMEMBERED` rule, kept across evaluations).
  - It is sound only while values are immutable once compiled code holds
    them: compiled code never mutates, the router verbs copy first, and a
    data answer belongs to the runtime once taken (memo equality already
    assumes this).
  - A later change that mutates a value array must drop the cache.
  - A shared part counts once per place, never once per walk.
- **`budget.js` and `rt.js`.** `budget.js` holds the cold parts: the `Trap`
  class, `nodesOf`, the exact UTF-8 counter, `K`, `cc` and the checked
  `x_join`, `NP`, `x_t` and encoders.
  - The app module imports what it calls from `budget.js` directly; the
    emitter's import list gains a second module.
  - `rt.js` never imports it, and the roster test
    (`code.rs:600–614`) reads `budget.js` as well.
  - `rt.js` (1,499 of 1,500 lines) changes only on existing lines: `act`
    (D1), the commit's write check and `sig` (D6).
  - Those checks are one-shot tests that need no counter: a string whose
    `length > ⌊MAX_STRING/3⌋` is encoded with `TextEncoder`, which turns a
    lone surrogate into U+FFFD's 3 bytes, and its byte length is compared.
- **The order is the runner's.** The translator keeps the stack's
  left-to-right order (`code.rs`, `flush`), and each check sits where the VM
  makes it. So an evaluation that would pass two bounds reports the same
  first trap on both executors.

### D4 — No compile-time elision

r1's type-directed elision was unsound: a resource or literal string is not
bounded by `MAX_STRING` (r1-a-7, r1-b-1). It was also unneeded, because
inline checks cost what the unmetered calls cost (D5). Everything is
checked.

### D5 — The performance ceiling

These numbers come from the harness in
`~/projects/x2apps/_reviews/llp1090-bench/` (`bench.js`, the model
`budget.js`, `serve.js` for Chrome), outside the repository. Each value is
the median of 7 runs of each shape after warmup, with opaque strings, on an
Apple M5 Ultra under Chrome 154 (V8) and Bun 1.4.2 (JavaScriptCore). Times
are ns per call.

| Shape | V8 | JSC |
|---|---|---|
| `map`, 48 numbers: native `.map` | 61.5 | 80.8 |
| … emitted loop, body arrow, local `$s` (D1, D3) | 27.0 | 54.5 |
| … outlined helper with a global counter (r1's design) | 123.5 | 147.2 |
| `map`, 1,000 strings → 3-field records: native `.map` | 4,833 | 4,520 |
| … emitted loop, parts summed inline (stage 3) | 1,933 | 4,471 |
| … emitted loop, outlined `K` (stage 2) | 7,000 | 9,018 |
| … outlined helper and `K` (r1's design) | 12,133 | 10,250 |
| A four-part template: `+` → `cc` | 9.2 → 9.7 | 6.0 → 9.9 |
| A binding with no list step | unchanged | unchanged |

r1-b found r1's outlined helper could not meet r1's ceiling; the
reproduction agrees, and D3 now specifies the emitted loop.

**The ceiling.**

- **Stage 3, in both engines on the harness:** an emitted metered
  `map`/`filter`, construction included, no slower than the native call it
  replaces. A concatenation costs at most 1.5 ns more.
- **Codes with no list step, construction, concatenation or checked call**
  are unchanged. Their constructions and concatenations pay `K` and `cc`
  (in the table).
- **App level:** the RealWorld load and press (`host/web-js/bench.mjs`) and
  a Caltrain hover drive are within noise of before, and every in-repo
  `app.js` grows at most 1 KiB brotli (`metrics.mjs --long`).
- **Stage 2** (outlined `K`) is held only to correctness.

In-repo consumers are small: Spark's, RealWorld's and Shared Elements'
filters, one `join`, and Caltrain's templates (now `cc`). The 48-number row
is the out-of-repo crypto bench's shape. A lane that misses the ceiling stops
after three rounds and reports. Parity is not traded for speed.

### D6 — A trap on the web is the runner's refusal

- **The trap class.** `budget.js` throws `Trap extends Refusal`, carrying
  `kind` and `pc`.
- **The journal text is the runner's.** Each site's journal text is the
  `Debug` of the `RunnerError` the runner returns there. Line prefixes stay
  each host's own; conformance compares the reason.

  | Where | Runner outcome | Reason text |
  |---|---|---|
  | Action body, derive, resource argument, handler or edge argument, root initializer | Commit (or boot) refused, rolled back | `Trap(IterationLimit { pc: 17 })` |
  | Binding, surface argument, subject, key or row initializer while the tree updates | Runner poisoned; writes kept, store restored (`commit.rs:64`) | `Instance(Trap(IterationLimit { pc: 17 }))` (`lists.rs:94–98`, `runner.rs:203–208`) |
  | A string action argument or slot write past `MAX_STRING` | Commit refused | `StringTooLong { name: "text" }` |

  - The JS `flush` catch already poisons. It now journals the `Instance(…)`
    form.
  - The commit's write loop (`rt.js:149`) refuses a top-level string over
    `MAX_STRING` after the body, with the slot's name. `sig` takes the name
    for string-typed slots only.
  - `act`'s shared `go` refuses a string argument over `MAX_STRING` in
    parameter order, for both entry points (`commit.rs:424–436`; D1).
  - A bake or boot that fails does so with the same text.
- **Three runner gaps close in stage 1, so there is one rule to match:**
  1. `encodeURIComponent` and `encodeRouteSegment` check `MAX_STRING` on the
     encoded result (`stdlib.rs:89–99`). The empty and dot refusal comes
     first, unchanged.
  2. `scroll-restoration` propagates a trap like every other virtualized
     binding (`collection/mod.rs:354`).
  3. difftest's `OUTSIDE` list covers both `StringTooLong` forms (D8).

### D7 — Conformance proves parity

- **The plans.** `host/web-js/conformance/budget.contract` and its `.steps`
  run on Carousel's sources (`// data: carousel`). `cards(n)` answers `n`
  six-node records (`apps/carousel/data/src/lib.rs:19–27`). Carousel's data
  crate gains an ignored second argument to `cards` (case 3) and
  `long(n, c)`, the string `c` repeated `n` times (cases 10, 13, 17 and
  18).
  - The async lane's `conform.mjs --synthetic --linux --strict` runs them on
    the wasm runner, the JS target and the Linux runner.
  - It already fails a step that one target refuses and another does not
    (`conform.mjs:378`).
  - It gains a comparison of the reason text for bound refusals: the kinds
    above and the `StringTooLong { name }` form. Other refusals keep today's
    refused-or-not comparison; the empty route segment's text differs
    already and is not this LLP's.
- **Cases.** Each step must succeed, or be refused with the same reason:
  1. `map(cards(65536), c => c)` succeeds (1 + 65,536 × 6 nodes, under the
     cap); `cards(65537)` traps `IterationLimit` at the `Map`.
  2. Nested `map` over `cards(300)` traps (90,300 steps).
  3. Two arguments of one resource, 40,000 steps each, succeed.
  4. Three row keys, two metered row-state initializers and two handler
     arguments, 40,000 steps each, succeed.
  5. Derive A maps 40,000 items, reads derive B (which maps 40,000), then
     maps 30,000 more. It traps at A's second `Map`. This proves B neither
     reset nor charged A.
  6. A handler argument and a list-edge argument that trap refuse the
     action. The reason is `Trap(…)` and the next step's state is
     unchanged.
  7. A row key that traps poisons, with `Instance(Trap(…))`.
  8. `join` over `cards(65537)` traps at the `Call`. A one-element list of a
     string past `MAX_STRING` returns it.
  9. Doubling `s = s + s` from `"é"`: the 25th click reaches exactly
     2²⁶ bytes (2²⁵ units, between `u` and `3u`) and succeeds; the 26th
     traps `StringTooLong`.
  10. Storing `long(2^26 + 1, "a")` into a slot is refused
      `StringTooLong { name }`.
  11. `map(cards(2000), c => Two(a=cards, b=cards))`: each item is 24,003
      nodes, and the sum crosses 2²⁴ on item 699 with
      `Trap(ValueTooLarge { pc })` at the `Map`.
  12. 40,000 copies of a 2 KiB ASCII string trap `ValueTooLarge` at the
      `Map` on item 32,769. 30,000 succeed, though their `3u` upper bound
      passes the cap at item 10,923: the exact recount (D3) carries on.
  13. A record holding `long(2^26 + 1, "a")` traps `ValueTooLarge`.
  14. Each encoder traps one byte past `MAX_STRING`.
  15. `[x, x]` over a cached 64-node list counts it twice, and a
      filter-identity subject is not mutated.
  16. `join(map(cards(40000), c => c.label), ",")` traps `IterationLimit`
      at the `join`'s `Call` (80,000 steps in one evaluation); with 30,000
      it succeeds.
  17. **Handler and row arguments.**
      - A row action's `input` handler whose argument is
        `long(2^26 + 1, "a")` is refused `StringTooLong { name }`, and the
        next step is unchanged.
      - `long(22369622, "€")` (67,108,866 bytes in fewer than 2²⁶ units),
        stored or passed, is refused the same way.
      - After a row key has poisoned the runner (case 7), a handler whose
        argument traps journals `Trap(…)`.
  18. With `resource k = long(1000, "a")` and
      `derive xs = map(cards(10000), c => k)` (10,001 nodes, so cached;
      10 MB exact, 30 MB as an upper bound), `Three(a=xs, b=xs, c=xs)`
      succeeds through the cached part's recount: 30 MB exact, 90 MB as an
      upper bound. `Seven(…)` (70 MB) traps `ValueTooLarge`.
  19. After every refusal, the next step's state is identical on all three
      targets (LLP 1005 §6).
- **State size.** The harness compares state, so cases 9 and 10 hold a
  64 MiB slot for one step. The step after them resets it.

### D8 — Lean

- **Eval and Runtime are unchanged.** The semantics is unbounded. The bounds
  are executor refusals, shown equal by running the executors (D7).
- **The checker.** `Contract.check` gains `type-option-option` and
  `type-too-deep`, so `difftest types` keeps the two checkers equal. A
  refusal adds no derivation, so `TypeCheck`'s soundness proof stands.
- **difftest.** A runner refusal of `IterationLimit`, `StringTooLong` (the
  trap or `RunnerError::StringTooLong { name }`), `ValueTooLarge` or
  `ValueTooDeep` is `OUTSIDE`. The generator already stays under the bounds
  (`gen/action.rs`, `GROWTH_CAP`).
- **The claim:** where no evaluation passes a bound, both executors agree
  with the semantics; where one does, both refuse the same step with the
  same reason.

## 4. Tests

- **Runner** (counted, not timed): `some(x)` measures as `x`; doubling
  through a local still traps; the encoders trap one byte past
  `MAX_STRING` and still refuse `""`; a trapping `scroll-restoration`
  poisons.
- **Checker:** `option<option<T>>` is refused, declared or inferred; depth
  64 compiles and 65 does not; every in-repo app compiles.
- **Emitter:**
  - a Code with no list step, construction, concatenation or checked call
    is emitted as today; one with a construction or concatenation but no
    list step has no `$s` and does call `K` or `cc`;
  - a `join` adds to `$s` at its `Call`;
  - `act` receives its string parameters' names and whether it takes `$r`;
  - a metered Code declares one `$s`, and a metered `expression()` is called
    in place;
  - each resource argument, row initializer and surface argument is its
    own;
  - handlers with arguments use `.t`;
  - every pc equals its `Instruction.pc`.
- **Conformance:** D7.

## 5. Stages

1. **Runner, checker, Lean (2026-10-05).** D2 (the measure and both
   refusals), D6's three runner gaps, and D8, with their tests and
   `docs/reference.md`'s limits. No JS changes.
2. **The JS target (2026-10-05).** D1, D3 with an outlined `K`, D6's JS
   side, D7's plans and the harness's reason comparison. The emitted code
   is re-measured on the harness. LLP 1088 §9.1's precondition is met, and
   the `QUEUE.md` entry is removed.
3. **Speed (2026-10-06).** Loop bodies sum construction parts inline. The
   D5 ceiling is held on the harness and the app benches.

LLP 1088 §9.1 then lands as an amendment to LLP 1088. Each new list function
states its steps and extent on both executors in D3's terms.

## 6. Considered and not taken

- **Counting option layers from static types in the JS target** (r1-a's
  fix). This makes typed translation a correctness dependency at every
  construction and walk. `type-option-option` gets the same bound without
  types, and refuses only programs the web build already gets wrong.
- **Boxing `Some` on the JS target.** It changes every value crossing
  `rt.js`, the DOM, `eq`, `conforms` and the data seam.
- **A global counter with save and restore, and outlined helpers** (r1).
  They measured 2× to 2.5× native (D5). A local counter is free and nests by
  construction.
- **Type-directed elision** (r1's D4): unsound and unneeded.
- **One counter per commit; charging a whole `map` at entry; UTF-16 counts in
  both executors; bounds in Lean.** These would merge budgets, report the
  wrong first trap, cost O(n) in the runner, and put a step count through
  every proof.

## 7. Open questions

1. **`type-option-option`.** It is a language restriction: `option<option<T>>`
   is refused, including from `first` or `at` on a list of options. The
   alternative is type-aware counting in the JS target (§6). The author
   recommends the refusal.
2. **`scroll-restoration` now poisons on a trap,** where it was ignored. Is
   that acceptable? The alternative is for the JS target to swallow the
   same trap at the same point.
3. **Is D5's ceiling right?** It is held per shape on the harness and within
   noise on the app benches. Making it a frame-time budget would need a
   fling bench on the web in this repository, which is apparatus.

## 8. Revisions

**r2 (2026-10-04).** Every finding of both Grok reviews is resolved or
rejected with evidence. The dispositions, finding by finding, are in the two
review records. The design changed in five ways:

- the local counter;
- handler arguments inside the commit;
- `type-option-option`;
- inline emission instead of elision;
- pinned conformance oracles.

**r3 (2026-10-04), the final revision.** It resolves the r2 delta review
(`llp/reviews/1090-r2.grok.md`):

- one argument check for both of `act`'s entry points, with `$r` skipped;
- the argument thunk forced on a poisoned runner;
- `K` and `cc` in unmetered Codes;
- `join`'s steps on the caller's `$s`;
- the cache's exactness rule;
- one-shot UTF-8 tests in `rt.js` through `TextEncoder`;
- the re-pinned `emit.rs` sites.

Cases 16 to 18 are new. Nothing is descoped.
