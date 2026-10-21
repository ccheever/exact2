# Code review round 2: per-column shadow values (04669175b0), Astra

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — Missed protocol-version assertion.** [scripts/tamber-remote.test.mjs:774](/tmp/rv-shcols-astra/scripts/tamber-remote.test.mjs:774) still expects `tamber/1.15`. This test starts the real `contract tamber serve` server; its remote `hello` preserves the server’s protocol, now `tamber/1.16`. The assertion therefore fails and prevents the remaining path-redaction assertions from running. Update the expected version and remove the stale “this change opened 1.15” comment.

   Concrete repro, in a writable environment:
   ```sh
   CARGO_TARGET_DIR=/tmp/rv-target-astra-shcols CARGO_BUILD_JOBS=4 \
     bun test scripts/tamber-remote.test.mjs \
     --test-name-pattern "no answer to a remote client"
   ```
   Expected failure: received `tamber/1.16`, expected `tamber/1.15`.

No other findings. The `1.15` Versions entry is unchanged from `HEAD~2`; the header and new `1.16` entry accurately describe the addition. The design tool displays the returned version, its `1.13` fixture matches its stub, and the Figma client accepts `1.16`. LLP 1122 references introduce no version conflict.

The shadow implementation follows the kernel’s text folding and residual-number evaluation. First-selected-tuple behavior, absent keys, zero normalization, terminal exclusion, inset/text shadows, app-defined bases, and mixed lists are consistent with the rulings.

Read-only review; no builds or tests run. `git diff --check HEAD~2 HEAD` passed.
