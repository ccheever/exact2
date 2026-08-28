# Review: LLP 1004 The Contract compiler (codex)

- **Family:** codex (OpenAI)
- **Provider/runtime:** `codex exec -s read-only`, model `gpt-5.6-sol`, `model_reasoning_effort=ultra` (verified in the run log), fresh session `01a04a16-de67-7b32-81ec-47d840ee4ad3`, workdir = review capsule (an export of the exact2 working tree, 174 files, structurally excluding `llp/reviews/1004*` and every orchestrator note; full read access within it)
- **Method:** llp-super-refine round 1, mutually blind to the same-round grok review (structural: the capsule contained no review artifact for this target).
- **Brief sha256:** a99148d511c577597404bf388db6f46c0e165f5d3d91f7788be0fe39971d481f · **Prompt sha256:** 825d6eb1d84faacce58f41445bbf407f8ae155df5dac7fe952a2df221c2a97c2 · **Capsule sha256:** 6f8b4483b9defb3fe225deaa01882de1e96271390a993b611c8582fb57ebae71
- **Target (Review, round 1) sha256:** bb1d64871b248e12aa29744825bde3aa5b8c448a8cea26a2727cbcba3dc3f082
- **Date:** 2026-08-28
- **Verdict:** **NOT READY** (8 MATERIAL, 4 MINOR)

---

## Round 1 — verbatim review

## Overall assessment

The Rust/shared-plan direction and runner → web → compiler sequencing are coherent; the 20 ms compiler slice can fit the 100 ms envelope, and D4 does not inherently foreclose future React or GPU tiers. However, several binding-rule conflicts and missing decision-level boundaries remain, especially around hot revision, host-supplied data, shared authorities, and verification.

## Findings

1. **MATERIAL —** The status says “super-refine loop opened” and “no implementer yet,” while the Summary decides the lane “before anyone writes it.” This conflicts with `rules/NOT-DOING.md` §Process, which forbids refine loops and says unassigned speculative work is not written; the required §Moving-something-off-this-list trade is absent. Being an RFC avoids the narrower Spec metadata rule, but not this broader prohibition. Resolve by limiting the RFC to decisions immediately required by the runner, deferring the rest until implementation is assigned, or recording the required binding trade.

2. **MATERIAL —** §1 calls Caltrain “the v1 app,” while D3 scopes the language to “the app,” D5 measures Caltrain, and §5 assumes its port. Binding `rules/NOT-DOING.md` §The-bar-that-makes-this-list-derivable says naming the app remains open and Caltrain is only the recommendation. Resolve by making Caltrain an explicit human-confirmed decision or making the language scope, benchmark, and exit independent of that still-open choice.

3. **MATERIAL —** D2’s crate DAG cannot uphold the existing authorities as written. It makes `exact-plan` dependency-free and gives `contract-lower` no connection to `kernel/tables/schema.json`, although LLP 1001 §1 makes that file the sole authority for node types, props, CSS style rows, enums, and kernel opcodes. D3 also introduces an unplaced JSON stdlib roster needed by both inference and the runner VM, while D2 names only a generated plan reader, not the encoder required by byte-identical lowering. Resolve by deciding the kernel-schema consumption edge, the shared stdlib/opcode roster owner and consumers, and generated encode/decode ownership without copying either vocabulary.

4. **MATERIAL —** D4’s replacement boundary is incomplete. The sentence “a `resource` whose typed value the host supplies through `shape`” treats `shape` as transport, but cited LLP 0508 §9 defines it only as validation/normalization of `resource name = expression as shape TYPE` and explicitly does not own resource lifecycle. Moreover, incorporated 0518 text makes Lane 2 a provider-shaped computation island, not ordinary resource input, with no island admitted by default. Resolve by choosing the positive v1 boundary and naming its source category, plan admission, cross-platform host owner, and first-pixel behavior.

5. **MATERIAL —** D4 does not fairly state the rejected alternative. “Moving the schedule math into Contract” is not the alternative to rejecting synchronous JS; cited LLP 0517 defines a typed pure-call seam and benchmarks the same named Caltrain helpers. `RULES.md` forbids app JS before first pixel, not categorically after it. Resolve by comparing that actual Lane-1 or post-boot alternative against resource-plus-stdlib, including boot closure, native parity, provider artifacts, and seam cost.

6. **MATERIAL —** FLAG: The Caltrain feasibility claim cannot be verified from this export. The app source and census are absent; older LLP 0480 records roughly 225–226 Contract lines, while LLP 0517 corroborates only the three named helpers. Nothing here verifies fifteen TypeScript calls, that all occur in derives, or their exhaustive reduction to one resource plus “~six” reusable stdlib entries. Because D4’s soundness and §5’s “Small” cost conclusion depend on this, supply a commit/path-backed inventory mapping every call site to resource, stdlib, or in-language computation.

