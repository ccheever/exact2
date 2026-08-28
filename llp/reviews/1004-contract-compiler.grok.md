# Review: LLP 1004 The Contract compiler (grok)

- **Family:** grok (xAI)
- **Provider/runtime:** `grok -p` (headless single-turn), model `grok-4.6`, `--reasoning-effort xhigh`, `--disable-web-search`
- **Method:** llp-super-refine round 1, mutually blind to the same-round codex review (structural: document-embedded prompt, no repository access; the prompt carried the brief, RULES.md, NOT-DOING.md, LLP 1000, LLP 1002 §Summary–§2, and the full target; a terminal ban; instructed to FLAG rather than assert on anything not embedded). One failed launch preceded this one (CLI usage error, no model output; recorded, not counted).
- **Prompt sha256:** 7a9d31fdf36e58ed33e9bb72dbbd2655efd73652c51633b8e575f55b896cd216
- **Target (Review, round 1) sha256:** bb1d64871b248e12aa29744825bde3aa5b8c448a8cea26a2727cbcba3dc3f082
- **Date:** 2026-08-28
- **Verdict:** **READY** (0 MATERIAL, 11 MINOR)

---

## Round 1 — verbatim review

The review needs the full RFC and the truncated binding files. I’ll read the offloaded prompt first so the findings are based on the complete document, not the cut-off excerpt.## Overall assessment

LLP 1004 is a decision document at the right altitude: D1–D6 are real rulings an implementer could not just infer, each with a reason, and §3 correctly leaves parsing, inference, cache, and bytecode to the spec. D2 places `exact-plan` where it belongs (shared leaf of compiler and runner, generated and uncommitted). D4 is the load-bearing call, and the resource-plus-stdlib shape is the one that matches closed expressions, the boot rule, and a fetch-shaped product without closing the React door. D5’s four levers are the right ones; the ≤20 ms figure is labeled provisional rather than treated as a measurement. D6’s exit is well-formed *once it is read with §4* (runner first, then web host, then compiler). What remains is citation and decision-quality cleanup, plus unverifiable exact1 census claims that the packet cannot confirm.

## Findings

1. **MINOR —** D2, heading and table: “Five crates, and cargo is the module guardrail” is immediately followed by a six-row table (`exact-plan`, `contract-syntax`, `contract-types`, `contract-analyze`, `contract-lower`, `contract`). The document title (“Rust, five crates, one table authority”) can be read as five compiler crates plus one table crate, but D2’s own heading does not say that. Resolve by making membership explicit: five `contract*` crates, `exact-plan` as the shared table authority — or say six crates and drop “five.”

2. **MINOR —** D2, “dependency direction is machine-checked by cargo’s no-cycle rule” vs. “A pass reaching backward or sideways is a `Cargo.toml` edit a reviewer sees.” Cargo will catch a cycle on the linear chain `syntax → types → analyze → lower → contract`. It will *not* catch `exact-plan` depending on `contract-syntax`, or `contract-types` depending on `exact-plan`; those are acyclic. The placement of `exact-plan` as a shared dependency of `contract-lower` and the runner is still correct. Resolve by stating what cargo actually enforces (no cycles on that DAG) and that isolation of `exact-plan` from the frontend is review of `Cargo.toml`, not a rustc rule.

3. **MINOR —** D4, “Alternative considered: moving the schedule math into Contract,” while the ruling itself is “no `use … from "*.ts"`.” Those are two forks. The TS-import ban is argued in §1 fact 2 (“`rules/RULES.md` forbids app JS before first pixel, and 0508’s expressions are closed”); the paragraph under D4 only costs Contract-math vs. resource+stdlib. RULES forbids *executing* app JS before first pixel and *transpiling source at boot*, not a build-time TS→bytecode lane the runner would have to admit. Closed expressions (D3) are the actual reason derives cannot call TS. `FLAG`: 0508’s closed-expression claim and 0518 Lanes 1–2 are not in this packet. Resolve by costing the TS-import alternative in D4 (runner grows a JS FFI / island lane; Caltrain keeps `data.ts`) and by citing RULES only for boot/bytecode, not as a blanket ban on app logic that compiled away before launch. The chosen ruling remains sound: it does not close the React door (a second authoring model still lowers to the same plan), and it respects NOT-DOING’s Aquifer and islands exclusions.

