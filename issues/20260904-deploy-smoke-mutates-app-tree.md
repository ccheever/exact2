# The deploy smoke writes a throwaway key and an asset edit into the live app

**Status:** Open
**Systems:** exact deploy, smokes
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 §4, `scripts/smoke.mjs`

`node scripts/smoke.mjs deploy` `keygen`s into a temp dir, then writes the public half into `apps/caltrain/app.json` and appends a byte to a live asset, restored in `finally`. SIGKILL leaves a throwaway public key in the manifest. Keys are an input to the compatibility id (`contract/cli/src/compat.rs`), so a leftover swap moves every native cohort until someone notices.

The drive also does not assert the https `--yes` refusal that 1030.000 lists as verified by this smoke.

Copy the manifest and asset into the temp dir (or a worktree) instead of mutating `apps/caltrain`. Add one `--origin https://… --yes` assertion.
