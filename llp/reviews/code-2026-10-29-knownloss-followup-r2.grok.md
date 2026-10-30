# Code review: ResetGate knownLoss follow-up, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `faa1260bed`.
- **Method:** one brief (sha256 `9a1b986fb5d8e6246a7455beb0af9b3eafb50b6893f1d87501f11954cb63784f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `QUEUE.md:4` — `deliverySync` is grouped with the calls whose content a restart loses, and the old qualifier is gone. `applyTogether` asks again after the resets (`Session.swift:282`), so a shipped update and any other `applyTogether` restart do not lose those facts. The loss is a single session's `apply` (`Session.swift:983`) and a dev-menu baked `session.boot` (`DevMenuMac.swift:436`, `Session.swift:857`), which never re-ask. The table still says this (`ResetGate.swift:173`). Put that qualifier back on `deliverySync` in the QUEUE line.

**Non-blocking**

1. `QUEUE.md:4` — "They come from the main queue" overstates the comment at `ResetGate.swift:153`, which says "most". A Mac alert commits `change` synchronously in `endPrompt` (`AlertsMac.swift:352`) before `endSheet` (`AlertsMac.swift:253`), while that runtime is already held. That is the moot field half, not a queued arrival. Say "most" in the QUEUE line too.

`KNOWN_LOSS` matches the table: the same 17 keys.
