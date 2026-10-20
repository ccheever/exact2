# Code review: LLP 1122 T7a (navigate, a status's location, framed copies that say their build; tamber/1.14), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `7465598517`.
- **Method:** one brief (sha256 `f24d738dfe029ab96f0c9057f7ff0ca331155a1843db7a4f71756859c767afdd`), the design inline, shared by both reviewers, each blind to the other's review; commit 7465598517. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 BLOCKER, 4 SHOULD-FIX).
- **Disposition:** taken in the round's fix commit (named in round 2's records):
  - A command is bound to the stream it came on. A frame of a replaced stream runs nothing and answers nothing, so its refusal never answers the new stream's navigation of the same seq. Tested in dev-scheduler (c), which fails without the binding (blocker 1).
  - The fragment goes only on the route asked once the router's entry is there. A handler's redirect keeps the app's place; tested, /post/0 to /post/9 (2).
  - A location is printable ASCII (percent-encoded), and a refusal's detail is bounded to 1 KiB, with the location in it cut at 200 characters (3).
  - The Chrome test holds a subscription open for presence (4).
  - The JS fixture routes through an authored `navigate` handler with a redirect. Leader moves by a link and Back are reported through the history hooks; the wasm fixture's fragment change by hand is reported through hashchange (5).
- **Not taken:** a routed wasm fixture. The wasm loop builds only workspace apps, and none with a router has a design. Its route step is input-glue.js's link step, which the link tests hold.

**One BLOCKER found.** Read-only review of `7465598517`; no files changed, builds, or test suites run.

1. **BLOCKER — queued navigation can attribute an old refusal to another client after a restart.**  
   [host/web/dev.js:871](/tmp/rv-dt-t7a/host/web/dev.js:871) and [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a/host/web-js/dev.mjs:712) queue commands without capturing their receiving stream. `report`/`rep` captures the current nonce when the command eventually executes. Concrete scenario: A’s navigation `seq:1` waits behind a pending hold; the server restarts; the page registers again and B sends the new server’s `seq:1`. A’s queued refusal then uses the new nonce, and [tamber-builds.mjs:453](/tmp/rv-dt-t7a/scripts/tamber-builds.mjs:453) delivers it to B and consumes B’s pending refusal record. Bind commands to their receiving registration and discard stale work before routing, changing the fragment, or reporting.

2. **SHOULD-FIX — fragment handling overwrites a router redirect with the requested path.**  
   [host/web/dev.js:481](/tmp/rv-dt-t7a/host/web/dev.js:481), mirrored at [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a/host/web-js/dev.mjs:712). Request `/private#details` when the app’s navigation handler redirects to `/login`. Both implementations accept the router change, exhaust their polling loop, then replace the address with `/private#details` while the router and `history.state.url` still describe `/login`. Status now reports a location the app did not accept. Only attach the fragment after confirming the intended route arrived; preserve redirects or report unsupported when it did not.

3. **SHOULD-FIX — URL serialization can make an unsupported report too large to deliver.**  
   [scripts/tamber-builds.mjs:65](/tmp/rv-dt-t7a/scripts/tamber-builds.mjs:65) accepts `"/" + "é".repeat(1000)`: even its UTF-8 representation is below 2 KiB. URL parsing expands its pathname to 6,001 characters. An unmatched route then includes that entire pathname in the diagnostic at [host/web/dev.js:475](/tmp/rv-dt-t7a/host/web/dev.js:475) or [host/web-js/router.js:200](/tmp/rv-dt-t7a/host/web-js/router.js:200), exceeding the 4,096-character report limit at [tamber-builds.mjs:126](/tmp/rv-dt-t7a/scripts/tamber-builds.mjs:126). The report receives 400, which the page ignores, so the sender gets neither navigation nor unsupported. Bound diagnostics independently and check the serialized location’s size.

4. **SHOULD-FIX — the Chrome tests can lose client presence during their own waits.**  
   [scripts/dev-copies.test.mjs:76](/tmp/rv-dt-t7a/scripts/dev-copies.test.mjs:76) establishes presence once, then waits for loading, registration, and reloading using bounds of 120 seconds. Presence expires after 35 seconds; `hello` polling does not renew it. A slow run therefore reaches the navigation assertion with a 409 “not present,” despite correct page behavior. Keep a subscription open or renew presence throughout these waits.

5. **SHOULD-FIX — the integration tests leave the main routing and leader-status paths untested.**  
   At [scripts/dev-copies.test.mjs:46](/tmp/rv-dt-t7a/scripts/dev-copies.test.mjs:46), the wasm fixture has no router and only changes a fragment. The JS fixture exercises the fallback router, with no authored `navigate` handler. All tested movements originate from the new command, which explicitly schedules status itself: removing the history hooks could still leave these assertions passing. Add coverage for routed wasm, an authored handler—including redirect/refusal—and leader navigation through links, Back, and hash changes, plus unchanged `replaceState` calls.

Otherwise, I found no cross-origin navigation or nonce exposure. The parent message contains only `type` and `id`; nested-frame and popup rejection remains the receiver’s responsibility. Write scope, presence checks, journaling, and refusal filtering within one server run look correct. The history hooks deduplicate unchanged facts, and the existing VM contexts remain safe without `window` or `history`. Named persistence requires unique, stable frame names; application renaming breaks continuity, and duplicate-tab detection retains the existing 60 ms BroadcastChannel limitation.
