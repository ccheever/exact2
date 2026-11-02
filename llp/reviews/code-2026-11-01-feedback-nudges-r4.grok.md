# Code review: feedback nudges and one-line notes, round 4 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `83fde38703`.
- **Method:** one brief (sha256 `33ffbd7972aaa8f689e94e419f76b0d6bd4ad6fe07eb5b3e4f30122b2499546c`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `bc5fec056e`.

Round 3 holds. Contract `file:line:col [id]` lines are kept, `note:` lines are dropped, the "before it answered at …" server tail is cut, `pending()`'s snapshot is what `archive` sends, a child-only signal suppresses the nudge, `[0, 2, 2]` stays ordered through one temp file, `never` does not write `last-command.json`, and error lines are captured only when stderr is not a terminal.

**BLOCKER** `scripts/feedback-note.mjs:83` and `:86`. The password key must sit on a word boundary, and only an `http(s)` query is stripped. Both layers leave the value in the diary, and `redact` (`scripts/feedback.mjs:66`) does not backstop it (`\b` plus an 8-character floor). `always`, or a yes to `ask`, then posts it.

- `errorTail('error: DB_PASSWORD=hunter2 rejected')` → `error: DB_PASSWORD=hunter2 rejected`
- `errorTail('Error: EXACT_TOKEN=abc123secret')` → unchanged
- `errorTail('error: connect postgresql://app:s3cret@db.internal/prod?sslpassword=hunter2hunter')` → `error: connect postgresql://…@db.internal/prod?sslpassword=hunter2hunter`

`withAppServer` forwards server stderr (`scripts/verify.mjs:390`), and `Error:` lines are kept. Fix: allow a prefix on the key, `\b([\w.-]*(?:password|passwd|pass|secret|token|api[_-]?key|auth\w*|cookie|session)\w*\s*[:=]\s*)\S+`, and strip `[?#]…` for every scheme, using the same `[a-z][\w+.-]*://` class as the userinfo replace. Use that key pattern in `redact` too.

**MAJOR** `scripts/feedback-note.mjs:80` and `:81`. Quoted values and `{…}` are masked in one pass, so a quote or brace inside the value ends the match.

- `error: expected "say \"hello secret\""` → `error: expected "…"hello secret\"…"`
- `error: got {meta:{ok:1}, card:4111111111111111}` → `error: got {meta:{…}, card:4111111111111111}`

That is source text and a secret on a line the note keeps. Fix: treat `\` plus the next character as part of the quoted body, and repeat `\{[^{}]*\}` until the line stops changing. `scripts/feedback-note.test.mjs:70` only covers flat quotes, one object, and an `https` query.

**MINOR** `scripts/app-cli.mjs:158`. `heard` keeps the last 8192 characters, then `errorTail` keeps diagnostic lines inside that slice. A long trailing line cuts off every earlier diagnostic. Reproduced with `agent ios` and no terminal: a 100KB `error: BIG …` line followed by `error: TAIL unique` left `last-command.json` as `["error: TAIL unique"]` (exit 65, full stderr otherwise intact). Fix: keep a rolling buffer of the last few complete lines (or run the diagnostic filter before the cap) instead of slicing the raw stream.

VERDICT: CHANGES
