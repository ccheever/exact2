# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and its first round.
- **Method:** one brief (sha256 `2f902c7c94682859db8bff2b0a777d629e96e6c2fe6b725678160ebdba0d7085`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: a bound shorthand overwrote written longhands on the JS target, the document writer omitted the rows; P2s on `inherit` under a bare style, nested bare styles' runs and the button title).
- **Disposition:** in the second-round commit.

---

Blocking issues remain:

- **P1 — Bound shorthand overwrites explicit longhands.** [rows.rs:315](/tmp/rv-s6a1-2/host/web-js/src/rows.rs:315): `font=(big ? "-exact-headline" : "-exact-body") font-weight=300 line-height=2` unconditionally writes the style’s weight/leading inline. The browser gets `400/normal` or `600/normal`; the kernel correctly retains `300/2`.

- **P1 — Document rendering still omits text-style rows.** [document.rs:570](/tmp/rv-s6a1-2/host/web/src/document.rs:570) calls `css_style_of`, bypassing `css_style_named`. Under a parent with `font-weight=700 line-height=2`, `font="-exact-body"` renders as `700/2` instead of `400/normal`. `project_keeping` also retains that incorrect CSS for adoption until the node is touched.

- **P2 — A descendant’s `inherit` suppresses its ancestor’s bare style.** [text_style.rs:108](/tmp/rv-s6a1-2/kernel/src/arena/text_style.rs:108), also [defaults.rs:314](/tmp/rv-s6a1-2/kernel/src/arena/defaults.rs:314): a parent with `font-size="-exact-headline"` and child with `font-size=12 font-weight="inherit"` produces kernel weight `400`. The live web page inherits `600`. Accumulating the descendant’s written bit incorrectly excludes the ancestor’s fallback.

- **P2 — Nested bare styles disagree between measurement and paint.** [text_style.rs:81](/tmp/rv-s6a1-2/kernel/src/arena/text_style.rs:81): on macOS, a `text font-size="-exact-headline"` containing `text font-size="-exact-body"` measures the inner run at `700`, because `above` prevents replacement. Per-node resolution selects the nearer body style, so Apple paints that run at `400`.

- **P2 — Button projection still overwrites the title’s shorthand weight.** [button.rs:191](/tmp/rv-s6a1-2/kernel/src/arena/button.rs:191): `button font-weight=700` containing `text font="-exact-body"` computes the child at `400`, then copies the button’s `700` over it. The new mask fix preserves that incorrect value, so native buttons remain bold.

The JS overwrite was reproduced in memory. Rust/native tests were not run; the worktree and index are unchanged.
