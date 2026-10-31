# Code review: the interim conform marker for apps whose wasm reference binds no packages (photo-editor, recorder, markdown), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `275683996c`.
- **Method:** one brief (sha256 `330411408f929702091a4067cb442ab70ac477cf54e54bb69cfb0ace1c300e93`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **A refused reference tap can still pass `--strict`.** At [conform.mjs:583](host/web-js/conform.mjs:583), an automatic `W.tap()` rejection becomes only a skipped step; `J.tap()` never runs. Recorder provides a concrete path: an unavailable microphone disables its Record button ([app.contract:59](apps/recorder/app.contract:59), [app.contract:157](apps/recorder/app.contract:157)). The skip predates HEAD, but marking the remaining differences at [conform.mjs:924](host/web-js/conform.mjs:924) now permits a green run despite that refused operation. This violates the stated non-difference-failure requirement. Record the failed tap through `fail()` without `difference: true`, and add a regression exercising this drive path.

**Non-blocking**

1. **Existing Linux exceptions are not universally failures.** [conform.mjs:410](host/web-js/conform.mjs:410) catches arbitrary Linux-operation exceptions and stops comparison with a skip, without calling `fail()`. Actual Linux differences remain failures, but the broader “every Linux failure fails” claim is false. This predates HEAD. Separate intentional unsupported-operation stops from unexpected errors, recording the latter as failures.

2. **The classification test checks supplied flags, not their production assignment.** [conform.test.mjs:167](host/web-js/conform.test.mjs:167) supplies realistic-looking entries but manually labels differences. It covers state/tree/screenshot, while omitting layout/paint/refusal/sound differences, Linux entries, and cross-browser execution. Extend coverage through the failure-producing paths; this would expose blocking item 1. The exact-key and literal-reason assertions **do** reject a fourth app or changed reason ([conform.test.mjs:158](host/web-js/conform.test.mjs:158)).

The remaining checks pass by inspection:

- Exact own-key matching excludes synthetic and prefixed names ([conform-marked.mjs:26](host/web-js/conform-marked.mjs:26)); cross-browser runs bypass marking entirely ([conform.mjs:924](host/web-js/conform.mjs:924)).
- All seven listed comparison categories receive the difference flag. Recorded build/open errors, exceptions, scripted-step failures, app-test failures, and Linux comparisons remain unmarked.
- Marked counts and details reach JSON, Markdown, and stdout before strict exit is decided ([conform.mjs:931](host/web-js/conform.mjs:931), [conform.mjs:945](host/web-js/conform.mjs:945), [conform.mjs:971](host/web-js/conform.mjs:971)).
- The async lane discovers these tests, retains all three apps and cross-browser runs, and parses `FAIL` rather than `MARKED` lines ([async.mjs:115](scripts/async.mjs:115), [async.mjs:166](scripts/async.mjs:166), [async.mjs:294](scripts/async.mjs:294)).

Static review only; no edits, network, tests, or builds.

REQUEST CHANGES
