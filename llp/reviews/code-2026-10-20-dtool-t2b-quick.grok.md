# Code review: design-tool.test T2b's canvas frame target asked again (main red on a loaded machine), quick review, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ff59dc3c45`.
- **Method:** one brief (sha256 `084262f96f24c17c1769597173a99d2744e1185b2b8444e2ce8e6fa4c18ee6ff`), shared by both reviewers, each blind to the other's review; commit ff59dc3c45 (the main-red standing permission's quick review). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (no findings).
- **Disposition:** none to take.

Nothing blocks. The poll does not turn an in-process canvas frame into a pass, and the loop is sound.

1. **No findings.**

The predicate is unchanged: an `iframe` target whose `url` starts with `http://localhost:<port>`. Chromium only creates a DevTools agent host for a local root (`is_local_root()`: no parent, or the parent is in another process). A frame that stays in the tool's process is not one, so `Target.getTargets` never lists it. Ten seconds of polling leaves `isolated` false and `expect(isolated).toBe(true)` fails. `http://localhost` and `http://127.0.0.1` are different schemeful sites, and Playwright 1.64 does not pass `--disable-features=…site-per-process` or `--disable-site-isolation-trials`. With site isolation off, or with a same-site frame, there is still no `iframe` target, so the test stays red; the poll only delays that failure. A loaded machine does not drop desktop site isolation or co-locate a cross-site iframe into the parent's site-locked process. A crashed renderer whose target is gone or whose url is `about:blank` / `chrome-error://` also keeps failing. A crash that leaves the committed url in place would have passed the old one-shot check too. This was not that failure: the old expect was false.

The lag account fits the red run. The iframe starts as same-process `about:blank`, then commits in a new process. Chromium creates the child agent host mid-navigation, while it is still provisional, so `GetURL()` can still be empty or `about:blank` after the parent's `Page.frameNavigated` has already given Playwright `http://localhost:…`. `locator('#exact-inspect').waitFor()` only proves that document is queryable; it does not prove the agent host is bound to the committed frame yet. Under load that browser-side update can sit behind the renderer answering the DOM query. `getTargets` reads the host live, so the next polls see the url once the host catches up. A target that never updates still fails at 10 seconds. The other permanent causes above do not start matching within that window on one congested run and then pass on an idle one.

The loop is the usual poll shape. The sleep is the `for` update, so the first `Target.getTargets` runs immediately, later tries are 200 ms apart, and time is checked before each try. A hit still pays one extra 200 ms because the update runs before the `!isolated` check; that does not skip a sample or extend a miss into a pass. The session is opened once and reused for sequential `send`s. Re-attaching every 200 ms would only add load. The page is not navigating during the poll (the iframe is), so that session stays the right one; a dead session throws and the test fails rather than passing.
