# Review round 3, LLP 1121 stage 10 A1, Grok (xhigh), security pass

Reviewed: e6f255b0a6 (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

No BLOCKING or SHOULD-FIX. Round 2’s four items hold, and no read or propose method still writes files, git, the DTCG baseline, or another client’s held proposal.

**NIT** — `docs/tamber-protocol.md:635` still says a held propose keeps 64 and “the oldest goes.” That is only the write-scope rule. Below write, someone else’s proposal is not dropped (409 if the caller has none). The Scopes section at `docs/tamber-protocol.md:184` is the one that matches the code.

What checks out:

- **Cap race.** `room()` is gone. `hold` decides in the same turn as `proposals.set` (`scripts/tamber-server.mjs:853`). Below write, only that client’s oldest is removed; with none, it returns 409 and drops nothing. The check sits after the awaits, with no await between it and the insert. `revert` and `import` pass `scope` through (`:969`, `:1014`). A write hold still evicts the global oldest. The trailing `while` at `:859` cannot run for a below-write hold: the map never enters that insert above 64. `accept`’s `restore` still evicts globally, and `accept` is write.
- **Export.** Below write the wire forces `keep: false` (`scripts/tamber-server.mjs:630`). Rust returns the text with `baseline: null` before `keep()` (`contract/cli/src/tamber_serve.rs:777`). The Rust test removes `.exact/tamber-dtcg` and asserts it stays absent. A write export still keeps the baseline.
- **Selection cache.** `remember` keeps registered builds only and caps at 256 (`scripts/tamber-server.mjs:197`). A below-write `inspect` only prunes and reads that map (`:658`); it does not call `channel.inspect`. `refresh !== true` (including `on`) is write via `needs`.
- **Dispatcher.** Unknown methods are 400. `needs` uses `Object.hasOwn`. `dispatch`’s `local` defaults to false (`:575`); the local token path passes true (`:561`). `subscribe` and `events` use the same table and the `{error}` envelope. `hello`, `describe`, `resolve`, `explain`, `uses`, and `history` only read. `propose` without `hold` is `transaction(..., false)`, which returns before any commit. `import` and `revert` only plan, then hold. Git used from `history` and `revert` is log, show, diff, and cat-file.

Tests cover the table, per-method 403s, own-versus-other drop, the 64-cap and the overlapping 63-cap race, a failed hold dropping nothing, inspect refresh sending no build frames, and export `baseline: null`. The 256-entry selection cap is not directly tested; the implementation is the four lines above. Client identity is still the body’s `client` string; server-set `client` is A2, and these checks are sound once that lands.
