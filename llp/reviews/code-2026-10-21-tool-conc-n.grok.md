# Code review: design-tool.test beside another run, narrow verification of 3c4a6c99f0 (round 2 answered) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** narrow verification of 3c4a6c99f0 (round 2 answered); HEAD `3c4a6c99f0`. The author (Claude) is not a reviewer.
- **Verdict:** one BLOCKER (the resident dist's parent is never made), answered in 5b2eece8d9.

I'll verify the four HEAD fixes against the previous round, read-only, and report only whether each one closes its finding without a new problem.The HEAD diff is small. I'll read each change in context and check whether it actually closes the finding.The resident-dist change does not hold. One blocker.

1. **BLOCKER** — `host/web-js/dev.mjs:1016`. `EXACT_WEB_DIST` is now `<target>/web-dist-resident/<16 hex>`, but nothing creates `web-dist-resident`. The bake publishes with `replaceBuild` (`host/web/serve.mjs:610` writes `<dist>.lock`, `:626` renames the stage onto `dist`). Both need that parent. The old path sat directly in `app.target`, which the JS build already creates. On a clean target the lock write throws `ENOENT`, the resident build exits, and every native-client start fails the same way. The hash does separate two loops once the directory exists.

The other three close their findings. Both loops in `dev-builds.test.mjs:217` pass the same `CARGO_TARGET_DIR`, and `resolveApp` uses it, so the outside apps share one `target/`. The inspect output is `inspect-<digest>-<pid>.json` in both `dev.mjs:378` and `dev-gallery.mjs:166`, and each loop still deletes only that path. `claimPort` skips `except` before taking a turn (`port-lock.mjs:81`); the harness passes the checkout port (`design-tool-harness.mjs:17`), including on the wrap from `checkoutPort + 1`.
