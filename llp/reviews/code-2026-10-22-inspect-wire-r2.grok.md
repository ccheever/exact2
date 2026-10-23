# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 2 of 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 2 of 3; HEAD `a0fd4a2ae4`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; Astra four SHOULD-FIXes, Grok one, all taken in c700c1d7a2.

I'll review the round-1 fixes and the later tool note as a read-only pass: the three commits, then the selection, inspect, and fromSite behavior they claim to fix.The brief is a read-only check of the round-1 fixes and the later tool note, against the three commits. I'll pull those diffs and the surrounding selection and inspect code.The diffs show the eight claimed fixes plus the landing note. Next I’ll read the join, select, and host paths around those changes to see whether each one actually holds.Nothing blocks. The eight round-1 answers match the code.

1. **SHOULD-FIX** `apps/design-tool/app.contract:942` — `upExpect` is set in `selectUp` (`app.contract:1210`) and never cleared, and `upNote` is any later selection whose chain is shorter than that leftover depth. After one move up, a normal click on a shallower view is reported as "The page's tree had fewer above it," including when the move itself failed or the page refused it and the selection never changed. The stub test only checks the immediate short landing.

The other answers hold. Apple returns `unsupported` / `inspect` for any `select` frame and does not change inspect mode (`Inspect.swift:126`), and trim keeps `fromSite` (`Inspect.swift:64`). `scrubbed` drops `fromSite` on the republished `selected` event; `join` already did. No selection is a 409 via `s?.site` (`tamber-server.mjs:796`). Select requires write even with `refresh: true` (403). Wasm refuses a selection whose generation is not current; the JS page refuses when its plan is not. `sitesUp` walks outward to the first level that owns a node (`tamber-inspect.mjs:651`). `ancestors` is one parent walk (`lineage.rs:9`); inherited rows still call `site_of` in `source.rs`.
