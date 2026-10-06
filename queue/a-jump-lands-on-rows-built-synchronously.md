**A jump lands on rows built synchronously** (2026-09-19): a scroller drag or a
250,000-point jump takes one main-thread period of 31–44 ms. The rows are never
blank; the frame is late. The first report creates 68 views for six to eight
visible rows (13 ms in the runner and kernel, 8.6 ms in the presenter); a second
follows when measured heights move the landing (5 ms), then visible paragraphs
paint. Shorten the per-row work before spreading the fill across frames, which
would expose blank rows.

*Filed under “Next, in order (2026-08-29)”.*
