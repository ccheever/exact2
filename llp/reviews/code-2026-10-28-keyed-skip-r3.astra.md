# Code review: runner keyed-row skip (x2/keyed-skip), round 3, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `17270cbd32`.
- **Method:** one brief (sha256 `6b4cf1092ccbfd9e50b9a1cf8509f1fb9606dae438a68272e790ea3b90158d88`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed

## Brief

Round 3 of 3 reviewing the branch: diff origin/main..HEAD, two commits, unchanged since round 2. Read-only: no checkout, restore, stash, reset or edits.
It is a runner change (exact-runner), runtime level. Round 1's brief has the change, the position audit and the measurements: /tmp/iosfix-rv-keyed1-brief.txt. Read it first.
Rounds 1 and 2: Astra's P2 (the fresh-build oracle) is fixed in 17270cbd32. Round 2 was CLEAN from both reviewers.
Round 3: try to BREAK it. Construct, on paper or as a plan sketch, a Contract program whose visible output after a prepend (or any reorder: a move to the end, a swap, a sort, a filter that removes rows above) differs from a fresh build of the new list with this change, where it didn't before. Cover:
- reorders other than a prepend, through both reposition and update_item;
- a `match` or `if` arm inside a row (an ArmFrame);
- a component instance inside a row: does a component body's read of the row's `i`, passed as a prop, count?;
- slot or derive defaults evaluated in a row;
- `with`/LoadBound frames between the row and an inner read;
- a row inside a row inside a virtualized list;
- the host-side virtualized collection's rebinding of wrappers to keys, in case a wrapper re-used for a different key skips evaluation.
Also: does anything outside runner/src/instance call reposition or update_item, or read frame.index, and assume a moved row was re-evaluated?
Checked: Excord on the iOS simulator, one history fling of 3 pages. Before: 3,147 evaluations, 1,107 of them unchanged. After: 1,966, with 19 unchanged.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings. I could not construct a fresh-build mismatch introduced by these two commits.

- **Reorders:** move-to-end, swap, sort and removal-above all use the same old/new index comparison in [region.rs:313](/tmp/iosfix-rv-keyed3/runner/src/instance/region.rs:313) and [collection/mod.rs:1016](/tmp/iosfix-rv-keyed3/runner/src/instance/collection/mod.rs:1016). Neither path assumes a prepend or a positive displacement.
- **Arms, bound values and nested rows:** the paper sketch `virtualized Each → When(true) → Each → Each → Match(Some(LoadIndex(3))) → text LoadBound(0)` still tracks the outer position. Each intervening region shifts its position mask outward; the outer row receives bit 0. A bound-value read counts through its subject; a direct read inside an `ArmFrame` counts through its depth. See [deps.rs:354](/tmp/iosfix-rv-keyed3/runner/src/instance/deps.rs:354).
- **Components and derives:** yes, a component body reading a prop passed as `i` counts. Props and child derives are substituted into expressions before lowering, leaving the relevant `LoadIndex`. See [use_node.rs:241](/tmp/iosfix-rv-keyed3/contract/syntax/src/inline/use_node.rs:241).
- **Slot defaults:** a row state initialized from `i` can differ from a fresh build after reordering, but that difference already existed: initialization happens at creation, and state follows the key. The previous all-dirty update did not reinitialize it. See [region.rs:426](/tmp/iosfix-rv-keyed3/runner/src/instance/region.rs:426).
- **Wrapper rebinding:** a wrapper reassigned to another key gets new slots and frames, then `u.full = true` and `update_row(..., !0, ...)`; it cannot take the optimized skip. See [reuse.rs:334](/tmp/iosfix-rv-keyed3/runner/src/instance/collection/reuse.rs:334).
- **External assumptions:** no callers of these runner methods exist outside `runner/src/instance`. The external `frame.index` reader is [vm.rs:793](/tmp/iosfix-rv-keyed3/runner/src/vm.rs:793). Indices are updated even when evaluation is skipped, and event lookup assembles live frames.

Read-only source review and paper sketches; no tests rerun.

VERDICT: CLEAN
