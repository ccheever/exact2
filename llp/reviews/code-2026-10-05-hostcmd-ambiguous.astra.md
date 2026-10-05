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