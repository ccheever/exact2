# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), round 3, second narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `497de689b9`.
- **Method:** one brief (sha256 `72b39d28326de5ed5fb63e06ebc2b0b9dced5bacfb9d8b1c373ddf361f85c875`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. The full Mac suite reruns on the final tip.

## Brief

Final narrow re-check (host change, macOS: early pointerdown before focus in a text area or an input). HEAD 497de689b9; the last re-check reviewed 9bd2e2f84b. Read `git diff 9bd2e2f84b..HEAD` (2 commits, small), with context from `git diff 4293f59072..HEAD` as needed.

Findings from the last re-check, and the fixes:
- Astra P1: the re-posted up could be consumed before tracking starts (another local monitor's nested loop), leaving tracking waiting. A press released before its tracking now runs no tracking at all: in tracksPointer, the `reported` branch returns without track(), because its up is gone and AppKit's loop would wait for another.
- Astra P2 and Grok P2: a second up while the first is held fell through to the detached-source fallback and released mid-bubble. The fallback now requires mouseChain.early == nil. Test: testOnlyThePresssOwnUpIsHeld goes through NSApp.sendEvent, with and without the editor detached. On 9bd2e2f84b the detached case gave ["down-z","up-z","up-zo","down-zo","down-zo","up-zo"]; it passes now.
- Grok P2: a nested mouse-down during bubbling hit the stale release at the top of pressEditorFirst and released the press early. The stale release now skips while earlyBubbling. Test: testAClickWhileTheDownBubblesLeavesThePressAlone, red on e8cc923edf (up before focus) and passing now.
- Named residual, not fixed: a focus or blur handler that spins its own event loop between the re-post and the editor's tracking could still take the up, and AppKit's tracking would then wait for the next up. This is the general AppKit hazard of any loop eating a button's up, and it exists without this change too. It will be stated to theme2 at sign-off.
Tests: EditorPointerOrderMacTests has 18 and EditorPointerMacTests 4, all passing. The full Mac suite on 9bd2e2f84b was 302/0; the suite on 497de689b9 is running.

Check only:
(a) Are these three fixes correct?
(b) Did they introduce any new double report, stuck hold or lost up?
(c) Is the residual fairly characterised?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

No actionable findings in `9bd2e2f84b..497de689b9`.

- [MouseChainMac.swift:363](/tmp/iosfix2-rv-pd5-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:363): skipping tracking for an already-reported press correctly avoids waiting for its consumed up.
- [ExactViewMac.swift:337](/tmp/iosfix2-rv-pd5-astra/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:337): the guard prevents the detached-source fallback from releasing an early press mid-bubble.
- [MouseChainMac.swift:499](/tmp/iosfix2-rv-pd5-astra/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:499): skipping stale-release handling during bubbling correctly prevents a nested down from scheduling that premature release.

I found no new double report, stuck hold, or lost up introduced by these fixes.

The residual is fairly characterised, with one qualification: an up observed before `tracksPointer` starts is now covered. The remaining hazard concerns consumption that bypasses the monitor, or occurs after `early` is cleared. Local monitors observe application event dispatch, not every queue consumption. [Apple documentation](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)

Static review only; I did not rerun the tests.

VERDICT: APPROVE
