# Code review: the test grammar's wheel step and Excord's macOS twins, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `4774cf8bd3`.
- **Method:** one brief (sha256 `e8d36ca57ecc3e4cbf77b8ca81fb457018a06a04ee6943379f5a863722a0f4be`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

Clean.

The three failed-history tests in `apps/excord/history.test.contract` match their pre-move bytes, and the fourth matches after its `on web, ios, tvos, android, linux, windows` clause. The file's `epoch` and `time-zone` lines follow `routing.test.contract`. `app.test.contract` is 1,422 lines. Nothing else names those tests by file. `verify` picks up every `*.test.contract` beside the app.

`testScope` maps `mac` to `macos` and is what `runTests` matches against an `on` list. The seven grammar hosts are split between the drag tests and the `on macos` twins.

`tap "id" wheel <dx> <dy>` follows the pinch path through the parser, JSON (`"form":"wheel","wheel":[dx,dy]`), and `s.tap(t, {wheel})`. A positive `dy` grows the offset on the hosts that apply it: the Mac agent sends pixel-unit scroll events with precise deltas, so `-2500` moves 2,500 points toward the start, which is the sign and unit the new Mac twins use.
EXIT=0
