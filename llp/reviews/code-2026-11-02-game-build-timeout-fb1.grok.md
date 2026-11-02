# Code review: the new-game build test's cold-build timeout (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `f3dfb685c3`.
- **Method:** one brief (sha256 `98c684e02ab4a6559fa867763b0a21d2697b5c1804c8bc69caae20cbe5a3139f`); a quick review under the main-red permission (Astra was at capacity). The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER, MAJOR, or MINOR findings.

`f3dfb685c3` only raises the bun:test deadline on "a newly generated game builds, reports empty pins and captures without editing" from `600000` ms to `1500000` ms and adds the one-line comment. That is 1,500 s, the same millisecond unit as the old 600 s. The test still deletes `CARGO_TARGET_DIR`, so the build stays in the game's own temp target. Each `run()` arms a 180 s timer, resets it when stdout or stderr emits, and on silence SIGKILLs the detached process group; the close handler then fails the step. A quiet hang still ends at 180 s per command. The 1,500 s limit is the budget for a build that keeps printing, which is what ran past 600 s (391 crates, `CARGO_BUILD_JOBS=4`). The same test's last recorded full cold pass is 1,253 s under high load (LLP 1086, 2026-10-08), so this ceiling sits above that run.

VERDICT: APPROVE
