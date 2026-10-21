# Review round 2, LLP 1121 stage 12 B1 (package components' params), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

Round 1’s short-name fix is sound. Two gaps remain:

1. **SHOULD-FIX — [tamber_serve.rs:389](/tmp/rv-s12b1-r2-astra/contract/cli/src/tamber_serve.rs:389):** The retention cache now tracks package components, but excludes their token dependency definitions. Prime `Map.pin → reading.pin → reading.old`, then change `reading.pin` to reference `reading.new` without changing the component declaration. Cached retention still keeps `reading.old`; varying that now-unused token reports `rebuild`, while a fresh server reports `live`. Include package token dependencies in invalidation and test this sequence.

2. **SHOULD-FIX — [packages.rs:317](/tmp/rv-s12b1-r2-astra/contract/lower/src/theme/packages.rs:317):** Round 1’s default-reference restriction still has two bypasses. `glyph: icon = icon.app` skips validation because icons live in `folding.icons`. Likewise, `pin: color = ink` escapes when the app’s `ink` is a profile color: `profiles()` replaces the alias with `transparent`, erasing its folded names. Check original icon aliases explicitly and apply the existing whole-profile-alias safeguard to param defaults. Add refusal tests asserting the package declaration’s location.

Caps and whitespace checks passed; changed sources satisfy 1499 lines. Cargo tests were blocked before compilation by the sandbox denying `.cargo-build-lock`; release-byte parity remains unverified.
