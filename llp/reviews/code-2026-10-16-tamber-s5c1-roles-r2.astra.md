# Code review: per-session role tables (Tamber stage 5, commit 1), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `5905605537eca1d26a862abf07ab26ebd0f1d9cf76986e302a5acde7c370e2b0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (gradient resolve unbound; detached Swift sheets; platformBackground unscoped; out-of-order candidates; colour-report test; owner leak).
- **Disposition:** in the second-round commit.

---

Blocking issues remain.

- **P1 — Gradient callbacks remain unbound.** [gradient.rs:73](/tmp/rv-s5-2/host/apple/src/gradient.rs:73): `exact_gradient` bypasses both runtime wrappers. Boot A with red `AccentColor`, then B with blue; A’s `linear-gradient(90deg, AccentColor 0px, transparent 100px)` resolves through B’s installed table when Swift paints it.

- **P1 — Detached Swift surfaces still lose their session.** [ThemeRoles.swift:58](/tmp/rv-s5-2/host/apple/Sources/ExactKit/ThemeRoles.swift:58): `adopt` silently returns outside `within`. `showModal` commands run after that scope ends ([Session.swift:1151](/tmp/rv-s5-2/host/apple/Sources/ExactKit/Session.swift:1151)), leaving dialog sheets unregistered. After B applies a batch, A’s sheet placeholders and grouped-list callbacks resolve B’s roles. Mac sheets never register an owner either.

- **P2 — Background lookups still use the global fallback.** [NodeExtrasIOS.swift:274](/tmp/rv-s5-2/host/apple/Sources/ExactKit/IOS/NodeExtrasIOS.swift:274): `platformBackground` passes no view. After B applies, keyboard dismissal/layout in A calls `paintUnderKeys`, which can overwrite A’s modal background with B’s color—even when the modal was correctly adopted.

- **P2 — Out-of-order candidate disposal restores a dead table.** [roles.rs:388](/tmp/rv-s5-2/kernel/src/style/roles.rs:388): install live L, then candidates C1 and C2; drop C1 before C2. C1 discards its predecessor, and C2 restores dead `Weak(C1)`, leaving unscoped resolution with no overrides. `applyTogether` actually discards candidates in this order ([Session.swift:295](/tmp/rv-s5-2/host/apple/Sources/ExactKit/Session.swift:295)). The single-candidate test misses it.

- **P2 — The color-report test skips repainting.** [abi_roles_tests.rs:118](/tmp/rv-s5-2/host/apple/src/abi_roles_tests.rs:118): neither runtime receives a scheme report, so `PaintMotion::colors_changed` returns immediately because `dark` is `None`. The negative `text_color` assertions therefore cannot detect incorrect session binding during repaint. Initialize appearance and assert actual repaint output.

- **P2 — Dead sheet owners accumulate.** [ThemeRoles.swift:27](/tmp/rv-s5-2/host/apple/Sources/ExactKit/ThemeRoles.swift:27): existing roots return before pruning; `adopt` only inserts. Repeated modal creation in a long-lived session retains dead owner entries and their strong table references until a new presenter root is registered.

The wrapper bindings, outer-scope preservation, and Android handoff look sound. The reboot test now checks emitted colors. Tests were not run in the read-only sandbox.
