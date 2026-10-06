# LLP 1080: Agent inspection fidelity — what the agent still cannot see or do as a person would

**Type:** RFC (master; the decisions live in the three sub-documents)
**Status:** Draft r1 (master). Each sub-document is at r2 after an Astra (max) review: all three r1s were NOT READY (14, 15 and 16 findings), and each r2 opens with its dispositions. Charlie asked for this on 2026-10-03 ("write up an LLP for all of these … then implement each one"); that is also the human word LLP 1080's apparatus needs (CLAUDE.md: "agents … add none without a human saying so"). **Landed 2026-10-03:** the screenshot fix (10ad7b14, 116e0909); 1080.001 (through 58b3a55a); 1080.002 (through 3be2d66f); 1080.000 stages 0–1 (through 3c72e442) — real `tap` only, held contacts still `unsupported` (its Q2 is Charlie's), the phone pass owed. Each passed three Astra xhigh code-review rounds (`llp/reviews/code-2026-10-03-*`); what each still owes is in its own as-built section.
**Systems:** Apple host (`host/apple/Sources/ExactKit/IOS/AgentIOS.swift`, `Mac/AgentMac.swift`, `host/apple/pointer.swift`, a UI-test carrier), web host (`host/web/glue.js`, the CDP carrier in `scripts/agent.mjs`), Linux host (`host/linux/src/agent.rs`), tooling (`scripts/agent.mjs`, `scripts/agent-inspect.mjs`, `scripts/smoke.mjs`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5), Tuft session 1791014443.232549, from 2026-10-03, in the order of §3
**Date:** 2026-10-03
**Related:**
- LLP 1012: the agent API, the host contract, `delivery`.
- LLP 1035.002: `layout <target>` and its `native{}`; D7, bounded reads.
- LLP 1035.003: reproducible native gestures; contact phases.
- LLP 1079: `perf`, the tenth operation. It already covers frame and hitch stats, so this document does not.
- `rules/DEFERRED.md` §Agent API.

## Summary

The agent drives an Exact app through nine operations (ten once LLP 1079's `perf` lands). It sees Exact's logical
tree, one node's native facts at a time, and the host's focus, keyboard and
navigation. Three gaps remain where a drive passes and the app still fails
for a person:

1. **Input on iOS is not a finger.** `tap` there is `delivery: activation`,
   a hit-test followed by a direct call. Contact phases are `unsupported` on a
   phone. The simulator's real-pointer path (`pointer.swift`) no longer finds
   a window under Xcode 27's Device Hub (`queue/`). The `_delayTouchesForEvent`
   nil-insertion crash (LLP 1012, summary) passed every agent drive because
   no drive sent a `UITouch`. → **LLP 1080.000**.
2. **The platform's own view tree is invisible.** `layout <target>` names
   the one view a node mounted. Nothing shows what else UIKit or AppKit holds
   under the `ExactView`, or whether that agrees with the kernel. A leaked
   pooled view, a stale row, or a frame or alpha that disagrees with the kernel
   can only be inferred from a screenshot. → **LLP 1080.001**.
3. **What assistive technology sees is invisible.** No host reports the
   accessibility tree it exposes. The web's own computed accessibility tree
   is never used as the oracle that "the web is the standard" makes it.
   → **LLP 1080.002**.

Frame timing and hitches, the fourth gap raised on 2026-10-03, are LLP 1079's
`perf` and are not repeated here.

## 1. Constraints every sub-document holds

- **No new operation.** A new input is a form of `tap` or `type`. A new
  question is answered from `tree`, `state` or `layout` (`DEFERRED.md`
  §Agent API).
- **No sixth blocking check.** New proofs are `smoke.mjs` steps and tests in
  existing binaries. They run on the async lane, not the gate.
- **Observations, never a second model.** Each new reading is taken from
  the platform: UIKit's views, `UIAccessibility`, the browser's AX tree. It is
  never Exact's intent re-derived. A space or fact a host cannot observe is
  absent, or `{"unavailable": …}` where absence would be ambiguous
  (LLP 1035.002 D4).
- **Bounded.** Every new list obeys LLP 1035.002 D7 and says `truncated`.
- **Delete, don't deprecate.** Whatever a sub-document supersedes is
  removed in the same change.

## 2. The sub-documents

| | Gap | Answer | Where it reads |
|---|---|---|---|
| 1080.000 | iOS input is not a finger | real `UITouch`es from a long-lived XCUITest runner, on the simulator and on a phone; held contact phases only if the stage-0 probe proves them | `tap` reports `delivery: platform` |
| 1080.001 | the native tree | a bounded native subtree under a target, plus an agreement report limited to the kinds that can be judged soundly (stray, parked-visible / leaving-interactive, frame, hidden) | `layout` |
| 1080.002 | the accessibility tree | the platform's exposed AX tree, keyed to view ids, with four fixed findings; DEFERRED's "accessibility audit" narrowed by Charlie's waiver | `tree --ax` |

## 3. Landing order

1. **Small fix first:** the iOS `screenshot` waits for in-flight image
   decodes (`queue/`, 2026-09-25). Independent of the three.
2. **1080.001.** It has the fewest unknowns and runs on the carriers that
   already exist. Its agreement step then guards the two after it.
3. **1080.002.** Web first, as the oracle, then iOS, macOS and Linux.
4. **1080.000.** It brings the new carrier and the most platform risk
   (Xcode 27, device signing).

Each step lands on `origin/main` after an Astra (`gpt-6-astra`, xhigh) code
review. The documents themselves are reviewed by Astra at max effort before
implementation starts (`llp/reviews/1080*.astra.md`).

## 4. Not in this document

- `perf`, frame timing, hitches, traces: LLP 1079.
- A devtools UI, a views server, or a live inspector (`DEFERRED.md`).
- Android. There is no Android host.

## 5. From the Signal clone

LLP 1080.000 §10 records what the Signal clone needs, in its order of need:
the native shell under the agent (stage 3's presentation flip, ideally ahead
of the native aim, and targets D4 does not name: the search controller's field
and Cancel, the title-view segment, and a module's custom title view); a
stage 3 proof that `tree --ax` reads that shell (adding 1080.002 D3's chrome
roots if it does not); and a proposed whole-gesture
`drag` form of `tap` for six gestures whose outcome can be checked after the
lift: the edge swipe back, swipe-to-reply, the viewer's dismiss drag, the
swipe-action reveal, a page sheet's swipe to dismiss, and the mic's press,
slide and release.

Charlie approved items 1 and 4 on 2026-10-03. Item 4 is built: LLP 1080.000
§11, `tap <target> drag <dx> <dy> [from <x> <y>] [press <ms>] [over <ms>]
[hold <ms>] [during "<op>" …]`, with the edge-swipe fix it found. Stage 3
stays with this document's implementer.
