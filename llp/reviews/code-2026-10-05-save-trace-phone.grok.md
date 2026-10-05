# Code review: LLP 1079 D5 on a phone, Save Trace's share sheet and `agent.mjs trace --phone` (8fca848db..b058e4442), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `b058e4442`, plain output.
- **Method:** one brief (sha256 `c872d708423aa6e7f345c98911dda6c3e4c94f79ec3ba77af9d74751d6929e95`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken (astra's 2). 2 taken: a simulator `simctl` knows is read from its container or refused at once ("no trace saved"); `devicectl` runs only for a device `simctl` does not know. 3 taken (astra's 3). 4 taken in part (astra's 4). 5 taken (astra's 1).

---

LAND WITH CHANGES

1. BLOCKING — `scripts/agent.mjs` is over the 1,500-line cap. `rules/RULES.md:16` and `scripts/caps.mjs:154` count `text.split('\n').length` and fail when it is greater than 1,500 (no baseline). At `8fca848db` this file was exactly 1,500; at HEAD it is 1,524, and it is the only source file over the cap, so `bun scripts/caps.mjs` fails. The new code is the whole overrun: the header line (`scripts/agent.mjs:17`), `phoneTrace` (`1360–1375`), and the phone branch (`1395–1401`). Move `phoneTrace` to `scripts/agent-inspect.mjs` and fold the dispatch into the existing one-line `trace` branch (`1394`) so this file adds no lines.

2. MATERIAL — A simulator with no saved trace is handed to `devicectl`, which this change says cannot copy simulator files. `phoneTrace` (`scripts/agent.mjs:1366–1373`) treats `simctl get_app_container` status 0 as a simulator only when `tmp/trace-latest.json` is already there. If the container is found and the file is not (Save Trace never tapped, or `tmp` was purged), it still runs `devicectl device copy from` with a 20s timeout. That command cannot succeed for a simulator, and the error blames the cable copy. If status is 0, require a non-empty container path, copy that file, or throw `no trace` immediately. Call `devicectl` only when `simctl` did not recognize the device.

3. MINOR — The new usage says `trace --device <name|udid>` (`scripts/agent.mjs:17` and the usage string at `1403`), but `--device` is a boolean (`scripts/agent-launch.mjs:139`) and `--phone` is what takes the name (`146`). `trace --device <udid>` leaves the udid in `ops`, so the file branch at `agent.mjs:1394` runs first and tries to read that string as a local trace. `trace --phone <name>` and bare `trace --device` work. Document `trace --phone <name|udid>` and `trace --device` (optional `--phone`), matching `parseFlags`.

4. MINOR — The new behavior the driver depends on is not pinned. `FrameSamplerTests.swift:45–47` compares bytes at `ExactSession.latestTrace`, so a rename of that constant stays green while `agent.mjs:1367` and `1372` still read the hardcoded `tmp/trace-latest.json`. `phoneTrace` is private and has no test: a wrong `devicectl` flag, a missing `--app` bundle id, or the simulator fallthrough in finding 2 would not fail. Assert the filename string `trace-latest.json`, and drive `phoneTrace` against a fake container directory (file present, file absent, `simctl` status non-zero).

5. NIT — The destination name keeps `/` (`scripts/agent.mjs:1364`, `[^\w.\/-]`). A phone named `n/a` becomes `traces/trace-n/a-….json`; `writeFileSync` then throws because the parent directory does not exist, after the trace was read. Replace `/` as well.

Holds: a production bake returns before either write (`FrameSampler.swift:161`); Share… is added only after success and is compiled out of tvOS (`DevMenuIOS.swift:103–109`, `113–125`); the iPad sheet is anchored like the dev action sheet (`118–121`); `trace-latest.json` is the same bytes as the stamped file (`FrameSampler.swift:189`). No earlier `llp/reviews/code-2026-10-05-save-trace-phone*` review.
