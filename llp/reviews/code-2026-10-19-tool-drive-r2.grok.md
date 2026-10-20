# Code review: The design tool's direct drive and verify on the checkout's own port (scripts/tool-drive.mjs), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6f08d91bbf`.
- **Method:** one brief (sha256 `45e120903b3d3a32772cecd9d71e6e35c1037d63302ec1577d87df807569b96c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** none needed

Nothing blocking.

The round-1 fixes match `resolveApp`. `namedDir` takes a path, then `EXACT_APP_DIR`, then the name. A path is rewritten to `design-tool` after staging, at the same argv index `stageToolTests` keeps. `--help` and `-h` return before any env change or copy. `--app=` is ignored here, and both CLIs reject that token in `guardFlags` before `resolveApp`.

The rewrite is safe for another cwd and for a symlink. The path is resolved and realpath-compared only to decide that this checkout's design tool is the one named. The value passed onward is the bare name `design-tool`, and `EXACT_APP_DIR` is the absolute stage directory from `stageTool`, so later resolution no longer depends on the original cwd or the symlink.

The entry-point test matches the refusal paths. `verify` calls `heldLine` before `webBuild` when the staged `app.ts` origin answers and no turn is held on the lock port. `agent` reaches `driveServer` before `open`; `--storage` plus `--reset-storage` sets `fresh`, so an answer throws the `server.url` line before a build. `17994` is outside `checkoutPort`'s `17741`–`17940` range. The stage left at `target/design-tool-test-17994` is gitignored, same as the other port stages. A held `17994` cannot pass the test for a source-tree run: that URL is printed from the resolved app's `app.ts`, and the source tree still says `127.0.0.1:7740`.
