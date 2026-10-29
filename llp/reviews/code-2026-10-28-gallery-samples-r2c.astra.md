# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 2 re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c91722b959`.
- **Method:** one brief (sha256 `18eb00ecc3a07e75110cecd1873106474f1d39c52d9f6b6493bac85bdd3755d1`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

SHOULD — [contract/cli/src/gallery.rs:1096](contract/cli/src/gallery.rs:1096): Placeholder role lookup uses escaped source text. An enum placeholder `"-exact-label	"` resolves to the label colour when rendered, but stripping quotes leaves literal `	`, so `taken` misses it. A sampled foreground then also gets `-exact-label`, recreating the collision. Resolve the enum’s raw first value before escaping and cover this spelling in the regression test.

The other requested cases and width revert look clean. Rust tests were not run; no files changed.

VERDICT: findings
