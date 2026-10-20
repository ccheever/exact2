# Code review: caps runs the helps' flag parity before every push (scripts/help-parity.mjs), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4f4583b361`.
- **Method:** one brief (sha256 `723922e6eac3496b2ce131ed49ec7f50cc0eba6ead7f3517bea8f2b6c70a43d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; five should-fix, one nit
- **Disposition:** taken in ed11a1803c: every slice fails closed; a source error leaves the other checks running; a missing verb is said; the --help all rows check carried over (help:hidden); caps tested failing in throwaway repositories; each limit said alone. The wording and form assertions stay only in help.test (no flag edit moves them), as the module's header says

No blocking findings. Six findings from static review of `4f4583b`:

1. **Should-fix — Most source slices do not fail closed.** In [scriptFlags](/tmp/rv-helpcaps-1/scripts/help-parity.mjs:25), `between` and `readiness` use unchecked `indexOf` results. A missing start marker can produce an empty slice. For `add` or `docs`, whose documented flags are empty, that silently passes. This weakness predates the extraction, but contradicts the new helper’s promise to reject moved slices. Validate both boundaries and their order, as `pairTtlOf` does.

2. **Should-fix — A missing short-help verb discards accumulated findings.** The [short-help loop](/tmp/rv-helpcaps-1/scripts/help-parity.mjs:68) evaluates `helpText(verb).length` before checking `HELP[verb]`. Removing `web`, `test`, or `agent` first records `help:verb`, then throws because `helpText` returns `null`. Caps fails closed, but reports only the generic exception, losing the useful missing-verb diagnostic and other accumulated problems. Check existence before rendering.

3. **Should-fix — One source error suppresses unrelated checks.** The [source-error catch](/tmp/rv-helpcaps-1/scripts/help-parity.mjs:55) immediately returns. A moved `pairTtlOf` plus an undocumented flag in `verify` produces only `help:sources`; help lengths, index budgets and forms are also skipped despite remaining independently checkable. Preserve source errors while continuing checks whose inputs are available.

4. **Should-fix — Equivalence covers the listed comparisons, not every assertion in the containing tests.** The helper omits assertions from [the short-help test](/tmp/rv-helpcaps-1/scripts/help.test.mjs:109) and [the index test](/tmp/rv-helpcaps-1/scripts/help.test.mjs:132): `More`/`Ports`, flags appearing in rendered full help, absence of `More` in full help, prescribed web/agent wording, `status` avoiding `undefined`, required index forms, and the formless-index assertion. For example, removing `Ports` or the required `verify` form can pass caps while failing help.test. These are additional pure checks to carry over if full equivalence with those test blocks is required.

5. **Should-fix — The fixtures do not prove caps rejects parity failures.** The [copy-and-edit fixture](/tmp/rv-helpcaps-1/scripts/help-parity.test.mjs:14) usefully exercises parser additions/removals and one broken marker, but calls the original helper with the original `HELP`. It cannot exercise help-side edits or checkout-local module loading. The caps assertion checks only a success summary, ignoring exit status; deleting caps’s problem-forwarding loop would leave these tests passing. Add an isolated negative integration fixture asserting diagnostics and exit status, including a throwing helper and multiple simultaneous problems.

6. **Nit — The full-help remediation is ineffective.** The [36-line diagnostic](/tmp/rv-helpcaps-1/scripts/help-parity.mjs:65) suggests moving a row to `more`, but this limit measures `{ all: true }`, which includes both collections. Recommend combining or shortening rows instead. The index diagnostic also says both limits are exceeded when its condition requires only one.

The extracted parser and verb table preserve the previous checks. Flag-set equality, numeric limits and form-flag acceptance match, including the existing short-help `.length` measurement rather than UTF-8 bytes.

The synchronous `require` is valid under [Bun’s documented ESM interoperability](https://bun.sh/docs/runtime/module-resolution); this module graph has no top-level await. Caps requires the helper from the selected `root`, so its relative `HELP` import belongs to that checkout. Generic repositories without the helper still skip it, and module-load/execution errors become caps failures. The cross-root mismatch occurs in direct calls such as the fixture, not normal caps root selection.

No files changed; no builds or tests run.
