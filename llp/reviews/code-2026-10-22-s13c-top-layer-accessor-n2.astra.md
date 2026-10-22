# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 2, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a8a6e5ae12`.
- **Target:** a8a6e5ae12.
- **Method:** one brief (sha256 `e8c38872eb342ec4bb24b9bdfbb1d7f87665a20ed5a7be829580610335bbc8bc`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in ab4c231328 (the sticky machinery reads the accessor; a top-layer box's insets; Swift strings, row ids across lines) and 9540ee1a92 (the doc nit; sticky_nodes in the test)

VERDICT: WRONG

1. **[should-fix] Sticky exemptions still hide layout decisions.** [scripts/position-reads.test.mjs:39](/tmp/iosfix-rv-a2/scripts/position-reads.test.mjs:39) and its sticky-machinery entries remain unsound for the newly fixed case. A neutral-header popover now reports `Absolute`, but its effective row remains `Sticky`. Consequently, [style.rs:1321](/tmp/iosfix-rv-a2/kernel/src/style.rs:1321) still discards its insets, and [sticky.rs:91](/tmp/iosfix-rv-a2/kernel/src/kernel/sticky.rs:91) still permits a sticky constraint. `top_layer::apply` only corrects the position. These exemptions can leave an absolute popover with sticky scrolling and ignored authored insets; their audit classifications need correction.

2. **[should-fix] Swift strings can conceal subsequent reads.** [scripts/position-reads.test.mjs:71](/tmp/iosfix-rv-a2/scripts/position-reads.test.mjs:71): the Swift path recognizes comments without recognizing string boundaries. A valid line `let marker = "/*"` makes the scanner blank subsequent `view.style["position_type"]` reads until `*/` or EOF. A URL string similarly hides a read later on the same line. Both returned zero reads in in-memory probes. Swift strings must be recognized before interpreting comment delimiters.

3. **[should-fix] Row-ID reads still escape through whitespace.** [scripts/position-reads.test.mjs:24](/tmp/iosfix-rv-a2/scripts/position-reads.test.mjs:24): `s.get(StyleId :: PositionType)` is missed. Splitting `StyleId::` and `PositionType` across lines also escapes because scanning happens per line at [line 133](/tmp/iosfix-rv-a2/scripts/position-reads.test.mjs:133). Both returned zero reads in probes. Match the qualified path across whitespace and line breaks.

The accessors themselves agree: `written` is the correct authorship signal, and both runner paths preserve it. Author-sticky popovers and non-popover boxes retain the expected classifications. The added audit rows, including Swift’s missing row, account for the additions; finding 1 prevents approving their justification.

Two repository scan tests passed normally. All three passed with scratch filesystem operations replaced in memory; additional probes reproduced findings 2–3 and confirmed the repaired destructuring and test-module cases. Cargo was blocked before execution by the read-only sandbox, so finding 1 is source-traced. No files changed.
