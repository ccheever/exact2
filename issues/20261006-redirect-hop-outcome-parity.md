# Resolve intermediate redirect outcome divergence and conflicting design text

**Status:** Open
**Systems:** grant policy, web/native parity, architecture documents
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1071 redirect deviation; LLP 1016; LLP 1027.001; existing redirect issue

The declared browser redirect compromise has a larger observable deviation than its current design text states. This is related to `20261006-web-fetch-follows-redirect-before-grant-check.md`, but concerns successful outcomes and contradictory authority rather than the already documented contact with an ungranted final destination.

Reproduced in Chrome using the real `fetchWith`: granted origin A returns 307 to ungranted B; B returns 307 back to granted A. The runtime returns a successful 200 and reads the final body because it checks only `response.url`. B also received the original POST body. Native ibex checks each hop before opening it and refuses at B, so it never reaches that final response.

LLP 1071's “Found beside them” section says every redirect producing a Response retains outcome parity. This case disproves that statement. LLP 1016's browser redirect restriction and LLP 1027.001 describe refusing browser redirects altogether, which conflicts with the implemented and newly declared follow policy.

Choose and state one coherent current policy: either reject browser redirects to provide conservative grant enforcement, or explicitly accept/document intermediate-hop outcome divergence as well as contact and CORS differences. Browser page code retains direct fetch; this issue does not claim grants create a browser sandbox. Reconcile the governing documents and developer-facing reference.

Acceptance: a three-origin/hop fixture demonstrates the chosen behavior on wasm web, JS web and native; the result is compared against the declared deviation, and no document still promises parity that the accepted browser policy cannot provide.
