# Code review: feedback nudges and one-line notes, round 3 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `88cffa5258`.
- **Method:** one brief (sha256 `ed23328dd0daf018498e0ca7f45859b3bb23bce96de91307a446b7529d6e576f`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `83fde38703` except cross-process locking of two simultaneous sends, documented as uncoordinated; round 4 reviews it.

Round 2 holds. Gutter lines stay out, refused flags and a bare `--backend` are logged and nudged, a signal still suppresses the nudge after a later exit code, the relayed stdout pipe is drained, a note written during `send` stays unsent, the day's nudge and the disclosure are created with `wx`, and the docs say commit. A send still requires standing `always` or a yes to `ask`. A `*` entry can only be `local` or `never`. `EXACT_FEEDBACK=0` refuses every send.

### MAJOR

**Exact's own compiler errors are dropped.** `scripts/feedback-note.mjs:67` keeps a `file:line:col` line only when the next word is `error`, `warning`, or `fatal`. `CompileError` prints `app.contract:14:10 [syntax-expected] …` with no such word (`contract/cli/src/lib.rs:215`), and a failed build script indents it (`  app.contract:3:1 [lower-unknown-attr]`, `scripts/contract-diagnosis.test.mjs:16`). Both are dropped. The line that remains is Cargo's `error: failed to run custom build command`. `docs/diary.md:32` describes `<file>:<line>:<col>: error`, which this compiler does not emit. Excerpt lines (` 14 | source`, `contract/cli/src/excerpt.rs:56`) correctly stay unmatched. Fix: also accept `/^\s*\S+:\d+:\d+\s+(?:warning\s+)?\[[\w-]+\]\s/`, and add that fixture next to the signing-team case.

**The value mask still lets secrets and app paths through.** `scripts/feedback-note.mjs:73` keeps a quoted token of up to 31 identifier characters when it has no four-digit run. Probed and kept whole: `"hunter2"`, `"sessionToken"`, and `"sk_live_shortkey"`. `redact` only matches `sk_live_` when 16 characters follow (`scripts/feedback.mjs:61`), so the short key survives the diary too. Line 66 also keeps every `note:` line. Drives print those as advice (`scripts/agent-test.mjs:466`), including `mutation … refused: ${why}` (the data module's message) and file paths; `assets/secret-chime.wav` was kept, and those lines fill the five slots. Fix: drop bare `note` from the keyword pattern. Keep a quoted token only when it is lowercase, at most 15 characters, and has no digit (`"x"`, `"line-clamp"` still pass). Mask any `sk_`/`pk_`/`rk_` prefix at any length before that exemption.

**A non-TTY native build no longer has one stdout/stderr stream.** `scripts/app-cli.mjs:191` turns `[0, 2, 2]` into two pipes. Alternating `writeSync(1)` and `writeSync(2)` came back as every stdout line, then every stderr line. Agents have no terminal, so this is the drive path, and the log they read is reordered. Fix: pass one write end in both slots. Doing that preserved `OUT i` / `ERR i`. Drain that single stream with the existing 300 ms close.

### MINOR

**`never` still stores the error tail.** `scripts/app-cli.mjs:169` calls `recordLast` before the standing answer is checked, so `.exact/last-command.json` keeps those lines after `never`. Skip `recordLast` when `standing(dir) === 'never'`.

**The docs say every note carries the command's diagnostics and whether that command had a terminal.** `docs/diary.md:31` and `README.md:338`. The tail is stored only when stderr is not a terminal (`scripts/app-cli.mjs:154`), and `scripts/feedback-note.mjs:103` reports the note process's stdin. Say both.

VERDICT: CHANGES
