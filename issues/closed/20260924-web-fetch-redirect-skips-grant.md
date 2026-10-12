# Web fetch follows redirects without re-checking the grant

**Status:** Closed
**Resolution:** 2026-09-24 restriction superseded on 2026-10-11; see the dated correction below. Browser fetch now follows redirects by default and checks the final response URL.
**Systems:** Web host, Data seam
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1027 D6; LLP 1016

An app whose only network grant is one origin can still read a response from another origin. The web host checks `granted()` on the URL it is about to request, then tells the browser to follow redirects. The body that comes back is whatever the final response was. Native fetch admits the origin again on every hop and says why: a grant for `a.example` must not become a grant for `b.example` because `a.example` answered with a `Location` (`vendor/ibex/crates/ibex2/src/stdlib/fetch.rs`, `fetch_stream`).

`host/web/glue.js` sets `redirect: "follow"` unless the call is a bundled-asset GET or carries an explicit source scope. Ordinary app fetches are the follow path. Nothing after `fetch` compares `response.url` to the grant.

A module granted only `net.fetch https://api.example` requests `https://api.example/r`. That host returns 302 to `https://app.example/account.json`, or to any URL the caller can influence on the granted host. The page's `fetch` uses `credentials: "same-origin"`, so a redirect back to the page origin sends cookies and the body is readable with no CORS. A redirect to another host that opts into CORS is readable too. When the body is hidden, the browser has still issued the second request, including to a loopback or link-local name.

Asset GETs and scoped calls already use `redirect: "error"`. Do the same for ordinary fetches, or follow in script and run `granted()` on each `Location` before the next request, and do not send cookies to a different origin. Done when a redirect off the granted origin is a refusal on the web, as it already is on Apple and Linux, and a same-origin redirect still completes.

**Correction (2026-10-11):** The earlier resolution described the 2026-09-24 restriction, not current behavior. Web requests now follow redirects by default and check the final response URL. On render-server pages, CSP `connect-src` blocks a redirect to an ungranted origin outside the page's own origin before contact; fetch reports `Network`. Static and dev pages have no CSP: the browser can send a POST through an ungranted hop and accept a response if the chain returns to a granted origin, while native refuses before contacting that hop. `host/web/request-refusal.test.mjs` covers both web paths without CSP; `host/render/src/serve_tests.rs` covers the rendered page's `connect-src` list.
