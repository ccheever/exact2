# Navigated iframes receive authenticated identity

**Status:** Closed
**Resolution:** Web and Apple iframe identity is now origin-bound, revoked on navigation, and unavailable to opaque guests.
**Systems:** Web host, Apple host, WebView
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1020

The web and Apple iframe arms implement Castle `user.getCurrent` replies with
target origin `"*"` (`host/web/glue.js:91-140`,
`host/apple/webarm/WebArm.swift:175-229`). They associate a guest with its
iframe `WindowProxy`, which survives cross-origin navigation.

An iframe originally authorized for an authenticated Castle page can navigate
to an attacker document and request the same method. The host broadcasts the
reply containing `userId` and `username` to the now-untrusted document. This
violates LLP 1020's origin-bound guest authority.

Track the currently committed guest origin, validate every incoming message's
origin, and reply to that exact origin. Use `"*"` only for intentionally opaque
origins with an explicit capability rule. Add a navigation regression that
proves identity is not delivered after an origin change.
