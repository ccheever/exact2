# Code review: feedback nudges and one-line notes, round 1 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `a1ee449658`.
- **Method:** one brief (sha256 `7800da7566c084671d8881592dd7d092ee38606c0eb8535e5641cfa816044e7c`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `4536dc5e10` (Astra's) and `2e7a214d38` (Grok's); round 2 reviews `2e7a214d38`.

The allowed tests: 11 passed, 1 failed. Consent paths look sound (`never` / `local` / `*` / `EXACT_FEEDBACK=0` do not send). Two Majors should be fixed before landing.

## MAJOR

**`[0, 2, 2]` native-drive builds never enter the error tail** — `scripts/app-cli.mjs:176` and `:274`

Relay runs only when `stdio === 'inherit'`. `test ios` / `agent macos` / `test windows` spawn `agent-build.mjs` with `[0, 2, 2]`, so the child writes straight to fd 2 and bypasses the `process.stderr.write` hook. A failed signing/compile (the case the new note is for) leaves `.exact/last-command.json` with empty `stderr`. `ios`/`verify` inherit and are captured; the integration test never runs `test ios`.

Fix: when `hear` is set, spawn that build as `['inherit', 'pipe', 'pipe']` and forward **both** pipes through `process.stderr.write` (keep JSON stdout clean). Wait for both streams to end (keep the 300ms cap). Add a `test ios` fixture that fails in `agent-build` and assert the note quotes the summary.

**Integration test fails when stderr is colored** — `scripts/feedback-note.test.mjs:117`

`bun test scripts/feedback-note.test.mjs scripts/feedback.test.mjs` failed here: Bun painted `console.error` (`FORCE_COLOR`), so lines are `\x1b[31mexact: …\x1b[0m` and the regex that wants `\nexact:` does not match. `errorTail` already strips CSI for the diary; the assertion does not.

Fix: in `drive()`, set `FORCE_COLOR: '0'` and `NO_COLOR: '1'`, or strip CSI before `assert.match`.

## MINOR

**README disagrees with the nudge kinds** — `README.md:337`

It says a finished drive, build, or verify prints the line. Code only prints the success line for a passing `test` or `verify` (`scripts/feedback-note.mjs:48`). Match `docs/diary.md:37-39`.

**`feedback status` omits “What is sent”** — `docs/diary.md:7-8`, `scripts/feedback.mjs:181-185`

The intro still says AGENTS.md names status “in one clause” (it is now a section). `instructions()` still starts at `## What to keep`, so status never names Expo or `EXACT_FEEDBACK=0`. Point the intro at the Feedback section, and start `instructions()` at `## What is sent, and stopping it` (still skip all of that for `never`).

**Repo `AGENTS.md` does not say where it goes or how to stop** — `AGENTS.md:12-15`

README and `docs/diary.md` do. Add “goes to Expo only if this project’s answer allows; `feedback never` or `EXACT_FEEDBACK=0` stops it.”

**`errorTail` only drops gutter/caret lines** — `scripts/feedback-note.mjs:59-62`

A source line with no `|`/`│` (older clang, tsc pretty `10 const x =`) is kept. Exact’s `tsc --pretty false` and current Swift gutters are covered; the test only uses `  4 | …`. Drop `^\s*\d+\s+\S` as well, and add a no-gutter fixture.

**Relay timeout leaves the pipe open** — `scripts/app-cli.mjs:191-193`

Grandchildren holding stderr: `run()` returns after 300ms and generated `exact.mjs` `process.exit`s, so the CLI does not hang. Lines after 300ms still print and never reach `recordLast`. After the timeout, `destroy()` the child’s stderr (and stdout if you pipe both).

Consent, `never` keeping nothing, `*` only withholding, signal silence (`exit >= 128`), daily throttle, TTY skip, and the 400-byte Feedback cap all look right.

VERDICT: CHANGES
