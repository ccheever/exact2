# Code review: the interim conform marker for apps whose wasm reference binds no packages (photo-editor, recorder, markdown), round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ad9dbc5c52`.
- **Method:** one brief (sha256 `6ab1fc994bb89902a2fa96357c91d0ae8632539c53a00f4e6a49aba2bfafc904`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. A refused reference tap on a marked target is now a plain failure and stays one. `referenceTapRefused` (`host/web-js/conform-marked.mjs:42-44`) calls `fail(step, …)` with no difference flag when `crossBrowser` is null and the target is in `WASM_UNBOUND`. The drive’s catch is that call (`host/web-js/conform.mjs:586`), and `fail` records `difference: true` only when its fourth argument is truthy (`host/web-js/conform.mjs:288-298`). `markUnbound` moves an entry only when `f.difference === true` (`host/web-js/conform-marked.mjs:27`), which runs before `--strict` (`host/web-js/conform.mjs:927`, `976-981`). The tap id is added to `tapped` before the attempt (`host/web-js/conform.mjs:584`), so the refusal is one failure and the run stays red.

The same class does not remain on the wasm/JS pair. These three apps have no `.steps` file, so the automatic tap loop is their only discovered operation. A scripted reference refusal already calls `fail` and does not run the JS page (`host/web-js/conform.mjs:561`). A clock both pages refuse still runs on both and is then compared (`host/web-js/conform.mjs:557-578`); a one-sided or malformed clock is a `fail`. The cross-browser touch skip is gated on `crossBrowser` (`host/web-js/conform.mjs:553-555`), and `markUnbound` does not run in that mode.

The Linux catch at `host/web-js/conform.mjs:413` still records a skip and stops the Linux comparison. That is the pre-existing follow-up already left for later: the wasm and JS pages have already taken the step.

**Non-blocking**

none. `differ` (`host/web-js/conform.mjs:358`) is the only `fail(…)` that passes the difference flag. The seven wasm-vs-JS sites call it: state `429`, tree `432`, layout `450`, paint `462`, screenshot `491`, bound refusals `533`, sound lines `534`. Linux comparisons stay plain `fail` calls (`441`, `454`, `537-538`).

The source test (`host/web-js/conform.test.mjs:189-201`) is not vacuous. It requires that exact helper line, exactly seven `differ(step,` lines in this order with those call shapes, no other line matching `fail(…, true)` or `fail(…, !crossBrowser)`, the two main Linux `fail` lines, and the drive’s `referenceTapRefused` call. Inlining a flag, adding an eighth site, or dropping a site fails it. `conform.test.mjs:155-163` still rejects a fourth app or a changed reason, and `203-215` keeps a marked tap refusal in `failures` with `marked` empty.

APPROVE
