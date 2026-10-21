# Code review r2: LLP 1079 D5 on a phone (8fca848db..626244172), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `626244172`, plain output.
- **Method:** one brief (sha256 `ac856b30fdee689d9dad80dbf76d0aebc8c49f2597bd6f91ea47863cae6f0e1e`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken (astra's 1): exit 148 alone leads to `devicectl`; a shut-down simulator is refused with `simctl`'s message (booting one for the user is not the driver's call); status 0 with no path is the unsaved error. 2 DEFERRED (astra's 2).

---

LAND WITH CHANGES

1. **MATERIAL** — A simulator `simctl` recognizes still takes the `devicectl` copy. `phoneTrace` (`scripts/agent-inspect.mjs:333-340`) runs `devicectl device copy from` whenever `simctl get_app_container` does not return a status-0 path. A shutdown simulator is one `simctl` knows: on SignalRef 18 Pro (`73196F98-…`, state Shutdown) that command exits 149 with `Unable to lookup in current state: Shutdown`, and its 151 data containers are still on disk. `devicectl` then exits 1 immediately with `Transfer Files is not supported by this device`. `trace --phone "SignalRef 18 Pro"` therefore reports that no trace was saved or the phone is unreachable, and a `tmp/trace-latest.json` in that container stays there. The same branch runs for a booted simulator whose app is not installed (exit 2, `No such file or directory`, reproduced for `com.exact.caltrain` on x2-peek). `phone()` will hand either device over: a named pick is returned with no boot and no reality check (`host/apple/devices.mjs:140-144`). Round 1's taken rule was that `devicectl` runs only when `simctl` does not know the device. The booted case where a container path comes back and the file is missing does throw first (`agent-inspect.mjs:335`).
   Fix: call `devicectl` only when `simctl` reports `Invalid device` (exit 148). On Shutdown, copy `tmp/trace-latest.json` from that device's on-disk data container, or boot and retry `get_app_container` (`simulator()` already boots, `host/apple/devices.mjs:46-51`). Any other recognized-simulator failure throws `simctl`'s stderr. Status 0 with empty stdout throws the unsaved error. Pin those four outcomes with the `run` argument.

2. **MINOR** — A wrong `phoneTrace` branch stays green. `FrameSamplerTests.swift:48-55` pins the literal `trace-latest.json` and that a second save replaces those bytes. Nothing calls `phoneTrace` (`scripts/agent-inspect.mjs:327`), so finding 1 does not fail a check.
   Fix: the four `run` cases in finding 1. The share sheet's mid-screen anchor matches the dev action sheet (`DevMenuIOS.swift:118-121`, `:84-87`); round 1 deferred a UI test for it.

Round 1's other taken items hold. The destination name is one path component (`agent-inspect.mjs:329`). `phoneTrace` is in `agent-inspect.mjs`, and `scripts/agent.mjs` is 1,500 lines by `caps`' `split('\n').length`. The usage is `trace --phone <name|udid>` and `trace --device` (`agent.mjs:16`, `:1379`). A production bake returns before either write (`FrameSampler.swift:161`; writes at `:185-189`); Save Trace is added only when a sampler exists (`DevMenuIOS.swift:81`). Share… is compiled out of tvOS (`:103-108`, `:113-125`). The device copy uses the screenshot `devicectl` argv (`agent.mjs:683-685`) and, as the as-built says, is still unrun on a phone.
