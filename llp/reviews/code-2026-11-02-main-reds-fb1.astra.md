# Code review: three main-red fixes (help parity, CARGO_TARGET_DIR, Snapback receipt) (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `68acb04654`.
- **Method:** one brief (sha256 `80266b93124e356ed623719b77c7d006a81e7ec3bbc304ab9edba0def8b8de06`); a quick review under the main-red permission. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER, MAJOR, or MINOR findings. All three fixes are appropriate, complete, and minimal.

- [scripts/help-parity.test.mjs:11](/Users/admin/x2scratch/fb1-rv-reds/scripts/help-parity.test.mjs:11): Adds the source file that `scriptFlags()` now reads. Restores the fixture without weakening assertions or suppressing errors.
- [game/new-app.test.mjs:495](/Users/admin/x2scratch/fb1-rv-reds/game/new-app.test.mjs:495): Isolates the default-target assertion from inherited `CARGO_TARGET_DIR` and restores any prior value in `finally`. `resolveApp()` correctly honors the override; no product change is warranted.
- [snapback4/readme-tested.json:6](/Users/admin/x2scratch/fb1-rv-reds/snapback4/readme-tested.json:6): The refreshed receipt matches current file digests, version/revision pins, all five examples with two runs each, and both README claim dates. The date follows the retest script’s UTC convention. Given the reported successful rerun, this restores qualification without bypassing validation.

The permitted Bun command produced **6 passes and 3 failures**, all three caused by sandbox-denied temporary-directory creation (`EPERM`). The committed-receipt check passed. I did not rerun the generated-workspace test or full example qualification.

VERDICT: APPROVE
