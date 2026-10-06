**LLP 1031 owed after the 2026-09-03 landing** (the handle, `ExactKit`, the macOS
sample host, the manifest stage; the iOS sample host landed 2026-09-04 — each pane its
own navigation controller, `smoke.mjs host-ios`): **content-height containment** (D3's
guard is written; bounded only today); **the request and store crossings** (D4's
contract; triggered by an adopter whose client cannot be wrapped); a request in flight under a destroy (needs a fetching app — Weird Castle — as the
fixture); and the host smoke's remount step, which took ~3 s in some runs: measured 2026-09-04 in
isolation at 42 ms (`clock settle` after the pop) and 7 ms (a's layout) — the seconds
appeared only while two agent lanes were compiling on the same Mac, so it reads as a
settle bound under load, not a remount cost; watch for it on a quiet machine.
Weird Castle is on today's exact2 (2026-09-04: the registry shape, `host!` with
`COMPAT`, `app.json`), but its `tests/app.rs` does not compile since LLP 1027 stage 3 —
six assertions want `Answer::Now(...)` around a `Value` — and the file carries another
session's uncommitted edits; whoever owns them wraps them.

*Filed under “Next, in order (2026-08-29)”.*
