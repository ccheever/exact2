# Web host never sets HTML `type` or `inputmode`

**Status:** Closed
**Resolution:** Web inputs now emit native type and inputmode attributes rather than CSS properties.
**Systems:** Web host
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017 P9 (the words are the web's), QUEUE.md (password masking claimed on web)

`host/web/src/host.rs` remaps kernel props to DOM names, but `PropId::Type` and `PropId::InputMode` (schema ids 15 and 39) are not in the match. They fall through to `data-type` and `data-inputmode`. The only `type` the web host writes is Toggle's hardcoded `checkbox`.

Apple sends the schema names (`host/apple/src/host.rs` `props_for`) and the presenters read `props["type"]` / `props["inputMode"]` — `NSSecureTextField` for `type="password"`, UIKit keyboard traits from `inputMode`. On the web a password field is a plain text input and `inputmode` never reaches the mobile keyboard.

QUEUE says password masking landed on web/macOS/iOS. The kernel row and the Apple arm did; the element on the page did not.

Fix: map `PropId::Type` → `type` and `PropId::InputMode` → `inputmode`. Do not let Toggle's checkbox overwrite an authored type. Fixture: an `input type="password"` in the corpus whose create op contains `"type":"password"` as an HTML attribute, not `data-type`.
