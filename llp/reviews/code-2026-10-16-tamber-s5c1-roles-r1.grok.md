# Code review: per-session role tables (Tamber stage 5, commit 1), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `ca7ee0a29b38c30758796dac786666f6dc6a2a7cb7e966f9feb41f268c5671c1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only against the stage 5 brief and the role-table paths you listed, without touching the worktree.The brief path missed. I'll locate it and inspect the HEAD commit without changing the tree.The brief is in `~/x2theme`. Next I’ll read the role-table commit and the resolve paths it claims to rebind.Blocking. The session table is real, but Apple still resolves whoever booted last. That is the only host this commit had to get right.

[P0] Apple calls resolve the last install, not the runtime — `host/apple/src/abi/registry.rs:127`

`with_entry` binds that runtime's `Roles` (`registry.rs:171`). Every call that actually resolves does not. `exact_boot`, `exact_frame`, `exact_advance`, and `exact_colors` go through `with_runtime` (`exports.rs:240`, `exports.rs:532`, `exports.rs:747`), which never binds. `overridden`, `themed`, `painted_override`, `fallback`, `theme_css`, and `metrics::of` all read `bound()` (`roles.rs:317`), which is the active scope or else the weak `install` left behind (`runner.rs:1073`). Boot window A (`CanvasText = #010101`), then window B (`#020202`). A's next trait report takes `exact_colors`: `set_colors` reads `roles::generation()` (`colors.rs:91`), which is B's generation mixed with the report (`roles.rs:370`), repaints A's nodes with B's overrides, and stores that composite. Later reports on A match it and stop. A's `exact_frame` / scheme / resize restyles do the same. `prepare` then `discard_plan` (`abi.rs:951`) is the one-session form: the candidate boot installs, discard drops that `Rc`, the weak dies, and the live runner still holds its table but the next frame resolves the empty default. `commit_plan` (`abi.rs:910`) never installs the host it keeps. A refused `Runner::boot` does not install; a successful candidate that is then dropped does.

[P2] `install` overwrites a different session's scope — `kernel/src/style/roles.rs:355`

The boot scope is dropped before `install` (`runner.rs:1072`). If any bind is still active, `install` replaces that slot. `with_entry(A)` then a boot of unbooted B (B has no roles yet, so it does not bind; the two entries are different `RefCell`s) leaves the rest of A's call on B's table, including a re-entrant `exact_*` from B's measurer. Current exports do not nest boot inside `with_entry`; the comment treats the clobber as the reload feature.

[P2] The new tests never take the path they name — `host/apple/src/abi_tests.rs:1419`, `contract/cli/tests/it/theme.rs:405`

`two_sessions_on_one_thread_resolve_their_own_role_overrides` boots and reads `overridden` only inside `with_entry`. It never calls `with_runtime`, `colors`, or `frame`, so it passes while the exports above fail. `a_reload_with_other_overrides_re_presents` drops the first runner, then checks that `generation()` changed and that the installing thread's `overridden` is the new colour. It does not call `set_colors`, does not keep two sessions alive, and does not check the comment's same-set claim. The runner tests that `bind_roles` themselves, and the refused-boot test, do test what they say.

Not blocking elsewhere. `Roles.kept` is registered in `Roles::new` and lives with the runner's `Rc`; the wide test's 6,000-intern churn matches that, and a dead weak upgrades to empty rather than a freed id. Linux is one `Presenter` and one host and resolves at paint from the install. Web bakes `theme_css()` in `Host::open` while that runner is still alive. JS `emit` holds the `set_role_overrides` `Rc` across `theme_css`; `modes.rs` only resets the token cache and does not drop that table. The `2^20` mix in `generation()` changes when the bound table or the report changes; it is not a second bug once `bound()` is the session being painted.
