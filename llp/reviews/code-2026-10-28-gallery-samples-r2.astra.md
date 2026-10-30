# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `dc33c1da96`.
- **Method:** one brief (sha256 `573803583235aa3996689ea4982939f8fe0c5b89f551c76a1ad660f24b2558e0`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD** — [gallery.rs:1035](contract/cli/src/gallery.rs:1035): `align-self="auto"` triggers `flex-start`, although `auto` inherits the parent’s alignment. A root `column align-self="auto"` therefore shrinks where it previously filled the cell, reproducing round 1’s regression. Attribute presence alone is insufficient.

- **SHOULD** — [gallery.rs:1034](contract/cli/src/gallery.rs:1034): Root `Use` and conditional nodes bypass the sizing fix. An exported wrapper around `FollowButton(shape="pill", …)` still stretches its auto-width button, while placing FollowButton directly now works. Inspect the effective roots; fixed-width `Ring`/`Dot` fixtures do not expose this failure.

- **SHOULD** — [gallery.rs:1057](contract/cli/src/gallery.rs:1057): Ranking includes colour props that never receive a sample. With `ink: enum<"-exact-system-blue", "-exact-system-red">` followed by `mark: string`, used directly as background and foreground without conditions, `ink` gets its first enum literal through `placeholder`. It nevertheless consumes rank zero, so `mark` also receives blue. Rank only sampled arguments and reserve colours supplied by other placeholder paths.

- **SHOULD** — [gallery.rs:1061](contract/cli/src/gallery.rs:1061): Raw string comparison misses equivalent role spellings. A variant supplying `-apple-system-label` or `-EXACT-LABEL` leaves `-exact-label` available, allowing foreground and background to coincide again. Compare resolved role IDs.

The four roles have resolution paths on every requested host; the web CSS generator check passed. Resolved `100%` widths remain full width, and `max-width` still caps stretch. An empty `free` list cannot panic.

Rust integration tests were not run: this read-only checkout has no build artifacts.

VERDICT: findings
