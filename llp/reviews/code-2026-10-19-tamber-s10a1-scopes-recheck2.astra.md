# Quick re-check 2, LLP 1121 stage 10 A1, Astra

Reviewed: 8fa99f9cca (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

BLOCKING — [scripts/tamber-history.mjs:20](/tmp/rv-rl-a1e/scripts/tamber-history.mjs:20): `history`/`revert` still permit [Git’s lazy fetching](https://git-scm.com/docs/partial-clone#_fetching_missing_objects). In a partial clone, reading an uncached historical blob can spawn `git fetch`, execute configured transport/credential helpers, and write object packs. Set `GIT_NO_LAZY_FETCH=1` for every history Git subprocess and fail when required objects are absent.
