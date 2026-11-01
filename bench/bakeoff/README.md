# The bake-off

The loop of LLP 1116: agents build the same iPhone app specs with Exact and with SwiftUI or UIKit, a blind grader
compares each pair, a scroll probe measures how each app's lists feel, and what Exact lost becomes the next round's
changes. Rounds 1 to 20 ran from `~/bakeoff`, which is gone; this directory rebuilds it, so its numbers start a new
series and are not comparable with round 20's.

| Piece | What it is |
|---|---|
| `specs/<id>/SPEC.md`, `data/` | nineteen iPhone apps, each with seeded data; the service specs include frozen servers |
| `pairs.json` | each spec's comparison stack and builder model, fixed so rounds compare pair against pair |
| `prompts/` | the builder prompt (`common.md` plus the stack's) and the grader's |
| `native/` | the SwiftUI and UIKit builders' skeleton: `swiftc` and an assembled `.app`, no Xcode project |
| `build.mjs` | one build: the builder unattended, its wall clock and tokens, the app it left installed |
| `feel/` | the scroll-feel probe: real touches, a passive recorder in the app, metrics against UIKit's physics |
| `drive.mjs`, `drive.md` | the grader's driver: any app by bundle id, through the XCTest touch runner |
| `grade.mjs` | one blind pairwise grade by Claude: requirements, bugs, tells, scores, a verdict |
| `round.mjs` | a round: pin, build, feel, grade, report |
| `report.mjs` | the round's table; `results/<round>/` keeps what each round recorded |

```sh
BAKEOFF_ROOT=/Volumes/ExternalSSD/bakeoff bun bench/bakeoff/round.mjs n0 build --native-only 1 --parallel 3
BAKEOFF_ROOT=/Volumes/ExternalSSD/bakeoff bun bench/bakeoff/round.mjs g0a all --native n0 --parallel 3
BAKEOFF_ROOT=/Volumes/ExternalSSD/bakeoff bun bench/bakeoff/round.mjs g1 all --native n0 --heldout timetable --parallel 3
```

A round's apps, logs and screenshots stay in `${BAKEOFF_ROOT:-~/bakeoff}/rounds/<name>`; `report` copies records, feel results,
grades, builders' notes and the summary into `results/<name>/`.

The native baseline is named with `--native`; later rounds build Exact only and reuse that baseline for feel, grading
and the report. `--native-only 1` builds the baseline's native arm once. `--heldout <id>` leaves the held-out existing
spec out of rounds after g0a and g0b. `notes`, `settings` and `trips` are cold-cache builds and run alone; the report
lists their first-build minutes. Set `BAKEOFF_ROOT` explicitly so apps, logs, pins and locks stay on the external SSD.

- Builders: GPT-6.1 Sol (high) through `codex exec`, for both arms. The blind grader remains Claude Opus 5.5.
- Exact builders use a detached checkout pinned at the round's commit (`${BAKEOFF_ROOT:-~/bakeoff}/pins/<rev>`) with this directory
  removed, so the grader's prompt is not there to read.
- Simulators: `bake-b1`…`b6` build, `bake-g1`, `g2` grade, `bake-feel` probes (iPhone 18 Pro, iOS 27). Apps are
  uninstalled between uses, never erased.
- Bundle ids and names are neutral (`dev.bakeoff.<spec>.<random>`), so the grader cannot tell the arms apart by them.
