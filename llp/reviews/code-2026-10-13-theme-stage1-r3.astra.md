# Code review: LLP 1121 stage 1 (theme and its reach), round 3 (the last), 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `80d3b12ed3c6b4335e475484f8dfaefc234008536e4d8aa4ac2ec6f833535f86`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 1 MAJOR, 2 MINOR; Grok 1 MAJOR, the same).
- **Disposition:** applied after the last round without re-review (33053cf7d), each with a test

---

Three findings remain:

1. **MAJOR — The recursion guard suppresses valid overrides.** [kernel/src/style/roles.rs:379](/private/tmp/rv-stage1/kernel/src/style/roles.rs:379). It filters tint-valued overrides of every role. `CanvasText="LinkText"` with `AccentColor="#ff0000"` compiles, but default text now falls through to LinkText’s fallback instead of following the red theme accent. Explicitly naming `CanvasText` still parses to LinkText and follows the accent. Restrict the guard to the recursive accent case. This is separate from the recorded authored-accent limitation.

2. **MINOR — Gradient keywords can still become colours.** [contract/lower/src/theme.rs:136](/private/tmp/rv-stage1/contract/lower/src/theme.rs:136). The compiler accepts names such as `srgb`, `oklch` and `hue`. Executing the mapper with `srgb="#112233"` changes `linear-gradient(in srgb, #000, #fff)` into `linear-gradient(in #112233ff, #000, #fff)`. The keyword refusal remains incomplete.

3. **MINOR — Deeper functions still allow substitution inside them.** [host/web-js/src/style.rs:450](/private/tmp/rv-stage1/host/web-js/src/style.rs:450). The extra nesting level fixes the reported example, but `light-dark(rgb(0 0 0 / calc((1))), brand)` still substitutes `brand`. I reproduced this with the current mapper. Balanced function scanning is needed to enforce the stated boundary.

27 targeted tests passed using existing stage-1 binaries. No fresh build or native UI run was performed; the worktree is unchanged.

VERDICT: fix (MAJOR kernel/src/style/roles.rs:379; MINOR contract/lower/src/theme.rs:136; MINOR host/web-js/src/style.rs:450)