7. **MATERIAL —** D5 reintroduces the “hot revision surfaces” expressly excluded by `rules/NOT-DOING.md` §Runtime. Its cited LLP 0500 confirms that an in-place, state-preserving plan patch is an HMR `HotRevision`, while LLP 1001 deliberately leaves HMR generation/address fields unbuilt. The same passage also misstates the cited design: LLP 0500 chooses segment patches and defers patch format, while declaration identity is not bare name—it must be scoped and combined with compatibility checks. Resolve with full-plan replacement/reset for v1, or make the required NOT-DOING trade and decide patch ownership, scoped identity, type-reset rules, and generation fencing.

8. **MATERIAL —** D6’s “one fixture per construct” is insufficient for its claimed evolution discipline. A single positive expected-plan fixture does not exercise each rejection/stable diagnostic ID, semantic branches, cross-construct interactions, or encoded-byte/admission behavior. Resolve by making one positive fixture a floor, requiring negative fixtures per rule ID and interaction/regression fixtures where semantics compose, and stating whether canonical bytes or only decoded tables are compared. The runner-first sequencing itself makes the future exit temporally coherent.

9. **MINOR —** D2 says “Five crates” but lists six; the likely meaning is five compiler crates plus runner-created `exact-plan`. Cargo’s no-cycle rule checks acyclicity, not the intended layering—acyclic sideways dependencies remain possible. Clarify the count and ownership, and describe exact direction as review-enforced through visible manifest edits.

10. **MINOR —** D3 says every plan from “an older compiler is refused by digest,” but no declared digest identifies compiler age; format and opcode digests reject incompatibility, not an otherwise compatible older producer. Name the compiler/semantic digest and its preimage, or narrow the claim to incompatible format/opcode/roster digests.

11. **MINOR —** D5’s ≤20 ms slice is numerically coherent with 100 ms p50, but “measured rather than hoped” overstates a provisional, receipt-free allocation. “Recompile the changed module” also needs to include importers invalidated by exported type or component-interface changes. Define it as an initial allocation, require recomputation of the invalidated dependency closure, and bind endpoints, warm-state workload, environment, and end-to-end attribution when the implementation spec begins.

12. **MINOR —** FLAG: The exact1 source and direct LLP 0518/0542 documents are absent, so the compiler/package line counts, 13,294-line committed reader, and detailed package-content census cannot be verified. The exported research supports the qualitative architecture but not those precise numbers. Supply the census commit/commands or remove the precision.

## Verdict

NOT READY


---

## Round 2 (delta round) — verbatim review

- **Runtime:** `codex exec -s read-only`, `gpt-5.6-sol`, effort ultra (verified in the run log), fresh session `01a04a25-1e89-7dd3-8390-0e72cd6676dc`, workdir = capsule-r2 (export excluding `llp/reviews/1004*`; carried BRIEF.md, DELTA-INSTRUCTIONS.md, DELTA-r1-r2.diff)
- **Prompt sha256:** abfa8e6f50bdad3e448b115ebd2ce37c0d0ee53f1a9ee1bc7c5fa90f757b8b12 · **Capsule sha256:** 8571f6b76170e8590dd58dfb9c89b485fb3594d0a5896202090c7c1581cb9ee0
- **Target (r2) sha256:** 1b2f422f0d9113df4bbc748f3692d4823540604b5f7b5f037f92ce2ba6c43314
- **Date:** 2026-08-28
- **Verdict:** **NOT READY** (5 MATERIAL, 4 MINOR)

## Overall assessment

r2 improves the crate layering, compiler corpus, exit condition, and factual census; the cited counts check out. However, it introduces several MATERIAL defects in compatibility admission, D4’s boundary, reload semantics, and compliance with binding rules. No MATERIAL PRE-EXISTING concern remains.

## Findings

1. **MATERIAL —** **IN-DELTA.** D2 says plans encode `opcodes.json` and `exact-kernel` vocabulary, but D3’s refusal tuple names only format, roster, and compiler identity digests. It never states that either omitted vocabulary is covered. A changed VM opcode or kernel ordinal could therefore remain valid bytes while acquiring different meaning, contrary to LLP 1001 §1’s fail-closed schema rule. Define digest preimages and bind format, VM opcodes, stdlib roster, and kernel schema explicitly or through a defined composite; compiler crate version plus configuration is not a safe implicit substitute.

2. **MATERIAL —** **IN-DELTA.** D4’s “one resource” mapping is not executable as written. The Appendix classifies `nearestStationIds`, `getSelectedStation`, `getDirectionBoard`, `getTrainRunById`, and `searchStations` as data, but the pinned source shows that they transform argument-dependent runner state. Inbound-only `Runner::supply(resource, bytes)` provides neither plan-to-host arguments nor a refetch/dependency protocol. Supplying raw data leaves the transformations unowned; computing their outputs in the host is provider-shaped computation—the cited island lane—not “only data.” Locate every transformation in Contract/the roster, or admit and cost a typed request/provider path, then correct §5’s claimed port cost.

3. **MATERIAL —** **IN-DELTA.** D4 does not fairly state the rejected LLP 0517 alternative. Section 1 incorrectly derives import-call rejection from 0508 §4.3, while 0508 §6.6 explicitly recognizes import-rooted calls. LLP 0517 supports one TypeScript provider source through `native-hermes` and `wasm-page-js`; shipping native JS engines and reimplementing every helper are alternative costs, not simultaneous ones. A baked first frame also satisfies the literal no-JS-before-first-pixel rule, although it incurs staging and staleness. Cost these branches separately; HBC/staged reload being outside exact2’s scope can still justify rejection.

