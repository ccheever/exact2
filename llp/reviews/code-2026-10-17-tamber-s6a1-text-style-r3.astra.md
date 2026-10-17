# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change and its first two rounds.
- **Method:** one brief (sha256 `ce2ad40156435625a08a37068b42b72edad60b0ae28c53626679f946bca478cf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a P1: a bare style's inline weight survived the JS runtime's adoption; P2s: a bound longhand saying nothing dropped the style's weight on the JS target, a nearer bare style's clear hid a further one, a move left a bare style below measured).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

One blocking P1 and three P2 findings remain:

- **P1 — [host/web/src/document.rs:590](/tmp/rv-s6a1-3/host/web/src/document.rs:590): Bare-style weights survive JS adoption.** A Rust-rendered bare headline receives inline `font-weight:600`. After adoption, changing an ancestor’s bound weight from `none` to `700` leaves the child at `600`; the kernel computes `700`. The JS template deliberately omits this fallback, but adoption preserves the renderer’s declaration.

- **P2 — [host/web-js/src/rows.rs:319](/tmp/rv-s6a1-3/host/web-js/src/rows.rs:319): Inactive bindings suppress shorthand defaults.** `binding(row).is_some()` also catches bindings currently returning `none`. For `aria-level=(big ? 1 : 4)`, switching to level 4 removes the implied `700` without supplying headline’s `600`; the JS page inherits `400`, while the kernel computes `600`. Nullable longhands beside a bound `font` have the same problem.

- **P2 — [kernel/src/arena/text_style.rs:119](/tmp/rv-s6a1-3/kernel/src/arena/text_style.rs:119): A nearer bare style blocks explicit inheritance.** Put `text font-size="-exact-body" font-weight="inherit"` beneath `column font-size="-exact-headline"`. The nearest bare candidate is rejected by its written-clear mask without continuing upward; `copy_inherited` likewise retains only that candidate. The kernel measures `400`, while the web inherits `600`.

- **P2 — [kernel/src/txn.rs:1275](/tmp/rv-s6a1-3/kernel/src/txn.rs:1275): Reparenting can retain stale text measurements.** In the Neutral column, move a bare-headline container containing bare-body text beneath a parent explicitly setting weight `600`. The container remains `600`, so comparison reports no change, although the descendant changes from `400` to `600`. Its metric revision remains unchanged, allowing cached measurements at `400` to survive.

Adoption behavior reproduced in memory. No files changed; Rust build/tests were not run.
