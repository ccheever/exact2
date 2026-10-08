# Independent review

Reviewer: `/root/review_clamp` (separate agent).

Reviewed source:

- `host/apple/Sources/ExactKit/TextRaster.swift`: SHA-256 `2b824d1461e03645071b3d545ffc0bbfd773d5263475a9a58fb0f2f6c6333870`
- `host/apple/tests/ExactKitTests/TextPaintTests.swift`: SHA-256 `71eb39ae4cd8923b80f1cba181ba09deced9c08ca1c394a2b34b3a423f80b14e`

Finding: no blocking correctness findings in the two-file diff. Replacing the last line before computing its alignment positions is correct. Swift array replacement does not mutate cached CTLine objects. Cache ownership and admission/scheduling remain unchanged. The last-line clamp adds its necessary shaping work to the first-pixels path; no claim of unchanged measured wall time is supported.

The reviewer requested an independent pixel oracle. Incorporated before the red test run: the layout reference paints `p.lines` through `job(nil)`, so the new clamp operation is not reapplied to both sides of the assertion.

Runtime review: inspected original before/after window pixels, default capture, and remount. After pixels meet #300, without restyle. `after-drive.log` retains 300×20 layout. Six before-regression failures are exactly the newly covered first-pixels equality assertions; the old worker/layout comparison passes.

At code/runtime review completion the final full gate runner was still running. Its final status and source identity are recorded separately in `verification-bun142/report.json`.

Focused test follow-up: `verification/007.stdout.log` passes all 39 TextPaint/TextMetrics tests, including the six clamp/alignment cases, urgent-versus-worker pixel parity, and cold-cache residency budget. The first runner's only failed gate was Rust test subprocesses using an outdated Bun; corrected-PATH results are in `verification-bun142/report.json`.

Final gates: all seven recipe commands passed; source_unchanged=true. Verified implementation commit: `d185e65fbb87abac56a0c557b80fd111c59801ad`. Committed source and recipe match the passing report.
