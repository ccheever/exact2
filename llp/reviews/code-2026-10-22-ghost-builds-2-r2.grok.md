# Code review: the ghost builds' follow-up (res close backstops, a test per door), round 2 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `5741c64e03`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; round 1 closed; no findings.

I'll review the latest commit read-only and check whether the three round-1 findings are actually closed.The request is a read-only review of the two commits, so I’ll inspect the diffs and the surrounding call sites directly.Nothing blocks. The three round-1 findings are closed, and the rest of the two commits does not add another defect.

1. **Wasm `/__dev` backstop is tested.** `host/web/tests/dev-stream-hand-off.test.mjs:18` starts `host/web/dev.mjs --app theme-fixture --wasm` under the hand-off preload, registers on `GET /__dev` (the wasm stream, not `/__dev/page`), and expects the build to drop when that socket is destroyed. `host/web-js/hand-off.preload.mjs:12` keeps one side listener and retargets `to` on every later `listen` of that port, so an earlier probe does not keep the side port or the socket.

2. **Door-test timeouts.** `scripts/tamber-remote.test.mjs:247`, `scripts/tamber-canvas.test.mjs:990`, and `scripts/tamber-relay.test.mjs:659` each pass `30_000`. Setup plus the 5 s wait fits in that.

3. **First SSE event is read whole.** `scripts/tamber-remote.test.mjs:234` and `scripts/tamber-canvas.test.mjs:979` append `data` until `\n\n`. Both writers emit one compact `data:` frame ending in `\n\n`, and `JSON.stringify` does not put a raw newline in the payload. The relay test already accumulated bytes.

No findings.
