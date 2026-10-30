# Code review: the conformance drive's clock refusals on a poisoned runner, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `fa6609d320`.
- **Method:** one brief (sha256 `6cb921af6097e599d43aee79fa3c768aa3f9a2620d30acf6d29ca90aa1e0de22`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the HEAD commit as a read-only defect check against the budget-poison settle rules, starting with the diff and the harness it changes.The diff is small and concentrated in the clock path. I'll read `clockAll`, Linux comparison, and how refusals land in the report before judging whether a divergence can slip through.The page-vs-page poison check does what was asked. A Linux disagreement does not: it is recorded as an equal step and `--strict` stays green.

[P1] A Linux clock that is not the same `Poisoned` refusal does not fail the run — `host/web-js/conform.mjs:510`

`clockAll` records `refused: "Poisoned on every target"` with `differences: 0` as soon as the two pages agree, then asks Linux inside `onLinux`. That helper treats a throw as a host crash: it appends `{ linux: 'stopped' }`, closes Linux, and does not call `fail` (`conform.mjs:404`). `--strict` only exits non-zero when `report.failures` is non-empty, so this stays green. The stop also skips every later Linux compare, including case 17.

That is not how a refusal disagreement is already handled. For a scripted step, Linux answering after the pages refused goes through `answered('linux')`, which calls `fail`, and only then stops. Linux throwing any non-malformed error when the pages already refused is swallowed and the comparison continues. `clockAll` uses the crash path for both.

Failing input: wasm `clock: clock: Poisoned`, js `clock: clock: t=0 refused advance: Poisoned` (the strings `operationReply` actually throws), and Linux `clock('settle')` either returns `{ clock: 50, settled: true }` or throws `clock: clock: TimerFireLimit { limit: 4096 }`. Pages record an equal step, Linux stops at `settle` or `clock +60000`, `report.failures` is unchanged.

Page-vs-page one-sided, differing, and non-`Poisoned` refusals still throw outside `onLinux` and hit `fail(driveAt)`. A malformed page reply is rethrown before the verdict, so it still fails the drive. No `LINUX_APART` entry uses the step names passed through, `settle` and `clock +60000`, so the apart early-stop does not fire. `clockRefusal` only strips host `clock:` prefixes and the JS `t=… refused advance:` wrapper. `TimerFireLimit { limit: 4096 }`, a trap journal line, and `HandlerArguments { … error: Poisoned }` stay distinct. `RunnerError::Poisoned` carries no cause, so two different poisons are not separable in this string; the poisoning step is still compared afterward. `bun test host/web-js/conform.test.mjs`: 9 pass. Those tests never call `clockAll`, and the three-way `clockVerdict` case is not the drive's Linux check.

Non-blocking:
- `driveAt` is always `` `${step} clock ${arg}` `` (`conform.mjs:506`), so a failing closing jump is labeled `clock +60000 clock +60000`, and a follow-up settle of the scripted `clock settle` line is `clock settle clock settle`. The report row special-cases that collision; the failure label does not.
- An accepted closing refusal and the compare after it are both named `clock +60000`. Each accepted settle also adds a `differences: 0` row, so "steps compared" and "steps equal" count the refusal plus the later state compare. The "Clock refusals compared" section is the only place the refusal is called out.
- The new test feeds the page's single `clock:` prefix. The driver throws `clock: clock: …`. The `/^(clock: )+/` quantifier covers that, but the test does not.

CHANGES
1. `host/web-js/conform.mjs:510` — Linux not matching `Poisoned` is swallowed by `onLinux` (`conform.mjs:404`) after an equal "on every target" row is already stored, so `--strict` stays green and later Linux steps (case 17) never run. Input: both pages throw `clock: clock: Poisoned` / `clock: clock: t=0 refused advance: Poisoned`, Linux answers `{ clock: 50, settled: true }` or throws `clock: clock: TimerFireLimit { limit: 4096 }`.