4. **MINOR —** D4/§5 vs. §1 fact 2. §1: Caltrain’s “237 lines of Contract call fifteen TypeScript functions.” §5: the port is “a rewrite of `data.ts` into a host-supplied resource plus ~six stdlib functions. Small.” `FLAG`: line count, the fifteen names, and `data.ts` are not in this packet; only three names are given. “Small” is a hoped partition until those fifteen are mapped to resource, stdlib, or existing Contract (`each`/`when`/`derive`). Resolve with that map; it is the evidence D4’s cost — and the runner VM roster D4 gates — can be checked.

5. **MINOR —** D5, “`rules/RULES.md`: dev restart, request to present, 100 ms p50. The four levers, named so ‘fast’ is measured rather than hoped” and “provisionally **≤ 20 ms p50**.” The RULES cell is “Dev restart, request to present,” not “edit → present.” A resident, no-spawn compiler (lever 1) is specifically *not* a restart; the 20 ms number is a new named slice, not a row in RULES. The arithmetic (one slice of a 100 ms present path) is coherent, the levers are the right ones (spawn, recompile, full replace, state reset), and “provisionally… bound when the resident driver exists and measured on the Caltrain app” is the correct epistemic status. Resolve by declaring ≤20 ms as this RFC’s named trade, and by saying what the pre-compiler web host actually measures (plan reload or hand patch → present), so the remaining ~80 ms is a measured remainder rather than a hope.

6. **MINOR —** D5 “the plan patch is the unit (an edit compiles to a diff of tables applied in place — LLP 0500 D1)” against NOT-DOING’s exclusion of “HBC compilation, hot revision surfaces, staged reload.” `FLAG`: 0500 is not in this packet, so identity with those exclusions cannot be checked. The 100 ms loop needs *some* in-place update, and “state survives by declaration identity” is a real runner obligation that §4’s runner-first order must already honor. Resolve with one sentence that plan-patch apply and declaration identity are in the runner’s first landing, are owned as format by `exact-plan` (D2), and are not the excluded HBC/hot-revision/staged-reload apparatus.

7. **MINOR —** D3 “enumerated stdlib roster in JSON” and D4 “stdlib roster entries, implemented once in Rust and evaluated by the runner’s VM” never name the crate. D2 already gives `exact-plan` `opcodes.json`. If those are not the same document, the runner lane that this RFC exists to gate has two authorities for one VM. Resolve by stating that the Caltrain-needed roster *is* `opcodes.json` (or that it is a second file next to it in `exact-plan`), so compiler admission and runner implementation cannot drift.

8. **MINOR —** D6, “Adding a construct is grammar + rules + lowering + one fixture.” That formula omits `contract-types` and `contract-analyze`, which D2’s exhaustiveness story depends on. One accept-fixture per construct is also not enough by D6’s own rule (“a construct with no fixture does not exist”) for `writes` checking, closed types, or diagnostic ids — those are reject rules. The Caltrain exit supplies integration, which is the right complement. Resolve by aligning the add-a-construct checklist with the crate DAG and by requiring a typed-id reject fixture per rule.

9. **MINOR —** D6, “the compiler’s exit is *‘compiling `app.contract` yields the tables the runner already runs.’*” Well-formed at compiler-lane time: §4 builds the runner first and freezes `exact-plan` against hand-built plans. At RFC time there is no runner, so “already runs” reads as circular if §4 is skipped. Resolve with one clause: the exit is evaluated after the runner corpus exists; this RFC’s job until then is to constrain that corpus (D3 constructs, D4 boundary, D5 identities/patches).

10. **MINOR —** §1 facts 1–2. “parser 4.3K, inference 4.5K, analysis 9.3K, lowering 4.2K… ~22K lines inside a 220K-line package”; “the Rust one 13,294 lines, committed”; “Caltrain’s 237 lines”; “fifteen TypeScript functions.” `FLAG`: none of these counts are in the binding files. They are motivational, not invariants of D1–D6; D1 still stands on one build/one generator/exhaustiveness, D4 on the boundary shape. Resolve by dating them as the 2026-08-28 census and not treating the integers as constraints.

11. **MINOR —** D3/D4/D5 measure the language, the JS boundary, and the 20 ms bind against Caltrain, while NOT-DOING still records “**Open decision:** name the app. Recommendation is the Caltrain app.” The RFC does not claim to close that decision, but D4’s roster and §5’s “small” rewrite are app-specific. Resolve by one line: v1 is scoped to the recommended app until the open decision is named.

## Verdict
READY


---

## Round 2 (delta round) — verbatim review

