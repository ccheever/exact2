# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `619c082be6`.
- **Method:** one brief (sha256 `d865d654bad0c3700822d1e195fbf96658a69ec811718bf07f43f84c986af13e`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [host/terminal/src/requests.rs:65](host/terminal/src/requests.rs:65): Terminal continuations still bypass `admit`. A background continuation carrying `long_poll` reaches `Work::Now`/`Work::Later` and executes without refusal or an ordered fence. `Storage::background` also passes it through unchecked.
- [host/render/src/pages.rs:71](host/render/src/pages.rs:71): `Dispatch::Again` bypasses admission through `executor.again`, which receives only the ticket. An invalid long-poll continuation can therefore produce a successful outcome and reach `parse`. Validate before dispatch.

**Should-fix**

- [host/render/src/lib.rs:909](host/render/src/lib.rs:909): Long-poll admission errors enter `held.busy`, producing HTTP 503. They should be distinguished from capacity failures and recorded as semantic refusals.

**Nits:** None.

The shared executor’s check and ordered fence are correct; storage validates before rewriting. No added rejection of non-polls: [request.rs:454](runner/src/request.rs:454) returns immediately when absent. Wasm still validates before dispatch at [host.rs:1055](host/web/src/host.rs:1055). The queued rollback issue is pre-existing.

The allowed test command failed before execution because the read-only sandbox denied Cargo writes.
