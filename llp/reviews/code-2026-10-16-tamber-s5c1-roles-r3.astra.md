# Code review: per-session role tables (Tamber stage 5, commit 1), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `04322f1dad5d2acff0a74edb60ff2014efa4ca7ef9375812ed1227033d1c9707`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (tint-role gradient stops; deferred titles; Mac popover and split panes; sheet tint).
- **Disposition:** applied without re-review (the last round).

---

Still blocking: four P2 findings.

- **P2 — Boxed gradients lose indirect accent overrides.** [gradient.rs:96](/tmp/rv-s5-3/host/apple/src/gradient.rs:96) snapshots only `overridden(id)`. With `AccentColor="#ff0000"` and `linear-gradient(90deg, LinkText 0px, transparent 100px)`, `LinkText` stays unresolved. `exact_gradient` then binds an empty table and paints the platform accent instead of red.

- **P2 — Deferred navigation titles capture another session’s colour.** [NavigationTitleIOS.swift:203](/tmp/rv-s5-3/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:203) queries unattached `self`. If A changes its rich title during a navigation transition, B applies a batch, then A’s deferred `sync` creates the title, the lookup selects B’s table. That colour remains captured. Use `host.presenter.root`.

- **P2 — Detached Mac content still selects the last presenter.** [ThemeRoles.swift:70](/tmp/rv-s5-3/host/apple/Sources/ExactKit/ThemeRoles.swift:70) follows only physical superviews. Neither native popover content nor the split-detail pane is adopted. After B applies a batch, an empty native textarea in A’s popover or split pane draws its placeholder using B’s override.

- **P2 — Sheet adoption does not carry the session tint.** [ModalIOS.swift:140](/tmp/rv-s5-3/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:140) registers the table but leaves `tintColor` inherited. The theme tint is assigned only to `presenter.root`; modal/dialog content sits outside it. A session with red `AccentColor` therefore loses red on sheet controls resolving `@tint`.

The installed-stack removal, owner pruning, explicit sheet-table ownership, and grouped-list lookups look sound. The revised ABI-binding and candidate assertions exercise their claims. The [gradient test](/tmp/rv-s5-3/host/apple/src/abi_roles_tests.rs:143) never resolves either emitted CSS payload and only rejects blue, missing the first finding.

Tests inspected, not executed. Worktree and index untouched.
