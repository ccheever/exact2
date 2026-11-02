# Code review: three test-drift main reds (verify, windows, Snapback registry) (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `45750c2e7c`.
- **Method:** one brief (sha256 `6ecc05b27737364040a993289b8d52e4b98589dd03c8f18bccd5548059a4210a`); a quick review under the main-red permission. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** the MINOR is addressed in the landed verify.test commit: `plain()` and `agent()` run with EXACT_FEEDBACK=0 too.

- **MINOR — [scripts/verify.test.mjs:803](/Users/admin/x2scratch/fb1-rv-reds/scripts/verify.test.mjs:803):** Feedback isolation is incomplete: `plain()` still inherits feedback settings, and its entire stderr is checked at line 836. It normally passes because the earlier failed `plain()` at line 824 consumes that day’s friction nudge. Crossing UTC midnight between those calls permits another nudge and fails the assertion. Give `plain()` the same `EXACT_FEEDBACK: '0'` environment override.

The remaining changes are minimal corrections to test drift: the manifest supplies `appId`; shared validation rejects malformed coordinates; an undeclared route returns 404; and the password fixture explicitly enables the documented reset endpoint while retaining authentication assertions.

Static review only; no tests run.

VERDICT: CHANGES
