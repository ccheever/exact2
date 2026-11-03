# Push accepts missing flag values and can select the default origin

**Status:** Closed
**Severity:** P2
**Systems:** Publisher CLI (`scripts/push.mjs`)
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`bun scripts/push.mjs <app> --origin` accepts the incomplete command. `parseArgs` assigns `undefined` instead of refusing it (`scripts/push.mjs:43-55`), and `pusher` then selects the manifest's configured origin (`scripts/push.mjs:179-187`). With a production origin and signing key configured, a command whose explicit destination is missing can proceed toward publishing to that default. Push publishes without a separate confirmation step.

The same parser accepts missing `--channel` and `--keys` values. It also consumes a following flag as the value: `app --channel --origin /fixture/staging-origin` becomes channel `--origin`, leaves `origin` unset, and treats the intended origin as an extra positional argument. Missing or nonnumeric `--seq-above` becomes `NaN`. Deploy's parser already refuses missing values and following `--` flags (`scripts/deploy.mjs:67-89`).

**Verification.** Executed the unchanged parser extracted from `scripts/push.mjs` in `/tmp/exact-review-push-arguments.mjs`, following the function-extraction approach used by `scripts/push.test.mjs`. Assertions confirmed the undefined origin, fallback selection, consumed flag, and `NaN` values. This invokes no publisher, signing operation, subprocess, or network request; the publication consequence follows from the subsequent option selection in `pusher`.

**Fix.** Validate every valued flag before opening an origin or loading a signer. Refuse absent values and another flag in the value position. Require `--seq-above` to be a nonnegative safe integer for which a higher safe sequence can be allocated. Refuse extra positional arguments.

**Acceptance.** Incomplete and malformed flag combinations fail before any build or origin operation; valid origin, channel, key directory, and sequence arguments retain their behavior.

The existing `20261102-push-channel-can-be-named-blobs.md` concerns accepted channel names, not missing arguments or unintended fallback selection. No matching ticket was found in this checkout or the older Exact issue directories.

**Resolution.** Every valued push option requires a nonempty value. Sequence floors must be safe nonnegative decimal integers; extra positional arguments are refused.

**Verification after the fix.** The push argument regressions passed missing values, flag-shaped values, invalid sequences and extra arguments in the 21-test publisher batch.
