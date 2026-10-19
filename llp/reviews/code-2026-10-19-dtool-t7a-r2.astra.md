# Code review: LLP 1122 T7a (navigate, a status's location, framed copies that say their build; tamber/1.14), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `8da06494ec`.
- **Method:** one brief (sha256 `ba3577e2a000e7b09a3317af89ed81f04576a775b2fffd64e3911841d6b5d19f`), the design inline, shared by both reviewers, each blind to the other's review; commits 7465598517, 3e2edd5186 and 8da06494ec, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 3's records):
  - Arrival compares the route as the router spells it, decoded, so an escaped character (`|`) still arrives and gets its fragment (1).
  - The server bounds a location as a URL spells it once resolved as well as given, so an admitted one can't serialize past 2 KiB (2).
  - Scheduler test (c) has a routable stand-in that counts its routes. A navigation queued behind a pin and replaced by a reconnect routes nothing and answers nothing; the new stream's navigation of the same seq routes. The test fails without the stream guard (3). The JS page has no vm harness: its guard is the same two checks, and the Chrome test covers its routing.

**Nothing blocks.** Two SHOULD-FIX findings and one NIT remain.

1. **SHOULD-FIX — router normalization can silently drop the fragment.**  
   [host/web/dev.js:484](/tmp/rv-dt-t7a2/host/web/dev.js:484), mirrored at [host/web-js/dev.mjs:712](/tmp/rv-dt-t7a2/host/web-js/dev.mjs:712), compares the requested path literally with the router’s entry. Send `/post/a|b#c` to an app with `/post/:id`: the server accepts it, but the [router serializer](/tmp/rv-dt-t7a2/host/web-js/router.js:31) writes `/post/a%7Cb`. `there()` never becomes true, so both pages finish without `#c` or an `unsupported` report. An in-memory probe reproduced this in both handlers. Compare against the router’s canonical location while retaining the redirect protection.

2. **SHOULD-FIX — the server bounds the input before URL serialization.**  
   [scripts/tamber-builds.mjs:67](/tmp/rv-dt-t7a2/scripts/tamber-builds.mjs:67) accepts `'/#' + '"'.repeat(700)`: 702 printable ASCII characters. URL serialization expands it to 2,102 characters. Both pages apply the fragment, then their `here()` returns `null` ([wasm](/tmp/rv-dt-t7a2/host/web/dev.js:458), [JS](/tmp/rv-dt-t7a2/host/web-js/dev.mjs:707)), without reporting unsupported. Thus an admitted command makes the copy’s location disappear. Validate the serialized path, query and fragment against the bound. This is distinct from the accepted policy of reporting `null` for an independently reached over-limit address.

3. **NIT — scheduler test (c) proves suppressed reporting, but not suppressed routing.**  
   [host/web/tests/dev-scheduler.test.mjs:302](/tmp/rv-dt-t7a2/host/web/tests/dev-scheduler.test.mjs:302) uses a page without a router and never sends the reused sequence on `n4`. Removing only the initial stale-stream guard would leave this test passing: the later refusal guard suppresses output, although a real routable stale command could execute. Use a routable stub with a call counter, queue it behind another command, and send the same sequence after reconnecting. Cover the JS page too. The current implementation itself correctly handles this case.

Round 1 dispositions:

- **Astra 1/2/4 and Grok 1–3:** fixed. Both pages bind navigation before queueing; agent launch keys survive; redirects keep their destination; normalized `//` paths are refused; history exceptions produce unsupported; stale refusals are not journaled; the integration test holds presence open.
- **Astra 3:** diagnostic bounds are fixed; serialized-location bounds remain incomplete as finding 2 describes.
- **Astra 5:** the authored JS handler, redirect, link/Back and wasm hashchange coverage are added. Routed wasm and unchanged `replaceState` behavior remain untested.
- **Grok 4:** the documented unique-name assumption remains. Both pages use separate keys only for framed `exact-copy-\d{1,2}` names, read without writing or sending them. The single-copy reload assertion checks ID stability.
- **Grok 5/6:** raw bidi characters are now rejected; the documented over-limit `null` policy and defensive no-router guard remain. Sender-only filtering looks correct, though the new tests do not exercise delivery to two actual clients.

Read-only review and small in-memory probes only; no files changed, builds or test suites run.
