# Web fetch delivers a redirect's next hop before the grant check

**Status:** Closed
**Resolution:** Accepted final-URL admission on web; `host/web/request-refusal.test.mjs` and `host/web/http-body.test.mjs` record destination hits and POST return chains; render-page `connect-src` blocks only hops outside the page origin and app grants, so a source-ungranted app-granted hop remains reachable.
**Systems:** host/web, host/web-js, grants
**Severity:** P1
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** vendor/ibex2 fetch.rs (check each hop before open); host/web/request-refusal.test.mjs

Both web request paths follow a redirect in the browser, then check the final URL against `net.fetch`. On static and dev pages without CSP, an ungranted intermediate origin can already have received the request. Render-server pages block a hop outside the page origin and app grants with `connect-src` before contact; a source-ungranted hop inside app grants remains reachable.

`host/web/http-body.js` and `host/web-js/admission.js` pass `redirect: 'follow'`. After `fetch` resolves they refuse when `response.url` is outside the grants, with `outside the app's grants (net.fetch): redirected to <origin>`. A 307 keeps the method and body, so that body is delivered to the next hop before the refusal.

Reproduced with Bun 1.4.2 against `fetchWith`: a grant for `http://127.0.0.1:<granted>` only, POST body `review-test-payload`, 307 to another loopback port. The runtime returned `Refused` naming that port, and the ungranted server recorded `{method: POST, body: review-test-payload}`.

The native executor does the opposite. `vendor/ibex/crates/ibex2/src/stdlib/fetch.rs` checks the grant on every hop before `transport.open`, and the transport is specified not to follow redirects. `native_post_refuses_a_source_ungranted_middle_hop_before_contact` in `host/apple/src/executor_tests.rs` checks that a POST to granted A is refused before the source-ungranted B receives it, even though the app grants B and the chain could return to granted C. The web tests lock in the contact: `host/web/request-refusal.test.mjs` expects `destinationHits` to be 2 (wasm and JS) and 1 (the early request). The error string matches native; the request does not.

`redirect: 'error'` would stop the second hop and also refuse a redirect whose destination is granted. A hop-by-hop follow has to use `redirect: 'manual'` and only continue when `Location` is visible and granted; an opaque cross-origin redirect cannot be inspected, so it has to be refused rather than followed. Update the hit-count assertions in the same change. The docs line that a redirect "must stay inside the grants" should say the next hop is not sent.

## Audit (2026-10-08)

LLP 1071, “Found beside them”, currently declares browser follow plus final-URL admission and explicitly treats web grants as parity rather than a browser security boundary. The next-hop contact here is that declared limitation. Changing it to reject/manual requires a policy decision; this record does not authorize that change. The separate intermediate-hop outcome issue still identifies a stronger deviation than the parity wording states.
