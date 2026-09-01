# A hash-valid URL plan that then fails boot does not fall back to baked

**Status:** Closed
**Resolution:** Linux now falls back to the baked plan when a fetched plan passes transport checks but fails boot gates.
**Systems:** Linux host
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001 §2 steps 4–5, `host/linux/src/app.rs`, `host/linux/src/fetch.rs`

`fetch_app` does what Stage 1 asked for on the wire: envelope major, plan `url`/`sha256`/`bytes`, same host:port, byte-count then SHA-256. Transport, length, and hash refusals print `exact url: …; booting the baked plan`.

If the bytes hash and then `Presenter::boot` / `Runner::boot` refuse (wrong `kernelSchema`, `AppMismatch`), the process exits 1. Baked fallback is only around `fetch_app` / `fs::read`, not around boot. A LAN envelope that is the wrong app or the next format version bricks a one-shot Linux run instead of keeping the binary's plan.

QUEUE already records that a native pre-download `kernelSchema` compare is owed. Until then, boot failure after a clean hash should fall back the same way a hash mismatch does.

Fix: if `Presenter::boot` of the fetched bytes fails, log the refusal (both app names, or the schema digest) and boot the baked plan. Same shape as the fetch-failure path. Add a test that a foreign `app_id` over `EXACT_PLAN=http://…` does not exit 1.
