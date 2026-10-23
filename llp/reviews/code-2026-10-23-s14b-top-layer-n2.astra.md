# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), narrow re-check 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `894220519e`.
- **Method:** one brief (sha256 `efe2db7df5a8c13dbcac978a91b5f35a02edd6e25376ddc52d0cd68b987f695f`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

RIGHT.

No blockers found in the requested diff or affected callers.

- `excluded` skips block roots and their descendants during selection, and rejects retained anchors that cross into a block. Scrolling inside a block remains eligible.
- `validate` checks current top-layer membership. Host-supplied blocks remain separately prohibited, and registration removes stale implied blocks before layout.
- `top_layer_origin` correctly handles both axes, root direction, and negative margins under [CSS 2 §10.3.7 and §10.6.4](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width). Terminal placement and the runner’s fallback consume its border-box origin consistently.

By inspection, removing the corresponding fixes makes the regressions fail: the row jumps 48 points, former-popover registration is rejected, vertical placement returns `0` instead of `-40`, RTL returns `0` instead of `720`, and the clamped-margin case returns `700` instead of `720`.

Rust tests were not executed: this checkout has no built test artifacts, and building would violate read-only. An in-memory arithmetic comparison passed 20,480 cases.
