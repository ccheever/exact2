# Code review: feedback nudges and one-line notes, round 5 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `9294d18f70`.
- **Method:** one brief (sha256 `bfcf205620d3276b47ef29c757a17925dd11eab0b4400da6f01446e60890d06e`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** both findings (shared by both reviewers) are addressed in `247ad6a041` and `ef7c888a18`.

**BLOCKER** — Relayed stderr keeps a suffix and drops the prefix. `scripts/app-cli.mjs:209` and `:211`.

`child.stderr` is drained immediately and forwarded with `process.stderr.write`, which queues. The launcher then calls `process.exit`. A caller that has not read yet loses the head of the log. With 20,000 lines, an eager reader got the whole stream (`ios` 980,000 bytes, `agent ios` merge 320,000) and the real exit code. Pausing both pipes until `exit` left `ios` at 261,421 bytes starting `yyyyy\nL005350`, and the merge at 57,856 bytes starting `O016384`, codes `3` and `9`. Holding the process open instead retained the full 1.4 MB dump in memory.

Fix: record the tail, then `writeSync` each chunk to `process.stderr.fd` (copy it out of the pump buffer first) and pause the child pipe while that blocks. Return from `spawned` only after that write finishes. Test a caller that does not read stderr until `exit`.

**MAJOR** — A 240-character cut turns a closed single quote into a leak. `scripts/feedback-note.mjs:82`, `scripts/app-cli.mjs:163`.

Closed `'` quotes are masked. The unclosed sweep is only `"` and `` ` ``. The relay stores `line.slice(0, 240)` before `errorTail`, so the closer often never arrives. `errorTail("test: expected '" + secret + "'")` is `test: expected '…'`. The same line sliced to 240 still contains `supersecret-token-`. `test: process didn't finish 'supersecret` is kept whole, and `redact` does not blank it.

Fix: while quotes are still placeholders, mask an unclosed `'` that is not an apostrophe, `.replace(/(?<![A-Za-z])'.*$/, '…')`, and add this sliced line beside the escaped-quote fixture.

`bun test scripts/feedback-note.test.mjs scripts/feedback.test.mjs`: 13 pass. Those tests read stderr as it arrives and only close `"` / `` ` `` quotes, so they miss both cases.

VERDICT: CHANGES
