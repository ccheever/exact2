# The web and the runner still read grants a native host refuses

**Status:** Open
**Systems:** Web host, runner store, bake, grants
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report F1 (2026-09-24); vendor/ibex2/src/grant.rs; js/src/lib.rs Module::inspect

Seth's Crew port (report of 2026-09-24, F1) declared `secret.keep crewHost`. Native hosts parse the whole grant set with `ibex2::grant::GrantSet::parse`, which refuses a camelCase secret name, and before this report they then held no grants at all, `net.fetch` included, with no message. The same report's fix makes the TypeScript bake refuse such grants (`Module::inspect`, `js/src/lib.rs`), makes native hosts journal the parse error and name it in every refusal, and renames `apps/realworld`'s `jwtToken` to `realworld.jwt`.

Two readers still disagree with the grammar, one line at a time and never refusing:

- `runner/src/store.rs` `Store::new` takes any `secret.keep <name>` as granted.
- `host/web/glue.js` `granted()` takes any `net.fetch <origin>` line; its comment promises "the same refusal everywhere".

With the bake refusing, an app built by the scripts can no longer reach them with a bad set. But hand-built grants still can, and a Rust-only app's grant constants are never validated at bake: `exact_bake::compatibility_id_sources` (`bake/src/compat.rs`) hashes them without parsing, because `exact-bake` does not depend on ibex2.

Done when a Rust-only app whose grants do not parse fails its bake naming the line, as a TypeScript app's now does, and the web and runner readers take their grants from one parse of the whole set (or refuse the set as native hosts do) rather than line by line.
