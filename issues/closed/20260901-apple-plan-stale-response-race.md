# Apple plan boots race stale responses

**Status:** Closed
**Resolution:** Apple URL resolutions now carry generations, cancel superseded fetches, and ignore every stale completion.
**Systems:** Apple host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001

Apple can run `resolveAndBoot` from initial resolution, reconnects, and
SSE-triggered refreshes without a generation or cancellation token
(`host/apple/swift/PlanURL.swift:164-248`). A slow earlier page/envelope/plan
chain can finish after a newer one and boot stale bytes over the newer app.

This is separate from dropping the last-good app on a boot refusal: every
candidate may be valid, but completion order—not request order—chooses the
winner.

Give each resolution generation an identity, cancel superseded URLSession
tasks, and check the generation immediately before hashing and booting. Add a
deterministic transport test where generation A completes after generation B
and cannot replace it.
