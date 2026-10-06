**The Mac's page viewport takes legacy scrollers' gutters** (2026-09-27, found fixing the
smoke on a Mac with a mouse): a `scroll` node now keeps overlay scrollers (LLP 1010's
zero `scrollbar_width`), but `PageScrollView` follows the system style, so the scroll
fixture reads `viewport 405×845` over a root laid out 420 wide — a sideways page scroll
in every app on such a Mac. `PageScrollMacTests` hold the legacy gutters on purpose;
decide whether the page follows LLP 1010 too. (The macOS canvas reference at 2x was
re-recorded on 2026-10-03, with the Label placeholder and color-matched sky.) The Markdown motion
fixture's 8-in-24 failure (2026-09-19) did not recur in 5 runs.

*Filed under “Next, in order (2026-08-29)”.*
