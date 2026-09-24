# Web fetch follows redirects without re-checking the grant

**Status:** Closed
**Resolution:** Fixed: ordinary and speculative early browser fetches reject redirects before any next-hop request. Real local Fetch regression confirms ungranted POST destination sees zero requests. Same-origin redirects are also refused because browser manual responses hide Location; use the final URL (LLP 1016 D2).
**Systems:** Web host, Data seam
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1027 D6; LLP 1016

An app whose only network grant is one origin can still read a response from another origin. The web host checks `granted()` on the URL it is about to request, then tells the browser to follow redirects. The body that comes back is whatever the final response was. Native fetch admits the origin again on every hop and says why: a grant for `a.example` must not become a grant for `b.example` because `a.example` answered with a `Location` (`vendor/ibex2/src/stdlib/fetch.rs`, `fetch_stream`).

`host/web/glue.js` sets `redirect: "follow"` unless the call is a bundled-asset GET or carries an explicit source scope. Ordinary app fetches are the follow path. Nothing after `fetch` compares `response.url` to the grant.

A module granted only `net.fetch https://api.example` requests `https://api.example/r`. That host returns 302 to `https://app.example/account.json`, or to any URL the caller can influence on the granted host. The page's `fetch` uses `credentials: "same-origin"`, so a redirect back to the page origin sends cookies and the body is readable with no CORS. A redirect to another host that opts into CORS is readable too. When the body is hidden, the browser has still issued the second request, including to a loopback or link-local name.

Asset GETs and scoped calls already use `redirect: "error"`. Do the same for ordinary fetches, or follow in script and run `granted()` on each `Location` before the next request, and do not send cookies to a different origin. Done when a redirect off the granted origin is a refusal on the web, as it already is on Apple and Linux, and a same-origin redirect still completes.
