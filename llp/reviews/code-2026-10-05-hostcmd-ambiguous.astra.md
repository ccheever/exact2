# Code review: ambiguous host-command calls refused, LLP 1089 D1 amended (58f264433..38b6fddf9), 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `38b6fddf9`.
- **Method:** one brief (sha256 `a601e783ad4fe7f0c8e492b6b249dd634fce0194b536697ece07121cb7b1eb26`), the same one sent to grok; round 1; blind to the other review. Requested by Charlie through the coordinator (the ruling is Astra's, `/tmp/astra-hostcmd.out`; this is a separate code review). The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** all three fixed in `25a4539ca`:
  1. *A shape named `action`.* A prop or inject counts as an action only when its type is `action` and the file declares no shape of that name (the test `an_ambiguous_call_is_refused_once_and_a_shape_named_action_is_no_action` compiles a `close: action` record whose child calls `close()`). The file holds every imported shape, since `use` merges them.
  2. *Navigation loses the related location.* `symbols_json` runs `authored_action_hint` before resolving files, as `compile_file_output` does; test `an_ambiguous_call_in_an_imported_component_is_refused_alike_with_its_declaration` asserts build and navigation agree, with both locations in the imported file.
  3. *§6 contradicts D1.* The r1 rejection of `type-call-ambiguous` is marked superseded by the 2026-10-05 amendment.

---

1. **Must-fix — [contract/syntax/src/inline/calls.rs:108](/tmp/rv-hc1/contract/syntax/src/inline/calls.rs:108): record props are misclassified as actions.** Declare `shape action`, pass `action(value=1)` to a child’s `close: action` prop, and call the host’s `close()` in that child. This now gets `syntax-call-ambiguous`, although the prop is a record: [type resolution gives shapes precedence](/tmp/rv-hc1/contract/types/src/lib.rs:316). The new predicate checks only the spelling `action`. Fix it to distinguish the primitive action type from a declared shape, for both props and injects. Add acceptance regressions for local and imported shapes.

2. **Should-fix — [contract/cli/src/symbols.rs:244](/tmp/rv-hc1/contract/cli/src/symbols.rs:244): `symbols` loses the required related declaration.** For a component with `close: action` and a statement `close()`, this path converts the type error directly into `CompileError`, whose related locations are empty. It never calls the new enrichment at [symbols.rs:915](/tmp/rv-hc1/contract/cli/src/symbols.rs:915). Compile, build JSON, and Lean receive that enrichment. Apply `authored_action_hint` before resolving source paths here too; test diagnostic parity using an imported component, including both locations’ files and spans.

3. **Nit — [llp/1089-action-composition.rfc.md:564](/tmp/rv-hc1/llp/1089-action-composition.rfc.md:564): the rejected-alternatives section contradicts amended D1.** It still rejects ambiguity checking because Caltrain’s `setScheme` and Files’ `share` “compile today.” The former wrapper is explicitly refused by this commit. Mark this rationale as superseded by the October 5 ruling or remove it.

Verdict: DO NOT LAND
## Round 2, 2026-10-05

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `25a4539ca`; brief sha256 `cc5d9614273fc93a7eb4492c52ebfde92e78ca169c97fb5a65c647eb621fbace` (the fixes `38b6fddf9..25a4539ca` against this review's round-1 findings and dispositions). Blind to grok's round 2.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** both fixed in `2cdc57f87`, by one change: the after-the-fact filter is gone. The ambiguous calls' spans are in `Shapes::ambiguous` before any body is checked, and a statement among them is never checked as the host command (`actions.rs`), nor is `postMessage`'s surface (`posts.rs`), in the child and after lifting alike. So no argument-span error leaks (`share(1)` on an action `share` reports only the ambiguity), and `children`'s boundary is never shifted by a removal. The test asserts `share`, `close("swiped")` and `postMessage("hi", "nowhere")` give exactly three `syntax-call-ambiguous` and nothing else; with the skip disabled it fails.

---

1. **Should-fix — [contract/types/src/lib.rs:1304](/tmp/rv-hc1/contract/types/src/lib.rs:1304): argument-level host diagnostics still leak through.** Declare `action share(value: number)` and call `share(1)` from another action. Besides `syntax-call-ambiguous`, this reports `type-share-argument`: [share validation](/tmp/rv-hc1/contract/types/src/checks.rs:835) attaches that error to the argument, so the exact-statement-span filter misses it. **Fix:** skip host validation for already-ambiguous statements, including expanded copies; add a regression asserting only the ambiguity diagnostic.

2. **Should-fix — [contract/types/src/lib.rs:1303](/tmp/rv-hc1/contract/types/src/lib.rs:1303): filtering invalidates the saved child-error boundary.** Extend the new `close("swiped")` fixture with a child prop `value: number`, `text value + 1`, and pass `value="oops"`. `children` is recorded before filtering; removing `type-close` moves the expansion-only `type-operand` inside that boundary. [prefer_call_sites](/tmp/rv-hc1/contract/types/src/lib.rs:1417) consequently preserves a cascading error in the child alongside the actual bad-prop diagnostic. **Fix:** maintain the surviving prefix count when filtering, or suppress the duplicate before insertion; add this combined-error regression.

Verdict: LAND WITH FIXES
## Round 3 (the last), 2026-10-05

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `2cdc57f87`; brief sha256 `c66dc5c1dd0fce52d54874fe770ad17864c63c3b0630610223d033ddf12774bd`. Blind to grok's round 3.
- **Verdict:** LAND.
- **Disposition:** nothing to fix.

---

1. **Should-fix — resolved**, [actions.rs:189](/tmp/rv-hc1/contract/types/src/actions.rs:189): ambiguous `share(1)` and `close("swiped")` now skip host validation, including lifted copies whose statement spans are preserved. [posts.rs:58](/tmp/rv-hc1/contract/types/src/posts.rs:58) likewise skips `postMessage` surface validation.

2. **Should-fix — resolved**, [lib.rs:1285](/tmp/rv-hc1/contract/types/src/lib.rs:1285): the post-check filter is removed, so the saved `children` boundary remains valid. For `value="oops"` with child text `value + 1`, [prefer_call_sites:1415](/tmp/rv-hc1/contract/types/src/lib.rs:1415) suppresses the expansion-only error while preserving the bad-prop diagnostic.

No new actionable findings in the full diff. Verified by code inspection; no builds or tests run.

Verdict: LAND