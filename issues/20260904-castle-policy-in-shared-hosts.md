# Castle application policy lives in Exact's shared hosts

**Status:** Open
**Systems:** Web host, Apple webview, Application boundary, Weird Castle
**Severity:** P1
**Author:** Codex, at Charlie Cheever's request
**Date:** 2026-09-04
**Related:** LLP 1020 §9; LLP 1020 D2/D2r; rules/RULES.md §Scope

Charlie confirmed on 2026-09-04 that Weird Castle is an example application
and its logic must not bleed into Exact's shared implementation.

Production code currently does:

- `host/web/glue.js`: `castleUserFromSrc`, `replyCastleSdk`, and message
  listeners recognize `castleSdk`, answer `user.getCurrent` from query
  parameters, and intercept Castle messages before generic app delivery.
- `host/apple/webarm/WebArm.swift`: the injected wrapper implements that
  same protocol and `castleUserLiteral` constructs app identity.
  `localDocument` recognizes `CastleEmbed` / `castle-card` and injects
  `exact-fullbleed` CSS plus JS that changes the guest's sizing and shape.

The review executed the current `localDocument` method against a local HTML
fixture containing a 100×140 `castle-card`; it appended forced viewport
sizing. This is application-specific production behavior, not merely a
test fixture or a historical reference in an LLP.

Move the protocol, identity policy, and content adaptation into
`~/projects/weird-castle` or its app-owned integration. Keep generic frame
containment, sandbox enforcement, origin/navigation checks, message
transport, and agent inspection in Exact. Use LLP 1020 D2r's narrow reply
seam if a real generic capability is missing; do not create a Castle-named
hook, registry, or arbitrary script injection API to preserve the leak.
The application's wrapper must preserve source/origin checks and must not
send its auth token into a guest. An opaque guest remains unprivileged.

Done when ordinary Exact apps have no Castle protocol or HTML rewriting in
their shared runtime, and Weird Castle's identity, layout, messaging, and
navigation/reload behavior still work on web/macOS/iOS. Drive the external
app through the existing scripts; an app adapter must not require shared
host changes. Replace any protocol-specific core smoke assertion with a
generic request/reply fixture and move the Castle assertion to its app.
References in research and deliberately isolated test data are not production
logic and should not be removed merely to make a text search empty.
