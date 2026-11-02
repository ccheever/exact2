# Code review: feedback nudges and one-line notes, round 4 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `83fde38703`.
- **Method:** one brief (sha256 `33ffbd7972aaa8f689e94e419f76b0d6bd4ad6fe07eb5b3e4f30122b2499546c`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `bc5fec056e`.

1. **MAJOR — Raw server stderr bypasses the refusal scrub.** [scripts/app-cli.mjs:158](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:158) captures every parent stderr write. `withAppServer` enables forwarding (`scripts/verify.mjs:390`), which writes the server’s raw stderr at line 244. Reproduce with a recognized server that prints `error: customer Alice balance 500` and exits, then run `test` without a terminal and add a feedback note. The refusal suffix is masked, but the separately forwarded line survives `errorTail()` and `redact()` unchanged; `always` sends it. **Fix:** exclude server-forwarded bytes from feedback collection while preserving their terminal output. Cover the actual forwarding path in an integration test.

2. **MAJOR — Compiler values and escaped strings still leak.** [scripts/feedback-note.mjs:80](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.mjs:80) preserves every short lowercase compiler string, including app values. A partitioned iframe with `src="supersecret"` produces `lower-iframe-partition-src` (`contract/lower/src/frames.rs:92`); its diagnostic retains `"supersecret"` through both filters. Separately, the quote matcher ignores escaping: `errorTail('error: ' + JSON.stringify('"private-diagnostic-payload"'))` retains `private-diagnostic-payload`. Both were reproduced with read-only calls. **Fix:** parse escaped delimiters correctly and restrict preserved names to recognized property/tag positions, rather than exempting arbitrary lowercase strings. Add regressions for both cases.

3. **MAJOR — The new relay exposes unredacted output through a shared temporary file.** [scripts/app-cli.mjs:192](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:192) opens the relay with `w+` and default permissions. Reproduction: with `TMPDIR=/tmp` and umask `022`, run a native drive with redirected stderr; while its build runs, another local user can read `/tmp/exact-relay-<pid>-1.txt`. It contains the complete unredacted build output, including source excerpts, before filtering. This also happens under `never`. **Fix:** create an exclusive temporary file with mode `0600`, preferably inside a private temporary directory, and retain cleanup on completion.

The permitted test command yielded two passes; eleven tests stopped at fixture creation because the sandbox denied `mkdtemp` (`EPERM`).

VERDICT: CHANGES
