# Code review: design-tool.test beside another run, follow-up round on 5e5435b866 (round 1 answered), b85aa54004 (per-dist JS dev stage) and a93c2f4d22 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** follow-up round on 5e5435b866 (round 1 answered), b85aa54004 (per-dist JS dev stage) and a93c2f4d22; HEAD `a93c2f4d22`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; its SHOULD-FIXes and NITs answered in 3c4a6c99f0, but for the GPU cache (stated out of scope).

No BLOCKER found for two copies in one checkout. T2b’s lock covers teardown; T2’s error check is correctly ordered. The nested stage works with `replaceBuild`, and I found no other direct stage readers or cleaners. Same-dist loops still collide, as expected.

1. **SHOULD-FIX — [dev-builds.test.mjs:216](/tmp/rv-dt-conc2/host/web-js/dev-builds.test.mjs:216):** The regression test doesn’t explicitly share `CARGO_TARGET_DIR`. Without that inherited variable, these workspace-less apps get separate targets, so the old implementation can pass. Give both loops one temporary target.

2. **SHOULD-FIX — [dev.mjs:1015](/tmp/rv-dt-conc2/host/web-js/dev.mjs:1015):** Native forwarding still assigns every resident loop `<target>/web-dist-resident`. Distinct apps sharing a target can overwrite each other’s served artifacts despite separate JS output directories.

3. **SHOULD-FIX — [dev.mjs:377](/tmp/rv-dt-conc2/host/web-js/dev.mjs:377):** Identical plans share `inspect-<digest>.json`; each completion deletes it. Concurrent inspection can remove another loop’s result before it reads it, producing HTTP 503.

4. **SHOULD-FIX — [build.mjs:725](/tmp/rv-dt-conc2/host/web-js/build.mjs:725):** The GPU cache remains shared by target and app name. Its unprotected removal/rebuild can race another loop’s build or copy.

5. **NIT — [design-tool-harness.mjs:17](/tmp/rv-dt-conc2/scripts/design-tool-harness.mjs:17):** Starting at checkout-port + 1 avoids the stub normally, but `claimPort` wraps through all 200 ports. It can still claim the stub’s port when the other 199 are unavailable; explicitly exclude it for a strict guarantee.

Read-only review; no builds or tests run.
