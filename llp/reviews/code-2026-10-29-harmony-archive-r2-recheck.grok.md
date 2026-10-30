# Code review: Harmony production archive (development-only grant lines), round 2, narrow re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `6df77db73c`.
- **Method:** one brief (sha256 `d30be7cfbbac6484e8827243991358ec646d90f1adf01d4095fdb3f95b517d64`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. Commit message of `6df77db73c` (unchanged from `f2696d6d5b`). It still says ``developmentGrants`, before the backend origin's selection`, but `configure` now runs it after (`js/bake/src/typescript.mjs:984`). It also still says a production build "carries no CallKit"; the test comment was corrected, the message was not. Amend it to say the marker runs after origin selection, and that the archive has no `device.calls` grant (so no `voip`/`audio` background mode), with the calls implementation still linked.
