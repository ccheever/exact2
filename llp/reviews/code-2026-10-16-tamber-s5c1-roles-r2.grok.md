# Code review: per-session role tables (Tamber stage 5, commit 1), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `5905605537eca1d26a862abf07ab26ebd0f1d9cf76986e302a5acde7c370e2b0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

I'll review round 2 of the per-session role tables read-only: the brief, both commits, and the paths round 1 called out.The brief is commit 1: session-owned role tables. Next I’ll read both commits and the code those round-1 findings named.Round 2 is `6e072aa`. I’ll read that diff and the role-binding paths it changes.The Swift sheet and page-ground path is blocking. The round-1 Rust binds and the two-session restore hold; one new test does not exercise the path it names.

[P1] Unscoped role lookups paint whichever presenter applied last — `host/apple/Sources/ExactKit/ThemeRoles.swift:70`

`style(in:)` with no view uses `applying`, otherwise `last`, and `apply` points `last` at every batch, roles op or not. `UIColor.platformBackground` (`host/apple/Sources/ExactKit/IOS/NodeExtrasIOS.swift:272`) still calls `ThemeRoles.color` with no view. `paintUnderKeys` (`host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:430`) runs from layout, outside `ThemeRoles.within`, and writes that color onto the open modal sheet. Two embedded sessions with different `background` or `Canvas` overrides: session A presents a sheet (adopted correctly inside `within`), session B applies any later batch, then A's next layout sets A's sheet ground to B's table. The same fallback is the page ground in `paintCanvas` (`host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1372`) when a trait change rebuilds `CanvasKey` after another session's `generation` bump, and the backdrop fallback in `NavigationIOS.swift:106`.

[P2] Dropping a session that is not the installed one loses the session under it — `kernel/src/style/roles.rs:387`

`Installed` keeps a single `over` weak, and `uninstall` either writes it back or discards it. Boot A, then B, then C on one thread. Drop B while C is installed: B's weak to A is thrown away because `INSTALLED` points at C. Drop C: it restores B, which is already dead. A is still alive and `roles::overridden` is empty. Apple frames hide this because `with_runtime` / `with_entry` bind the callee, but the installed table — what a resolve uses when no scope is active, and what the next boot records as `over` — is none. The new theme test only drops in LIFO order of two.

[P2] The colour-report test never resolves a role — `host/apple/src/abi_roles_tests.rs:122`

`set_colors(Vec::new())` reaches `PaintMotion::colors_changed`, which returns before any role read while `dark` is still `None` (`kernel/src/motion/paint.rs:234`). The fixture never reports an appearance, so the batch has no `text_color` whether or not `with_runtime` bound that session. `resolved()` does check the bind; this test does not check a re-present.

[P3] Dismissed sheet views stay in the owners map — `host/apple/Sources/ExactKit/ThemeRoles.swift:57`

`adopt` inserts and never removes. The sweep in `table(of:)` runs only when the root is not already a live owner, and a stable presenter hits the early return on every later `apply`. Each presented sheet leaves an entry for the life of the process. Lookups are saved by the `owner.view === v` check, so a reused address does not return the old table.

Round 1 otherwise holds. `with_runtime` binds for the call, and the host's `bind_roles` covers the boot that builds the first batch, including a reboot inside the call that bound the old runner. `install` no longer writes `BOUND`. A refused runner dropped while it is still the installed one restores the previous table; Android uninstalls on the boot thread after the first frame and `adopt` installs on the thread that runs the host. The two-session `with_runtime` test and the reboot batch check do test those paths.
