# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ab563ed6e2`.
- **Method:** one brief (sha256 `7b5c668b0ac28482f0e3435acc1a53fce62a7a5d36a62e572a9a0cfac0472ec2`), shared by both reviewers, blind to the other review; commit 486ae7b8cf against round 1. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 P2).
- **Disposition:** taken in `14e6c27147` (with rounds 1 and 2 and the contract tests' staging, squashed): staging keeps what the build writes there (`app.contract.d.ts`, any `.d.ts`), so an unchanged launch builds nothing; it hashes the staged sources (names and contents, sorted) into `.exact-stage` and, when that signature differs from the last staging's, removes the dist's build marker, so whichever process staged first, the next build is made again (a deleted source included); the unit test now checks the marker kept when nothing changed, gone when the sources changed, and the declarations kept. Also in that commit, the lead's top priority: `bun exact.mjs test` in apps/design-tool runs a stage with its stub and tests on the checkout's port. Not taken: a browser frame refusal from 7740 (both reviewers judge the exact CSP assertion enough).

**Changes requested: two P2 findings.**

1. **[P2] Pre-staging consumes the deletion signal before the build checks it.** [design-tool.test.mjs:25](/tmp/rv-dt-port2/scripts/design-tool.test.mjs:25) calls `stageTool()` and discards `changed`. After a successful build, delete imported `dials.ts` without changing its importer and rerun the suite. That first staging removes the file; the subsequent staging in `buildTool()` returns `changed: false`. Since `webChanges()` examines surviving files, [tamber-server.mjs:1094](/tmp/rv-dt-port2/scripts/tamber-server.mjs:1094) accepts the old bundle.

   Executing the actual functions against an in-memory filesystem reproduced `changed: true → false`, `webChanges.all: []`, and success with **zero build calls**. Round 1’s deletion finding remains unresolved. Invalidation must survive staging until a successful build; obtaining the drive’s app path should not consume it.

2. **[P2] Generated declarations cause repeated builds with unchanged sources.** [tamber-tool-port.mjs:38](/tmp/rv-dt-port2/scripts/tamber-tool-port.mjs:38) removes staged files absent from the original app and marks the stage changed. In a clean checkout, the original app has no `app.contract.d.ts`, but [the development build writes one into the staged app](/tmp/rv-dt-port2/host/web-js/build.mjs:247). The next `buildTool()` deletes it and rebuilds; that build recreates it. This also affects commands checking whether an existing tool can be reused.

   The in-memory probe produced **two build calls for two unchanged invocations**. Generated declarations should not invalidate the authored source inventory.

The other dispositions check out:

- Alternate-port drives select the staged app, and `appServer()` returns `null` for it.
- Zero or multiple matching API literals are refused.
- Invalid explicit ports are rejected. Both direct and forwarding listener paths use the validated numeric port.
- The port-lock and hash-collision comments are corrected.
- I found no regression in the default 7740 origin/build path or the original contract-test stub setup.

The omitted browser-refusal test is reasonably justified by the exact CSP assertion and existing [header-application tests](/tmp/rv-dt-port2/scripts/tamber-server.test.mjs:459). The remaining coverage gap is build invalidation: the staging test checks file removal without checking that rebuilding follows.

Verification used isolated imports and in-memory probes. No builds, browser drives, or contract suites were run; no files changed or mutating Git commands were used.
