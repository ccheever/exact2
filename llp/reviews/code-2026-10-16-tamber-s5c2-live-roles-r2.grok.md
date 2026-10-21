# Code review: role overrides go live (Tamber stage 5, commit 2), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `71d3a11e3cb6539c3a7a34fea239edadcde8b58dcfe27f11418ffb01eca0c4c6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

I'll review round 2 of the role-overrides commit read-only: the brief, what round 1 changed, and whether each finding is actually fixed.The brief is commit 2: role overrides become live edits. Next I’ll read both commits and the round-1 fix list against the code they touch.Round 1 is `c87f62a` on top of the live-overrides commit. I’ll read those diffs and the role-resolution paths they name.The session table is now updated in place. I’ll check the keep/table order, generation checks, and whether a live edit still reaches each host.Auto-compacting conversation (81% full)...
Conversation compacted.
I'll finish the read-only review of the round-2 role-override changes, starting from the review skill and the spots still open from the last pass.[P1] A live AccentColor edit does not repaint Mac tint rows — host/apple/Sources/ExactKit/ThemeRoles.swift:131

On a roles batch, `Session.apply` updates the table and then calls `applyThemeTint`. The Mac branch only assigns `viewport.backgroundColor`. A row that names the tint is still sent as `{"sys":"@tint",…}` (`ColorValue::themed` leaves tint roles as the role), so `Host::update` sees the same style string and skips the op (`host/apple/src/paragraph.rs:377`). Nothing calls `reapplyColors`, `accentChanged`, or `systemColorsChanged`. A box with `background-color="AccentColor"` stays the old blue after a batch that sets AccentColor to `#ff0000`. iOS does update: the same function replaces `root.tintColor`, and `tintColorDidChange` re-applies those rows.

Round-1 items that are fixed: in-place `Roles::set` (a scope bound before the edit reads the new table; `find` does not hold the `RefCell` across the return); Linux `boot_paint` / `sync_paint` bind the candidate around colour capture; `set_tokens` matches rows by the role spelling (`-exact-secondary-label`), not the bare token name; the JS tint is `root: true` and is written only on a rule whose `selectorText` is exactly `:root`; dev CSS uses `var(--exact-…)` and the prerender strip list includes those declarations; `applyThemeTint` re-grounds adopted iOS sheets and `retheme` re-inks nav titles; the web live walk is bound for the whole receipt, including `theme_css`.

Checked and not defects: `set` copies the table before `borrow_mut`, and nothing re-enters it while that borrow is held. The new wide `Keep` is registered in `Keep::of` before the old keep is dropped; the table assignment only drops `Copy` colours. Apple compares `roles::generation()` with `roles::generation()` while that session is bound (the `1 << 20` mix is not compared against `Roles::generation()`). Release JS/CSS for a legal plan is unchanged: a role cannot vary by an axis, tint is omitted from tuple rules, and `var(--exact-…)` in class CSS is dev-only. With two sessions, candidate boot binds for the runner’s own apply and again for paint capture; the web and Apple commit walks bind too.

Nothing else blocking.

Test gaps: `a_reload_adopts_its_paint_under_its_own_roles` only asserts the pixel is red after reload, when one session remains. An unbound resolve during boot would still paint red, because the newest installed table is already the candidate; only the debug assert catches it, and only if a resolve actually runs in that window. `modes.test.mjs` checks that the mock writes `--exact-accent` onto the `:root` rule and not the tuple rule; it does not run a cascade, so it does not show an authored ancestor `accent-color` winning. `kernel/tests/wide_roles.rs` holds a wide colour across `Roles::new` only, not `Roles::set`. No test covers the Mac tint path above.