- **Prompt sha256:** 6b645cdedb22dfdac9a5015548e91405080b3f919dd4c742f21abe29bfa0f5a3 (brief + delta instructions + RULES.md + NOT-DOING.md + LLP 1000 + LLP 1001 §1 + the r1→r2 unified diff + the full r2 target; no repository access; terminal ban)
- **Target (r2) sha256:** 1b2f422f0d9113df4bbc748f3692d4823540604b5f7b5f037f92ce2ba6c43314
- **Date:** 2026-08-28
- **Verdict:** **NOT READY** (1 MATERIAL, 5 MINOR)

The review brief was truncated in this message; I’ll read the full offloaded prompt so the review is based on the complete document.## Overall assessment

r2 turns D1–D3, D5, and D6 into real decisions: alternatives are named, cargo is no longer asked to police layering, whole-plan replacement is aligned with `NOT-DOING.md` §Runtime, and the compiler exit is deferred until the runner corpus exists. D4 is still the load-bearing ruling for the runner lane, and the *boundary* (no `*.ts` imports, no expression-level JS, `Runner::supply` as a data seam, roster in Rust, ambient host state as a capability-call) is sound and does not close the React or GPU doors. What does not hold is D4’s own check that the Caltrain inventory is “small”: the Appendix parks parameterized query helpers in “compiled or host-fed data (one `resource`)” in a way that contradicts both “no computation enters, only data” and the cost used to reject “schedule math in Contract.” That is an in-delta decision-quality defect; the rest is altitude-appropriate.

## Findings

1. **MATERIAL —** **IN-DELTA.** D4: “The Appendix maps all twenty helpers to these three buckets; the map is the check on ‘small,’ and it holds until the census is rerun.” §5: “a rewrite of `data.ts` into compiled data, one host-fed resource, four roster entries, and one command.” Appendix row “Compiled or host-fed data (one `resource`)” lists `nearestStationIds`, `searchStations`, `getDirectionBoard`, `getTrainRunById`, and `getSelectedStation` beside genuine data (`getStations`, `getDefaultUserLocation`).

   Those five are parameterized functions (search, nearest-N, filtered board, lookup, selection), not values. D4 forbids computation at the seam (“no computation enters, only data”). Alternative (ii) rejects putting “sorting, distance” in Contract as language growth. The roster row is only formatters (`formatClockTime`, `formatCountdownMinutes`, `distanceSummary`, `walkSummary`). That leaves no legal bucket for the queries. Treating them as host-fed views also recreates alternative (i)’s rejected cost — a per-host implementation of each helper.

   The ruling “expressions call the roster; data is data; nothing else crosses” is implementable as a boundary; the check that it is *sufficient and small* for the recommended v1 app is not. Resolve by rebucketing each of the seven names as exactly one of: compiled table, host-fed resource (and then say whether `supply` may be driven by host-observed app state, and drop “one”), roster opcode, existing D3 construct, or an explicit app rewrite that deletes the helper. If lookup/filter/sort/distance are roster or in-language, retract or narrow alternative (ii). **FLAG:** `data.ts` bodies are not embedded, so the classification is from names and the RFC’s own alt-(ii) list, not from reading the helpers.

2. **MINOR —** **IN-DELTA.** D4: “maps all twenty helpers to these three buckets” vs Appendix’s four rows, and `elapsedMilliseconds` (1 — “plain subtraction; needs no entry”) sitting in “Roster entries.” The three D4 bullets are resource / roster / capability-call; “Already Contract” is a fourth fate, and `elapsedMilliseconds` is that fate. Resolve by counting four fates and moving `elapsedMilliseconds` out of the roster row so §5’s “four roster entries” matches the table without a parenthetical.

3. **MINOR —** **IN-DELTA.** D2 crate table: r2 removes the `exact-plan` row (`depends on: nothing`) while D2 still has `contract-types` and `contract-lower` depend on it, and `contract-lower` also on `exact-kernel`, with kernel vocab “not redeclared.” Cargo-acyclicity vs review-layering is now honestly stated; `exact-plan` as the runner-created shared format crate is the right placement. What the table no longer decides is `exact-plan`’s own deps. If plan tables carry node types or style rows, `format.json` must not copy `schema.json` (LLP 1001 §1: “a second copy of any table anywhere is a defect”). Resolve with one line: `exact-plan` depends on nothing and does not list kernel vocab (stitching only in `contract-lower` and the runner), *or* `exact-plan` depends on `exact-kernel` / reads `schema.json` in `build.rs`. That is a runner-lane input, not a compiler-internal.

