# Code review: LLP 1080.000 §10, what the Signal clone needs from 1080 (151c5c0a5..1d5ca39bc), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `1d5ca39bc`. Rebased onto origin/main before landing: the reviewed `1d5ca39bc`, `a8363f493` and `116e492f4` landed as `c6ff66a8c`, `748c47500` and `214e2e554`; the landing commit is the one adding round 3.
- **Method:** one brief (sha256 `2eb8cbdb68f4bba25d9e9d750ce3c279ad6f1bf4d35a9f43b02a2ca7d2027ae4`), the same one sent to grok; round 1; blind to the other review. Requested by the coordinator for Charlie's "agents can't see native chrome or drive real touches". The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all eight taken in `a8363f493` (landed as `748c47500`):
  1. The two edge-swipe episodes are separated; the builds 1–4 cause is left unknown, and 2d461a0c is credited only with the build 7 tab-stack delegate report, undriven.
  2. The mic item names the 500 ms clock check (`app.contract:220`) and platform timing before any `clock`, limits "cannot start" to activation and `recognizedPan`, and says stage 1's real `tap` already sends both pointer events.
  3. "Does each" became "a candidate for each"; outcomes are asserted from `state` and `tree`, and what the call cannot prove is listed.
  4. The sketch gains `from`, `press` and `hold`, derives velocity from `over`, and keeps D2, D4, D5 and D8 by name.
  5. Reading the shell is its own item (2): `layout`'s walk and `axRoots` do not reach UIKit's bars, and the ask is bounded, UIKit-exposed observations.
  6. D4's existing kinds are named as planned; only the search field and the title-view segment are asked for. G3's existing gestures are told apart from the two it lacks.
  7. The axe evidence names the `UIMenu` and lost-tap limits, and calls the mic result a hold inside one command, not P3.
  8. The overlays-under-the-bars bug class is the motivation in item 1, and the keyboard gap is recorded as outside LLP 1080. "Every screen" became "the shell", and "every native screenshot" became the simulator captures of the shell.

---

LAND WITH FIXES

1. **Must-fix — [§10, edge-pop evidence:787](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:787).** The proposed cause does not fit builds 1–4: retained tab stacks arrived in build 5 (DIARY:456–542). `2d461a0c` fixes the later tab-stack delegate bug; it does not establish the earlier failure’s cause. **Edit:** separate those observations and leave both the original cause and post-fix real-touch verification unresolved.

2. **Must-fix — [§10, mic:767](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:767).** The proposed hold omits D6’s clock distinction. The clone sends only when `now() - recordStart >= 500` ([app.contract:220](/Users/admin/.tuft/projects/signal-exact2/app.contract:220)); default agent timing freezes that clock. Also, today’s platform tap already delivers pointer events, albeit without a controllable hold. **Edit:** limit “cannot start” to activation/recognized-pan delivery, and require `--timing platform`, before any `clock` operation, for the duration-sensitive recording proof.

