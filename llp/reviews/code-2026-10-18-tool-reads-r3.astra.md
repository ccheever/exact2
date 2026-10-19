# Code review: the design tool's reads during a scrub, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the net change after the redesign.
- **Method:** one brief (sha256 `6b33a16fcee0556d7286b18c4ab5f09a1133e817529ff73df364347c7d54f400`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (two scrub transitions in one answer; a bystander draft re-proposes; no test).
- **Disposition:** fixed in round 3's commit.

---

Nothing is blocking.

1. **Should-fix — [apps/design-tool/app.ts:1079](/tmp/rv-dq-3/apps/design-tool/app.ts:1079): batched scrub transitions skip the refresh.** If one answer contains A’s cancellation followed by B’s opening, `paced` remains `scrub N M`. I reproduced this with the actual scrub/session code. Hello, scope and uses therefore miss the boundary refresh; source locations changed during A can remain stale through B. Include the active scrub ID or a transition counter in the key, and test cancel/reopen within one answer.

2. **Should-fix — [apps/design-tool/app.contract:703](/tmp/rv-dq-3/apps/design-tool/app.contract:703): bystander drafts still propose on every status.** During another client’s scrub, this tool’s `scrubShown` is false, so every answer updates `proposeAt` and issues another `propose` request. On the JS loop, that can also invoke the design reader for classification. Make `proposeAt` follow `changes.paced`, retaining the local `scrubShown` guard and existing draft dependencies.

3. **Should-fix — [apps/design-tool/app.test.contract:458](/tmp/rv-dq-3/apps/design-tool/app.test.contract:458): the pacing behavior lacks regression coverage.** No tests changed. This existing test detects another `describe`, but would pass if hello, dry commit and uses still ran on every scrub status. Add request-count assertions for suppression, boundary refreshes, file/commit wakeups, batched transitions, and gap/restart recovery.

Read-only Bun probes passed the normal end-ID paths, including refusal after server loss during Keep, plus file/commit wakeups and gap/restart clearing. I found no ordinary lost-end path that permanently leaves pacing enabled. Full browser/build suites were not run because they write artifacts.
