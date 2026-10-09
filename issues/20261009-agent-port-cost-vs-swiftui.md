# Porting an app costs agents ~2x the time and ~5-6x the tokens of SwiftUI: required reading is ~100K tokens and still lacks the native iOS patterns

**Status:** Open
**Systems:** docs, agent onboarding, exact new
**Severity:** P1
**Author:** Claude (Tuft), for Charlie Cheever
**Date:** 2026-10-09
**Related:** issues/20261009-ios-confirmation-shape.md, docs/contract-for-agents.md, docs/agent-pitfalls.md, CLAUDE.md

Users report "I converted my Expo app to SwiftUI and it took less time and tokens and was less buggy than Exact." A benchmark reproduces it.

**Method (2026-10-08).** A 612-line Expo Router app ("Shelf": 3 tabs, searchable/filterable list, detail with edits, modal add form with validation, Open Library fetch with error/retry and pull-to-refresh, persistence). Four agents (Opus 5.5) in parallel, each on its own iOS 27 simulator, identical prompts except the target: 2× SwiftUI, 2× Exact. Four independent graders drove each result with `axe` against a 24-item checklist. Caveat: the Exact runs used a checkout of e5801055d (2026-10-06), 572 commits behind origin/main at the time; the machine was shared and loaded.

| | SwiftUI ×2 | Exact ×2 |
|---|---|---|
| Wall time | 9.9 / 10.0 min | 21.3 / 18.7 min |
| Input tokens processed | 2.1M / 2.4M | 11.8M / 15.0M |
| Output tokens | 28K / 29K | 42K / 41K |
| Peak context | 86K / 110K | 254K / 264K |
| Checklist PASS / 24 | 24 / 23 | 23 / 22 |
| Tests written | 0 / 0 | 9 / 8 (pass on web and iOS) |
| App source lines | 959 / 903 | 628 / 700 |

**Where it goes.** Output tokens are only ~1.5×; input is ~5–6×. The cost is context carried, not code written:

1. *Required reading.* CLAUDE.md says to read `contract-for-agents.md` first, then `agent-pitfalls.md`; the agents also read README, `contract-for-humans`, `contract-grammar`, `reference` and LLPs — ~70–100K tokens before the first line of app code, carried on each of 68–81 calls. The SwiftUI agents read no docs. (Today on main those files are ~125K tokens together; the agents guide grew from ~26K to ~31K, pitfalls from ~12K to ~16K.)
2. *The guides still lack the native iOS patterns a typical app needs.* Both agents grepped example apps (`apps/native-fixture`, `apps/messages-legacy`), Apple host code and RFCs (1021, 1059) to find: a segmented control (a tablist), the confirmation (`alertdialog` appears only in pitfalls), the context menu, pull-to-refresh (`refresh=`/`refreshing` is in neither guide; found by searching host code), and which tab-bar symbols exist (only the schema lists them).
3. *Contract itself was not the problem.* One port compiled on the first try; agents called the diagnostics "clear, quick". Web tests passed on their first run.
4. *Cold tooling.* `setup --check` ~2 min cold (24 s warm); the first `contract` call ~2 min while the CLI compiles; first iOS build 3–4 min.
5. *Real-time waits.* Both agents wrote `clock +1000 real` into tests after a tap on a confirmation during a push transition failed. `clock settle` works (verified on main 2026-10-08); the driver's error now names it.

**Proposed.** (a) A short starter guide (≤10K tokens) as the only required reading, with the long guides lookup-only (`contract vocab`, grep). (b) One canonical "native iOS app" recipe app with tests — tabs, push, modal sheet, header buttons, segmented control, confirmation, context menu, pull-to-refresh, list + search, form validation, persistence, fetch — that agents copy from. (c) Prebuild the contract CLI / make the cold first call fast. Then re-run the benchmark: target ≤1.5× SwiftUI time and ≤2× input tokens at equal checklist score.

Bugs it found in Exact, fixed alongside this issue: a field's Cut/Copy/Paste with no handler crashed iOS (NodeView forwarded to UIView); header bar items ignored the button's authored `color`. Open: issues/20261009-ios-confirmation-shape.md.

Benchmark artefacts on the Tuft machine that ran it: `~/bench/convert-eval/` (REPORT.md, fixture, checklist, runs, grader screenshots, transcript analyzer).