4. **MATERIAL —** **IN-DELTA.** D5’s whole-plan replacement with state preservation remains a hot-revision surface despite changing the transport from patch to replacement, contradicting `rules/NOT-DOING.md` §Runtime. It is also unsafe as specified: generation fencing is rejected even though live resources or tasks can complete after replacement, and the compiler-static `(component path, declaration name, type digest)` cannot distinguish runtime instances created by keyed `each`. Either perform a true teardown/reset with no migration, or record the Runtime trade and define runtime-instance identity plus cancellation or generation refusal for old-plan work.

5. **MATERIAL —** **IN-DELTA.** Section 4 says the `NOT-DOING.md` §Process trade “is [Charlie’s] to record,” leaving it unresolved while the prohibited refine loop, READY verdicts, and revision ledger already exist. Authorization satisfies the agent-document approval rule, but not the binding move-off rule requiring the unblock and corresponding doing-list removal in the same change. Record that trade or end/remove the loop apparatus before readiness.

6. **MINOR —** **IN-DELTA.** Section 3 leaves the v1 app unnamed, while D3’s language scope, D4’s roster, D5’s workload, and D6’s exit all depend on it; the earlier runner lane also creates `stdlib.json`. Add app selection and a fresh inventory as an explicit prerequisite to the runner lane, or select Caltrain now.

7. **MINOR —** **IN-DELTA. FLAG.** D5 honestly labels the ≤20 ms compiler slice an allocation, but “the plan … is kilobytes” and “loading it costs less than a patch format” are unmeasured claims about an unbuilt plan and runner. Bytes alone do not bound validation, state matching, layout, host application, or presentation. Label these as hypotheses and require selected-app replacement-to-present measurement before treating whole replacement as feasible.

8. **MINOR —** **PRE-EXISTING.** D3 calls 0508 the language basis while saying its decisions are merely “consulted.” Because research never binds exact2, the normative adoption boundary remains ambiguous beyond D3’s short construct list. State either that only those enumerated clauses are decided, or require the exact2 implementation spec to transcribe the applicable 0508 semantics and explicit deviations.

9. **MINOR —** **PRE-EXISTING.** D6’s canonical-byte fixtures verify compiler lowering, but not runner behavior for later constructs or roster entries; only the initial hand-built plans are stated to be runner-executed. Require accepted plan fixtures—especially each new roster entry—to be shared with runner tests carrying observable state, operation, or effect expectations.

## Verdict

NOT READY


---

## Round 3 — ABORTED (no review received; not counted)

- Launched 2026-08-28 against r3 (sha256 `ffc206190c64e9697c5488134fb6f48a246a000ab0b1494e2a638a83a5f27a1b`) as a final delta round; stopped by the author's instruction ("stop now and build the compiler") before the model responded. No partial output is recorded as a review.

## Close-out (2026-08-28)

- **Final verdicts bind to r2** (sha256 `1b2f422f0d9113df4bbc748f3692d4823540604b5f7b5f037f92ce2ba6c43314`): codex NOT READY (5 MATERIAL, all in-delta; no MATERIAL pre-existing), grok NOT READY (1 MATERIAL in-delta; no MATERIAL pre-existing). Round 1 verdicts (on r1) are superseded by round 2.
- **r3 (sha256 `ffc206190c64e9697c5488134fb6f48a246a000ab0b1494e2a638a83a5f27a1b`) is UNREVIEWED.** It folds every round-2 finding; the dispositions are the r3 text itself (see its `Revised:` line) and, per finding: codex r2 #1 → D3 (three digests); #2 and grok r2 #1 → D4 (Rust data source, request/response seam, build-time constant evaluation, rebucketed Appendix); #3 → D4 alternatives (per-branch costing; 0508 §6.6 acknowledged); #4 → D5 (teardown and reset; no migration); #5 → §4 (recorded as the author's obligation — the RFC cannot edit a rules file); codex #6/grok #6 → §3 (naming the app is a runner-lane prerequisite; constructs listed in the Appendix); codex #7/grok #4 → D5 (unmeasured claims removed; allocation scoped); codex #8 → D3 (only the enumerated constructs are decided; the spec transcribes semantics and deviations); codex #9 → D6 (fixtures also run on the runner); grok #2 → Appendix (four fates plus command; `elapsedMilliseconds` moved); grok #3 → D2 (`exact-plan` depends on nothing, carries no kernel vocabulary); grok #5 → §1/Appendix (numbers dated, method stated).
- Loop terminal state: **author stop after two completed rounds** (budget 3; launches: codex 3 incl. the aborted one, grok 4 incl. one CLI-usage failure and the aborted one). Convergence on pre-existing text was reached in round 2 by both families; in-delta convergence was not tested. Recommendation: the author ratifies or amends D1–D6 directly; the build (the plan lane) is the next instrument, not another round.
