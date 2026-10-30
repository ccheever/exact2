# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `70e7123573`.
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

- **P1 — iOS test compilation fails** at [IframeCaptureTests.swift:304](/tmp/iosfix2-rv-pdf3-astra/host/apple/tests/ExactKitTests/IframeCaptureTests.swift:304). The shared test now references `ConfiguringFn`, `Self.configured`, and `Self.configuring`, but their declarations at lines 497–507 are inside `#if os(macOS)`. These names are unavailable on iOS, preventing `ExactKitTests` from compiling. Move the shared hook helpers outside that conditional.

No other actionable findings in either diff. The sandboxed frame’s immediate mount assertion matches the synchronous mount path. ScreenTime causality remains unproven. Tests were not rerun in this read-only workspace.

VERDICT: CHANGES