3. **Should-fix — [§10, gesture claims:759](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:759).** “Does each” promises an unverified result. Apple’s API credibly supplies one press–drag–release with initial/final holds; it does not guarantee edge-pop recognition or the app’s intended outcome. [Apple documentation](https://developer.apple.com/documentation/xcuiautomation/xcuicoordinate/press%28forduration%3Athendragto%3Awithvelocity%3Athenholdforduration%3A%29). **Edit:** say “candidate for each; requires a native-presentation drive.” Distinguish window dispatch from action success. It could test Q3 without P3, but proves neither intermediate feedback, reversal/finger-following fidelity, held reads, nor real pinch.

4. **Should-fix — [§10, proposed grammar/barrier:761](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:761).** The sketch lacks an explicit edge starting point, distinguishes neither initial nor final hold, and presents duration where XCTest accepts velocity. Its phase-only success wording also omits D5’s attribution safeguards. **Edit:** retain D2/D4/D5/D8 explicitly, including bounds and overflow/ambiguity refusals; specify start-point and hold semantics and nominal duration-to-velocity conversion. A separate whole-gesture `tap` form is permitted by DEFERRED and avoids Astra finding 1; Q2 remains unresolved.

5. **Should-fix — [§10, native observations:738](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:738).** Enabling native presentation does not automatically expose its controls through 1080.001: `layout … native` starts at the authored view and stops at opaque platform/controller interiors ([AgentNativeIOS.swift:197](/tmp/rv-agentview/host/apple/Sources/ExactKit/IOS/AgentNativeIOS.swift:197)). Badge readback is additional observation work. **Edit:** explicitly request bounded UIKit-owned observations; keep AX limited to UIKit’s exposed facts and preserve 1080.001’s existing opacity/agreement limits.

6. **Should-fix — [§10, “extended … after r2”:746](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:746), and master §5.** Header search and bar menus already existed in r2’s commit; D4 already lists `navigationBarButton`, `menuItem`, and `segment`. **Edit:** separate existing kinds needing Signal coverage from genuinely additional search targeting/typing and badge observations. Likewise, identify the *whole-gesture delivery form* as new; edge-pop and swipe-action proofs already belong to G3, conditionally on P3.

7. **Should-fix — [§10, axe evidence:781](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:781).** “Fails exactly the system-chrome cases” is too narrow. The diary also records menu-selection failure and subsequent `axe tap` losing delivery after a long press (DIARY:586–588, 841–845). **Edit:** include those limitations and describe the mic result as observed application behavior, without implying a P3-equivalent continuity proof.

8. **Should-fix — [§10, needs:728](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:728).** Missing consumers are the real software keyboard/interactive dismissal and toolbar inset (DIARY:597–609), plus fullscreen/root-overlay coverage above native bars (807–840). **Edit:** add these as coverage requests within existing stages. Narrow “every screen … UIKit chrome” and “every native screenshot” to the relevant simulator chrome captures; the diary also contains fullscreen presentation and Charlie’s phone screenshots.
## Round 2, 2026-10-03

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `a8363f493`; brief sha256 `9aa1315f5b373868ece9625a1971c987f8e2d851dd527049bc77ee0de38c0665` (round 1's brief, pointed at the whole note `151c5c0a5..a8363f493`). Blind to grok's round 2.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all eight taken in the landing commit:
  1. Checked: the navigation and tab controllers' views are mounted under the session's root node, inside the viewport (`NavigationIOS.swift:233`, `NavigationTabsIOS.swift:140`). The "under the bars" claim is gone; a viewport screenshot *should* show the bars after the flip, marked unverified.
  2. Checked: the AX walk recurses through subviews (`AgentAccessibility.swift:251-262`), so it probably reaches the bars. Item 2 now asks for a stage 3 proof rather than asserting a missing root, keeps `layout`'s opaque controller interiors as deliberate, and puts the raw `badgeValue` under 1080.001 D4 only if the AX value lacks it.
  3. Checked: the delete sheet is authored (`menus.contract:101-117`). The `alertAction` claim is removed.
  4. A zero-displacement hold is `press(forDuration:)`; the reply rule requires movement only for a non-zero delta, within a tolerance; durations are finite and non-negative.
  5. `over` is a requested duration converted to velocity in pixels per second, with the observed timing reported.
  6. "Checked after the lift" replaces "needs no read"; transient feedback, finger-following motion and frame-level jumps are named as unproven, and a real pinch, held reads and the keyboard are listed as outstanding.
  7. The module's custom title view (`SignalHooks.swift`, `ConversationTitle`) and the search bar's Cancel are added as targets.
  8. The review files land in the same commit. Bounds cite 1080.001 D4 and 1080.002 D7.

---

LAND WITH FIXES

1. **Must-fix — [§10, lines 752–753](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:752): screenshot boundary is wrong.** `presenter.viewport` contains the document and its mounted navigation/tab controllers (`NavigationIOS.swift:233`, `NavigationTabsIOS.swift:140`). Its capture does not inherently exclude their bars. **Edit:** delete “under the bars … stays that way”; describe viewport versus whole-window capture without promising which chrome is excluded.

2. **Must-fix — [§10 item 2](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:761) and [master §5](/tmp/rv-agentview/llp/1080-agent-inspection-fidelity.rfc.md:92): missing AX coverage is asserted without establishing it.** `axRoots` lacks explicit chrome roots, but its recursive walk can reach chrome inside the session’s mounted controllers. Conversely, `layout … native` deliberately stops at opaque controller interiors (`AgentNativeIOS.swift:197–204`). **Edit:** distinguish unverified AX coverage from the deliberate layout boundary; request coverage verification and a bounded chrome-observation extension. Report UIKit’s AX value in AX; raw `badgeValue` belongs in native observations.

3. **Must-fix — [§10, lines 745–746](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:745): the clone’s delete sheet is not an `alertAction`.** `menus.contract:108–117` authors ordinary columns/buttons, with neither `dialog` nor `alertdialog`; the diary’s build-10 fix moves that overlay above the containers. **Edit:** remove this claimed stage-3 dependency. D4’s `alertAction` covers UIKit confirmations, not this sheet.

4. **Should-fix — [§10 drag shape/barrier](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:781): stationary holds cannot satisfy “moved … and ended.”** The diary’s successful mic proof is a still hold. **Edit:** explicitly permit zero displacement using `press(forDuration:)`; require began/ended for that case and observed movement only for nonzero drags. Specify finite, nonnegative durations, positive travel duration for nonzero displacement, and positional tolerance.

5. **Should-fix — [§10, lines 778–787](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:778): `over` needs qualification.** Apple documents velocity in **pixels/second**, while aim coordinates use points; it does not promise an exact travel duration or release velocity. **Edit:** call `over` a requested duration, require coordinate-unit verification, and report observed timing. The public API is a credible candidate for each named single-contact gesture, not evidence that each recognizer accepts it. [Apple API](https://developer.apple.com/documentation/xcuiautomation/xcuicoordinate/press%28forduration%3Athendragto%3Awithvelocity%3Athenholdforduration%3A%29)

6. **Should-fix — [§10, “none … needs a read”](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:773): outcome coverage is overstated as interaction coverage.** The atomic `tap` proposal respects DEFERRED and avoids Astra finding 1; it must remain separate from P3/G3’s held-contact proofs. Final state cannot establish immediate recording feedback, finger-following motion, or the diary’s transient scrolling jumps. **Edit:** say “these post-release outcome checks”; retain those fidelity gaps, real pinch recognition, and keyboard interaction as outstanding.

7. **Should-fix — [§10’s two additional targets](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:754): the custom conversation title is missing.** DIARY build 5 and `SignalHooks.swift:23–32,93` describe a hooked title view whose real tap opens details. Neither `navigationBarButton` nor the proposed title segment covers it. **Edit:** add this consumer and ask how its actual native view is targeted; also include search Cancel/focus/blur in the search-field proof.

8. **Should-fix — [review attribution](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:727); Nit — [bounds citation](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:767).** Neither named `agentview` review file exists at this commit, and 1080.001 has no D7. **Edit:** remove or correct the review attribution; cite 1080.001 D4 and 1080.002 D7 for their respective bounds.
## Round 3 (the last), 2026-10-03

- **Method:** `codex exec` as before, `-C` a detached worktree at `116e492f4` with this change's two review files removed (blindness); brief sha256 `ae2b4fef32cc4cbb9528e4d16de3853ec9f2a351ad004e80daaeddfd562cc47d` (the whole note, report only what is still wrong). Blind to grok's round 3.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three taken in the landing commit. No fourth round, by the three-round rule:
  1. The occlusion exemption is gone: `from` lies in the target, D4's refusal unchanged, and an edge start targets the visible route or scroll ancestor containing the point.
  2. `type <root> <location>` is called location navigation; Back is `tap back` under activation delivery (the clone's `back` action, with its cleanup).
  3. A native page sheet's swipe to dismiss (Settings, New Message) joins the gesture list, in §10 and in the master's §5.

---

LAND WITH FIXES

1. **Must-fix — [§10.3, line 794](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:794).** Allowing an edge start to be “exempt from D4’s occlusion check” contradicts the unchanged-design claim and permits touching an unrelated covering view. **Edit:** remove the exemption; target the visible scroll/route ancestor containing the start point, preserving D4’s refusals and observed-hit matching.

2. **Should-fix — [§10, line 845](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:845).** `type <root> <location>` is navigation, not Back. The clone’s `navigate` calls `open`; its `back` calls `back` and performs conversation-close/photo cleanup ([app.contract:108](/Users/admin/.tuft/projects/signal-exact2/app.contract:108)). **Edit:** say “location navigation”; identify `tap back` under activation delivery separately as the existing non-gesture Back path.

3. **Should-fix — [§10.3, gesture inventory](/tmp/rv-agentview/llp/1080.000-real-touches-on-ios.rfc.md:782).** Native sheet dismissal is missing: the diary identifies Settings and New Message as UIKit page sheets, distinct from the viewer’s transform drag. **Edit:** add their swipe-to-dismiss as a whole-gesture candidate, with post-lift route/state assertions, or explicitly list it as outstanding. Update the master’s corresponding inventory.