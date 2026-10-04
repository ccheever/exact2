# Code review: platform timing's first clock (LLP 1080.000 §12), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `97374ee85`.
- **Method:** one brief (sha256 `666b968f99bb78a02712b38fb727bdaf66cd7af26bdbffc153084116974c8fdb`), the same one sent to grok; round 1; blind to the other review. Requested by the Signal effort's coordinator after the clone's agent found swipe-to-reply failing on build 13. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** all five taken in `bd7d2c0bf`, by a different design:
  1. The takeover sets `session.clock = session.now()` first, before any wait; nothing samples the wall later.
  2. `advanceStepped` takes a `floor` (the takeover time, or the clock as it stood) and never sets `session.clock` below it, intermediate stops and error exits included; the landed and replied clock is floored too.
  3. The driver no longer keeps one flag: before each platform-timing `clock` it sends `take` to the session it addresses (a no-op once that session is taken, and again after a new incarnation leaves the clock nil) and counts from that reply.
  4. No arithmetic on `to`: it stays absolute, and the runner catches up from wherever it stood, so a runner not at 0 is no longer assumed. The takeover is the explicit private field `take` on the existing `clock` operation.
  5. `AgentClockIOSTests` (in the iOS selection) boots the fixture, puts the motion engine at the wall with a `tick`, and asserts an error-free `take` at or past the wall, an accepted `holdBegin` after it, a no-op second `take`, a refused backwards target, and, for a first `{"to":100}`, the clock left at the wall and a hold accepted. §12's proof sentence says what it locks, and that it was not run against the old code.

---

DO NOT LAND

1. **Must-fix — [Agent.swift:406](/tmp/rv-clock/host/apple/Sources/ExactKit/Agent.swift:406).** The diagnosis is valid, but the handoff still permits reversal. `shift` is sampled before image/reply waits, while `session.clock` remains nil. Motion can advance during those waits; the first seek then freezes an older time. A subsequent idle `settle` cannot repair it. **Edit:** validate the request and freeze the takeover time before any run-loop waits.

2. **Must-fix — [Agent.swift:502](/tmp/rv-clock/host/apple/Sources/ExactKit/Agent.swift:502), [Agent.swift:436](/tmp/rv-clock/host/apple/Sources/ExactKit/Agent.swift:436).** Catch-up still overwrites the host clock with earlier runner times. With motion at 2,000 ms, a timer sending a request at 100 ms sets `session.clock = 100` while awaiting its reply; holds delivered then fail. A timer refusal can leave that reversal permanent. **Edit:** preserve the host’s takeover floor throughout intermediate advances and error exits; add request-producing and refusing-timer regressions.

3. **Must-fix — [agent.mjs:1079](/tmp/rv-clock/scripts/agent.mjs:1079).** `clockTaken` and `s.now` belong to the driver, although sample-host sessions have independent clocks. Example: a leaves `s.now = 1000`; switch to untouched b at wall 5000 and request `+2000`. Its first chunk targets 2000, gets shifted to 7000, then the final target 3000 is refused. **Edit:** track synchronization and current time per selected session, reconcile incarnation changes and authoritative clock replies, and preserve clocks across carried reloads.

4. **Must-fix — [Agent.swift:420](/tmp/rv-clock/host/apple/Sources/ExactKit/Agent.swift:420).** Unconditionally adding wall time assumes the runner starts at zero and makes absolute targets inconsistent. With runner 1000, wall 2000 and nil `session.clock`, the driver’s supposedly zero-length preflight lands at 3000. Raw `to:5000` also lands differently from `s.clock(5000)`, which preflights first. **Edit:** use the actual runner/host times for takeover; keep `to` absolute and express takeover/relative movement explicitly within the existing `clock` operation. Correct §12 accordingly.

5. **Must-fix — [AgentClockTests.swift:9](/tmp/rv-clock/host/apple/tests/ExactKitTests/AgentClockTests.swift:9).** The session never boots. Both forward calls return `clock: not booted`; assertions accept their fallback clock values, and the final assertion accepts any error. It would fail without the patch, but proves only arithmetic—not restored holds. **Edit:** boot a fixture, assert successful replies, advance real motion and verify a subsequent hold. Include this test in the iOS selection, which currently selects only `*IOSTests.swift` ([build.mjs:1476](/tmp/rv-clock/host/apple/build.mjs:1476)); revise §12’s proof claim.

