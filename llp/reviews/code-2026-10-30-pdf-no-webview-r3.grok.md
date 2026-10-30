# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `70e7123573`.
- **Method:** one brief (sha256 `b7e87f29f25a294e7723ca624698cb912782e07c8700cb5f511a918f09d5cef0`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 9618374f83 (the iOS build) and 9efb1ada9f (the partition clear), each tested

## Brief

Code review, round 3 of 3 (host change: a local-PDF iframe shows the PDF's view from its mount and makes no web view it then drops; the ScreenTime KVO red, NOT proven to be the cause). Tip 70e7123573 is HEAD of this worktree; base 125ee17e91. Review the whole change fresh with `git diff 125ee17e91..HEAD`, and round 2's fixes with `git diff 3e5b6f9eee..HEAD`.

Round 2 findings (Astra P2 and Grok P2, the same one; Astra P3), and the fixes:
- **A sandboxed local PDF mounted as a PDF view.** The sandbox reaches the arm only after the async read, and the view is taken before that. Now WebViews.configuration sends "pdf" only when the frame has no sandbox and no partition, so such a frame takes a WKWebView at its mount, as D10 has it. Test: a third frame in scripts/fixtures/iframe-pdf.contract (a sandboxed local PDF); IframeCaptureSessionTests asserts its node's view is a WKWebView right after the mount. That fails on 3e5b6f9eee (an ExactPDFView).
- **The no-web-view test only saw attached views (P3).** It now also installs the arm's exact_web_set_configuring hook (the host's moment before each web view is made) and asserts it was never called through the PDF's load.
Full suites on 3e5b6f9eee: Mac 306/0 and iOS 0 failed. On this tip they are running. Caps is green.
Condition (1) results, stated: the real arm looped 200 times beside a cargo build, twice on main and twice on the branch, gave 0 occurrences each.

Check hard, final round:
- anything left that can show a PDF view for a frame that ends up WebKit's, or build a web view for a local PDF;
- the hint against a src that changes from a local PDF to sandboxed, or back;
- the kind-7 guard exception;
- makeUI;
- the hatch ordering;
- the fixture and test changes (do they prove the claims; anything flaky, such as the sandboxed frame's check right after the mount);
- the LLP 1020.002 D10 row.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the whole branch against `125ee17e91` and the round-2 fixes against `3e5b6f9eee`, without editing anything. First I'm loading the review instructions and the two diffs.The tip is `70e7123573` on a clean tree. I'll read both diffs and the surrounding PDF-view, hatch, and test code before judging the round-2 fixes.The round-2 diff is small. I'll read the mount path, src updates, kind 7, hatch ordering, and the tests that claim to prove them.The hint is applied before the view is taken, and sandbox still arrives later. I'll trace view choice, `makeUI`, kind 7, and hatch replacement against a src or sandbox change.Auto-compacting conversation (81% full)...
Conversation compacted.
The round-3 review is still open. I'll re-read the mount, partition-change, and test paths against the tip before writing findings.## P2 — Clearing `partition` on an unsandboxed local PDF builds a web view and drops it

`WebArm.configure` treats every partition change as a web-view replacement. When the new partition is nil and the frame already holds a local PDF, that new web view is installed and then destroyed on the next turn.

1. A partitioned `app:/…pdf` mounts as WebKit (`WebModule.swift:310` omits `pdf`). The host still reads the file and leaves the bytes and `src` on the arm (`WebModule.swift:398–419`). While `partition != nil`, `serve` refuses it (`WebArm.swift:790`, `servePartitioned`).
2. Removing `partition` changes the configuration, so `update` calls `configure` and then `replaceView` (`WebModule.swift:360–363`) and returns. Only the partition changed, so there is no new read.
3. `configure` sees the name change, tears down the view that was actually shown, then always builds another (`WebArm.swift:582–589`):

```582:589:host/apple/webarm/WebArm.swift
        let replacing = built != nil || pdf != nil
        if replacing { teardown() }
        partition = name
        if replacing {
            _ = build()
            closePDF()
            markLoading()
            scheduleServe()
        }
```

`closePDF` returns immediately because `pdf` is nil (`FramePDF.swift:160`). `platformView` is the new `WKWebView`, and `replaceView` puts it on the node (`WebModule.swift:462–477`).

4. The queued `serve` (`WebArm.swift:788`) sees `partition == nil`, `sandbox == nil`, and `application/pdf`, so `showPDF` runs `teardown()` on that web view (`FramePDF.swift:144`) and kind 7 swaps in the PDF view.

The end state matches D10. The web view created at `WebArm.swift:586` is constructed, inserted, and dropped without ever loading the PDF — the same construct-and-drop this change closes at mount. Adding a partition keeps the new web view; sandbox add/remove goes through `setSandbox` → `serve` and does not take this branch.

## Checked, holding

**Mount hint.** `pdf` is sent only when both `sandbox` and `partition` are nil (`WebModule.swift:310`). `sandbox=""` is a real prop, so the fixture frame is not hinted. `create` configures before `platformView` (`WebModule.swift:276–277`), and `applyProps` runs before `embedFrame` (`PresenterMac.swift:1098–1099`).

**Src ↔ sandbox.** The hint is recomputed on the next `configure`; `options = values` runs even on the early return (`WebArm.swift:576–580`). Sandbox arrives through `setSandbox` (`WebArm.swift:648`). An in-flight read sends the latest `entry.sandbox` (`WebModule.swift:409`, `418`). `setSandbox` and `setSrc` share one `serve` (`WebArm.swift:766–768`). Adding a sandbox `closePDF`s into a web view that stays. Removing one `showPDF`s and tears down the view that was shown.

**Kind 7.** A failed `app:/` read sets `fileFailed`, sends a nil document and a nil `src`, and reports `error` itself (`WebModule.swift:404–413`). `closePDF` emits kind 7 `{}` (`FramePDF.swift:159–164`), which the guard lets through (`WebModule.swift:564`) so the fallback web view is shown. Kind 0 and kind 2 stay dropped.

**makeUI.** `waitingPDF` calls `makeUI` (`WebArm.swift:266–267`). `FrameUI.init` (`FrameWindows.swift:104`) does not touch `webView`. `configure` sets `agent` before the first view. `build` reuses that delegate (`WebArm.swift:384`). `confirmExternal` takes a `PlatformView` (`FrameWindows.swift:436`), and the PDF link passes the PDF view (`FramePDF.swift:114`).

**Hatch order.** Production create is `embedFrame` then `elements.created` (`PresenterMac.swift:1099–1101`). `replay` releases waiting frames before `built` (`ElementHatches.swift:178–180`). `showPDF` reuses the same `FramePDF` (`FramePDF.swift:145`), so `replaceView` sees the same object and does not emit `changed` (`WebModule.swift:465`). The session test attaches the hatch after mount; `changed == nil` there, and the HTML swap fires `changed` once (`IframeCaptureSessionTests.swift:65`, `94`).

**Tests.** The `boxed` assert (`IframeCaptureSessionTests.swift:51`) runs on the same main-thread turn as `press`. The read is `WebViews.reads.async` then `main.async`, so it cannot run before that assert. If it had, `serve` would still keep the `WKWebView` because `sandbox` is `""`. The hook is `exact_web_set_configuring`, invoked inside `buildFrame` / `buildPartitioned` before the `WKWebView` exists (`WebArm.swift:447`, `493`, `507`). `configured.count == 0` after load (`IframeCaptureTests.swift:316`) means `build()` did not run. The session test is what proves the host hint. `testASandboxedPDFStaysWebKits` (`IframeCaptureTests.swift:435`) passes `configuration: [:]` and sets the sandbox after the view, so it does not. The `gone` wait (`IframeCaptureSessionTests.swift:69`) accepts a web view that was there from the mount; the “starts on the PDF view” claim for a failed read is not asserted. Tip suites were still running and are not a result here.

**D10.** The first-view row (`llp/1020.002-a-browser-shell.rfc.md:674`) matches the mount rule. The section is already scoped to an unpartitioned, unsandboxed frame (`:662`); the sandbox row (`:678`) and the following bullet (`:687`) keep partitioned, sandboxed, and remote PDFs on WebKit. The `WebArm.swift:433` cite on that sandbox row was already the web-view constructor at `125ee17e91`. The direct-file sandbox header is `serveFile` (`WebArm.swift:864`). That cite predates this change.

VERDICT: CHANGES
