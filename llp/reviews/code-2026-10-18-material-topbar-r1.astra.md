# Code review: the Android top app bar's title and rows, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed (the read-only sandbox cannot start on `expo-build-1000`), a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `78d796d07`.
- **Method:** one brief (sha256 `960b6dc230561f28cea541785b7c82f022921589a9d24d0fe315748a4579167e`), blind to the author's reasoning. The author (Claude) is not a reviewer.
- **Answered:** 1 fixed (the title's room is what leads and trails leave it, and it moves back from what trails), with a unit test for 3 in place of a specimen; 2 fixed (the door's settle is from the clock's own time).

## Review

1. **Correctness [P2] — [look.rs:387](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars/look.rs:387): title measurement incorrectly subtracts the minimum start inset.** Compose measures using the navigation and action widths, then applies the minimum inset when positioning. With a 412 dp bar, no leading control, and one 48 dp trailing button, Compose allows 348 dp of title text; this code allows 340 dp. A 344 dp title that previously fitted now ellipsizes. Compute available width separately from the title’s position.

2. **Correctness [P3] — [main.rs:197](/home/ccheever/projects/exact2-review-s2/apps/material-specimens/linux/src/main.rs:197): the push step makes the subsequent clock advance ineffective.** `Presenter::clock` takes an absolute time. After advancing to 1000, the call at line 205 requesting 300 silently does nothing. A field focused after pushing would receive zero settling time instead of 300 ms. Advance relative to the current time. The current static `top-app-bar-up` specimen hides this error.

3. **Tests — [Specimens.kt:165](/home/ccheever/projects/exact2-review-s2/host/android/reference/src/main/java/dev/exact/reference/Specimens.kt:165): the specimens miss the width regression in finding 1.** Both titles are short and neither bar has actions. Add a long title with no leading control and a trailing button. I found no existing assertion made false by this commit.

4. **Schema: no findings.** The rows follow the existing Material-only `field-` declarations. Other columns remain unset, existing indices remain stable, and the neutral-sheet census excludes them. The generated table grows and the global schema digest changes; control and node structure sizes do not.

5. **Other: no findings.** The constant reader handles the pinned declarations, and the height callers retain their previous 64 dp behavior.

Source inspection only; no files changed or tests run.

READY WITH CHANGES