4. **MINOR —** **IN-DELTA.** D5: compiler slice “≤ 20 ms p50 for a one-file edit — an initial allocation … not a measurement today,” while incremental work is “the changed module and the closure of modules invalidated by its exported types or component interfaces.” The 20 ms figure is no longer hoped-as-fact; the work it covers is. A one-line type-export change is not a one-file compile. Resolve: the allocation includes the invalidated importer closure, or the 20 ms applies only when exported types/interfaces are unchanged. Same section: “a plan the size of the v1 app is kilobytes, and loading it costs less than a patch format would” is still hoped — the good reason to drop LLP 0500 is `NOT-DOING.md` §Runtime plus “v1 needs neither,” not an unmeasured load comparison. **FLAG:** plan size and load cost are not measurable from this message.

5. **MINOR —** **IN-DELTA / FLAG.** §1 and Appendix census at exact1 `a8e633907`: 237-line Caltrain twin, twenty helpers, 33 `grep -o` call sites, `packages/exact-contract/src` ≈ 220K, parser 4,280 / inference 4,517 / analyze 9,330 / lower 4,252, committed Rust reader 13,294. Declared motivational and not binding, so they do not need to carry D1–D3. They *do* still carry D4’s “small” claim (finding 1). Not embedded: LLP 0508 (§4.3 closed expressions, §9 `shape`, §10.3 `capability-call`), LLP 0517’s pure-call seam, LLP 0518’s three lanes, LLP 0542’s resident shape, LLP 0500’s patch/HotRevision (cited only as not taken). Research citations that justify a chosen or rejected path should be treated as unchecked, not confirmed.

6. **MINOR —** **PRE-EXISTING.** D3 lists `task` (and the rest of the 0508 construct set) “scoped to the v1 app”; the Appendix inventories only `data.ts` helpers, not constructs the 237-line twin actually uses. D6 already says a construct with no fixture does not exist, so unused 0508 surface can die at spec time — but “scoped to the v1 app” is not yet evidenced for `task`. **FLAG:** `app.contract` is not embedded.

## Verdict

D1, D2’s DAG split, D3’s refuse-by-digest, D5’s whole-plan + declaration identity, D6’s accept/reject/interaction corpus, and §4’s authorized process exception are ready to fold. D4’s boundary ruling should stand; its inventory-as-check must be repaired before it gates the runner.

NOT READY


---

## Round 3 — ABORTED (no review received; not counted)

- Launched 2026-08-28 against r3 (sha256 `ffc206190c64e9697c5488134fb6f48a246a000ab0b1494e2a638a83a5f27a1b`) as a final delta round; stopped by the author's instruction ("stop now and build the compiler") before the model responded. No partial output is recorded as a review.

## Close-out (2026-08-28)

- **Final verdicts bind to r2** (sha256 `1b2f422f0d9113df4bbc748f3692d4823540604b5f7b5f037f92ce2ba6c43314`): codex NOT READY (5 MATERIAL, all in-delta; no MATERIAL pre-existing), grok NOT READY (1 MATERIAL in-delta; no MATERIAL pre-existing). Round 1 verdicts (on r1) are superseded by round 2.
- **r3 (sha256 `ffc206190c64e9697c5488134fb6f48a246a000ab0b1494e2a638a83a5f27a1b`) is UNREVIEWED.** It folds every round-2 finding; the dispositions are the r3 text itself (see its `Revised:` line) and, per finding: codex r2 #1 → D3 (three digests); #2 and grok r2 #1 → D4 (Rust data source, request/response seam, build-time constant evaluation, rebucketed Appendix); #3 → D4 alternatives (per-branch costing; 0508 §6.6 acknowledged); #4 → D5 (teardown and reset; no migration); #5 → §4 (recorded as the author's obligation — the RFC cannot edit a rules file); codex #6/grok #6 → §3 (naming the app is a runner-lane prerequisite; constructs listed in the Appendix); codex #7/grok #4 → D5 (unmeasured claims removed; allocation scoped); codex #8 → D3 (only the enumerated constructs are decided; the spec transcribes semantics and deviations); codex #9 → D6 (fixtures also run on the runner); grok #2 → Appendix (four fates plus command; `elapsedMilliseconds` moved); grok #3 → D2 (`exact-plan` depends on nothing, carries no kernel vocabulary); grok #5 → §1/Appendix (numbers dated, method stated).
- Loop terminal state: **author stop after two completed rounds** (budget 3; launches: codex 3 incl. the aborted one, grok 4 incl. one CLI-usage failure and the aborted one). Convergence on pre-existing text was reached in round 2 by both families; in-delta convergence was not tested. Recommendation: the author ratifies or amends D1–D6 directly; the build (the plan lane) is the next instrument, not another round.
