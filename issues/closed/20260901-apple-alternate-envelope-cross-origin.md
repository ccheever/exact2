# Apple fetches alternate envelopes cross origin

**Status:** Closed
**Resolution:** Bounded Apple envelope fetches now enforce the app origin before alternate requests and after redirects.
**Systems:** Apple host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001

`host/apple/swift/PlanURL.swift:90-108,143-160` follows the HTML
`rel=alternate` envelope URL before validating it against the final page
origin. Native `URLSession` has no browser CORS boundary, so a page can direct
the host to another port, another origin, or an internal address. Later plan
and events checks are also only hostname checks, as covered by the related
origin ticket.

Pin normalized `(scheme, host, effective port)` from the final page URL and
enforce it at every redirect and every page -> envelope -> plan/events edge
before issuing the next request. Test cross-scheme, cross-port, redirect, and
alternate-link cases.
