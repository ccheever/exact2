# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 2 re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `7b2a50c6ca`.
- **Method:** one brief (sha256 `002ee35eaa1121dce5ee649a06098a305140ffb00a9a2e291313cea76bc29e03`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

SHOULD — [contract/cli/src/gallery.rs:1096](contract/cli/src/gallery.rs:1096): Add a regression test; the fix is correct, but its reachability rationale is false. `described` filters props by conditional use, and `placed` additionally admits enums compared in `if`. An enum used directly as `background-color=ink` reaches the placeholder path—existing `Tinted.ink` already does.

Use `ink: enum<"
-exact-label", "-exact-system-red">` without comparisons, plus `mark: string` used as `color=mark`. Assert `mark="-exact-system-blue"`. Before the fix, the escaped newline hides the occupied label role, so `mark` gets `-exact-label`; afterward, the raw value reserves it correctly. The existing fixture misses this because its first enum value is blue, while the first sample is label.

Tests were not run: this checkout has no build artifacts, and the review is read-only.

VERDICT: findings
