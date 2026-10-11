# Web fetch delivers a redirect's next hop before the grant check

**Status:** Open
**Systems:** host/web, host/web-js, grants
**Severity:** P1
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** vendor/ibex2 fetch.rs (check each hop before open); host/web/request-refusal.test.mjs

Both web request paths follow a redirect in the browser, then check the final URL against `net.fetch`. The ungranted origin has already received the request.

`host/web/http-body.js` and `host/web-js/admission.js` pass `redirect: 'follow'`. After `fetch` resolves they refuse when `response.url` is outside the grants, with `outside the app's grants (net.fetch): redirected to <origin>`. A 307 keeps the method and body, so that body is delivered to the next hop before the refusal.

Reproduced with Bun 1.4.2 against `fetchWith`: a grant for `http://127.0.0.1:<granted>` only, POST body `review-test-payload`, 307 to another loopback port. The runtime returned `Refused` naming that port, and the ungranted server recorded `{method: POST, body: review-test-payload}`.

The native executor does the opposite. `vendor/ibex2/src/stdlib/fetch.rs` checks the grant on every hop before `transport.open`, and the transport is specified not to follow redirects. `the_platform_does_not_launder_a_redirect_past_the_capability_check` fails if the ungranted server is hit. The web tests lock in the leak: `host/web/request-refusal.test.mjs` expects `destinationHits` to be 2 (wasm and JS) and 1 (the early request). The error string matches native; the request does not.

`redirect: 'error'` would stop the second hop and also refuse a redirect whose destination is granted. A hop-by-hop follow has to use `redirect: 'manual'` and only continue when `Location` is visible and granted; an opaque cross-origin redirect cannot be inspected, so it has to be refused rather than followed. Update the hit-count assertions in the same change. The docs line that a redirect "must stay inside the grants" should say the next hop is not sent.

## Audit (2026-10-08)

LLP 1071, “Found beside them”, currently declares browser follow plus final-URL admission and explicitly treats web grants as parity rather than a browser security boundary. The next-hop contact here is that declared limitation. Changing it to reject/manual requires a policy decision; this record does not authorize that change. The separate intermediate-hop outcome issue still identifies a stronger deviation than the parity wording states.
