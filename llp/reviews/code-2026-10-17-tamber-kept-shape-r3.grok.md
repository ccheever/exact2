# Code review: One retained-shape check for propose and the dev loops (theme2's ruling, Tamber stage 5), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `3303d38a24fa8511e80c5e4f86b62709c03745593086417d9c6397e5da66dd48`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph). A first run ended without a verdict after compacting its context; this is the second.
- **Verdict:** nothing blocking; one P2 (the accent per tuple).
- **Disposition:** applied without re-review (the last round).

---

I'll review the round-2 fixes and the HEAD~2..HEAD range read-only, starting with the review instructions and the commits themselves.[P2] Count each tuple's accent, not one bit for the token — `tamber/src/session.rs:349`

`shape` sets `accent` with `values.iter().any(...)`, so a kept token that already names an accent role in one tuple stays `live` when a different tuple starts or stops naming one. `Plan::structure` and `runner::design::shape` do not move (the stored text is the role name, which has no `(--`, and the slice count stays). The JS loop still rebuilds: `modes::design` records a bool per tuple (`host/web-js/src/modes.rs:488`) and `sameShape` compares that vector (`host/web-js/dev.mjs:211`), because companion `--exact-accent` rules are written only for tuples that are not the accent. A view that names `ink`, with `tone = warm | cool` and `ink = #111111` / `when tone=cool: AccentColor`, then `set-token` of the default to `LinkText`, is classed `live`. The warm tuple's accent flag goes from false to true, the warm companion rule must disappear, and a scrub the tool offers is refused with "it changes the design's shape". The test at `tamber/tests/it/edit.rs:961` only covers a token that was not an accent role at all.

Nothing is blocking. That edit does not change `Plan::structure` or `design::shape`, and the JS loop will not apply the batch. The round-2 flip of a non-accent value to `AccentColor`, the respelled role override, and the design-tool `-exact-label` drive match the plan. `Role::accent` follows `roles::web_accent`.
