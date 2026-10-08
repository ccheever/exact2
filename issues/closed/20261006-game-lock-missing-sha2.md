# game/Cargo.lock is stale on contract-lower's sha2 dependency

**Status:** Closed
**Resolution:** Fixed by 8dde07780; locked cargo tree resolves contract-lower with sha2 without changing the lock.
**Systems:** game, contract-lower
**Severity:** P2
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** LLP 1096 D1; contract/lower/Cargo.toml

`contract/lower/Cargo.toml` depends on `sha2` (a declared sound's digest, LLP 1096 D1). The root `Cargo.lock` records it. `game/Cargo.lock` lists `sha2` as a package but not as a dependency of `contract-lower`.

`cargo tree --locked --manifest-path game/Cargo.toml -p contract-lower` fails: `cannot update the lock file ... because --locked was passed`. The same command without `--locked` (offline) rewrites exactly one line, adding `sha2` to `contract-lower`'s dependency list. That rewrite was produced while reviewing and has been reverted, so the committed lock is still the stale one.

A game build that does not pass `--locked` rewrites the lock as a side effect. One that does pass `--locked` does not build. Regenerate `game/Cargo.lock` and commit the `sha2` edge. The unused `cosmic-text` patch warning on that workspace is separate and not this failure.
