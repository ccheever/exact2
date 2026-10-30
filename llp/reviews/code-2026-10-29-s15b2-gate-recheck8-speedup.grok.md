# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck8-speedup, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `d3c24db63c56b8205c39d58c21d8d53c1bce998a1daa3041c2d0e387ba189c00`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited.
- **Verdict:** request changes (1 medium: P2).
- **Disposition:** taken in 80ac1b403a.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch, whose reviews are otherwise clean (re-check 7, /tmp/iosfix-rv-g9-brief.txt). Read-only: no checkout, restore, stash, reset or edits.
This commit speeds the drives:
- driveScreens: a web or web-wasm shot now runs the SDK's scripts/agent.mjs directly (`web --app <the copy's name from its exact.mjs>`) with EXACT_APP_DIR=<copy> and EXACT_WEB_DIST=<the side's dist> (cloneWebEnv). Before, it ran the copy's `exact.mjs agent web`, whose app-cli rebuilt the whole 9.5 MB app on every drive: about 2.5 min a shot, now 7 s. The agent's own assertWebDistApp still refuses a dist that isn't a complete build of the selected app (scripts/agent.mjs). Native hosts are unchanged.
- baseline mode now builds the side first (buildSide(copy, 'before'), every clone host) and shoots with CARGO_TARGET_DIR at that side's target. Before, it drove whatever was built.
- consumerPixels already builds both sides (buildSide) before shooting.
Check:
- that the dist driven is always the side's freshly built one: consumerPixels, baseline, and the screen reruns (--screen) that call drive;
- that nothing else the copy's exact.mjs did for an agent drive is lost on the web: app-cli.mjs `run` for verb agent, host web (warnings, backend origin preflight, --chrome, --scale);
- the name regex on the copy's exact.mjs;
- the retry and keyboard paths still using the same drive options;
- tests.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD on the s15b2 gate branch, read-only, against the brief and the drive-speed changes. First I'll read the review instructions and the commit itself.The brief is the prior re-check; this pass is only the drive-speed commit at HEAD. I'll inspect that diff and the call sites it changes, without touching the tree.The diff is only `scripts/stage12-gate.mjs`. I'll trace `cloneWebEnv`, `buildSide`, and the web agent path to see whether the side's dist is what actually gets driven.[P2] Pass an absolute app directory to the web agent — `scripts/stage12-gate.mjs:191`

A web shot sets both `cwd` and `EXACT_APP_DIR` to the raw `copy` string. The agent resolves `EXACT_APP_DIR` from that working directory, so a relative `--copy` or `--before` is applied twice. `bluesky` becomes `…/bluesky/bluesky`, and `../work/bluesky` becomes `…/work/work/bluesky`. `resolveApp` then throws `no app at …`, and every web and web-wasm shot fails. `consumerPixels`, including `--screen` reruns, passes those paths through unchanged. `baseline` mode does not, because it passes `resolve(opts.copy)`. Set `EXACT_APP_DIR` to `resolve(copy)`.

The side dist itself is the one `buildSide` just wrote: `consumerPixels` and `baseline` both build into `target/stage12-gate-<side>` and drive with that `CARGO_TARGET_DIR`, and `cloneWebEnv` points `EXACT_WEB_DIST` at that side's `web-js-dist` or `web-wasm-dist`. Retry and keyboard redrives reuse that same `drive` options object. The name regex matches the generated launcher (`name: "<app>"` from `JSON.stringify` in `game/new.mjs`). On a web agent drive, backend preflight still runs in `scripts/agent.mjs`, and `--chrome` / `--scale` were only ever injected for Apple hosts, which still go through the copy's `exact.mjs`. Compiler warnings are printed by `buildSide`'s `web-build`, not again on each shot. `scripts/stage12-gate.test.mjs` never launches a web drive, so it does not catch the relative path.

VERDICT: CHANGES
