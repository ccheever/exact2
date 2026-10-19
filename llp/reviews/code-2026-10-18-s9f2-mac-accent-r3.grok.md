# Code review: LLP 1121 stage 9 F2, the Mac's accent cells in a key window, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the net change after round 2.
- **Method:** one brief (sha256 `e17a8a09b7fca931587588ca0aaad4696b3b52dab3bc458a8f365f94c6566dc2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (no findings).
- **Disposition:** fixed in round 3's commit.

---

Nothing is blocking. The round 1–2 fixes hold on the paths this run actually takes.

`EXACT_AGENT_ACTIVATE=1` is set only on the macOS `agent.mjs` spawn in `scripts/measure-controls.mjs`. `agent.mjs` forwards the environment, and both `ExactMac/main.swift` and `AgentMac.swift` require agent mode as well. Activation is `makeKeyAndOrderFront` plus `activate(ignoringOtherApps:)`, only when that variable is set. Other agent runs still use `orderFrontRegardless`.

The previous app is restored on every exit this run can hit:

- Normal end calls `exit(0)` on the main queue (`Agent.exitAfterStorage`), so the `atexit` handler runs there and calls `frontAgain` (`yieldActivation` on macOS 14, then `activate()`, skipping a process that has already quit or is this one).
- Ctrl-C is `SIG_IGN` plus a main-queue signal source that calls `exit(130)`, so the same `atexit` handler runs on the main thread and not inside the signal handler.
- After `spawnSync` returns, the script’s `finally` runs `lsappinfo setfront`. A missing app has an empty `lsappinfo info` (checked), so it is not relaunched. A live app’s `pid = N` and `bundleID="…"` match the parsers. Status 0 is success only when the output does not contain `ERROR` (the tool’s error prefix; `permErr` is the status string beside it). Otherwise it sends an Apple Event `activate` and, if that fails too, writes the failure to stderr.

Both screenshot replies include `active` (`NSApp.isActive &&` the window’s `isKeyWindow`). The measure uses the viewport shot, and it throws if either picture’s `active` is not `true`.
