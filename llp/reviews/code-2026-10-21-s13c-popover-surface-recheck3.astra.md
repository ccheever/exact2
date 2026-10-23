# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), third narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6e615a7891`.
- **Method:** one brief (sha256 `b9443c091c08c7f054e9bbe967ee91af3ea7cf59595b2718649d4b81c1fe96b8`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** the nit taken (the scroll view unwrapped before its radius is asserted); the calc(0% + 12px) case noted as within the accepted safe direction (no default surface)

VERDICT: RIGHT WITH NITS

1. **nit — [PopoverSurfaceIOSTests.swift:205](/tmp/iosfix-rv-ps6/host/apple/tests/ExactKitTests/PopoverSurfaceIOSTests.swift:205):** `percent.scroll?.layer.cornerRadius ?? 0` passes if the scroll view is absent. Unwrap `percent.scroll` before asserting its radius, so the assertion proves the scrolling case exists.

Both previous P2 findings are closed. The four production `{"pct":10}` objects now make `unevenCorners` true, suppressing default glass and its circular scroll clip. The revised fixture supplies those objects through the real Swift batch decoder.

No regression found for the requested cases: unwritten corners remain `.number(0)`; uniform authored `12` and `0` remain eligible for glass with their authored radius; an unwritten scrolling popover retains its default radius.

`Dimension::Points` emits only a JSON number at [style.rs:502](/tmp/iosfix-rv-ps6/host/apple/src/style.rs:502), decoded as `.number` at [BatchReader.swift:261](/tmp/iosfix-rv-ps6/host/apple/Sources/ExactCore/BatchReader.swift:261). Pure point calculations also simplify or resolve to points before serialization.

The accepted conservative exception exists at [Backdrop.swift:195](/tmp/iosfix-rv-ps6/host/apple/Sources/ExactKit/Backdrop.swift:195): `calc(0% + 12px)` remains `Dimension::Calc(0,12)` and crosses as `{"pct":0,"px":12}`. It therefore loses default glass despite describing circular corners, including when supplied through a token. This follows theme2’s accepted safe direction; it is not a should-fix finding.

Static review only; no files changed or tests run.
