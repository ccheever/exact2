# Code review: design-tool-harness's port after another file loaded the port module (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one round (a quick review: a test-only fix); one brief (sha256 `198e8608ccb26f45566e1876b74ae8fecd38c9c0eeb64f158aa9601ecc8d178a`), shared by both reviewers, each blind to the other's review; HEAD `33b40499ba`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 1 SHOULD-FIX
- **Disposition:**
  Both reviewers found the same BLOCKER, and it is fixed post-round, in its own commit: the test cleared EXACT_TOOL_PORT, loaded the harness (which caches its constants and sets the variable), then deleted the variable again, so a later file in the same process would spawn children with no port (7740) against a harness on the checkout's. The test now runs its scenario in a subprocess with the variable unset: no environment and no cached module reach another file. Astra's SHOULD-FIX (an explicit override lost) goes with it. The fix is confirmed: tamber-server.test.mjs and design-tool-pair.test.mjs run in one process, 31 of 31, the trusted-tunnel T6 among them (it failed there at main). The new test is red at main's harness. Grok notes the lane's glue step runs `bun test --parallel`, which isolates files, so the flake hit only a run by hand of several files together.

1. **BLOCKER — [scripts/design-tool-harness.test.mjs:20](/tmp/rv-dt-hns-astra/scripts/design-tool-harness.test.mjs:20):** If this test loads the harness first with the variable initially unset, cleanup deletes `EXACT_TOOL_PORT` but leaves the harness cached at the checkout port. Later `design-tool.test.mjs` or pair tests reuse those constants while children inherit no override and serve 7740—recreating the timeout. Run this regression scenario in an isolated subprocess.

2. **SHOULD-FIX — [scripts/design-tool-harness.test.mjs:10](/tmp/rv-dt-hns-astra/scripts/design-tool-harness.test.mjs:10):** With an explicit port override and an uncached harness, deleting the override makes the harness select the checkout port, but line 14 expects the original override. Restore it after priming the port module, before importing the harness.

The harness fix itself looks correct: the query instance’s `stageTool` and `TOOL_DIST` match fresh children. Cached server constants remain different, but explicit `toolDir` passes through correctly, and existing in-process framing tests use their own cached `FRAME_ANCESTORS`. `design-tool.test.mjs` receives the same port correction; its framing servers run in children.

Static review only; no changes, builds, or tests.
astra exit 0
