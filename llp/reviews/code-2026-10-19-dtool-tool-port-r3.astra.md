# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a7f607ec78`.
- **Method:** one brief (sha256 `da9fcf2c31c36acfa284f7e4cbecfde5e57e55ab2e7c9b29cf481a057e96a0d5`), shared by both reviewers, blind to the other review; commit 9c9afd5737 (round 2's fixes and the contract tests' staging). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 P2).
- **Disposition:** taken in `da449db1c7` (rebased on main with stage 10 B, whose tamber-tool-files.mjs now takes `TOOL_DIST` from tamber-tool-port.mjs): `apps/design-tool/exact.mjs` calls app-cli `run`, so `bun exact.mjs test` (and `agent`, `verify`) in apps/design-tool reaches the staging (Grok P1); `stageToolTests` turns a test file or glob named in the tool's sources into the stage's (Astra P2, Grok) and always sets `EXACT_WEB_DIST` to the stage's dist; a build records the stage signature it was made from (`builtFrom`) and `buildTool` accepts it only while staging gives the same, so a build in flight while a source goes is not current (Astra P2); the stub's stage prunes any file gone (Grok); a port the tests name for a page of their own (8765) is refused (Astra); `checkoutPort` hashes the real path. Not taken: the zero-padded spelling on a five-digit port is six digits, which pageOf refuses by length, correctly (both); two processes staging while a source is deleted (Grok) is QUEUE'd (a lock would close it); the invalidation test drives the signature directly (the cross-process cases are covered by the new builtFrom test). A direct `scripts/agent.mjs`/`scripts/verify.mjs` of the tool still uses 7740 (QUEUE).

**Changes requested: two P2 findings.**

1. **[P2] An in-flight build can undo the deletion invalidation.** [tamber-tool-port.mjs:67](/tmp/rv-dt-port3/scripts/tamber-tool-port.mjs:67) records the latest staging signature and removes the current build marker, but does not coordinate with build publication.

   Concrete interleaving: process A’s compiler has captured `dials.ts`; the source is deleted; process B stages that deletion and removes the marker. A then finishes and [publishes a new marker](/tmp/rv-dt-port3/host/web/build.mjs:79), whose input digests are collected **after compilation**. The next staging sees the same signature, leaves that marker intact, and `webChanges()` finds nothing newer. The old bundle is accepted despite its missing import. An in-memory interleaving reproduced the surviving marker and empty change list. Invalidation needs to remain tied to what the successful build actually consumed.

2. **[P2] Explicit test filenames bypass the rewritten test file.** [app-cli.mjs:89](/tmp/rv-dt-port3/scripts/app-cli.mjs:89) changes `dir`, but [test-file resolution](/tmp/rv-dt-port3/scripts/app-cli.mjs:193) still prefers files in the original working directory.

   From `apps/design-tool`, run `EXACT_TOOL_PORT=17999 bun exact.mjs test web app.test.contract`. The app and stub use 17999, but the compiler receives the original test file. Its `fail fetch` still targets 7740, so the “no server” test fails; its self-origin canvas cases also name the wrong port. `--test`, absolute paths, and matching globs have the same gap. Explicitly selected files belonging to this app need to select their staged copies.

Smaller points:

- **A valid override of `EXACT_TOOL_PORT=8765` breaks the positive canvas tests.** Their [fixed example page](/tmp/rv-dt-port3/apps/design-tool/app.test.contract:722) becomes the tool’s own port, which `pageOf()` correctly rejects. Choose a fixture port distinct from the selected tool port.
- With default five-digit ports, `07740` becomes something like `017999`. [The five-digit regex](/tmp/rv-dt-port3/apps/design-tool/app.ts:109) rejects it before the self-port comparison, weakening that test’s coverage.
- The added invalidation test [changes `.exact-stage` directly](/tmp/rv-dt-port3/scripts/tamber-tool-port.test.mjs:35); it does not exercise an actual source deletion or concurrent publication.

The sequential round-2 fixes hold: generated declarations survive unchanged staging, deletion removes the marker, and a failed build leaves it absent for the next attempt. The default test invocation correctly selects the staged app, stub, and dist. Both stub imports resolve correctly; server discovery and the port turn follow the rewritten API, and the browser drive follows `EXACT_WEB_DIST`. I found no regression to a person’s default 7740 tool or another app’s test command.

Reviewed `9c9afd5737` through source inspection and in-memory probes. No builds or test suites ran; no files or Git state changed.