Caps and diff whitespace checks pass. Web is excluded from the new handshake. No builds or runtime tests were run.
## Round 2, 2026-10-04

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `bd7d2c0bf`; brief sha256 `b20b2a233c4b47e77095e4dec7efc068ec07f327126f029c6ebefbadf211a87f` (round 1's brief, pointed at the whole change). Blind to grok's round 2.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all four taken in `1f94dd3fb`:
  1. `take` no longer seeks: it freezes the clock (under platform timing, once) and replies where it stands. The driver sends it only for a relative or absolute seek, never before `settle`, which takes over within its own call, so one `clock settle` keeps one 20 s bound.
  2. The takeover floor is `max(now(), the runner's clock)` (read from `tags`). A test clears the host's clock by hand after a seek 60 s ahead and asserts `take` reports at least that; no known path leaves the runner ahead with the host's clock unset, and the comment and §12 say so.
  3. The driver no longer depends on its own `timing` option: it sends `take` on the Apple hosts whatever the timing, and the host, which knows its own (`ExactEnv.agentFreezes`), takes over or just reports.
  4. §12 no longer names reorder (it clamps to the host's and the engine's time) and says only a target behind the motion engine's time set the clock back.

---

LAND WITH FIXES

1. **Must-fix — [scripts/agent.mjs:1079](/tmp/rv-clock/scripts/agent.mjs:1079).** `take` performs a full seek with its own 20-second deadline, followed by another for `settle`. With an overdue timer and an unanswered request, one `s.clock('settle')` can wait roughly 40 seconds, violating LLP 1012 §2. Skip the preliminary `take` for `settle`; the host already handles its takeover. Prefer making `take` only freeze/report time, leaving advancement to the requested seek.

2. **Should-fix — [Agent.swift:407](/tmp/rv-clock/host/apple/Sources/ExactKit/Agent.swift:407).** Wall time is insufficient when the runner already stands ahead. Example: runner/motion at 60,000, `session.clock == nil`, wall at 3,000; first `to:100` freezes at 3,000 and returns early, leaving subsequent holds rejected. Establish the floor from at least `max(session.now(), runnerClock)` before waiting or refusing. Add this pre-advanced-runner regression.

3. **Should-fix — [scripts/agent.mjs:1079](/tmp/rv-clock/scripts/agent.mjs:1079).** The takeover depends on the JavaScript `timing` option, while native timing can come from `env.EXACT_AGENT_TIMING` or inherited macOS environment. Thus `open({host:'macos', env:{EXACT_AGENT_TIMING:'platform'}})` skips `take`; an initial `clock('+600')` still targets 600 and now gets refused after sufficient uptime. Resolve one effective timing mode and use it for both launch environment and driver behavior, including `runTests`.

4. **Nit — [LLP 1080.000:1031](/tmp/rv-clock/llp/1080.000-real-touches-on-ios.rfc.md:1031).** Reorder does not share the claimed refusal: `arrange_now` already clamps against host and engine time. Remove reorder from that claim. Also qualify the rewind diagnosis to targets behind the existing motion clock; sufficiently large seeks did not rewind it.

The central diagnosis is correct. By inspection, the second new test fails on the old implementation for the reported reason. Changed sources meet the line cap; no builds, tests, or checks were run.
## Round 3 (the last), 2026-10-04

- **Method:** `codex exec` as before, `-C` a detached worktree at `1f94dd3fb`; brief sha256 `1c58093dbe554f4d664e20bd7cf910934219481a129aebb9cddc7f7d9d86fb15` (the whole change, report only what is still wrong). Blind to grok's round 3.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND.
- **Disposition:** the nit taken in the landing commit: §12 says the first successful seek after the takeover catches the runner up, not the takeover itself.

---

LAND

1. **Nit — [llp/1080.000-real-touches-on-ios.rfc.md:1061](/tmp/rv-clock/llp/1080.000-real-touches-on-ios.rfc.md:1061):** Takeover alone does not advance the runner; `take` returns immediately, and a rejected absolute target also leaves it behind. Replace “at the takeover the runner’s clock jumps to the wall” with “the subsequent successful seek catches the runner up to at least the takeover time.”