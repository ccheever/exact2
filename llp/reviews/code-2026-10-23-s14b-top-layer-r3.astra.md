# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 3, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e2a0e96d7e`.
- **Method:** one brief (sha256 `15ecb68756d77b6defa29a6f82ba3bf733b46233069e0ef185265c1ce432a363`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** both taken in 7df0ba99f8

1. **[P2] Region registration rejects stale implied blocks.** [kernel/src/region/state.rs:797](/tmp/iosfix-rv-s14b3/kernel/src/region/state.rs:797)

   Repro: give an otherwise valid region owner `popover="manual"`, lay it out, clear `popover`, then call `set_content_region(Some(binding))` before another layout. Clearing the prop removes its `top_layer_slots` entry but leaves the implied block until `imply_blocks`. Validation therefore rejects the now ordinary owner as “region owner in the top layer.” Registration calls `imply_blocks` only **after** this validation succeeds ([kernel.rs:493](/tmp/iosfix-rv-s14b3/kernel/src/kernel.rs:493)). Eligibility must distinguish current host blocks from stale implied blocks. The new refusal test covers an actual popover, not this transition.

2. **[P2] The placement helper loses vertical and RTL positioning rules.** [kernel/src/kernel/block.rs:454](/tmp/iosfix-rv-s14b3/kernel/src/kernel/block.rs:454)

   The shared `axis` calculation always clamps two auto margins’ negative share to zero and always prefers the left inset when margins are fixed. Those rules differ by axis and direction, as the repository’s existing absolute solver already handles.

   Concrete failures:
   - In a 320px-high viewport, `height=400 top=0 bottom=0 margin-top="auto" margin-bottom="auto"` produces `y=0`; the vertical equation requires `y=-40`.
   - In a 320px-wide RTL viewport, `width=80 left=0 right=0 margin=0` produces `x=0`; the right inset should win, giving `x=240`.

   Both affect terminal painting and the runner’s no-host-rectangle fallback. The added placement tests use LTR and positive remaining space, so they miss these cases.

No files changed. Repository tests were not run.
