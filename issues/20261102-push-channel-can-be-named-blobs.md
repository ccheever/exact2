# `exact push` accepts a channel named `blobs`

**Status:** Closed
**Systems:** Push and deploy (`scripts/push.mjs`, `scripts/deploy.mjs`, `scripts/origin.mjs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A push with `--channel blobs`, or `deploy.channel` set to `blobs`, writes the stream under `.exact/blobs/<compatibilityId>/`, inside the content-addressed blob tree. Listing `.exact/blobs` then mixes blob files with stream directories. A publish plants `exact.json` and `releases/` among immutable blobs. Deploy already refuses this name.

**Cause.** `streamPath` interpolates the channel. `deploy.mjs` refuses a channel of `blobs` and any channel outside `^[A-Za-z0-9._-]+$`. `push.mjs` takes `opts.channel`, else the manifest, else `prod`, and does not run that check. `--channel root` is also accepted. The web pointer lives at `.exact/root/web/exact.json`.

**Reproduction.** `bun scripts/push.mjs <app> --channel blobs --origin <dir>` against an origin that already has a head for that cohort. The stream directory is `.exact/blobs/<32 hex>/`. Deploy of the same channel name stops with "cannot name a directory under .exact/".

**Fix.** Call deploy's channel check from push before opening the origin. Refuse `blobs` and `root`.

**Acceptance.** `--channel blobs` and a manifest channel of `blobs` fail before any write. `--channel prod` still publishes.

**Resolution.** Push and deploy share channel validation, rejecting reserved names case-insensitively and refusing dot path components.

**Verification after the fix.** Push argument regressions passed for reserved channel variants and valid channels in the 21-test publisher batch.
