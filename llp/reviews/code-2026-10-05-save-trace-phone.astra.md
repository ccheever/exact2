# Code review: LLP 1079 D5 on a phone, Save Trace's share sheet and `agent.mjs trace --phone` (8fca848db..b058e4442), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `b058e4442`.
- **Method:** one brief (sha256 `c872d708423aa6e7f345c98911dda6c3e4c94f79ec3ba77af9d74751d6929e95`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):** 1 taken: the device's name becomes one filename component (anything but word characters, `.` and `-` is `_`). 2 taken: `phoneTrace` moves to `agent-inspect.mjs` and the CLI's trace branch stays one line, so `agent.mjs` is back under the cap. 3 taken: the forms are `trace --phone <name|udid>` and `trace --device`. 4 taken in part: the test names `trace-latest.json` literally and checks a second save replaces it; driver and share-sheet tests are DEFERRED (the copy needs a phone; `phoneTrace` takes its process runner as a parameter for one).

---

DO NOT LAND

1. **MATERIAL — Device names can escape the trace directory.** [agent.mjs:1364](scripts/agent.mjs:1364) preserves `/` before resolving the filename. Executing the helper with mocked I/O reproduced `Work/iPhone` failing with `ENOENT`, and `Phone/../../outside` writing under `target/`, outside `target/traces/`. Sanitize the name as a single filename component; test both cases.

2. **MATERIAL — The mandatory size check fails.** The additions at [agent.mjs:1360](scripts/agent.mjs:1360) push the file from 1,500 to 1,524 lines by `caps`’ counting. `bun scripts/caps.mjs` exits 1, violating [RULES.md:16](rules/RULES.md:16). Move the trace handling into an existing appropriate module and rerun the check.

3. **MINOR — The advertised `--device` syntax misroutes input.** [agent.mjs:17](scripts/agent.mjs:17) suggests `trace --device <name>`, but [parseFlags:139](scripts/agent-launch.mjs:139) treats `--device` as boolean. Consequently, `trace --device "My iPhone"` reads a local file named `My iPhone` through [agent.mjs:1394](scripts/agent.mjs:1394). Document separate forms: `trace --device` and `trace --phone <name|udid>`.

4. **MINOR — Tests do not pin the new handoff contract.** [FrameSamplerTests.swift:46](host/apple/tests/ExactKitTests/FrameSamplerTests.swift:46) derives the expected filename from the implementation constant and checks only one save. Renaming that constant would pass while breaking the driver’s literal filename; writing latest only once would also pass. Assert the literal path and replacement after a second changed save. Add focused coverage for driver failures and Share’s URL/iPad anchor.

Source inspection found the production guard intact, sharing excluded on tvOS, and an iPad popover anchor present. No additional ordering/lifetime regression identified. Apple builds, UI interaction, and hardware transfer were not run in this read-only checkout. No matching earlier review files were present.