# A binding past the list-step bound poisons the whole runner

**Status:** Closed
**Resolution:** option C, A first: a prop or style binding that traps unsets its row with one journal line and the runner goes on; the compiler warns of a binding in a `for` that walks that `for`'s own list (`lower-row-own-list`). LLP 1017.003 D3, LLP 1090 D6.
**Systems:** Runner (instance update, poison), web JS target (`host/web-js/rt.js`), Contract semantics
**Severity:** P2
**Author:** Claude (Opus 5.5), from the T3 Code clone's timeline drive
**Date:** 2026-10-30
**Related:** LLP 1017.003 D3, LLP 1090 D6, LLP 1005 "Atomicity"

**Problem.** One evaluation may take 65,536 list steps (`MAX_LIST_STEPS`, `runner/src/vm.rs:54`). A prop or style binding past it traps `IterationLimit`, and the trap poisons the runner: every later answer, timer and action is refused, and the app stays frozen until it is relaunched. The bound is per evaluation, so a binding whose cost grows with a list passes every list the app was tried with and stops the app on a longer one. In the clone, a per-token offset (`tokenAt`) summed the tokens before each code token, about 5N steps for the last of N tokens. A 1,500-line coloured code block has about 26,000 tokens, which is past the bound. The journal showed `fulfil N (data) refused: Instance(Trap(IterationLimit { pc: 343 }))`, then `timer 4 (transportPump) refused: Poisoned`.

**Cause.** `NodeInst::emit_bindings` propagates a binding's trap with `?` (`runner/src/instance.rs:963,970`; also `instance/looks.rs:100`). The failed settled update poisons the runner (`runner/src/runner/lists.rs:134`), and `poison` (`runner/src/runner/commit.rs:761`) drops every request, command, pending answer and background job. The web target does the same in `flush`'s catch (`host/web-js/rt.js:207`). This is the accepted behaviour: LLP 1017.003 D3 and LLP 1090 D6 say a trap while the tree updates poisons, and they defer recoverable tree traps. `runner/tests/it/now_screen/bad_data.rs:108-119` pins it.

**Reproduction.** This contract compiles with `contract build`. Its data source answers `items(n)` with `n` rows.

```
shape Item
  name: string

component A
  state n = 1
  resource items = items(n) as shape Item[]
  action grow
    n = 65537
  view
    text items.map(i => i.name).join(",") testId="names"
```

The runner boots. `act("grow")` is refused with `Instance(Trap(IterationLimit { … }))`, the journal says `act grow poisoned the runner`, and every later `act` returns `Poisoned`.

**Fix.** Option C, A first. Option A: treat a trapping prop or style binding as invalid at computed-value time, as Charlie's 2026-09-22 ruling already treats a refused bound value (LLP 1005 "Atomicity"). The row is unset (`ClearProp`/`ClearStyle`), one journal line names the binding's site and the trap, and the update continues. No transaction is needed, because the trap stays inside one binding. Traps in a subject, a key or a row initializer still poison. This changes LLP 1017.003 D3 and 1090 D6, the JS target's emitted bindings, conformance and the semantics' observation. Option B: keep the poison and make the compiler warn when a binding inside a `for` walks the list that `for` iterates (`contract/lower/src/warnings/rows.rs` covers only a lookup into another list). Option C: both. Recommendation: C, with A first.

**Acceptance.** Under A, a runner whose text binding traps `IterationLimit` journals one line, shows the row unset, is not poisoned, and fires its next timer. The runner and the JS target agree under `host/web-js/conform.mjs --strict` and `contract-difftest`.
