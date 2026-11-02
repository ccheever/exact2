# Code review: feedback nudges and one-line notes, round 2 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `2e7a214d38`.
- **Method:** one brief (sha256 `fb156b9ed13d14d8f7746fb3f0aeae6cc32c9c8977b711d7079536473dba119e`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `88cffa5258`; round 3 reviews it.

**MAJOR** — Quoted payloads and command lines still go into the note — `scripts/feedback-note.mjs:67`

`errorTail` collapses a quote only when the closer falls inside 400 characters. A longer quote is left as-is and `slice(0, 240)` then keeps the front of it, so `error: expected "<hundreds of characters of source or fixture>"` is stored and, under `always`, posted. A quote matching `[\w.:/@-]{1,40}` is kept, which covers `alice@example.com`, a short path, and a 32-character token. `redact` does not strip emails or that shape. The same allowlist (`:64`) keeps Cargo's `process didn't exit successfully: \`rustc\` … (exit status: 101)` because of the word `exit`, and each short backticked argument survives, so a failed native build attaches the compiler command. An unnumbered source line that itself contains `error`, `missing`, `invalid`, or `unknown` is kept as well. The gutter patterns only drop numbered lines and `|` lines.

Fix: if a quote is not closed within the cap, replace from the opener through the end of the line. Keep a quoted body only when it matches `^[A-Za-z_][\w-]{0,30}$`. Drop indented lines that lack an `error:`/`warning:` prefix. Add an email pattern to `redact`.

**MAJOR** — The grace-period close never runs once stderr has ended — `scripts/app-cli.mjs:206`

Native `test` and `agent` drives spawn `agent-build.mjs` with `[0, 2, 2]`, and the relay turns that into two pipes (`:191`). On `exit`, if stderr has already ended, the handler returns immediately and does not reach the 300ms `destroy` of stdout (`:208`). `end` on stderr also clears that timer while stdout is still open (`:209`). A grandchild that inherited the stdout pipe, with stderr already closed, leaves the `data` listener active, so the parent stays up after `recordLast`. Stdout bytes that arrive after that resolve are missing from `heard`. The new test only writes one `console.log` and then `process.exit`.

Fix: resolve only after every relayed stream has ended, and arm the grace timer whenever any relayed pipe is still open. On expiry, `destroy` both pipes, then resolve. Clear the timer only when both have ended.

**MINOR** — The docs describe a different note than the code — `game/new.mjs:907`, `docs/diary.md:7`, `scripts/feedback.mjs:209`

The generated Feedback section says the note attaches exact2's version. The note stores `git rev-parse --short=9`. README and `docs/diary.md` say commit. `diary.md` says `feedback status` prints where feedback goes first. `status` returns on `never` before that sentence. `diary.md:37` says a line that does not read as an error is dropped. The allowlist also keeps `warning`, `missing`, `unknown`, and `invalid`.

Fix: say "commit" in `DIARY_LINE`. Print the Expo / `EXACT_FEEDBACK=0` line for `never` as well. In `diary.md`, say "diagnostic" if warnings are supposed to be kept.

The 13 feedback tests pass. They do not cover a quote longer than 400 characters, a Cargo `exit status` line, or a stdout pipe that outlives stderr.

VERDICT: CHANGES
