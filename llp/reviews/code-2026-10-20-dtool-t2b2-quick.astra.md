# Code review: design-tool.test T2b's browser with site isolation on (main red under the pinned headless shell), quick review, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `b38a396a9d`.
- **Method:** one brief (sha256 `adb5193c35efb480027f95c1104abfd2c705182e4d87263043a005c2d28ea507`), shared by both reviewers, each blind to the other's review; commit b38a396a9d (the main-red standing permission's quick review). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 NIT, pre-existing).
- **Disposition:** (1) noted, no change: the light colour scheme is Playwright's context default on either browser; this commit doesn't touch it.

Nothing blocks HEAD. No BLOCKER or SHOULD-FIX findings.

1. **NIT, pre-existing:** [The light-theme comment](/tmp/rv-dt-t2b2/scripts/design-tool.test.mjs:1154) should attribute light mode to Playwright’s context default. It is not dependent on the browser build; explicitly setting `colorScheme: 'light'` would only clarify intent. [Playwright documentation](https://playwright.dev/docs/api/class-browser#browser-new-context-option-color-scheme).

The fix is correct. Chromium’s [headless implementation](https://github.com/chromium/chromium/blob/main/headless/lib/browser/headless_content_browser_client.cc) explicitly uses `--site-per-process` to enable site isolation. This establishes T2b’s required environment while preserving its assertion: without the matching `iframe` target, `isolated` remains false and the test fails.

[Browser sharing](/tmp/rv-dt-t2b2/scripts/agent-launch.mjs:374) includes launch options in the cache key. The flagged browser cannot change the default browser’s configuration, and the wrapper’s `close()` closes only its owned contexts. The additional cost is another browser retained until process exit. `CHROME` still selects the same executable; the flag reinforces [desktop Chrome’s existing isolation policy](https://www.chromium.org/Home/chromium-security/site-isolation/).

T2b remains Chromium-specific through CDP. I found no further shell-versus-full-browser dependency in its assertions; target-URL timing already has a bounded retry.

Read-only review; no builds or tests run.
