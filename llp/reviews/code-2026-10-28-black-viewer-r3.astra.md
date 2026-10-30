# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `837d22890b`.
- **Method:** one brief (sha256 `d7a7004171d308a9e888b22cbbec8c61c2ad6aad03db14610c1b7d5c1c5a4a55`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

Both round 2 blockers appear resolved by static inspection. Two correctness issues remain:

1. **[P2] Urgent downloads can starve other urgent downloads** — [host/linux/src/image/remote.rs:275](host/linux/src/image/remote.rs:275). New urgent requests and promotions use `push_front`, making urgent service newest-first. The streak limit protects only ordinary requests. With workers occupied and newer urgent requests continually arriving, an older urgent picture can time out without starting, repeatedly exhaust its retries, and remain blank. Preserve FIFO ordering within the urgent class, including promotions, or add aging that guarantees older urgent requests a turn.

2. **[P2] The thumbnail corrupts transparent images after loading** — [apps/excord/viewer.contract:124](apps/excord/viewer.contract:124). The thumbnail remains mounted after the original loads. For a resized PNG with partial transparency, both images composite together: matching 50%-alpha pixels combine to 75% coverage, changing the displayed picture and its edges. Remove the thumbnail once the original is successfully presented, using a completion mechanism supported on Linux/Android too; retain it while loading or after failure.

**Non-blocking**

None.

Reviewed `git show HEAD` and `git diff HEAD~4 HEAD`. No edits, network access, tests, or builds.
