# Taffy patch provenance is stale

**Status:** Closed
**Resolution:** Taffy patch provenance now lists the five actual patches, honest coverage gaps, and the update checklist.
**Systems:** Kernel, Vendoring
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** rules/RULES.md documentation authority

Root `Cargo.toml:10-11` and the introduction of
`vendor/taffy/EXACT-PATCHES.md` say the vendored fork contains two Exact
patches. The same file now enumerates five, and several named regression-test
locations no longer exist under the documented names.

The patch markers are present and core tests pass, so this is provenance drift,
not a demonstrated behavior failure. Update the count and test pointers from
the actual vendored diff, and add a review step to the existing vendor-update
procedure so the patch ledger changes with the fork.
