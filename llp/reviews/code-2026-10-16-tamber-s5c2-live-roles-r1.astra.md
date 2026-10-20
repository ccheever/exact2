# Code review: role overrides go live (Tamber stage 5, commit 2), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `cdafd3e03f532ffb1e10f8a3264d20f71a57a615e118ff333f2c99b8d534aad6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Linux reload unbound; nested scopes; JS accent masking; UIKit sheet grounds; prerendered dev pages).
- **Disposition:** in the first-round commit.

---

The Linux reload crash is blocking. I found five issues:

- **P1 — Linux still resolves roles unbound during reload.** [session.rs:88](/tmp/rv-s5c2-1/kernel/src/style/roles/session.rs:88): reload keeps the old session alive while the candidate reaches `boot_paint()` at [host.rs:337](/tmp/rv-s5c2-1/host/linux/src/host.rs:337), without binding it. A node with `background-color="AccentColor"` and a background-color transition reaches `ColorValue::resolve` and triggers the new debug assertion. Live commits also reach `sync_paint` before the binding added in `scale.rs`.

- **P2 — Nested bindings restore an obsolete role table.** [session.rs:198](/tmp/rv-s5c2-1/kernel/src/style/roles/session.rs:198): `Installed::replace` updates only the current `BOUND`, while each `RolesScope.previous` retains its old `Rc`. Bind the same runner twice, apply a role edit, then drop the inner scope: the still-active outer scope resolves the old colors.

- **P2 — A live global accent overrides an authored ancestor accent.** [modes.rs:487](/tmp/rv-s5c2-1/host/web-js/src/modes.rs:487): the update writes `--exact-accent` into every tuple rule. Put a `color-scheme` boundary beneath a parent with `accent-color="#00ff00"`, then edit global `AccentColor`. The boundary receives the global value directly, masking the inherited green; its descendants change incorrectly until reload.

- **P2 — Open UIKit sheets retain their old background.** [ThemeRoles.swift:123](/tmp/rv-s5c2-1/host/apple/Sources/ExactKit/ThemeRoles.swift:123): adopted sheets refresh only their tint. Their background was assigned during `loadView`, and `ThemeRoles.color` captures that table value. Open a sheet with a Canvas override, then edit Canvas: the sheet’s exposed background stays at the old color until recreated.

- **P2 — Prerendered development pages retain baked role colors.** [modes.rs:97](/tmp/rv-s5c2-1/host/web-js/src/modes.rs:97): role declarations now use `var(--exact-…)`, but the adoption cleanup list at line 345 recognizes only `var(--x-…)`. A Rust-prerendered `color="CanvasText"` keeps its inline color, which overrides the live class property, so subsequent CanvasText edits remain invisible.

Three JS tests passed; targeted in-memory checks confirmed the two JS runtime behaviors. The added Apple/JS tests check emitted data rather than these retained-view and cascade cases. Native tests and the complete release byte comparison were not run under the read-only sandbox. No worktree or index changes.
