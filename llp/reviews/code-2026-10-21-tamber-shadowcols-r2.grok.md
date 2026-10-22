# Code review round 2: per-column shadow values (04669175b0), Grok

**RIGHT WITH CHANGES**

`/Users/admin/x2bench/x2p4-s12-shadowcols-plan.md` is not on disk. This review uses LLP 1121 §10’s record of the 20:50 ruling and the later ruling in `04669175` (first `tamber/1.16` change). The shadow path matches that record. The `tamber/1.15` versions bullet is unchanged; `1.16` is a new bullet plus the `*1.16*` paragraph. `PROTOCOL` is `tamber/1.16`. Figma’s `newEnough` is `>= 1.14`, so `1.16` passes. The design tool stores `hello.protocol` and does not reject a minor. LLP 1122 does not pin `1.15`.

`shadow_text` is `scale::fold_lengths` with `scale::of` for that column, the same substitution the host uses once a residual only names unset bases (the checker inlines an app-set base first). Per-shadow numbers are that same `calc(substitute(…))` fold, then `+0` for zero. Explain uses the first tuple `branch_of` selects for the line, then folds that value per column. An empty map is omitted. The terminal is dropped only for shadows. Inset, text shadows, platform colours, and mixed lists stay on that path: a px-only shadow in a list has no key; the list’s text still folds.

1. **SHOULD-FIX** — `scripts/tamber-remote.test.mjs:774`. The remote-client test speaks to the real server (`contract tamber serve --stdio`) and still requires `tamber/1.15`. `PROTOCOL` in `contract/cli/src/tamber_serve.rs:77` is `tamber/1.16`. The bump updated `tamber_serve.rs`, `scripts/tamber-server.test.mjs:761`, and `scripts/design-tool.test.mjs:47`, and left this one. Repro: `bun test scripts/tamber-remote.test.mjs -t "no answer to a remote client"` fails on `expect(body(0).ok.protocol).toBe('tamber/1.15')`. Expect `tamber/1.16`.
