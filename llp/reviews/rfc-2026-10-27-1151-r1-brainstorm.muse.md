> **Panel provenance.** Family: Muse Wonder 1.0 (xhigh) · Provider and runtime: `muse exec --prompt-file … --model muse-wonder-1.0 --reasoning-effort xhigh --user-input-auto-resolve` · Date: 2026-10-27 · Redacted: no · Method: brainstorm, not a review: blind (shown no other panelist's output), one panel seat, given LLP 1151 r1 (sha256 `9a6314603673f3fd84041ea77d82cd47a1dc757ba61a3c4e63fa50105e335eec`), James Ide's "A verifiable Contract" and read access to the worktree at origin/main `8233879705`; asked for six to ten ideas that make a Contract app easier, cheaper and more natural to verify, each with what the author writes or runs, what it buys, what it costs and where it fits. The text below is the panelist's final message, unedited.

# Brainstorm: making a Contract app easier, cheaper, and more natural to verify

Panelist statement, read-only. Best first. “Searched” below means checked on executed trajectories; “proved” means a Lean theorem; “checked” means the Rust checker. No idea upgrades one word into another.

---

## 1. Twin-shadow properties: a property can wound only itself, never the app

**What the author writes or runs:** The RFC's test-file blocks, unchanged:

```
always "the phone is on one of its five screens"
  ["lock", "home", "settings", "display", "messages"].includes(screen)
```

`contract verify apps/typetour/app.contract` runs the app twice: once exactly as today (its tests, explore script, and walks, with no property anywhere in the program), and once as a shadow copy carrying each `always` as the RFC's hidden `@always:<n>` derive. Only the shadow runs under Lean, when Lean is present.

**What it buys:** The same searched claim as RFC stage 1 — the property held at every commit the search reached, on the runner and, when Lean ran, in the semantics — plus two separations the RFC's single-program design loses. First, the app's own verdict is computed on the app as written: a non-total property (`items[0]` on an empty list) produces `PROPERTY-UNEVALUABLE at step 3`, not a refused app commit, and the app's tests still pass or fail on their own. Second, any slot, derive, or outcome line where main and shadow disagree (excluding the `@always:` lines themselves) is reported as `PROPERTY PERTURBS THE APP` — settlement fuel and refusals are exactly where a hidden derive stops being free, and this detects it instead of assuming it away.

**What it costs:** The RFC's compiler entry that injects `@always:` derives (needed either way), plus alignment logic: run main and shadow `Case`s through the existing batched `check()` in [lib.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/lib.rs), split both observations into steps with the chunking `expectations` already uses, and compare. Roughly 80 lines there, 120 in [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs), no Lean changes. Runner cost doubles (milliseconds); Lean cost is the RFC's, not doubled, since only the shadow needs the semantics. Main risk: after a shadow refusal, later shadow state is meaningless — so property checking stops at that step, as the RFC does, but the app's run continues.

**Where it fits:** A material change to RFC D2 inside stage 1. D2 argues “then nothing else is new,” but its own cost list admits the property changes the app's refusals; that conflates a finding about the property with a finding about the app, and pollutes the differential comparison the harness exists to make. The twin keeps D2's one real advantage — both existing evaluators check the property, no third evaluator — and drops the conflation.

---

## 2. A refusal and poison census, on by default, no property needed

**What the author writes or runs:** Nothing new. Every `contract verify` ends with:

```
refusals over 23 cases, 412 commits (runner reasons; Lean agreed wherever it ran):
  tap "save" (apps/shop/app.test.contract:14:3)  refused 3/20 walks  # range(-1): a count is a whole number of at least 0
  clock +60000                                   poisoned 1        # render: …
all other dispatched actions committed at least once; "share" was never dispatched.
```

**What it buys:** The most common agent failure is silent: a tap refuses, the state does not change, and a weak `expect` still passes, so the diary records “works.” `check()` in [lib.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/lib.rs) currently discards this: `compare` strips every `#` reason line, and agreeing outcomes keep no observations at all. Surfacing them turns “nothing visibly happened” into a named, located finding, and gives `never refuses "a"` its honest searched precursor for every action, not only named ones. Claim: about the trajectories actually executed, on the runner, cross-checked against Lean wherever the semantics ran. It is not D3 and does not claim an action cannot refuse.

**What it costs:** Retain the runner's observation lines for agreeing cases when (only when) the caller is `verify` — a second entry point beside `check`, not a change to its verdicts — then aggregate `outcome refused|poisoned` chunks in [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs): step, event, the following `#` reason, and a count. Attributing a step to an action name is the stretch goal (via [observe.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/observe.rs) `site` plus the plan's handlers); event-plus-reason ships first. ~120 lines plus a unit test on canned observation text. Main risk: runner and Lean reason strings legitimately differ, so the census must count runner reasons only and never string-match across sides.

