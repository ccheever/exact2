# LLP 1089: Action composition

**Type:** RFC
**Status:** Accepted (r3, by the orchestrator under Charlie's delegation after three rounds; Grok 4.7 only — Codex budget exhausted). Each round was Grok 4.7 (xhigh). r1 had two scopes, semantics (`llp/reviews/1089-r1.grok-a.md`) and implementation (`llp/reviews/1089-r1.grok-b.md`); r2 had a delta review (`llp/reviews/1089-r2.grok.md`). All three were READY WITH CHANGES. r3 is the final edit, with no further round, and resolves every finding of r2. The `rules/DEFERRED.md` waiver is recorded by the orchestrator under Charlie's 2026-10-04 delegation ("make decisions without me"). Stage 1 is built as of 2026-10-04 (§9, "As built").
**Systems:** Contract compiler (`contract/{syntax,types,analyze,lower}`, `contract/cli/src/{lean.rs,symbols.rs}`), Lean semantics and difftest (`semantics/`), the JS target's conformance (`host/web-js/conformance`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 built 2026-10-04 (planned 2026-10-05) and stage 2 on 2026-10-05 (§5)
**Amends:** LLP 1017 §11 (the tail call becomes one case of a call); LLP 1006 §2 (statements); `rules/DEFERRED.md` **Actions**
**Related:** LLP 1017 P4c and §11; LLP 1035.005.000 D1 (effects inferred) and D2 (`let`); LLP 1088 D8 (one send per path); LLP 1016 D5; diaries `~/projects/x2apps/{spreadsheet,files,mail}/DIARY.md`. Research only: LLP 0082 §Actions ("actions may call actions"), LLP 0481 §4.1 (effects compose through calls and callback values; call cycles are rejected from the call graph).

## Summary

An action cannot call an action. The one exception is a child's tail call
to an `action` prop, which landed on 2026-10-03 (LLP 1017 §11) and is not
documented anywhere an agent reads. So app-building agents copy statements
between actions. Files writes "open" four times, and one copy kept the bug
a fix to the others removed. Spreadsheet copies "close the editor" into four
actions and "move and follow" into three.

This RFC lets an action call any action in its scope, anywhere a statement
goes, in one commit:

- **Expansion.** The compiler expands each call in place.
  Same-component calls expand before child lifting; prop and inject calls
  expand where tail calls are resolved today.
- **Reads.** They still see the starting state. A slot read that another
  frame's write, earlier on the same path, would make stale is refused.
- **Host commands.** A host command keeps its name.
- **Executors.** The plan, the runner, the JS target and the hosts do not
  change.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | An action calls its component's actions, action props and injected actions, as a statement, anywhere; a host command keeps its name | files F27, spreadsheet F21 | 1 |
| D2 | A call is its callee's statements, expanded in place: one commit, reads see the starting state | — | 1 |
| D3 | A slot read that a call would make stale is refused, on some path | — | 1 |
| D4 | No recursion; a running bound on expansion | — | 1 |
| D5 | Effects come from the expanded body, through every `Call` | — | 1 |
| D6 | LLP 1088 D8 names the calls | — | 1 |
| D7 | Where each kind of call expands | mail F18 | 1 |
| D8 | No executor change; the AST carries the call; existing plans stay byte-identical | — | 1 |
| D9 | Lean gets `Stmt.call`, so difftest checks the expansion | — | 2 |
| D10 | Diagnostics and docs; the tail call documented | all three | 1 |

## 1. Evidence

- **Files F27.** `openItem`, the Enter arm of `listKey`, `openFocused` and
  `openPlace` each set `location`, `remember`, `ahead`, `query`, `sel` and
  `cursorPath` (`files/app.contract:332`, `:269`, `:348`, `:364`).
  `openFocused`'s directory arm still omits `anchorPath = ""`, which is the
  bug the diary describes. One `arrive(path)` helper replaces all four.
- **Spreadsheet F21.** "Close the editor" (`editing`, `inBar`, `firstKey`)
  is copied in `down`, `commitMove`, `editorKey` and `openSheet`.
  `followSelection(r, c)` is a clean extract. `clipKey` calls `move` from
  sequential literal arms, which D3 must accept. A call does not merge the
  sends: `down` and `openSheet` send `committed`, `commitMove` sends
  `edited`, and LLP 1088 D8 keeps them apart.
- **Mail F18 needs no new call.** Mail was built at `c7bb6ab9d`, which
  already carried `inline/tail.rs`. Its swipe can be written as a row-local
  `dx`, reset before a last `if`, with `archive(id)` last in the arm. That
  is the tail call. The agent did not find it, because the docs
  (`contract-for-agents.md:238`, `:580`; `contract-for-humans.md:279`;
  `contract-grammar.md:169`) all say an action calls no action.
- **Probes** (`contract build --json` at `6cfb736a8`):
  - a root action calling `bump()` gets `type-unknown-command`;
  - a child calling its `go: action` prop before its last statement is
    refused as "only as its last statement";
  - an injected `go: action` can be bound with `press=go(1)`, but calling
    `go(1)`, even last, gets "not a host command" (`inline.rs:488` marks
    only `c.props`);
  - `action setScheme` calling the host command `setScheme(scheme)` builds
    (Caltrain, `apps/caltrain/app.contract:109`; files' `action share`
    likewise).
- **The machinery exists.** `tail::resolve` (`inline.rs:277`) splices a
  callee's statements into the caller, renaming every binder apart, and
  refuses cycles and arity mismatches. LLP 1088 D8's walk
  (`analyze/src/sends.rs`) already reads the resolved body.

## 2. Decisions

### D1 — An action calls any action in its scope; a host command keeps its name

A statement `name(args)` in an action body is:

- **a host command** when `name` is in `HOST_COMMANDS`. That is today's
  rule, unchanged. The list moves from `types/src/checks.rs:744`
  (`pub(super)`) into `contract-syntax`, which depends on nothing. The
  expander (`inline/calls.rs`) consults it before it looks for an action,
  a prop or an inject, and the type checker reads the same list. Without
  that, Caltrain's wrapper would expand into itself and be refused as
  `syntax-call-cycle`.
  `action setScheme` may still call the command `setScheme(scheme)`, and
  `press=setScheme` still binds the action. An action named like a host
  command can be bound but not called. There is no ambiguity refusal.
- **a call** otherwise, when `name` is an action of the same component, an
  `action` prop, or an `inject`ed name of type `action`.

A call may stand wherever a statement may: first, between assignments, in
any `if` or `match` arm, more than once.

- **Arguments** complete the callee's parameters, after any arguments
  curried where the action was passed (`close=dismiss("photo")`). No event
  payload is appended. A callee that takes one (`flip(id, checked: bool)`)
  is passed it explicitly.
- **No value.** A call is a statement. `let x = save()`, and a call inside
  an expression, are refused. Compute values with `fn`.
- **First-order.** An argument may not be an action.
- **Scope is unchanged.** A child reaches a parent's or the root's action
  only through a prop or an inject (D7).

### D2 — A call is its callee's statements, expanded in place

The compiler replaces each call with the callee's statements. This is
LLP 1017 §11's meaning moved from the tail to any position:

- the parameters become `let`s of the arguments, evaluated where the call
  stands;
- the callee's binders are renamed apart with `tail.rs`'s hygiene;
- its own calls expand in turn.

**One commit.** Caller and callees form one action, with one commit and one
rollback. A trap anywhere refuses the whole commit.

**Reads see the starting state.** Assignments land in statement order, and
the last wins. An answered send counts as a write before every assignment
(`commit.rs:543–549`; `runAction_last_write`, `Axiomatic.lean:509`).
`runAction_reads_prestate` (`:535`) is the read rule, and the tail call
shipped under it. So a callee does not see the caller's pending
assignments. Letting it see them was not chosen:

- a read's meaning would depend on which body it is written in;
- moving statements into a helper, the refactoring this RFC exists for,
  would silently change their meaning, and so would the shipped tail call;
- React, the model most agents bring, has the same snapshot.

### D3 — A slot read that a call would make stale is refused

Inside one body, a snapshot read is visible on the page (the docs'
`doubled = count * 2`). Behind a call, the write and the read are in
different bodies, and neither page shows the read is stale. It is refused,
not warned of, as LLP 1088 D8 chose.

**Frames.** The action is the outermost frame. Each `Stmt::Call` opens a
new frame, so two calls to one action are two frames. A call's arguments
belong to the callee's frame and are read before its body.

**Reads and writes are slot loads and slot stores**, on the expanded body
after capture conversion and child-derive substitution:

- **A read** is a reference to a state, mutation or router slot (row
  slots included). It can appear anywhere an expression stands:
  - assignment and `let` right-hand sides;
  - conditions and `match` subjects;
  - member receivers (`cell.n` reads `cell`);
  - templates;
  - command, send and call arguments;
  - `map`/`filter` arrows.
- **Not reads:** a name that is still a derive or a resource, and
  `pending(m)`. These read settled values that no statement in the body
  changes: `LoadDerive` reads `env.derives` (`vm.rs:574`), and the
  pending map changes after `vm::eval` returns (`commit.rs:464`, `:557`).
  `failed` does not take a mutation (`type-failed-argument`). A capture
  parameter is a parameter, not a slot, and a prop is read through one,
  as React's props are a snapshot.
- **A write** is an assignment. A send is not a write. Its answer lands
  before every assignment, so a later read of the mutation slot sees the
  starting value either way. An assignment to the mutation slot is a
  write.

**The rule** (`analyze-call-stale-read`): a read of slot `s` in frame F is
refused when some path to it writes `s` in a frame other than F.

**The walk is `sends.rs`'s.** It records each write with its frame, span
and guard, and refuses a read whose guard is not `exclusive` with a
recorded other-frame write of the slot. It keeps the rest of that walk:

- arms start from the incoming state, and the union is taken after;
- sequential `if`s that test one unchanged name against disjoint literals
  are exclusive (`sends.rs:11–15`, `exclusive`);
- `changed()` forgets a fact when its name is written.

Entering a `Stmt::Call` changes the frame and resets nothing else.
A name bound by a `map`/`filter` arrow parameter, a `match` binding or a
`let` is not a slot read. An arrow parameter or a `match` binding may
share a slot's spelling (`lists.rs:88–95`, `actions.rs:272`). So the walk
keeps a binder scope, as `actions.rs` does, and skips the names in it.
`map(xs, count => count + 1)` reads the item, not state `count`.

| | |
|---|---|
| `sel = next; follow()`, where `follow` reads `sel` | refused |
| `sel = next; follow(sel)` (the argument is in `follow`'s frame) | refused |
| `reset(); count = count + 1` | refused |
| `move(1, 0); move(0, 1)`: both read the starting cell, and the later write wins, so this is not a diagonal step | refused |
| `if k == "ArrowDown" or k == "Enter"` → `move(1, 0)`, then `if k == "ArrowUp"` → `move(-1, 0)` | accepted |
| `a = b; b = a`, and files' `location = to; back = remember(back, location)`, in one frame | accepted, as today |
| a child reading a prop after calling its owner | accepted |
| `message = ""; showStatus()`, where `showStatus` reads the derive `status` over `message` | accepted: a derive is a settled value |

The message states the snapshot without assuming which value the author
wanted:

> `follow` (called at line 44) reads `sel`, which `enter` assigns at line
> 43; `follow` sees the value `sel` had when the action started. Pass the
> value it should see: a `let` bound before line 43 keeps the starting
> value, and the value assigned at line 43 gives the new one.

### D4 — No recursion, and a running bound on expansion

- **Cycles are refused**, whether direct or through props and injects
  (`syntax-call-cycle`, renamed from `syntax-tail-cycle`, with the path
  `a → b → a`).
- **The bound.** Expansion counts every statement node it builds,
  `if`/`match` arms and their contents included. It returns
  `syntax-call-size` as soon as one action's running total passes 1,024,
  naming the action and the callee being expanded. A chain that calls the
  next action twice therefore never allocates the exponential body it
  refuses. Measured by statement-like lines: files' largest action is
  about 80, spreadsheet's 48 actions total about 304, and mail's largest
  is 26.

### D5 — Effects come from the expanded body, through every `Call`

`actions.writes` is the VM's allowlist (`lower/src/lib.rs:374`). The VM
traps a store or send outside it (`vm.rs:831`, `:860`). So everything that
reads effects recurses into `Stmt::Call`'s `body`:

- `Action::effects()` (`ast.rs:515`), which then gives every caller the
  union of its own writes and its callees' (LLP 1035.005.000 D1's
  "accounted for statically");
- `sends::check`;
- `check_mutation_then` (`analyze/src/lib.rs:229`), which also moves from
  the authored `c.actions` to the expanded actions, so a `then` action that
  calls a sender of its own mutation is refused;
- `contract symbols` (`cli/src/symbols.rs:348`), which computes writes
  and calls from the expanded action, so `symbols --name enter` answers
  "what does Enter touch". It reads the authored component today. Its
  own statement walk (`fn stmts`, `:595`) is a separate match from
  `effects()`. It gains a `Call` arm that records the callee as a
  reference and walks `body`. `--name` for a root action reads the
  expanded action of that name. A child's expanded name is `name#n`
  (`inline.rs:42`), so its entry is listed under the authored name.

Expanded statements keep the callee's spans, so the source map and `perf`
point at the lines that do the work.

### D6 — LLP 1088 D8, through calls

D8's walk is unchanged, except that it enters `Call` bodies (D5).
`commitEdit(); openSheet(id)`, where both send `saved`, is refused, and so
is `save(); save()`. The message names the frames:

> `enter` sends `saved` twice on one path: in `commitEdit` (called at line
> 41) and in `openSheet` (called at line 42).

### D7 — Where each kind of call expands

**Pass order.** Today the type pass checks each authored child
(`check_children`, `types/src/lib.rs:1209`), then expands
(`expand_all`, `:1217`), then checks the expanded root (`check_root`,
`:1225`). On the authored body, a sibling call or a non-tail prop call is
still a `Stmt::Command`, and `check_command` refuses it before any `Call`
exists. Probes confirm it: `inc()` and a `cb()` followed by a write are
both `type-unknown-command` today. So:

1. **Before `check_children`,** `contract_syntax` expands same-component
   calls in every component, the root included (`expand_calls(file)`).
   `check_children` then checks bodies that hold `Stmt::Call`.
2. **Inside `expand_all`,** after lifting, prop and inject calls are marked
   and expanded at every position. `check_root` then checks the lifted
   actions with every call resolved.
3. **In the child-scope check,** a prop or inject call has no body yet. It
   is checked as a call by its arguments only (D8), against the prop's
   or inject's declared `action` type. That check accepts a sibling call
   and a non-tail prop or inject call.

**`subst_stmts` gains a `Call` arm** (`inline/subst.rs`). It substitutes
`Call.body` like any block: state targets to `name#n`, child derives,
captures. It does not copy the body through as it copies a command's name
(`subst.rs:305–308`). The lifted, substituted body is what D3, D5, D6 and
lowering read.

**Same-component calls expand before lifting, in that component's scope.**
Root actions call root actions by their own names. In a child component,
the call is resolved among the child's authored actions, before `lifted`
renames them `name#n` (`inline.rs:42`, `:447`). The child uses `tail.rs`'s
hygiene: the caller's parameters and binders and the callee's are renamed
apart.

- Lifting then substitutes the already-expanded body once, so caller and
  callee share one capture list. No capture parameters are prepended.
  (r1's rule is deleted.)
- A child's derive in the callee is substituted with the rest of the body,
  as a slot expression.

**Prop and inject calls expand after lifting, where tail calls are
resolved today.** The marking loop (`inline.rs:483–524`) walks every
command at every position, including commands that came in through a
same-component expansion. It chains `c.props` with `c.injects`. A called
prop or inject must name an action (`go = bump` or `go = bump(x)`);
anything else is `syntax-call-target` (renamed from `syntax-tail-call`).

**The owner's statements.** A prop call runs the lifted owner action:

- in the owner's scope, with its curried prefix held as captures, exactly
  as the tail call resolves it;
- with the frames in force at the child's view, so row and arm slots
  resolve as they do for a tail call.

A child never writes its parent's state. It runs the owner's action, in the
same commit, the way a React prop callback works.

### D8 — No executor change; the AST carries the call

**The node.** A resolved call becomes `Stmt::Call { action, args, body,
authored, curried, binding, span }`:

- `body` is the callee's renamed-apart statements, with its parameters as
  leading `let`s;
- `authored` and `curried` count the source arguments and the arguments
  curried at the binding;
- `binding` is the span of `go=move(id)` (or `provide go = …`).

It replaces the `@tail:` marker and the `@check:` leftover.

**The argument cut.** A call's source arguments are checked against the
callee's **authored** parameters. On a lifted action, skip the leading
`@capture:` parameters first (`inline.rs:459–481`, `:602–606`), then skip
the curried prefix. A same-component call has `curried = 0`. The lifted
re-check in `check_root` uses the same cut. Today `lifted: true` only
skips the `let`-shadow check (`actions.rs:103`).

**Refinement.** `refine_params_from_view` (`component.rs:434`) walks only
view handlers (`:468–515`). A new `refine_params_from_calls` walks every
action body's `Call`s, with the same cut, in the same phase: after
`refine_params_from_view` and before `check_body` (`:204`, `:213`). A
helper written `action move(dr, dc)` and called only from `clipKey` is
then typed by its calls. It refines once, in declaration order. A type
learned later does not flow back, because LLP 1088 §9.2's deferral stands.
The type pass does not check `body` again.

**Lowering** compiles `Call.body` as its own block: its `let`s (the
parameters and the callee's locals) drop before the next statement
(`lower/src/stmts.rs:49–52`). Lean's `compileBlock` drops a `let` the same
way (`Lower.lean:407–410`). "In place" means those instructions stand where
the call stood, with no call opcode. Existing tail calls are last in their
block, so a nested block and today's splice emit the same drops.

**Existing tail calls stay byte-identical.** Plan action parameter names
are strings (`plan/src/builder.rs:507`). So the resolver renames a
caller's parameters exactly as `tail.rs:82` does today (`name@c{k}`, with
`k` drawn in the same order), and emits the same argument `let`s and the
same body with no extra opcode. An action with no call is left untouched,
as `tail.rs:77` leaves it.

**Below the compiler, nothing changes.** The plan format,
`FORMAT_DIGEST`, the runner, the JS target and the hosts stay as they are.
The JS target compiles from the plan (`host/web-js/src/main.rs`).

### D9 — Lean gets `Stmt.call`, so difftest checks the expansion

`contract lean` emits the expanded root, so both sides of difftest run the
Rust expander's output, and a renaming bug in it would not show. So:

- **Syntax.** `Stmt` gains `call (action : String) (args : List Expr)`.
  `args` is the callee's full parameter list: the caller's
  already-bound capture parameters and curried prefix, then the source
  arguments.
- **Semantics**, defined independently of the Rust renaming. The
  arguments are evaluated in the caller's environment against the action's
  starting slots. The callee's body then runs in a fresh local scope with
  only its parameters bound:
  - against the same starting slots;
  - under the same frames, so row writes land in the caller's rows;
  - threading the whole effect record: root writes, row writes, sends,
    commands and refreshes.

  If the callee refuses, the record is dropped and the action refuses.
  `exec` gains a depth bounded by the number of actions, which an
  accepted, acyclic program never exhausts.
- **Proofs** gain the `call` case:
  - `Big.lean`: sound, complete, deterministic;
  - `Axiomatic.lean`: a call rule, with `runAction_reads_prestate` and
    `runAction_last_write` unchanged;
  - `Soundness.lean`;
  - `Lower*.lean`, whose `Lower` expands `.call` as Rust lowering does,
    in a fresh local scope.
- **Emitter.** `lean.rs` emits `Stmt::Call` as `.call action args`, never
  `body`. The `Apps/` embeddings of tail-call users are regenerated.
- **Difftest.** `semantics/corpus/actions/calls/` covers:
  - a sequence, a call in an arm, and two calls;
  - a chain;
  - a same-component call in a child;
  - a prop call inside a row;
  - an inject call;
  - a same-spelling local in caller and callee;
  - a trap in a callee rolling back the caller.

  The random generator emits calls from action *i* to actions *j < i*.

### D10 — Diagnostics and docs

**Diagnostics:**

- **`type-call-arity`**, from the `Call`'s counts and the binding's line
  (a `Span` carries no text). For example: "`move` takes 2 argument(s)
  after the 1 curried at line 12, given 1"; a same-component call gives
  "`move` takes 2 argument(s), given 1".
- **`type-unknown-command`, for a name that some other component
  declares as an action.** The checker searches every component before
  answering: "`archive` is an action of `App`, not in `Row`'s scope; pass
  it as an `action` prop or `provide` it". A name no component declares
  keeps today's "not a host command" text.
- **`type-call-value`**: "a call is a statement and returns nothing;
  compute values with `fn`".
- **The rest:** `type-call-action-arg` (D1); `analyze-call-stale-read`
  (D3); `syntax-call-cycle` and `syntax-call-size` (D4);
  `syntax-call-target` (D7); D8's frame message (D6).

**Docs.** The grammar's production already admits the statement
(`contract-grammar.md:154`), so no syntax changes. The prose changes:

- `contract-grammar.md:169` ("a standalone call statement is a host
  command") and `:625` ("after tail calls are inlined");
- `contract-for-agents.md:238`, the table row at `:580` ("Copying an
  action's statements → call it: `arrive(path)`"), and §State and action
  semantics, with D3 and its `let` pattern;
- `contract-for-humans.md:279`.

The §Composition text shows the tail form explicitly. A child resets its
own state, then calls its `action` prop last in an `if` arm. That is
mail's swipe. `agent-pitfalls.md` gains "a helper sees the state the
action started with; pass the value".

## 3. Effect on each implementation

| | Stage 1 | Stage 2 |
|---|---|---|
| syntax | `HOST_COMMANDS` moves here; `inline/tail.rs` → `inline/calls.rs`: `expand_calls(file)` for same-component calls, run before `check_children`; `subst_stmts`'s `Call` arm; prop/inject marking at every position after lift; `Stmt::Call`; running size count; cycle and target refusals; `effects()` recurses | — |
| types | `expand_calls` before `check_children`; argument checks against authored parameters (skip captures, then curried); `refine_params_from_calls` before `check_body`; arity from the `Call`'s counts; value and action-argument refusals; the cross-component search for `type-unknown-command` | — |
| analyze | `calls.rs`, D3's walk on `sends.rs`'s guards; D8's frame message; `check_mutation_then` on the expanded body | — |
| lower | `Stmt::Call`'s body compiled as its own block | — |
| cli | `symbols`: expanded action, a `Call` arm in `fn stmts`; `lean.rs` emits `body` | `lean.rs` emits `.call` |
| plan, runner, JS target, hosts | none | none |
| Lean, difftest | — | `Stmt.call`, the effect record, `exec` depth, proofs, corpus, generator |
| JS conformance | — | `host/web-js/conformance/calls.contract` (async lane) |
| docs | D10 | the semantics README's `Stmt` row |

## 4. Tests

- **Compiler**: `contract/cli/tests/it/tail_call.rs` becomes `calls.rs` and
  keeps every tail case. It adds:
  - calls first, in the middle, in an arm, and twice;
  - a chain of three;
  - a same-component call inside a child, used in a row (today refused
    as `type-unknown-command` by `check_children`);
  - a prop call before the child's own writes (`tail_call.rs:174`'s case,
    today refused);
  - a call followed by a `let`, where the later local takes the callee's
    freed index;
  - `map(xs, count => count + 1)` after another frame writes state
    `count`: accepted;
  - an inject call;
  - curried and payload arguments;
  - an untyped helper typed by its calls;
  - `symbols`.

  `diagnostics.rs` asserts each new message whole, D3's table among
  them, and `then` self-send through a call. A Caltrain-shaped
  `setScheme` stays the command.
- **Plans unchanged.** Decode, on the base and on stage 1:
  - every in-repo app's plan;
  - two tail-call fixtures from `tail_call.rs`, since no in-repo app
    calls an action prop:
    - `VIEWER`: `close=dismiss`, then `close("swiped")` inside an `if`.
      It covers the caller parameter's `dy@c{k}`.
    - The curried `done=note("ada")` test. It covers the capture `let`s.

  Compare the bytes.
- **Lean**: `lake build` with the proofs; difftest `corpus`, `random --count
  500`, `types`, `lowering` (async lane).
- **Driven**: files' `arrive(path)` and spreadsheet's close-the-editor,
  `followSelection` and `clipKey` → `move`, each rewritten as calls.
  These apps are outside the repo; set `EXACT_APP_DIR`. Their authored
  tests stay green on web and macOS through `scripts/agent.mjs`. The
  stage 1 commit reports their expanded sizes.
- The five checks after each stage.

## 5. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, on
a branch from origin/main. Each commit passes the five checks.

1. **Stage 1, 2026-10-05: the compiler and docs (D1–D8, D10).**
   - **Census first.** Build every in-repo app and every
     `~/projects/x2apps` app. A D3 refusal of an existing tail call is
     fixed in the same commit if it is in-repo, and listed if it is
     outside, as LLP 1088 D8's rollout did. Print the count of same-frame
     reads after writes (§7.1).
   - **Exit:** the §4 compiler tests; plans byte-identical; files and
     spreadsheet converted and driven on web and macOS.
2. **Stage 2, 2026-10-05: Lean and difftest (D9),** with the conformance
   fixture.
   - **Exit:** the proofs build, and the corpus and a 500-case sweep
     agree.
   - **If the `Lower*` `call` case slips,** stage 2 lands the semantics,
     the other proofs and difftest. The lowering theorem is then stated for
     call-free bodies, the restriction is named in `semantics/README.md`,
     and it gets a `QUEUE.md` line. No `sorry` lands.

## 6. Considered and not taken

- **A runtime call** (a plan opcode, a JS function call). It costs a
  plan-format change, a VM frame, row frames for a lifted callee, both
  executors and Lean's VM model, for a size problem no app has. Trigger:
  an app hits D4's bound.
- **Callees that see pending writes** (D2): two read rules in one commit.
- **No D3, or D3 as a warning**: silent stale reads, in the very
  refactoring calls invite.
- **`type-call-ambiguous`** (r1): it refused Caltrain's `setScheme` and
  files' `share`, which compile today.
- **Capture prepending for sibling calls** (r1): replaced by expansion
  before lift.
- **A `fn` returning a record that an action spreads** (F21's
  alternative): it cannot send or call a host command, and it needs a
  multi-assignment form.
- **Keeping tail-only and documenting it.** That is the fix for mail
  (D10), but it leaves files and spreadsheet where they are.
- **Return values**: an expression with effects; `fn` computes.

## 7. Open questions

1. **D3 inside one frame.** Not widened by this RFC. The same-frame
   snapshot carries real logic: files' `go`/`goBack`, spreadsheet's
   `er = clampRow(ar + 1)` after `ar = …`, and `resizeMove`. Stage 1's
   census prints the count, which gates nothing.
2. **A Lean theorem for D3.** If no path reads a slot that another frame
   assigned earlier on that path, then loading each slot as the last
   assignment on the path (or the starting value when there is none)
   equals the snapshot load. Derives, resources, `pending` and sends lie
   outside the statement. It gets a `QUEUE.md` line when stage 1 lands.
   No implementer yet.

## 8. Revisions

- **r3** (2026-10-04, final; round 3 of 3): Grok 4.7 xhigh's delta
  review of r2 (`llp/reviews/1089-r2.grok.md`). Each finding was checked
  against the code, and each held:
  - D7's pass order: same-component calls expand before `check_children`,
    and prop and inject calls before `check_root`; `subst_stmts` gains a
    `Call` arm;
  - D8's argument cut skips captures, then the curried prefix, and
    `refine_params_from_calls` walks action bodies;
  - `HOST_COMMANDS` moves into `contract-syntax`;
  - lowering compiles `Call.body` as a block;
  - D3 skips binders;
  - the arity text names a line;
  - the byte fixtures are named;
  - `symbols` gets a `Call` arm.

  The orchestrator accepted it under Charlie's delegation.
- **r2** (2026-10-04): two Grok 4.7 xhigh reviews of r1, one family with
  two scopes. Every finding was checked against the code and every one
  held. Each is resolved in the decision it names, and the dispositions are
  in `llp/reviews/1089-r1.grok-{a,b}.md`. The DEFERRED waiver is recorded
  in its own commit.
- **r1** (2026-10-04): first draft.

## 9. As built

**Stage 1, 2026-10-04** (branch `fix/impl1089`). D1–D8 and D10 as decided,
with these refinements:

- **Three expansion steps** (`contract/syntax/src/inline/calls.rs`).
  `expand_file` expands same-component calls before `check_children` and
  again, as a no-op, inside expansion (D7.1). `resolve` expands prop and
  inject calls after lifting (D7.2), at every position, a callee of the
  same component's body included. `hygiene` renames each caller's own
  parameters and binders apart (`@c{k}`, `@b{k}`, drawn in `tail.rs`'s
  order) after `check_root`. The type pass therefore reads the author's
  names, so `type-let-duplicate`, `type-let-shadow` and
  `type-let-before-declaration` still speak in them in a calling action.
  Each step draws its own letters (`@s`/`@v` before lifting, `@t`/`@u`
  after), so no step's name spells another's.
- **A call's body is closed over its caller's names.** Only the arguments,
  and the parameters' `let`s that bind them, are the caller's expressions.
  A substitution of the caller's scope stops at the `let`s, and lifting
  carries the component's own names into the body.
- **Captures are arguments.** A same-component call in a child passes the
  instance's captures first and binds them again in the body, so `args` is
  always the callee's whole parameter list (D9) and the body's leading
  `let`s are always `args.len()`.
- **`symbols`** reads each component with its own calls expanded for
  `writes` (a prop or inject call writes another component's slots). A
  statement call refers to its callee. The `Call` arm walks the arguments
  only: an authored tree never holds one, and the body is the callee's own.
- **D3's message.** For a write in a callee, the advice's line is that of
  the call, the caller's own statement before which a `let` can stand.
- **The bound** counts as specified. The first refusal in declaration order
  is the first action to pass it.

**Census** (`contract build`, base `423e4c4bc` and stage 1). All 33
in-repo apps and all 32 `~/projects/x2apps` apps build on both, and D3
refuses none. No in-repo app needed a fix. Same-frame reads after a write
(§7.1): 85. That is 25 in 8 in-repo apps (messages 10, expose 5, realworld
3, messages-legacy 2, markdown-stress 2, textflow, reflow and fieldnotes 1
each) and 60 in 11 x2apps apps (files 15, studio 13, reader 11, spreadsheet
5, feed 5, flashcards 3, shop, jukebox and calendar 2 each, habits and calc 1
each). The count gates nothing.

**Plans unchanged.** Byte-identical on base and stage 1: all 33 in-repo
plans, `VIEWER` (its `dy@c1`) and the curried `note("ada")` fixture, and
every x2apps plan.

**Expanded sizes** (statement nodes): spreadsheet's largest action is
`down` at 24 (25 with `commitEdit()`), and its 57 actions total 299 (58
and 305 with `commitEdit`);
files' largest is `listKey` at 74, total 372; mail's is `recipientKey` at
26.

**Driven.** A scratch copy of spreadsheet with "commit the edit" (`down`,
`openSheet`) as `commitEdit()` passes its 15 web tests through
`scripts/agent.mjs`, plus one that clicks another cell mid-edit (16 of 16).
Files' `arrive(path)`, the rest of spreadsheet's copies and the macOS drive
are the apps' own changes, outside this repo.
