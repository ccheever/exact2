# Code review: a bound autofocus that is false is no attribute (web JS target), 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4e725bb59049cf57dc80db8d0568df4789a1c4a9d62bdbfd326d7e0abb864368`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings beyond Astra's NIT (more transitions in the plan, and conform's comparison omits props.autofocus), left: both reviewers traced the transitions and the agent's data attribute by hand.

---

No BLOCKER, MAJOR, or MINOR findings in `a1ba9277`.

- **NIT — [autofocus-bound.contract:6](/Users/admin/x2p2/apps/exact2-comp/host/web-js/conformance/autofocus-bound.contract:6):** The fixture only exercises initial `false`. Consider adding false→true→false→true transitions and explicit agent-property assertions; [conform.mjs:152](/Users/admin/x2p2/apps/exact2-comp/host/web-js/conform.mjs:152) omits `props.autofocus` from its comparison.

The implementation correctly follows [HTML boolean-attribute semantics](https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#boolean-attributes). Verified that:

- True→false removes the attribute without blurring existing focus.
- Re-enabling preserves the once-per-mount `offered` behavior.
- Carried-restart suppression and focus restoration remain consistent with wasm.
- Agent metadata switches correctly between true and false.

No other supported HTML boolean attributes were missing from `BOOL`. The inline change is reasonable under the cap; `rt.js` remains exactly 1,500 lines.

Validation: 11 existing autofocus tests passed, plus in-memory checks using the production signal, property, focus, checkpoint, and agent code. Browser conformance was not rerun because it writes artifacts.

READY