**Where it fits:** A cheap stage-1 addition that changes RFC D3/D4's default. The RFC reports refusals only for actions named in a `never refuses` block and reports unexercised actions only for those blocks. Silence should itself be a reported result; otherwise the cheapest verification an agent gets is biased toward false confidence.

---

## 3. Frame properties: “this action changes only that,” mined from observations, stated in one line

**What the author writes or runs:** `verify` prints what it observed, per dispatched action and for the clock; the author keeps lines by copying them into the test file:

```
observed over 412 commits, not proved:
  only "rotate" writes turns
  never clock writes counter, note, live

always "…"   (state properties, as in the RFC)
```

Kept lines are then checked on every later run, and the two above are exactly the shapes of two of the twelve hand-proved theorems (`rotate_only_turns`, `advance_keeps_user_state`).

**What it buys:** A searched frame claim — over every reached commit, no step attributed to another action (or to a clock advance) changed the named root slots — computed by diffing successive `slot` lines in the observation the harness already prints. No expression evaluator is needed: slot lines are canonical text on both sides. And this is the one property class whose proof tier already exists and is mostly `decide`: `runAction_untouched`, `BodyKeeps.untouched`, and `Reachable.advance_untouched` in [Invariant.lean](/Users/ccheever/projects/exact2-wt-verify/semantics/Contract/Invariant.lean). A kept frame line is therefore both the cheapest searched property and the most plausible first generated proof.

