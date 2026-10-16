# Code review: per-session role tables (Tamber stage 5, commit 1), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `ca7ee0a29b38c30758796dac786666f6dc6a2a7cb7e966f9feb41f268c5671c1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Apple exports unbound; Swift roles global; install before validation; nested boot clobber; Android handoff).
- **Disposition:** in the first-round commit.

---

Blocking issues remain.

- **P1 — Normal Apple calls bypass binding.** [registry.rs:154](/tmp/rv-s5-1/host/apple/src/abi/registry.rs:154): `with_runtime` invokes the bridge without binding roles. Boot A, then B with different overrides; A’s dispatch, timer and colour-report calls resolve through B’s table. The new ABI test uses `with_entry`, missing the production path.

- **P1 — Apple presenter overrides remain global.** [ThemeRoles.swift:14](/tmp/rv-s5-1/host/apple/Sources/ExactKit/ThemeRoles.swift:14): B’s boot overwrites the static table used by both presenters. A’s next Mac presentation reads B’s background; its accent lookups also read B’s design. Rust-side binding cannot isolate these Swift lookups.

- **P2 — Rejecting a candidate can remove the live session’s overrides.** [runner.rs:1073](/tmp/rv-s5-1/runner/src/runner.rs:1073) installs the candidate before host validation finishes. For example, Linux [image validation](/tmp/rv-s5-1/host/linux/src/presenter/delivery.rs:229) can subsequently fail and drop it. `INSTALLED` then points to a dead runner; the retained live session paints using default roles.

- **P2 — A nested boot overwrites its caller’s scope.** [roles.rs:355](/tmp/rv-s5-1/kernel/src/style/roles.rs:355): inside `A.bind_roles()`, successfully booting distinct session B replaces `BOUND` with B after the boot guard restores A. Subsequent work for A reads B until the outer guard drops. Installation cannot assume every nested boot replaces the enclosing session.

- **P2 — Android’s thread handoff loses the binding.** [jni.rs:215](/tmp/rv-s5-1/host/linux/src/canvas/jni.rs:215): `startAsync` installs roles on the boot thread, but `adopt` moves the host without installing them on the receiving thread. Subsequent role resolution sees no overrides despite the runner retaining its table.

The [reload test](/tmp/rv-s5-1/contract/cli/tests/it/theme.rs:389) checks generation inequality and lookup only; it does not verify re-presentation.

Static review only; tests were not run. No files changed.
