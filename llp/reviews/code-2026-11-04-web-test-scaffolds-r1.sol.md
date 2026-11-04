# Code review: the web test scaffolds, the whole branch, round 1, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `08e10f0c22` (2 commits on `bbe444fb1d`).
- **Method:** one brief (sha256 `4115c2c2b4e3163699a9d2c9a81677e645f08ad3553e7643f16e7407e82d6d13`), one reviewer, pointed at both commits, with four passes: adversarial correctness, simplification, writing and verification. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths, and its commit shas are of that worktree's history.

**1. Adversarial correctness**

- **Blocking — `08e10f0c22`, [sheet-leave.test.mjs:28](/private/tmp/claude-501/-Users-admin/8454b037-a354-4ecb-9fbf-e4c8a8685be5/scratchpad/sol/wt-scaffolds/host/web-js/sheet-leave.test.mjs:28), also line 49:** neither fixture has a recognized navigation root. Both new `main` elements sit inside an unkeyed `column`. Lowering supplies `navigationBack` only to the first top-level element (`contract/lower/src/nodes.rs:316`); these nested keys leave `NavPlace::Outside`. Consequently, the emitter omits `navigationRoots` (`host/web-js/src/emit.rs:600`). The tests still call real `sheetLeaves`, but can pass while navigation projection is entirely broken. **Edit:** make each keyed `main` the first top-level element, with only the loop inside it; place controls in a following sibling. Assert the covered route becomes inert and becomes active after Pop. Keeping the loop alone preserves the whole-parent replacement case.

- **Blocking — `08e10f0c22`, [sheet-leave.test.mjs:95](/private/tmp/claude-501/-Users-admin/8454b037-a354-4ecb-9fbf-e4c8a8685be5/scratchpad/sol/wt-scaffolds/host/web-js/sheet-leave.test.mjs:95), also lines 115–117:** the assertions race the real 400 ms exit animation across browser calls. A delayed observation makes the correct implementation fail the first test. With the decorated-textarea guard removed, that same delay lets the second test pass after the sheet finishes leaving. This breaks `docs/testing.md`’s reliability requirement and undermines the mutation claim. **Edit:** click, observe, and pause exit animations within one browser evaluation; finish and await them explicitly afterward. Check decorated-sheet absence in the same evaluation as its click.

No correctness finding in `f83dac49c2`: `/web/` resolves to the intended host directory, and the tests retain the real reorder module, deferred completion, refusal, hold-state, and offset assertions.

**2. Simplification**

No separate findings in either commit. The reorder server change is small and matches the existing scaffold.

**3. Writing**

- **`f83dac49c2`, commit message:6:** “failed both,” “mapped,” and “so it read” recount failure history instead of describing the dependency. **Edit:** replace the paragraph with: “The runtime’s module graph requests `/web/timer-observations.js`, whose source is `host/web/timer-observations.js`.” This also identifies the import as transitive.

- **`08e10f0c22`, commit message:6 and :11:** “has failed to compile since 05c258c192” and “steps and expectations are unchanged” describe history. “No host presents a sheet that is no route” uses awkward negative framing. **Edit:** describe the compiler requirement positively: “The compiler accepts modal sheets as keyed routes beneath a navigation root.” Delete the unchanged-expectations sentence.

Both titles use the indicative mood and satisfy the limit: 69 and 61 characters. Neither commit changes comments or references reviews.

**4. Verification**

Both commands were attempted from `host/web-js`:

| Commit | Command | Result |
|---|---|---|
| `f83dac49c2` | `bun test shared-reorder.test.mjs` | Stops before assertions: missing `playwright-core`. |
| `08e10f0c22` | `bun test sheet-leave.test.mjs` | Both cases stop at `mkdir(target)`: sandbox `EPERM`. |

The code supports the stated upstream import and compilation failures. I cannot independently confirm the passing runs, repeat counts, or mutation runs. Returning `false` from `sheetLeaves` should fail the first test; the decorated-guard mutation has the timing weakness above.

No files changed.

**Verdict: not ready.**
