# Code review: tamber tool --via's canvas check (a page that won't let the tool frame it), a narrow check of the post-round fixes (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a narrow check of the post-round fixes; HEAD `12d92fdfa7`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; no findings.

I'll verify those three round-2 fixes against HEAD only—read-only, no tree changes, builds, or tests.The commit touches the frame checker, the canvas line, and the contract. I'll read those against the three findings.Nothing blocks. All three round-2 findings are closed in `12d92fdfa7`, and the new code does not add a problem.

1. **Port advice** is decided from the source, not the stripped name. `portFixes` in `scripts/tamber-frame.mjs:66-69` is true only for an `http` `127.0.0.1` source (scheme written, or the page’s) at a numeric port, with no path past `/`. `http://127.0.0.1:7740/private` is false; schemeless `127.0.0.1:7740` on an `http` page is true. `refusalWords` (`tamber-frame.mjs:88`), the canvas-check JSON (`tamber-frame.mjs:105`), and the tool (`apps/design-tool/canvas-check.ts:15`) all use that flag.

2. **Ports compare as numbers.** `Number(...)` at `tamber-frame.mjs:48` makes `017977` match a tool on `17977`. Valid ports are exact integers, so this does not collapse two different real ports together. The name uses the same number (`tamber-frame.mjs:61`).

3. **A schemeless source is named with the page’s scheme** (`tamber-frame.mjs:61`), so `127.0.0.1:9999` on an `http` page is `http://127.0.0.1:9999`. That scheme is the same one `portFixes` uses.

No new findings. Read only; the test suite was not run.
