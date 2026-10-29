# The bake-off

The loop of LLP 1116: agents build the same iPhone app specs with Exact and with SwiftUI or UIKit, a blind grader
compares each pair, a scroll probe measures how each app's lists feel, and what Exact lost becomes the next round's
changes. Rounds 1 to 20 ran from `~/bakeoff`, which is gone; this directory rebuilds it, so its numbers start a new
series and are not comparable with round 20's.

| Piece | What it is |
|---|---|
| `specs/<id>/SPEC.md`, `data/` | twelve iPhone apps, each with a long list from seeded data (`specs/gen.mjs` writes it) |
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
bun bench/bakeoff/round.mjs r21             # build, feel, grade and report every pair
bun bench/bakeoff/round.mjs r21 grade --specs chat,inbox --redo 1
```

A round's apps, logs and screenshots stay in `~/bakeoff/rounds/<name>`; `report` copies records, feel results,
grades, builders' notes and the summary into `results/<name>/`.

- Builders: GPT-6.1 Sol (high) through `codex exec`, sandboxed with escalations reviewed (`--approve-for-me`), and
  Claude Opus 5.5 through `claude -p` in the `auto` permission mode. Muse is not logged in on this machine.
- Exact builders use a detached checkout pinned at the round's commit (`~/bakeoff/pins/<rev>`) with this directory
  removed, so the grader's prompt is not there to read.
- Simulators: `bake-b1`…`b6` build, `bake-g1`, `g2` grade, `bake-feel` probes (iPhone 18 Pro, iOS 27). Apps are
  uninstalled between uses, never erased.
- Bundle ids and names are neutral (`dev.bakeoff.<spec>.<random>`), so the grader cannot tell the arms apart by them.
