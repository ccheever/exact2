# Code review: design-tool.test beside another run, narrow verification of 3c4a6c99f0 (round 2 answered) (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** narrow verification of 3c4a6c99f0 (round 2 answered); HEAD `3c4a6c99f0`. The author (Claude) is not a reviewer.
- **Verdict:** one BLOCKER (the resident dist's parent is never made), answered in 5b2eece8d9.

1. **BLOCKER — [host/web-js/dev.mjs:1016](/tmp/rv-dt-conc2/host/web-js/dev.mjs:1016):** The new nested resident dist requires `<target>/web-dist-resident` to exist, but nothing creates it. On a fresh target, the `--bake` publication reaches [serve.mjs:610](/tmp/rv-dt-conc2/host/web/serve.mjs:610) and fails with `ENOENT` creating `<digest>.lock`; the resident exits and native forwarding returns 503. Create the parent before publication. An existing resident directory masks this regression.

The other three changes close their findings on static inspection, with no additional issues found. No files changed; no builds or tests run.
