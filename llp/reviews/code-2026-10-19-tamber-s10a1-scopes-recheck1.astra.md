# Quick re-check 1 after round 3, LLP 1121 stage 10 A1, Astra

Reviewed: 29109a490c (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

- **BLOCKING** — [scripts/tamber-history.mjs:70](/tmp/rv-rl-a1d/scripts/tamber-history.mjs:70), also line 102: `git show`/`log` honor `log.showSignature=true`. Signed commits trigger temporary-file writes and the configured signature-verification helper through history/revert. Add `--no-show-signature` to both.

- **SHOULD-FIX** — [host/web-js/dev.mjs:803](/tmp/rv-rl-a1d/host/web-js/dev.mjs:803): `toolReady()` still leads through `scrubDesign → designOf → toolFresh()`. If sources change between freshness checks, propose starts `buildTool()` and can fall back to `cargo run`. Reproduced with mocked freshness changing true→false. Use a reader path that cannot build or fall back.
