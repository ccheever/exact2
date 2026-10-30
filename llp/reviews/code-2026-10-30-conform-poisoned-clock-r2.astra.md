# Code review: the conformance drive's clock refusals on a poisoned runner, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `430e8c53cc`.
- **Method:** one brief (sha256 `5fdb7c331e48b486915097e32b21dc958712c00aea5ff2a0ceef9f73ba286818`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

No blocking findings in `430e8c53cc`.

The round-1 blocker is fixed: Linux answering or refusing differently now reaches `fail()` before Linux is closed, so `--strict` fails. `LINUX_APART` retains the existing `onLinux` behavior and step names. See [conform.mjs:504](host/web-js/conform.mjs:504).

Normalization requires exactly `Poisoned` after removing the known prefixes. Malformed driver errors propagate. One-sided poisoning fails at settlement; different budget trap reasons remain checked by the subsequent journal comparison. Case 17’s `argtrap` comparison remains reachable, and `driveAt` identifies the clock operation without duplicating the closing label.

Accepted refusals become explicit `refused` records with `differences: 0` and appear in Markdown. They add comparison rows alongside the ordinary checkpoints.

Non-blocking notes:

- [The tests](host/web-js/conform.test.mjs:135) cover the helper decisions well. They do not directly protect the enclosing `fail()` wiring, report generation, or continuation through `argtrap`.
- [Linux’s summary count](host/web-js/conform.mjs:928) excludes these additional refusal rows, even when their text names Linux. This affects accounting, not the strict verdict.

`git diff --check` passed. Bun stalled before reporting test results and was stopped. The existing budget report shows **73/73 equal, zero failures**, including the later `argtrap` and closing jump, but predates this revision; I did not rerun full conformance under the read-only constraint.

APPROVE
