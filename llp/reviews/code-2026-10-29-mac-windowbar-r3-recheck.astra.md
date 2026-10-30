# Code review: the Mac window bar's read set, round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `ea4879b5e3`.
- **Method:** one brief (sha256 `c195446ed5f6258d7d722945ca03a3ef688a9a70e57694b913e7088c95ad482b`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Both round-3 MAJORs are addressed in HEAD:

- `gates` captures hidden/route-inert state across the recorded roots and their physical/authored ancestors, including external invocation targets and labelledby nodes.
- `projected` captures actual `isHidden`, detecting NavigationHost unhiding a projected carrier. The shadow now compares owner/header/beside/parts visibility before and after `sync()`, so that repair produces a mismatch.

The two added tests reproduce Astra’s sequences with the shadow disabled and assert the resulting visibility. I found no remaining visibility omission in these paths.

Independent red/green execution and timing remain unverified: the targeted XCTest command failed during Swift initialization with `permissionDenied`. No files changed; diff checks passed.

VERDICT: APPROVE

1. **MINOR — [host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:166](host/apple/Sources/ExactKit/Mac/ToolbarReadsMac.swift:166):** The per-batch snapshot is not strictly O(read set): each hidden node linearly searches `mine`. With R recorded nodes and P projected parts, this can reach O(R×P); the projected parts alone produce quadratic membership work. It does not scan the page, and ordinary bars keep P small. Build an identity set once to make this new pass linear.
