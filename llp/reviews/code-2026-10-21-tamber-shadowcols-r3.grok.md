# Code review round 3: per-column shadow values (c82ec4ceeb), Grok

**RIGHT**

No findings.

Round 2 is fixed. `scripts/tamber-remote.test.mjs:774` expects `tamber/1.16` from the real server. The other live hello checks match `PROTOCOL` in `contract/cli/src/tamber_serve.rs:77`: both asserts in `contract/cli/tests/it/tamber_serve.rs` (146, 438), `scripts/tamber-server.test.mjs:761`, and `scripts/design-tool.test.mjs:47`. The design tool stores `hello.protocol` and accepts a later minor. Figma’s `newEnough` (`tools/figma-tamber/src/mapping.ts:370`) accepts 1.x from 1.14 up, so 1.16 passes. LLP 1122 leaves the protocol version unpinned.

The `tamber/1.15` versions bullet (`docs/tamber-protocol.md:118`) is the same text as on `origin/main`. The header and the hello line say `tamber/1.16`. The new 1.16 bullet (139) and the feature paragraph (615) match the ruling: token-level `columns` on explain’s value, each when, and each resolve tuple, `{value}` only, with the terminal left out; per-shadow `columns` with `value` and numeric `lengths` where `length_texts` is present; the key absent when there is nothing; a zero written as 0.

`shadow_text` (`tamber_base.rs:73`) is `scale::fold_lengths` with `scale::of` for that column, the same substitution the host uses once a residual only names unset bases. Per-shadow numbers are that same `calc(substitute(…))` fold, then `+0` for zero. Explain takes the first tuple `branch_of` selects (`tamber_base.rs:560`). An empty map is omitted. `shadow_columns` (`tamber_base.rs:63`) leaves the terminal out; `columns_of` still writes it for a length. Inset, text shadows, platform colours, and mixed lists stay on that path: a px-only shadow in a list has no key, and the list’s text still folds.

`cargo test -p contract --test it tamber_serve_shadow_columns`: 6 passed.
