# Honor caller redirect modes in the browser fetch facade

**Status:** Open
**Systems:** web JS data, fetch parity
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** host/web-js/admission.js:58; vendor/ibex2/src/stdlib/fetch.rs

`fetchWith` unconditionally writes `redirect: "follow"` after spreading the caller's init. This overrides `redirect: "error"` and `"manual"`, including a redirect mode supplied by an input Request. The native fetch implementation explicitly supports Follow, Manual and Error.

Reproduced in Chrome with a grant for one loopback origin and a same-origin 307 from /local-redirect to /finish. Both `fetchWith(set, url, {redirect:"error"})` and `{redirect:"manual"}` returned the final 200 response with body “finished.” The browser's ordinary fetch error mode rejects without following; its manual mode returns an opaque redirect. This discrepancy is independent of the declared default-follow grant compromise.

Preserve the browser's caller-selected redirect mode, including Request/init precedence. Validate unsupported values consistently. Keep final-URL admission where it is meaningful for a followed response and document the browser's opaque manual response.

Acceptance: error mode never contacts the destination, manual mode never follows and exposes the appropriate browser response, default/follow retain their decided policy, and native/browser facade tests cover both init and Request inputs.
