# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `2b4bae29e7`.
- **Method:** one brief (sha256 `919e13e6d6ebfe76a7073a18acb2f15f9d825d6c15b64014403fa39b812f331e`), shared by both reviewers, blind to the other review; commits f0192030bf 3b42c0fc18 2b4bae29e7. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium).
- **Disposition:** taken in `486ae7b8cf`: the tests' agent drives name the staged tool (`EXACT_APP_DIR`), whose app.json names no server, so no drive starts the stub on 7740; staging that moves or removes any file makes the build again (`changed`), so a deleted source can't leave a warm bundle accepted; the API literal must appear exactly once or staging refuses; an `EXACT_TOOL_PORT` that is not a port from 1024 to 65535 is refused, never 7740 by default (and port 80 no longer listens on 0: the listener takes `TOOL_PORT`); port-lock's note updated; the checkout-port comment admits a hash collision. Not taken: an actual browser frame refusal from 7740 (the CSP string is the whole mechanism, asserted exactly).

Changes requested. The suite can still occupy 7740, and the staged build can remain stale.

1. **[P1] Agent drives still start the stub on 7740.** [design-tool.test.mjs:250](/tmp/rv-dt-port/scripts/design-tool.test.mjs:250) retains `--app design-tool`. [agent.mjs:1397](/tmp/rv-dt-port/scripts/agent.mjs:1397) starts that original app’s backend even with `--url`. A read-only probe with `EXACT_TOOL_PORT=17999` returned `stub/tamber-stub.ts`, URL `http://127.0.0.1:7740`. Once the real tool moves away, a drive finding 7740 free starts the stub there. This integration suite therefore still holds the person’s port and can collide across checkouts. Drive the staged app or suppress backend auto-start for these drives.

2. **[P2] Source deletion does not invalidate a warm build.** [tamber-tool-port.mjs:26](/tmp/rv-dt-port/scripts/tamber-tool-port.mjs:26) removes deleted files, but [tamber-server.mjs:1093](/tmp/rv-dt-port/scripts/tamber-server.mjs:1093) relies on `webChanges()`, which examines surviving files newer than the marker. Scenario: build successfully, then delete imported `dials.ts` without changing its importer. An in-memory execution of the actual functions removed the staged file but returned `all: []`; the old bundle remains accepted instead of rebuilding and reporting the missing import. Cache identity must include the complete source inventory and contents, including deletions.

3. **[P2] The origin replacement still fails silently.** [tamber-tool-port.mjs:29](/tmp/rv-dt-port/scripts/tamber-tool-port.mjs:29). Changing the API literal to double quotes leaves the staged API and grants on 7740 while the server and framing policy use the chosen port. I reproduced this in memory. Multiple matching literals also replace only the first. Require exactly one match before staging, as review2 requested.

4. **[P2] Invalid port settings silently reclaim 7740.** [tamber-tool-port.mjs:11](/tmp/rv-dt-port/scripts/tamber-tool-port.mjs:11). `EXACT_TOOL_PORT=''`, `0`, or `invalid` survives the test’s `??=` and then falls back to 7740. Reject invalid explicit settings. Also, valid port `80` becomes an empty URL port at [tamber-server.mjs:1161](/tmp/rv-dt-port/scripts/tamber-server.mjs:1161), making the listener request port **0** while advertising 80; use the validated numeric port directly.

The remaining paths look sound by inspection and read-only probes:

- With a valid port, the staged API/grants, server, and spawned dev loops agree. Framing remains `'self'` plus exactly the chosen loopback origin; no wider policy or non-loopback tool origin was introduced.
- Source/output directories separate alternate builds from the default build. Default constants and source/destination paths remain unchanged; I did not perform a build byte comparison.
- Hash collisions serialize through `takePort`; leftover listeners are refused without being killed. The comment claiming another checkout never waits should acknowledge collisions.
- The contract-test stub entry remains in [QUEUE.md:1538](/tmp/rv-dt-port/QUEUE.md:1538).

Smaller coverage gap: [tamber-tool-port.test.mjs:16](/tmp/rv-dt-port/scripts/tamber-tool-port.test.mjs:16) checks the CSP string, not an actual rejected frame from 7740.

No files changed. No builds or mutating tests were run.
