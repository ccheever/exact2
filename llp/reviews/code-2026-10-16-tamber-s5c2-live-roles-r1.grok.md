# Code review: role overrides go live (Tamber stage 5, commit 2), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `cdafd3e03f532ffb1e10f8a3264d20f71a57a615e118ff333f2c99b8d534aad6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only against the stage 5 brief and the commit message, starting with the review skill and the diff.The brief path is missing, so I’ll review the commit itself against its message and the five hunt items. Next I’ll read the role-override diff and the host paths it claims to update.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll confirm the live-edit and bind paths against the current HEAD before writing findings.[P1] Re-parse rows under the role's spelling, not only the token name — `runner/src/runner/theme.rs:53`

A live edit indexes `name_bit` of the token name only. A row that names the role is indexed under the theme entry it matched, and that entry's name is the role spelling (`kernel/src/style/theme.rs:135`). Those differ for the documented bare form: token `secondary-label`, row `color="-exact-secondary-label"` (`contract/lower/src/theme.rs:38`, `docs/contract-for-agents.md:2942`). The session table and the host sheet move to the new colour, and `apply_live` re-sends every node, but the stored colour was baked at parse time, so Apple, Linux, the web host, and the terminal keep painting the old one. Same-spelling tokens (`Canvas`, `-exact-label`) do update. The JS development target does not have this miss: its theme map points both spellings at `var(--exact-…)`.

[P3] Bind the web host across the live receipt walk — `host/web/src/host/live.rs:24`

`theme_css()` binds and then drops before `batch_from`. The receipt now touches every node, and a colour still stored as a role (the tint, or a role whose value is another role) is emitted at `host/web/src/css.rs:660` through `bound()` with no session bound. One document resolves as the newest installed table, which `replace` already updated. Two web hosts on the thread — the case the new assertion is for — emit the newer session's overrides and assert in debug. Apple, Linux, and the terminal bind at the resolve sites this commit added.

Nothing else in the hunt is blocking. Release builds do not take live edits, and a release JS emit leaves `live_properties` off and skips constant role tokens in `properties()`, so it still writes concrete colours. Gaining or losing an override stays a rebuild; a profile colour stays a rebuild; a tint value and a role whose value names another role go live on purpose, and the one-step resolve does not loop. A snapshot of the same design does not swap the table. `Installed::replace` does not walk nested scope chains, and the live path replaces before `apply` binds, so that scope captures the new table. `bound()` on an already bound thread is one `Rc` clone.

The new host tests and `a_batch_sets_a_role_override_without_a_plan` (`contract/cli/tests/it/tamber.rs:1159`) only use token names that equal the role spelling, so they pass while the bare-name row stays stale. There is no byte compare of a release JS emit against the parent.
