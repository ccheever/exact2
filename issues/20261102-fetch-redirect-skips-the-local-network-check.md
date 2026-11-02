# A followed redirect skips the local-network check

**Status:** Closed
**Systems:** Web fetch admission (`host/web-js/admission.js`, `host/web/address-space.js`, `host/web/navigation.js`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A tool, wake or device fetch whose first URL is public can follow a redirect to a loopback or private host and return the body. The app needs a scheme grant such as `net.fetch https:`. The local-network permission is never consulted for the landing URL while that permission is still `unknown`, which is its initial state.

**Cause.** `fetchWith` calls `reachRefusal` on the request URL and then lets the browser follow redirects. After the response, it checks `admitsNetwork` on `response.url` and does not call `reachRefusal` again. `admittingGrant` treats a `fetch-scheme` grant as matching every host on that scheme, including `127.0.0.1` and private addresses. `redirectFor` replaces `follow` with `error` only when a less-public permission is something other than `null`, `unknown` or `granted`. Both spaces start as `unknown`, so `follow` is left in place until a permission query stores another state.

**Reproduction.** Grant `net.fetch https:` and no local-network permission. From a tool origin, fetch a public `https` URL that responds `302` to `https://127.0.0.1/` with a short body. The body is returned. The same tool fetch of `https://127.0.0.1/` directly is refused.

**Fix.** Apply `reachRefusal` to the final response URL. While a less-public permission is `unknown`, do not follow a redirect that could land there; `redirect: "error"` is the gate `redirectFor` already uses once the state is known.

**Acceptance.** The redirect above is refused with the local-network error, and the body is not read. A redirect that stays on public hosts still completes. A landing the person has granted is admitted.

## Resolution

For non-app requests, refuse automatic redirects until every less-public address-space permission is granted. Recheck the final response URL before reading its body.

## Verification

`bun test host/web/address-space.test.mjs` and the async-tool regression in `host/web-js/upload.test.mjs` pass unknown/denied/granted redirect policy and final-URL refusal cases. Browser opaque redirects are conservatively refused while a needed permission is unknown.
