# Castle application policy lives in Exact's shared hosts

**Status:** Closed
**Resolution:** Implemented 2026-09-04: Castle protocol and deck presentation live in the external app; generic host and external app smoke checks pass.
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

## Implementation and verification — 2026-09-04

Shared runtime Castle protocol handlers and Apple HTML rewriting have been
removed. Caltrain's fixture now asserts generic JSON message delivery and
cross-origin navigation rejection. Weird Castle's own `data/src/deck.rs`
and `deck-host.html` wrap downloaded single-file bundles, apply full-viewport
card layout, and own Castle SDK responses. Old unwrapped cached documents
are treated as a cache miss. The app no longer sends a username in its URL.

The app already authors an opaque sandbox, so its identity request was
always refused before this change. That refusal is preserved; this fix does
not grant an authenticated SDK. The app-owned wrapper introduces a nested
opaque frame: ordinary browser input reaches the deck; the Exact agent's
DOM outline reaches the wrapper, not that nested opaque DOM.

Verified in the browser through the existing agent driver: a 100×140 card
becomes its 420×900 viewport, has zero radius, sees the required CastleEmbed
configuration before its scripts, and gets the expected UNAVAILABLE identity
reply. The edited WebArm compiles. A native materialization probe checks
that Castle-looking local HTML is now returned byte-for-byte and escaping
paths are refused. An AppKit/WKWebView probe also executes the app-owned
wrapper and confirms full-viewport layout and the same identity refusal.

Integration: the external app builds and passes the existing smoke on web,
macOS and the iOS simulator. Its nine Rust application tests pass, covering
login, kept sessions, account changes, bundle materialization and manifest
identity; the GPU crate's test also passes. No live-account login was used.
The shared Caltrain web, macOS, iOS and Linux smokes exercise generic frame
messages, guest input, navigation and reload. The external sweep also fixed
the app's missing data-source identity and two generic Apple initialization
gaps: applying an already-booted viewport mode to window chrome, and sending
existing view insets to a replacement runner. No Castle policy was added
to accomplish either.
