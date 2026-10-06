# Code review: UIKit's zoom interactive dismissal lands unaligned (LLP 1035.001, the zoom's alignment), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x2zoom-rv`, edits denied.
- **Method:** one brief (sha256 `0333dd0135c70ca0f5f5d3e4d427608dc0aee9e8d34bdd89ecaf95662e795504`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 10fe88442 in a detached worktree. The author (Claude) is not a reviewer. A first run in `--permission-mode plan` returned only its preamble, with no findings; it was discarded and run again without plan mode.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r2):** 1 taken, the same as Astra's 1: the flag's transitions are `ZoomLanding.dismissal(begins:)` and `appeared()`, the closures pass straight through, and the test drives them. The live wiring is verified by real-touch simulator drives. 2: proposed LLP 1035.001 text, sent for Charlie's approval; not landed with the code.

---

READY

1. **nit** — `host/apple/tests/ExactKitTests/ZoomAlignmentIOSTests.swift:86`. The new test assigns `landing.interactive` itself and then calls `answer`. That locks the cache rule (a rect accepted while interactive is stored and returned again once the flag is clear). It does not call `interactiveDismissShouldBegin`, `ModalController.viewDidAppear`, or `sampleLanding`, so a wiring miss — flag never set, or `appeared` never installed — still passes.

2. **nit** — `llp/1035.001-native-interaction-ownership.rfc.md:588`. The alignment paragraph still says the zoom lands on the source element's drawn rect, including the close. This commit withholds that rect for an interactive dismissal (`ModalIOS.swift:164`). One sentence there would record the exception.

The flag's lifetime matches the calls that actually exist.

A completed drag sets `interactive` in `interactiveDismissShouldBegin` (`ModalIOS.swift:341`) and leaves it set. `viewDidAppear` does not run for a dismissal that finishes (`viewDidDisappear` does). The release stays unaligned, and that `ZoomLanding` dies with the controller. `presentationControllerDidDismiss` runs only after that animation, so `sampleLanding` (`ModalIOS.swift:366`) writes `landing.rect` too late to affect it.

A cancelled drag ends with the presented controller back in the appeared state. `viewDidAppear` (`ModalIOS.swift:61`) then clears the flag (`ModalIOS.swift:344`). A later Close samples a new rect and, with the flag clear, `answer(nil)` returns it. A second drag sets the flag again on the next `willBegin == true` call, which is before `alignmentRectProvider` (the provider runs only after `shouldBegin` returns true).

`willBegin == false` returns at `ModalIOS.swift:337` and neither sets nor clears the flag. That matches the header: `willBegin` is UIKit's default "this interaction would begin," and the closure's return value is the decision. A probe that is not the start of a dismissal does not arm unaligned mode. The same early return means a later probe during an in-flight drag cannot disarm it.

`shouldBegin` and `appeared` do not call each other. The flag is set before `shouldBegin` returns, so a provider call on that same stack already sees it.

The flag lives on the `ZoomLanding` created for that presentation, not on the route node. A replacement presentation builds another landing. If the captured route is no longer the one in `presenter.views`, `refusesDismissal` makes `shouldBegin` return false and it does not set the flag. An in-flight `true` stays `true` until the controller appears again, so the drag that already started stays unaligned.

`sampleLanding` writes `landing.rect` directly, so a Close still refreshes the cache while a drag is in progress. `answer` also stores a resolved rect before it returns nil (`ModalIOS.swift:165`). The provider withholds the rect only while `interactive` is true. After the cancel's `viewDidAppear`, the next programmatic close is aligned again.

`viewDidAppear` is the right end signal here. UIKit sends it after the appearance transition's animations have finished, and on a cancelled interactive dismissal the presented controller is the one that was disappearing and is now appearing again. Apple's fluid-zoom guidance is to drop temporary transition state in `viewDidAppear` or `viewDidDisappear`, because those run at the end of the transition. The first appearance also calls it, while `interactive` is still false. A finished dismissal does not call it, so the release is not switched back to a rect. Nested sheets in this host are `.pageSheet` or `.overFullScreen`, which do not disappear this controller, so dismissing one does not clear the flag mid-gesture.

The transition coordinator is the same moment, not a better one. `presentationControllerWillDismiss` is where UIKit says to install a coordinator callback, and `animate(alongsideTransition:completion:)`'s completion runs when that dismissal ends, with `isCancelled` set. That completion and `viewDidAppear` are the same point for a system transition. `notifyWhenInteractionChanges` is the wrong hook: it runs when the finger lifts, before the non-interactive completion segment, which is the lift-off that re-bases onto a rect. Clearing the flag there would hand the rect back to the segment this change is trying to keep unaligned.

Returning nil is the right lever. The Swift provider is `CGRect?`, refined from the header's "return `CGRectNull` for no preference." The constant-rect fixture jumps with any alignment rect; the unaligned release is the path with no rect. A different rect, including the route bounds, is still a preference and is what UIKit re-bases onto.

No retain cycle. `appeared` (`ModalIOS.swift:344`), the two option closures, and `sampleLanding` all retain `landing`. `landing` retains neither the controller nor `Presentation`. `shouldBegin` and `resolve` capture the host and the route weakly; `sampleLanding` captures the controller weakly.
