# Code review: Stage 12 E: @exact/cards, @exact/actions and apps/library-gate (LLP 1139 §5 E), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8249e8c027`.
- **Method:** one brief (sha256 `4210d74f8027aba74af1257b0a9cade63723e2b811886a967b7360f6d219b36b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** actions: blocking (actions.post-pad a px literal); cards: should-fix (context not a D5 variant); library-gate: blocking (the cell test fooled by a trailing comment or a conditional placement; markers unchecked); should-fix (the test unscheduled)
- **Disposition:** taken in 086a3086af: post-pad on space.md; library-gate's cells from the compiler's D5 variants, context and the other non-variant states as named sample- cells (no no-op rows to force them into D5); the test reads the IR, refuses control flow, parses every placement and checks markers; scheduled in async

| Target | Verdict |
|---|---|
| `@exact/actions` | **Blocking fix** |
| `@exact/cards` | **Should-fix: variant metadata** |
| `library-gate` | **Blocking: test permits missing/extra rendered cells** |

- **Blocking — spacing bypasses the base scale.** [actions.tamber:13](/tmp/iosfix-rv-pk1/packages/actions/actions.tamber:13): `actions.post-pad = 15px` violates x2-perf4’s spacing rule and cannot follow an app’s base-scale override. Alias a base spacing name; keep `15px` in the clone’s adoption override.

- **Blocking — the cell test has false passes.** [library-gate.test.mjs:43](/tmp/iosfix-rv-pk1/scripts/library-gate.test.mjs:43): the line regex ignores an extra component call with a trailing comment and counts a required call inside `if false`. Both passed in memory-only mutations. Parse component placements structurally, require unconditional cells, and compare their exact multiset. Add negative cases for both failures.

- **Should-fix — fixture “variants” disagree with D5 metadata.** [library-gate.test.mjs:30](/tmp/iosfix-rv-pk1/scripts/library-gate.test.mjs:30) treats every bool/enum as a variant. D5 requires use in a class/style-row condition. In particular, [PostCard’s context rendering:51](/tmp/iosfix-rv-pk1/packages/cards/post-card.contract:51) reads `context` only through structural conditions, text, and helper arguments, so the signed-off collector omits this intended variant. Derive expectations from compiler metadata and reconcile `context` with D5—through a meaningful conditional recipe/row, or an explicit structural-variant ruling. Keep semantic-state samples separately if desired.

- **Should-fix — the new test is not scheduled.** [library-gate.test.mjs:47](/tmp/iosfix-rv-pk1/scripts/library-gate.test.mjs:47) has no caller in the existing checks; [async.mjs:114](/tmp/iosfix-rv-pk1/scripts/async.mjs:114) enumerates script tests explicitly and omits it. Add it to that existing list.

Otherwise, the package source looks conformant: param defaults name package tokens; class rows fit their part groups; names resolve; helpers remain private; no platform reads appear. The Post button follows the authored-pressable exception. PostCard preserves the intended shell boundary, with one unconditional, unforwarded slot and no hiding or zeroing on its chain. The props are sensible apart from the variant-metadata mismatch above.

The fixture currently has all four exports across 17 cells, IDs on every declared part, and six fixed-size toggleable slot markers. The test catches ordinary cell deletion/duplication, a new direct export, and a conventionally formatted new bool prop. It does **not** protect markers: deleting all six also passed.

Both existing tests passed. No builds or file modifications were performed.