**What it costs:** Two pieces in the harness (~200 lines). Diffing step chunks is easy; attribution is the work — a `tap "x"` must be resolved to the action its handler names, by extending [observe.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/observe.rs) with an `action_at` lookup (boot, replay, `site_of`, read the plan node's handler), and clock steps are attributed to `clock` as a class, not to individual tasks. Mutation slots need care: a send's answer landing during a clock advance is a write by the clock in this vocabulary, so mined suggestions should exclude queue mutations (the same restriction `Reachable.slotIn` carries) and say so. The test-file grammar gains two small block forms in `contract/syntax` only if kept lines are to be checked; the mined table alone needs no syntax.

**Where it fits:** Extends RFC stage 1 in a direction D1–D5 do not cover: D5 mines only single-state value summaries, and `always` cannot say “nothing else changed.” Frame claims are what app diaries actually assert in prose (“rotating only affects the angle”), they fail informatively (naming the step that wrote the slot), and they route directly into the strongest existing proof machinery. Root slots only in the first version: the runner's observation skips child-owned slots, and pretending otherwise would be dishonest.

---

## 4. A digest-bound verify receipt for the diary

**What the author writes or runs:**

```
contract verify apps/caltrain/app.contract --walks 20 --worlds 4
```

and pastes the printed block into the app's diary, untouched:

```
verify receipt — reproduce: contract verify apps/caltrain/app.contract --walks 20 --worlds 4 --walk-seed 7
  app sha256:9f2c…  tests sha256:41ab…  embedding sha256:77e0…
  searched: 31 cases, 1,204 commits; actions committed 14/17 (never: delete, share, archive);
    clock actions 3/3; distinct oracle answers 212
  properties: 3 held, 0 violated, 1 unevaluable; refusals: 2 (census above)
  world: seeded oracle answering synchronously and never failing; empty store plus
    4 seeded stored variants; events are tap/type/clock only; no reload, no host inputs
  claim: searched, not proved, about the Rust runner; Lean agreed on 29/31 cases,
    2 outside the semantics (geometry)
```

**What it buys:** This answers the second half of the question — the person reading a diary. Today a diary says “verified” and the reader must trust the agent's prose. A receipt makes the claim checkable: if the source digest no longer matches the tree, the receipt is stale on its face; if it matches, one command reproduces it. Coverage is stated as denominators (actions committed out of actions declared, clock actions out of tasks), which is the difference between “no violation found” and “the violating screen was never reached.” The world line is not boilerplate; it is the claim's boundary, printed where the claim is quoted.

**What it costs:** Aggregation over data `verify` already computes once ideas 2–3 retain observations: counts of cases, commits, dispatched actions (against the plan's action list), and distinct oracle answers (the transcript in [oracle.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/oracle.rs) already records them). Hashing the app and test files that [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs) `test_files` already enumerates. ~150 lines, no new files in the repo, no Lean. Main risk: a receipt nobody re-runs becomes decoration — mitigated, not solved, by the digest: staleness at least becomes mechanically visible.

**Where it fits:** A material change to RFC D4. D4's report is prose warnings inside one run (“not exercised,” “without Lean”); a receipt is an artifact designed to travel into a diary and be falsified later. It adds no apparatus to the repo — it is output of an existing command — which is the shape `rules/` prefers.

---

## 5. World-varying search: vary the oracle and the store, not only the events

**What the author writes or runs:**

```
contract verify apps/shop/app.contract --walks 20 --worlds 8
```

Each walk is re-run under world *w*: oracle seed derived from the app path and *w*, a seeded device store for persisted states, and the prerendered variant when the app has tab panels. Typed inputs during walks are drawn first from literals in the app's own source and test files, then from observed oracle answers, then from the edge tables.

**What it buys:** RFC D3's walk varies which buttons are pressed but holds the world fixed: [oracle.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/oracle.rs) makes every answer a pure function of (seed, source, arguments), and `verify` derives that seed from the case name — so the same call gets the same answer on every run, forever. A property can pass because the one world searched was kind. Varying the world makes the searched claim quantify, empirically, over legal-shape answers and stored values — the same dimensions `Reachable` quantifies over in a proof, and the dimensions `stored_variants` and `prerendered_variants` in [lib.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/lib.rs) already vary for the corpus but not for `verify`. Drawing typed values from the app's own literals answers the RFC's open question 2 concretely: a search box gets a term the data actually contains, because a test or a handler named it.

**What it costs:** Thread a world seed through `Case` into `Oracle::for_case` (today it ignores everything but the name), build walk cases in [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs) reusing `Oracle::with_store` and the literal tables in [oracle.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/oracle.rs), and mine input literals from the `contract_syntax` AST of the app and its tests. ~250 lines. A hard commit budget must be printed, or worlds × walks × steps explodes quietly. Main risk: legal-shape but semantically impossible answers (an id nothing issued) produce violations that are real-in-the-model and noise on a device — every violation must carry its world seed so it is reproducible, and the receipt (idea 4) must count distinct worlds beside commits.

**Where it fits:** A cheap stage-1 addition that materially widens D3. The RFC's walk explores the app's controls; this explores the app's environment, which is where the semantics' theorems already place their quantifiers.

---

## 6. Let the checker prove the closed vocabularies: mined `enum<…>` edits to the source

**What the author writes or runs:** `verify` prints, and the author optionally applies, a two-line source edit:

```
closed vocabulary, observed 212 commits and every write is a literal or this parameter:
  state screen: enum<"lock", "home", "settings", "display", "messages">
  action go(target: enum<"lock", "home", "settings", "display", "messages">)
apply it and the checker holds the vocabulary; leave it and "screen" stays a string.
```

**What it buys:** For the single most common proved property in the corpus — Type Tour's five screens, whose hand proof exists precisely because `go(target: string)` accepts anything — this converts a 77-line Lean proof into a compile-time refusal. Any literal outside the vocabulary is refused by the Rust checker; a dynamic argument must itself be enum-typed; a stored or source-supplied value outside the vocabulary fails its shape check at restore. The author needs no Lean installed and no proof. Honest limit, to be printed with the suggestion: the Lean side currently reads an enum close to a string (see `Types.lean` on sort keys), so until someone checks what `ValTy` and `conforms` preserve, the proved-in-Lean claim is “a string,” and the vocabulary claim rests on the Rust checker plus the runner's conformance checks — tested against the semantics, not part of `dont_go_wrong`. The suggestion must say exactly that.

**What it costs:** A conservative write-analysis over the `contract_syntax` AST in the compiler's verify entry: for a root state, collect every assignment target across actions; suggest only when each write is a member literal, a copy of the state, or a parameter the suggestion simultaneously retypes — and never when a derive, resource answer, or record field flows into it. ~200 lines in `contract/cli`, no new syntax (`enum<…>` already exists; grammar documents it for finite string parameters), no semantics change for the searched/checker claim. Main risk: closing a vocabulary the server legitimately extends. That is why it is a printed suggestion with its evidence (commit count, write sites), never an automatic edit, and why it must refuse to suggest when any write is not accounted for.

**Where it fits:** A different direction than the RFC, and it contradicts two of its positions deliberately: D1's “an app's source carries none” and §3's dismissal of properties-that-are-types. The dismissal is right for properties in general — a property over two states is never a type — but a closed vocabulary already closed in fact is not a property wearing a type's clothes; it is a type the author had not written yet. The RFC's own friction evidence (four apps, twelve theorems) includes one theorem whose entire content is this vocabulary. Where the fact holds, the checker is the cheapest prover in the repository.

---

## 7. Call-closed “untouched”: make the `decide` tier survive action calls

**What the author writes or runs:** Nothing. This is proof infrastructure: when a later `--prove` (or a hand proof) needs `BodyKeeps` for a slot, bodies that reach writes only through called actions are discharged automatically instead of falling through to manual `wp`.

**What it costs and buys:** In [Invariant.lean](/Users/ccheever/projects/exact2-wt-verify/semantics/Contract/Invariant.lean), `Stmt.noAssigns` and `Stmt.noSends` return `false` at every `.call` rather than look into the callee — the README states this plainly — so any app that factors work into called actions loses the free tier and pays full `wp`-and-`simp` per action. The fix is a write-set computed over the program's action graph: `writesSlot p x a` by recursion with a visited list (cycles answer “may write,” which is sound for any `Program` term; the Rust checker refuses call cycles anyway, so nothing real is lost), a matching `sendsInto`, and one new induction over `ExecR` whose call case applies the callee's result — the `callPre_of_wp` pattern in [Axiomatic.lean](/Users/ccheever/projects/exact2-wt-verify/semantics/Contract/Axiomatic.lean) shows the shape. ~150–250 lines of Lean including the soundness lemmas. It buys the frame properties of idea 3 (and the untouched half of most `slotIn` proofs) for call-using apps, which is most real apps; Type Tour and Update Lab pass `decide` today precisely because their relevant bodies happen not to call. Risk: the visited-list fixpoint must be proved, not just computed — if the induction resists, a fuel-bounded version (fuel = number of actions) with a monotonicity lemma is the fallback.

**Where it fits:** Stage 2, but a deliberately narrow slice of RFC D6 to build *first* and measure. D6 automates the whole recipe (finite sets, options, integer doubles, 2⁵³ rounding); this automates only the tier that is already computational, on the four existing proofs and then on apps chosen blind, before anyone commits to the arithmetic tiers. If the call-closed tier discharges most real frame claims, D6's remaining scope shrinks to what is actually hard.

---

## 8. Search the device world before modelling it: fault and reload steps in `verify`, labelled unmodelled

**What the author writes or runs:** Steps authors already write for the agent driver, now honoured by `contract verify --device-search` instead of skipped:

```
test "a failed refresh keeps the old list"
  fail fetch "items" times 2
  tap "refresh"
  expect refusal "feed"
  reload
  expect state screen == "home"
```

Today [script.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/script.rs) refuses every such step by name, so `verify` prints `SKIP` and the test silently does not exist for verification — while the same test runs for real under the agent driver, whose grammar (`fail fetch`, `pass fetch`, `reload`, `expect refusal` in [docs/contract-grammar.md](/Users/ccheever/projects/exact2-wt-verify/docs/contract-grammar.md)) already defines what these mean on hosts.

**What it buys:** A searched claim about failure behaviour — a failed source keeps the prior value, skips its `then`, surfaces the refusal the test names; a reload restores persisted state and nothing else — checked on the Rust runner and the web JS target, the two implementations that actually ship. Reload-with-store is the exception that can stay modelled: persisted restore is already in the semantics (`Oracle.stored`, `restore`), so that half runs under Lean unchanged. Faults do not: `failed` is always false in the semantics, so fault steps are reported under the explicit heading `device-only, unmodelled` and never merged into a semantics claim. This finds the class of bug the gist's offline example exhibits, at search cost, years before a device-world model could prove anything about it.

**What it costs:** The largest item here. New `Event` variants in [script.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/script.rs), a failure mode in the harness oracle (a matching source call returns `Err`, with the refusal surfacing as the runner surfaces it), a re-boot path in [observe.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/observe.rs) that feeds the printed `store` lines back as the next boot's snapshot, and failure answers in the JS driver's transcript format in [js.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/js.rs). Mapping a `fail fetch` URL prefix to Contract-level sources is imperfect — fetches inside TypeScript/Rust data modules are below the Contract seam and invisible here — so the first version faults named sources and says that boundary in the report. Risk: inventing a fault semantics the hosts do not share; mitigated by checking runner against JS (two independent hosts' behaviours) and by reusing the authored tests' own `expect refusal` vocabulary rather than inventing outcomes.

**Where it fits:** A different direction than the RFC's staging, contradicting its order. The RFC defers everything about the device world to D11 (stage 4) because *proof* needs the model. Search does not: the hosts already implement failure and reload, the test language already expresses them, and the only thing missing is `verify` listening. Findings here are bug reports with reproductions, not theorems — and they are also the demand evidence that tells D11 which failures are worth modelling.

---

## 9. Verify the artifact that ships on the web: `contract verify --js`

**What the author writes or runs:**

```
contract verify apps/caltrain/app.contract --js        # no Lean required
```

The verify cases (authored tests, explore, walks) run through the existing JS lane — `exact-web-js` compiles the app, Bun drives it over the render DOM, observations compare against the runner line for line — and properties, census, and receipt are computed from that run exactly as from a runner run.

**What it buys:** For a web-shipped app, the JS module and its ~20 KB runtime are the artifact a person will actually run; the Rust runner is a second implementation standing in for it. A searched claim about the shipped target, under the harness's DOM and oracle, with no Lean toolchain installed — cheaper to obtain than the semantics comparison and, for web apps, about a more relevant artifact. `difftest corpus --js-only` already proves the machinery on other people's cases; `verify` simply never calls it ([main.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/main.rs) `run_js` exists; [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs) has no JS path). Limits stated on the receipt: Bun's DOM is not a browser, `Math.sin`/`Math.cos` are pinned to the runner's libm answers by the driver rather than checked, and an app `exact-web-js` refuses is `OUTSIDE-JS`, not a failure.

**What it costs:** Wiring, mostly: build the verify `Case` list once and hand it to `js::check`/`js::report`, reusing the shadow cases of idea 1 so properties evaluate in the JS target too. ~80 lines across [verify.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs) and `contract/cli`. Main risk: apps with native-only constructs refuse the JS target often enough that the flag is useful only for web-first apps — which is precisely the population that should use it, and the report names the refusal rather than hiding it.

**Where it fits:** A stage-1 alternative to RFC D4's fallback. D4 says that without Lean, run the runner alone. For a web app that is the wrong lone artifact: the runner is the model double, the JS target is the product. Runner-alone remains the fallback for native apps; `--js` is the natural cheap claim for web ones, and the async lane's `difftest-js` check already pays its maintenance cost.

---

### The through-line

Ideas 1–5 and 9 need no Lean, no new app syntax, and mostly no new syntax at all; they make the searched claim stronger (more worlds, more dimensions, coverage denominators) and harder to misread (census, receipt, perturbation detection). Idea 6 takes the one property class the checker can already prove and stops spending Lean on it. Idea 7 shrinks the eventual proof automation to its computational core before the arithmetic tiers are attempted. Idea 8 refuses to let the device's world wait for its model before anyone looks at it. Each prints, beside every claim, the world it was searched in — because the cheapest verification that misstates its limits costs more than the proof it replaced.
