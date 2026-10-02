# The web and the runner still read grants a native host refuses

**Status:** Closed
**Resolution:** Fixed whole-set grant admission across native, runner, bake, wasm and JS; shared URL corpus, real browser denial tests and importless logic-module proof pass.
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

## Verification (2026-10-02)

Native bindings, the runner store and both Rust/TypeScript bake paths now use
`exact-grants`; malformed I/O declarations discard the whole set and name the
line. The JS target enforces the same corpus with browser URL normalization,
source-scoped secrets and fetch, including global aliases and computed access.
Wasm normalizes URLs through its browser executor instead of carrying ICU.
Importless Rust modules use a local store mirror; the host filters snapshots
and validates the complete returned write set before applying any write.

- Shared native corpus: 18 unit tests plus the cross-runtime corpus; runner
  179 and logic ABI 11 unit tests; native data host 13 tests. Bake and targeted
  wasm-host grant regressions pass. Browser request suite: 8 tests, 219 assertions.
- Real Chrome fixture: six allowed fetch forms reach the server; a malformed
  grant set refuses secrets and all six forms before I/O (zero server hits).
  Building the malformed source fails with its line number.
- RealWorld rebuilt and driven on JS and wasm. A separately built Rust ABI
  fixture with dynamic grants, scoped store writes and reads has no imports
  and successfully returns its granted value; Caltrain's logic module also
  has no imports.
- Targeted clippy, formatting, staged source caps and boot graph checks pass.
  Whole-suite legacy projection/transform test failures and the existing
  10k-row theme performance issue are separate from this fix.
- Measured RealWorld JS entry: 24,943 to 27,845 bytes brotli-11; wasm core:
  306,204 to 309,825 bytes, below its 304 KiB ceiling.
