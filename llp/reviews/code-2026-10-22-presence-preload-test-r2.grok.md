# Code review: The wasm presence preload played in Chrome (presence-preload.test.mjs), round 2, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `d129e3e054`.
- **Method:** one brief, blind to the other reviewer; 2 rounds (the lead's ask for a browser test), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the presence-preload Chrome test change end to end, including the lane wiring, and look specifically for anything the first pass missed.This is a read-only second pass on `HEAD~2..HEAD`. I'll read the project instructions and the full diff, then check the test and lane wiring against the surrounding code.The change is the new Chrome test plus its lane step. I'll read the test against the accessibility-tree pattern and the glue/web-ax wiring it claims to follow.No findings.

The Chrome test and its lane wiring are sound, including the round-1 skip-message fix. `presence-preload.test.mjs` boots a plan whose layout transition is `none`, so the boot batch carries no presence row, then one tap turns on `2000ms linear` and removes the 40px row above. It reads the painted `state.presence` y at the tap, at `clock('+1000')`, and at `clock('settle')`. Those three checks fail if the view has already jumped (the no-preload case the commit describes) and pass only if it is still on a roughly linear 2s slide. `tap` goes through `agentSettled`, which waits for `presence.landed`, so the sample is not taken before the after-paint preload.

`web-ax-test` runs this file with `EXACT_REQUIRED=1` and the lane’s Caltrain wasm dist (`EXACT_WEB_LINK=all` from `prepare`). The glue step still discovers the file by glob and sets `EXACT_GLUE_FAST=1`; the skip now names `web-ax-test`, the same wording as `accessibility-tree.test.mjs`, and that is the step that actually runs it.
