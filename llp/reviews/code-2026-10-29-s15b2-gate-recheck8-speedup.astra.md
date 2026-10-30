# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck8-speedup, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `d3c24db63c56b8205c39d58c21d8d53c1bce998a1daa3041c2d0e387ba189c00`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium: P2, 1 low: P3).
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

- **P2 — [scripts/stage12-gate.mjs:187](/tmp/iosfix-rv-g10/scripts/stage12-gate.mjs:187): Relative `EXACT2` selects a different SDK.** The copy’s launcher resolves it against the copy directory; `sdkOf(copy)` resolves it against the gate’s working directory. Baseline mode can now build successfully, then fail every web drive by invoking a nonexistent or different SDK. Reproduced with `EXACT2=../iosfix-blank-rows`. Resolve the override against `copy`.

- **P3 — [scripts/stage12-gate.mjs:187](/tmp/iosfix-rv-g10/scripts/stage12-gate.mjs:187): Web compiler warnings are lost.** Bypassing `app-cli.run` removes `webBuildQuiet`/`warningLines`, while `buildSide` discards successful build output. Warnings therefore disappear from shot logs, and `.exact/warnings-web.txt` is no longer refreshed. Preserve those diagnostics once during each side’s build.

Validation: 16 selected tests passed. In-memory probes confirmed matching build/drive dist paths, both copies’ name regex matches, and identical retry/keyboard options. Full builds and screenshot drives were excluded under the read-only constraint.

VERDICT: CHANGES
