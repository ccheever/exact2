# Caltrain data accepts malformed arguments

**Status:** Closed
**Resolution:** Caltrain data queries now enforce exact arity, finite geographic domains, integer counts, and finite times.
**Systems:** Caltrain data
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005 §4

The Caltrain `DataSource` boundary in `apps/caltrain/data/src/lib.rs:224-283`
does not fail closed:

- sources ignore trailing arguments and `defaultLocation` ignores all args;
- `nearest` defaults a missing/wrong count to 3 and casts fractional,
  negative, nonfinite, or huge numbers to `usize`;
- location decoding accepts nonfinite and out-of-range coordinates;
- the module documentation advertises `trip(id)`, but no source implements it.

Validate exact arity and value domains for every source and return
`DataError::BadArguments` on mismatch. Add a table test for missing, extra,
wrong-kind, fractional, nonfinite, out-of-range, and oversized inputs; either
implement `trip` or remove the advertised API.
