# Review round 1, LLP 1121 stage 12 B1 (package components' params), Astra (gpt-6-astra, max)

RIGHT WITH CHANGES

1. **SHOULD-FIX — [params.rs:66](/tmp/rv-s12b1-r1-astra/contract/lower/src/params.rs:66):** Package design params enter `tokens()`, but `identities()` still lists only Contract components. Consequently, `describe` advertises `short: "Map.pin"` for both packages in the new collision fixture, although resolution rejects it as ambiguous. Use the shared resolver when emitting short names, or include design components in `identities()`. Test package/package and package/app collisions.

2. **SHOULD-FIX — [theme.rs:318](/tmp/rv-s12b1-r1-astra/contract/lower/src/theme.rs:318):** Retention now depends on package component declarations, but [the server’s cache key](/tmp/rv-s12b1-r1-astra/contract/cli/src/tamber_serve.rs:384) excludes them. After priming the cache, adding a package param leaves its root absent from cached retention; a subsequent override varying over an existing axis can incorrectly report `live`. Include package declarations/defaults in cache invalidation and test that sequence.

Changed source files satisfy the cap; whitespace checks passed. Tests were blocked before compilation by the sandbox denying the Cargo lock. Release-byte parity remains unverified.
