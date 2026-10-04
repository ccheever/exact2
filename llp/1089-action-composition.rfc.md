# LLP 1089: Action composition

**Type:** RFC
**Status:** Draft r1, 2026-10-04. Not reviewed. Stage 1 widens `rules/DEFERRED.md` **Actions**, so it needs Charlie's yes first (§7.1).
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`, `contract/cli/src/{lean.rs,main.rs}`), Lean semantics and difftest (`semantics/`), the JS target's conformance (`host/web-js/conformance`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-05 and stage 2 on 2026-10-05 (§5)
**Amends:** LLP 1017 §11 (the tail call becomes one case of a call); LLP 1006 §2 (statements); `rules/DEFERRED.md` **Actions** (on Charlie's yes)
**Related:** LLP 1017 P4c and §11; LLP 1035.005.000 D1 (effects inferred) and D2 (`let`); LLP 1088 D8 (one send per path); LLP 1016 D5; diaries `~/projects/x2apps/{spreadsheet,files,mail}/DIARY.md`. Research only: LLP 0082 §Actions ("actions may call actions"), LLP 0481 §4.1 (effects compose through calls and callback values; call cycles are rejected from the call graph), LLP 0482 §3.1.

## Summary

An action cannot call an action. The one exception is a child's tail call
to an `action` prop, which landed on 2026-10-03 (LLP 1017 §11) and is not
documented anywhere an agent reads. So app-building agents copy statements
between actions. Spreadsheet writes "commit the edit" five times. Files
writes "open" four times. Mail moved a row's drag state to the root.

This RFC lets an action call any action in its scope, anywhere a
statement goes, in one commit. The compiler expands each call in place, as
`inline/tail.rs` does for tail calls. Reads still see the starting state; a
read that a call would make stale is refused. The plan, the runner, the JS
target and the hosts do not change.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | An action calls its component's actions, action props and injected actions, as a statement, anywhere | spreadsheet F21, files F27, mail F18 | 1 |
| D2 | A call is its callee's statements, expanded in place: one commit, reads see the starting state | — | 1 |
| D3 | A read that a call would make stale is refused | — | 1 |
| D4 | No recursion; a bound on expansion | — | 1 |
| D5 | Effects come from the expanded body | — | 1 |
| D6 | LLP 1088 D8 names the calls | spreadsheet (D8 rollout) | 1 |
| D7 | An action-prop call runs the owner's statements | mail F18 | 1 |
| D8 | No executor change; the AST carries the call | — | 1 |
| D9 | Lean gets `Stmt.call`, so difftest checks the expansion | — | 2 |
| D10 | Diagnostics and docs | all three | 1 |

## 1. Evidence

- **Spreadsheet F21.** "Commit the draft, close the editor, clear firstKey"
  is repeated in five actions, and "move and scroll into view" in three. One
  change touched seven places. The root holds 38 slots and about 60
  actions.
- **Files F27.** Six assignments that open an item are repeated four
  times; a fix to `anchorPath` had to be made in each.
- **Mail F18.** The diary says "a child action can't invoke `swipe(id)`".
  Mail was built at `c7bb6ab9d`, which already carried `inline/tail.rs`, and
  "reset the offset, then on release archive" fits the tail form. The agent
  did not find the form because `docs/contract-for-agents.md:580` still says
  "Calling an action from another action → Put the statements there", and
  §State and action semantics says "No loops or general action calls". The
  tail call appears only in LLP 1017 §11 and in a D8 note in the grammar.
- **Probes** (`contract build --json` at `6cfb736a8`):
  - A root action calling `bump()` gets `type-unknown-command`, "an action
    is not callable from an action; put its statements here".
  - A child calling its `go: action` prop before its last statement gets
    "an action calls one only as its last statement (its tail call)".
  - An injected `go: action` binds `press=go(1)`, but calling `go(1)`, even
    last, gets "`go` is not a host command". The tail call marks only props
    (`inline.rs:487`).
- **The machinery exists.** `tail::resolve` (`inline.rs:277`) puts the
  callee's statements in place. Its parameters become `let`s of the
  arguments, and every caller and callee binder is renamed apart with `@`.
  It refuses cycles (`syntax-tail-cycle`) and arity mismatches. It leaves an
  `@check:<action>` command for the type pass. LLP 1088 D8's walk
  (`analyze/src/sends.rs`) already reads the resolved body. Nothing in the
  mechanism needs the tail position.

## 2. Decisions

### D1 — An action calls any action in its scope

A statement `name(args)` in an action body is a **call** when `name` is one
of these:

- an action of the same component;
- an `action` prop;
- an `inject`ed name of type `action`.

The call may stand wherever a statement may: first, between assignments,
in any `if` or `match` arm, more than once.

- **Arguments** complete the callee's parameters, after any arguments
  curried where the action was passed (`close=dismiss("photo")`). No event
  payload is appended; a callee that takes one, such as
  `flip(id, checked: bool)`, is passed it explicitly.
- **No value.** A call is a statement. `let x = save()` and a call inside
  an expression are refused. Compute values with `fn`.
- **First-order.** An argument may not be an action. Actions flow only
  through props, `provide`/`inject`, element bindings and calls.
- **Scope is unchanged.** A child cannot name a parent's or the root's
  action. It reaches one only through a prop or an inject (D7).
- **Host commands.** When `name` is both an action in scope and a host
  command, the call is refused (`type-call-ambiguous`, "rename the
  action"). Silently shadowing `focus` or `share` would change what an
  existing statement does. The refusal is at the call, so apps that never
  call such an action are unaffected.

### D2 — A call is its callee's statements, expanded in place

The compiler replaces each call with the callee's statements, LLP 1017
§11's meaning moved from the tail to any position. The parameters become
`let`s of the arguments, evaluated where the call stands; the callee's
names are renamed apart (the existing hygiene); its own calls expand in
turn.

**One commit.** Caller and callees form one action: one commit, one
rollback. A trap anywhere refuses the whole commit, so nothing the caller
wrote before the call lands.

**Reads see the starting state. Writes land in statement order, and the last
wins.** This is the rule every statement already follows (`docs/
contract-for-agents.md` §State and action semantics). Lean proves it as
`runAction_reads_prestate` and `runAction_last_write` (`Axiomatic.lean:535`,
`:509`). The tail call shipped under it. So a callee does **not** see the
caller's pending writes. The other choice, a callee that reads what the
caller wrote above it, was not taken:

- A read's meaning would depend on which body the statement was written in.
  `count = 1; doubled = count * 2` would read 0 inline and 1 behind a
  call.
- Moving statements into a helper, the refactoring this RFC exists for,
  would silently change their meaning, and so would the shipped tail call.
- React, the model most agents bring, has the same rule: `setX(1)` and then
  a helper reading `x` sees the render's value.

Values the caller computed are passed explicitly: `let next = …; sel =
next; follow(next)`, as the docs already teach for `let`.

### D3 — A read that a call would make stale is refused

Inside one body a snapshot read is visible on the page (the docs'
`doubled = count * 2`). Behind a call, the write and the read are in
different bodies, and neither page shows the read is stale. An agent reasoning from Svelte, Vue or plain JavaScript
expects the new value. This is a refusal, not a warning, as LLP 1088 D8
chose, because warnings go unread.

**Frames.** The expanded body is a tree of frames. The action is the
outermost frame, and each call opens a new frame, so two calls to one
action are two frames. A **write** is an assignment, or a `send`, which
writes its mutation's state (`pending(m)`). A **read** of slot `s` is:

- a direct read of `s`;
- a read of a derive that reads `s`, transitively;
- `pending`/`failed` of a sent mutation;
- an argument expression, which is read on the callee's behalf.

**The rule** (`analyze-call-stale-read`). A read of `s` in frame F is
refused when some path to it writes `s` in a frame other than F. The walk
is LLP 1088 D8's: path-sensitive, `if`/`match` arms start from the incoming
set, and the union is taken after the branch.

- `sel = next; follow()` where `follow` reads `sel`: refused.
- `reset(); count = count + 1`: refused.
- `move(1); move(1)`: refused, because the second frame reads what the
  first wrote. In JavaScript that moves by 2; in Contract, by 1.
- `a = b; b = a`, a swap within one frame: accepted, as today.
- A child reading a prop after calling its owner: accepted. Props are
  captured before the action's first statement runs (`@capture` parameters,
  `inline.rs:454`), so they are parameters, not slot reads, as React's props
  are.

Across a call boundary, then, every accepted program means the same
whether it is read as Contract's snapshot or as statements run in order.
Inside one frame, today's rule stands (§7.2). The message names both
sides and the fix:

> `follow` (called at line 44) reads `sel`, which `enter` writes at line 43;
> a read sees the state the action started with, so `follow` would see the
> old `sel`. Compute the value first and pass it: `let next = …`,
> `sel = next`, `follow(next)`.

### D4 — No recursion, and a bound on expansion

- **A cycle is refused**, direct or through props and injects
  (`syntax-call-cycle`, renamed from `syntax-tail-cycle`). The message
  shows the path, `a → b → a`. There is no recursion, so expansion
  terminates; LLP 0481 §4.1 reached the same rule from the call graph.
- **Expansion is bounded.** An action whose expanded body exceeds 1,024
  statements is refused (`syntax-call-size`, naming the action and its
  largest callee). Expansion duplicates code: a chain of actions that each
  call the next twice grows exponentially. The bound keeps plan and JS
  size proportional to what was authored.

### D5 — Effects come from the expanded body

LLP 1035.005.000 D1 inferred writes and said that indirect calls "must be
accounted for statically" before the inference extends to them. Expansion
does that by construction. The plan's `actions.writes` comes from the
expanded body, so it is the union of the action's own writes and its
callees', and the tail call already works this way. Also:

- **`analyze-then-self-send` reads the expanded body.** Today
  `check_mutation_then` reads the authored `c.actions` (`analyze/src/
  lib.rs:229`), so a `then` action that called an action sending the same
  mutation would pass.
- **`contract symbols`** lists each action's calls and its inferred writes
  through them, so `symbols --name enter` answers "what does Enter touch".
- Expanded statements keep the callee's spans, so the source map and
  `perf` point at the lines that do the work.

### D6 — LLP 1088 D8, through calls

D8's walk is unchanged. A caller's send and a callee's could not meet
before (LLP 1088 §10); now they can. `commitEdit(); openSheet(id)` where both send `saved`, or
`save(); save()`, is refused as two sends on one path. The message names
the frames:

> `enter` sends `saved` twice on one path: in `commitEdit` (called at line
> 41) and in `openSheet` (called at line 42).

Exclusive arms stay accepted, including arms that call different
senders.

### D7 — An action-prop call runs the owner's statements

A child does not write its parent's state, and nothing here changes that.
`go(id)` in a child runs the statements of the action its owner passed as
`go`. Those statements were compiled in the owner's scope: they are the
lifted action with its captures, exactly as the tail call resolves it today
(`inline.rs:483–524`). The call asks the owner's action to run, in the same
commit, the way calling a prop callback works in React.

- **Rows and arms.** A call runs with the frames in force at the child's
  view, as a tail call does today.
- **Sibling calls in a child.** A call to a sibling lifted action passes the
  caller's own capture parameters, which belong to the same instance, ahead
  of its arguments.
- **Injects.** A provided action that is called must name an action
  (`go = bump` or `go = bump(x)`). A ternary or other expression is refused
  (`syntax-call-target`, renamed from `syntax-tail-call`), as it is for a
  called prop today.

### D8 — No executor change; the AST carries the call

- A resolved call becomes one statement, `Stmt::Call { action, args, body,
  span }`. `body` is the callee's renamed-apart statements, with its
  parameters as leading `let`s. It replaces both the `@tail:` marker and
  the `@check:` leftover.
- The type pass checks `args` in the caller's scope against the callee's
  parameter types. It does not check `body` again; the callee was checked
  as its own action. A call is one more use for parameter refinement, as a
  handler binding is. LLP 1088 §9.2's deferral stands.
- Lowering emits `body` in place.
- D3, D6 and D10 read the call structure for frames and messages.
- **Nothing changes below the compiler.** The plan format, `FORMAT_DIGEST`,
  the runner, the JS target (it compiles from the plan, `host/web-js/src/
  main.rs`) and every host are untouched.
- **No regression:** every in-repo app's plan is byte-identical before
  and after stage 1, tail-call users included.

### D9 — Lean gets `Stmt.call`, so difftest checks the expansion

`contract lean` emits the expanded root, so both sides of difftest run the
Rust inliner's output, and a renaming bug in it is invisible. So:

- **Semantics.** `Stmt` gains `call (action : String) (args : List Expr)`.
  Its meaning, defined independently of the Rust renaming:
  1. evaluate the arguments in the caller's environment on the pre-state;
  2. run the callee's body in a fresh local scope with only its parameters
     bound, on the **same pre-state and the same pending writes**.

  `exec` gains a depth bounded by the number of actions, which an accepted
  (acyclic) program never exhausts.
- **Proofs** gain the `call` case: `Big.lean` (sound, complete,
  deterministic), `Axiomatic.lean` (a call rule; `runAction_reads_prestate`
  and `runAction_last_write` keep their statements), `Soundness.lean`, and
  `Lower*.lean`, whose `Lower` expands `.call` as Rust lowering does, in a
  fresh local scope.
- **Emitter.** `lean.rs` emits `Stmt::Call` as `.call action args`, never
  `body`; the `Apps/` embeddings of tail-call users are regenerated.
- **Difftest.** The corpus gains `semantics/corpus/actions/calls/`: a
  sequence, a call in an arm, two calls, a chain, a prop call inside a row,
  an inject call, a same-spelling local in caller and callee, and a trap in
  a callee rolling back the caller. The random generator emits calls from
  action *i* to actions *j < i*, with arguments, so cycles are impossible.
  The random sweep then checks the Rust expansion against `.call`'s
  definition.
- **D3 in Lean is not owed** (§7.4).

### D10 — Diagnostics and docs

Diagnostics:

- `type-unknown-command`, for a root action named in a child: "`archive`
  is not in `Row`'s scope; pass it as an `action` prop or `provide` it".
- `type-call-value`: "`save` is an action; a call is a statement and
  returns nothing. Compute values with `fn`".
- `type-call-arity`: "`move` takes 2 argument(s) after the 1 curried at
  `go=move(id)`, given 1". This replaces the arity text in
  `syntax-tail-call`.
- `type-call-action-arg`, `type-call-ambiguous` (D1); `analyze-call-stale-read`
  (D3); `syntax-call-cycle`, `syntax-call-size` (D4); `syntax-call-target`
  (D7); D8's frames (D6).

Docs:

- `contract-for-agents.md`: §State and action semantics gains calls, the
  snapshot across calls with the `let next` pattern, and D3's refusal. The
  table row "Calling an action from another action" becomes "Copying an
  action's statements → call it: `commitEdit()`". §Composition says that a
  child calls an `action` prop like a callback.
- `contract-grammar.md` and `contract-for-humans.md`: the statement form
  and the refusals.
- `agent-pitfalls.md`: "pass the value you just wrote; a callee reads the
  starting state".
- `rules/DEFERRED.md` **Actions**: rewritten on Charlie's yes. Still
  refused: recursion, a return value, an action as an argument.

## 3. Effect on each implementation

| | Stage 1 | Stage 2 |
|---|---|---|
| syntax | `inline/tail.rs` → `inline/calls.rs`: resolve every call, not only tails; `Stmt::Call`; mark calls to own actions, props and injects (`inline.rs:483`, `:875`); cycle, size, target refusals | — |
| types | call checks in `actions.rs` (scope, arity, value, action argument, ambiguity), replacing `check_command`'s action/prop branches (`checks.rs:986`); parameter refinement counts calls | — |
| analyze | `calls.rs`: D3's frame walk beside `sends.rs`; D8's frame message; `check_mutation_then` on the expanded body | — |
| lower | emit `Stmt::Call`'s body; drop the `@check:` case | — |
| cli | `symbols` lists calls; `lean.rs` emits a call's `body` | `lean.rs` emits `.call` |
| plan, runner, JS target, hosts | none | none |
| Lean, difftest | — | `Stmt.call`, `exec` depth, the proofs' `call` cases, corpus, generator |
| JS conformance | — | `host/web-js/conformance/calls.contract` (async lane) |
| docs | D10 | the semantics README's `Stmt` row |

## 4. Tests

- **Compiler**: `contract/cli/tests/it/tail_call.rs` becomes `calls.rs` and
  keeps every tail case. It adds a call first, in the middle, in an arm and twice; a chain of three; a
  prop call before the child's own writes and inside a row; an inject call;
  a sibling call in a child; curried and payload arguments; `symbols`.

  `diagnostics.rs` asserts each new message whole, D3's examples (refused
  and accepted) among them, and `then` self-send through a call.
- **Plans unchanged**: decode every in-repo app's plan on the base and on
  stage 1, and compare the bytes.
- **Lean**: `lake build` with the proofs; difftest `corpus`, `random --count
  500`, `types`, `lowering` (async lane).
- **Driven**: spreadsheet's commit-the-edit and files' open, rewritten as
  calls (`EXACT_APP_DIR`; these apps live outside the repo). Their authored
  tests stay green on web and macOS through `scripts/agent.mjs`. Mail's
  swipe is rewritten with the root's `swipeDx` moved back into the row.
- The five checks after each stage.

## 5. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, on
a branch from origin/main after Charlie's yes. Each commit passes the five
checks.

1. **Stage 1, 2026-10-05: the compiler and docs (D1–D8, D10).**
   - **Census first.** D3 can refuse an existing tail call whose callee
     reads what its caller wrote. Build every in-repo app and every
     `~/projects/x2apps` app. Fix any in-repo refusal in the same commit and
     list the outside ones, as LLP 1088 D8's rollout did. Count same-frame
     reads after writes for §7.2.
   - **Exit:** the §4 compiler tests; plans byte-identical; spreadsheet,
     files and mail converted and driven on web and macOS.
2. **Stage 2, 2026-10-05: Lean and difftest (D9),** with the conformance
   fixture.
   - **Exit:** proofs build, and the corpus and a 500-case sweep agree.
   - The `Lower*` `call` case is the piece most likely to slip. If it does,
     stage 2 lands the semantics, the other proofs and difftest. The
     lowering theorem is then stated for call-free bodies, the restriction
     is named in `semantics/README.md`, and it gets a `QUEUE.md` line. No
     `sorry` lands.

## 6. Considered and not taken

- **A runtime call (a plan opcode, a JS function call).** It would avoid
  duplicating code. But it costs a plan-format change, a VM frame for
  parameters and captures, row frames for a lifted callee, both executors
  and Lean's VM model, all for a size problem no app has. Trigger: an app
  hits D4's bound, or `metrics.mjs` shows plan growth from calls on a real
  app.
- **Callees that see pending writes** (D2): two read rules in one commit.
- **No D3**: silent stale reads, in exactly the refactoring calls invite.
- **D3 as a warning**: warnings go unread (LLP 1088 D8).
- **A `fn` returning a record that an action spreads** (F21's
  alternative): it cannot send or call a host command, and needs a
  multi-assignment form.
- **Keeping tail-only and documenting it**: fixes mail, leaves
  spreadsheet and files where they are.
- **Return values**: an expression with effects; `fn` computes.

## 7. Open questions

1. **The DEFERRED waiver.** The **Actions** entry still refuses "an action
   calling a root action by name, a call anywhere but the tail". This RFC
   offers no take. Charlie waived one for the tail call; does he waive one
   for this?
2. **D3 inside one frame.** Should D3 refuse every read after a write on
   one path, not only across calls? Then the snapshot could never surprise
   anyone, but the swap and the docs' own `doubled` example would be
   refused. Decide after stage 1's census counts how many in-repo and
   x2apps actions read after writing.
3. **The size bound.** 1,024 statements is a guess with headroom; stage 1
   measures the converted spreadsheet and files against it.
4. **A Lean theorem for D3**: a body with no cross-frame stale read
   executes the same under snapshot and in-order semantics. It would turn
   D3's justification into a proof. No implementer or date yet.

## 8. Revisions

- **r1** (2026-10-04): first draft.
