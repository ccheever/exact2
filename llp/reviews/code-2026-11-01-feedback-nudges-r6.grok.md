# Code review: feedback nudges and one-line notes, round 6 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `ef7c888a18`.
- **Method:** one brief (sha256 `8662dfab98f32c9dda002366af7d37d843e3091fe100d26b91c8fda002181108`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** superseded. The lead ruled (2026-11-02) that a note attaches no command output, so the stderr relay these findings concern was removed; the simplification has its own review.

**MAJOR** `scripts/feedback-note.mjs:83`. A second `{` that never closes is not masked, so an `always` note can send the value. `objects()` only sees the match of `/\{.*\}|\{.*$/`, and that pattern is not global. A closed object ends the match at the last `}`, and the rest of the line is left as written. The 240-character cut in `scripts/app-cli.mjs:164` makes this the usual shape of a long line, because it drops the later `}`.

```javascript
errorTail('test: got {ok:1} then {card:4111111111111111')
// ['test: got {…} then {card:4111111111111111']
errorTail(("test: got {ok:1} then {card:" + "9".repeat(400) + "}").slice(0, 240))
// the 9s remain
```

`recordLast` stores that tail, and `redact` does not treat digits as a secret. Run `objects` on the whole line after quote masking. It already collapses closed objects and rewrites an unclosed `{` through the end of the line. Add both strings to `scripts/feedback-note.test.mjs`.

**MINOR** `scripts/feedback-note.test.mjs:242`. The slow-reader test sets `EXACT_FEEDBACK=0`, and `heard` (`scripts/feedback-note.mjs:21`) is then false, so neither relay runs. `ios` inherits the caller’s pipe. `agent ios` uses `[0, 2, 2]` on that same pipe, not the temp file. The test still passes if the pause or the chunk pump breaks. Set the project to `never` instead. That keeps the nudge off and the bytes exact, and leaves the relay on.

VERDICT: CHANGES
