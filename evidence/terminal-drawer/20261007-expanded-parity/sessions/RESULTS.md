# Terminal session verification

Production Swift transport, terminal session reducer, and bounded output buffer were compiled directly from this worktree. Only the terminal view is a headless notification/message sink; no web renderer, GUI input, IME, or screenshot claim is made.

Pinned server: ../t3-code/target/t3-ref/src-1e2ecbd975/apps/server/dist/bin.mjs, Node /opt/homebrew/bin/node, port 16330. launch.json records PID and argv. HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_CONFIG_HOME and T3CODE_HOME/base-dir isolated beneath this directory; telemetry false; /bin/sh shell.

Final result: run-3.log, exit 0, all 23 assertions pass. compile.py builds main.swift plus the production sources. Errors.swift is an unchanged extraction of the unrelated T3PullRequestErrors enum needed by T3Protocol; view-only methods are stubbed in main.swift. source-sha256.json records key tested files.

Verified:
- Real attach creates a shell in the worktree, with project-root/worktree/custom environment variables.
- Real PTY reports 37x111 after resize; after disconnect, an offline fit to 42x123 is applied on reconnect without another fit and stty reports 42x123.
- Reconnect snapshots retain prior output.
- One 65,537-character write is refused by server length validation; one write failure/message, no client splitting.
- A 900,000-byte workload drains to its final marker; production output is bounded to 524,288 bytes, <=1,024 chunks, <=16,384 bytes/chunk; initial renderer cursor replays precisely the retained buffer.
- A separate production attach stream withholding ACKs stops at 8 delivered chunks, remains stopped across two 0.7-second observations, then drains to the completion marker after releasing ACKs.
- Close all terminals for thread is accepted, attached terminal sees closed; reopening yields an actual string history without the deleted marker.
- A real three-scope OAuth session (orchestration:read, orchestration:operate, review:write) connects, attach is refused with terminal:operate error, and remains one attach attempt after 2 seconds.

No tracked code changes were needed. No confirmed defect remained in this bounded session/transport pass. This does not prove whole-app deletion orchestration, normal-app relaunch persistence, original Electron pixel parity, or view/browser behavior; those are coordinator/other-worker checks.

The fixture server was stopped using its recorded PID after verification. Pairing tokens in local pair/server logs were consumed and are not publication artifacts; publish only the redacted result log, source hash, and harness if desired.
