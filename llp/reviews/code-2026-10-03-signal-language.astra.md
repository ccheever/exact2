# Code review: the Signal Clone's language rulings (tail call, pointer events, tab badges; d4789cc70..63e2bd8c7), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `63e2bd8c7`.
- **Method:** one brief (sha256 `4d72696f9b1d95ecc014a1fc44341b56359bea1c317748534fc19fa2583b40c0`), shared with grok; round 1; blind to the other review. Requested by Charlie through the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** every finding checked in the source. All fixed in `7cd8f3ea6` and the two commits before it (`23ae9ef83`, the web pointer's document-level up; `e132adca9`, the caps folds). #7 is fixed for the hosts' state and argued for the delivery:
  1. *Caller parameters capture callee reads.* Fixed by renaming apart. Every caller binder, parameters included, and every callee parameter and binder takes an `@` name, so nothing an author wrote can capture an inlined read. Test: `the_called_action_reads_the_root_never_a_caller_local_or_parameter_of_its_name`.
  2. *Arguments bypass the child's scope.* Fixed. `tail_args` infers every argument in the child's scope, whatever the prop's signature. Test: `an_argument_must_be_a_name_the_child_has`.
  3. *Callee parameter types erased.* Fixed. Each call leaves `@check:<action>` with its arguments, the type pass holds them to the callee's parameter types (curried ones included), and lowering drops it. Test: `the_arguments_are_held_to_the_called_actions_parameter_types`.
  4. *Nested pointer nodes disagree.* Fixed. The innermost enabled pointer node takes the pointer on every host. On the web and the JS target the first handler marks the event as it bubbles. On iOS each recognizer skips a touch that a nearer enabled pointer node takes (`nearer`). macOS already walked to the innermost. Tests: the nested case in `host/web/tests/pointer.test.mjs` and `testTheInnermostEnabledPointerNodeTakesTheTouch` (iOS). LLP 1005 now says so for every host.
  5. *Capture consumes descendant presses.* Fixed before this round's disposition: the web's up is heard on the document and there is no capture (`23ae9ef83`, the same on the JS target). Test: the wrapped child keeps its press, in `pointer.test.mjs`.
  6. *A down-only handler sticks after an outside release.* Fixed by the same change. The document listener runs whatever handlers the node has and clears `held`. Test: a lift elsewhere, then later downs.
  7. *A removed held node never hears its up.* The state is fixed and the delivery is argued. macOS clears `pointerHeld` on that view's release and on reset. The web clears `held` from the document listener. iOS forgets the touch on reset. Delivering an up to a node that has left the kernel needs a runner path that runs a handler for a dying view, which the runner does not have (`dispatch` requires a live view). That is new apparatus, not a fix here. LLP 1005 now says the end is heard unless the node is removed while the pointer is down, and that an app keeps the node mounted until the up, as the clone's mic does.
  8. *macOS native buttons are silent.* Fixed. `NativeButtonMac` reports down from `mouseDown` and up from `sendAction`, before the action, so DOM's order holds, and again after its tracking loop, a no-op once released. There is no Mac native-button test, because a native button needs the runtime's face to build.
  9. *Caps.* Fixed in the commit before (comment lines folded, no code changed). `bun scripts/caps.mjs` passes.
  10. *Badge false positives.* Fixed. A badge is a shown box with a visible fill whose only node child is one shown, non-empty text, and there must be exactly one such box. Tests: the Home tab's filled pill around a symbol and a text gives no badge, the badge leaves with its box, and a hook's badge survives a face change.

---

1. **Must-fix — Caller parameters capture the callee’s state reads.** [inline/tail.rs:63](/tmp/rv-lang1/contract/syntax/src/inline/tail.rs:63) starts capture checking with no bound names; only subsequent `let`/`match` bindings are collected. If the root has `n = 7`, its callback reads `n`, and the child invokes it from `go(n: number)` with `42`, the expanded callback reads **42**, not the root’s 7. Include caller parameters in capture checking, or preserve lexical bindings through hygienic renaming. Add a regression covering this collision.

2. **Must-fix — Tail-call arguments bypass the child’s scope check.** In [actions.rs:383](/tmp/rv-lang1/contract/types/src/actions.rs:383), `args.iter().zip(params)` checks nothing for ordinary `cb: action`, whose parameter list is empty. Consequently, a child’s `cb(secret)` can resolve `secret` from the root after expansion even though the child never declared or received it. Infer every argument in the original child scope, regardless of whether the callback signature is known.

3. **Must-fix — Expansion discards callback parameter types.** [inline/tail.rs:168](/tmp/rv-lang1/contract/syntax/src/inline/tail.rs:168) replaces parameters with untyped lets after checking only argument count. A callback declared `record(value: string)` can therefore receive `42` through a tail call when its body also accepts numbers—for example, `result = toString(value)`. Validate arguments against the resolved callee signature before erasing the call, including arguments supplied through currying.

4. **Must-fix — Nested pointer nodes disagree across hosts and lose releases.** [pointer.js:10](/tmp/rv-lang1/host/web-js/pointer.js:10) and [PointerIOS.swift:15](/tmp/rv-lang1/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:15) do not select the innermost observer. Web events bubble through every pointer node; UIKit ancestor recognizers also observe the touch. macOS selects only the innermost node. On web, the outer node additionally overwrites the inner node’s capture, leaving the inner node without its up. Select one owner without blocking unrelated gestures, and test nested observers on both web targets and UIKit. LLP 1005’s innermost-node claim currently misdescribes these implementations.

5. **Must-fix — Web pointer observation consumes descendant presses.** The unconditional capture in [input-glue.js:124](/tmp/rv-lang1/host/web/input-glue.js:124), also present in [pointer.js:13](/tmp/rv-lang1/host/web-js/pointer.js:13), changes event targeting. With pointer handlers on a container and `press` on a child button, the container captures the button’s down; the ensuing up/click targets the container, so the child loses its press. Observe the contact’s end without stealing capture from controls or existing gestures. Add a real browser interaction test for this case.

6. **Must-fix — A down-only handler drops subsequent downs after an outside release.** [pointer.js:11](/tmp/rv-lang1/host/web-js/pointer.js:11) records `held`, but captures only when `pointerup` is authored and clears it only through element-local up/cancel listeners. Mouse-down inside, then release outside: the next down is rejected as already held. [input-glue.js:119](/tmp/rv-lang1/host/web/input-glue.js:119) has the same defect. Track contact completion independently of whether an up callback exists, including capture loss.

7. **Must-fix — Removing a held node does not deliver the promised cancellation.** [MouseChainMac.swift:133](/tmp/rv-lang1/host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:133) requires a surviving presenter/view to release; destruction disconnects that view, and presenter reset does not clear `pointerHeld`. UIKit’s [PointerIOS.swift:33](/tmp/rv-lang1/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:33) simply forgets the touch on reset, while web retirement suppresses callbacks. A down action that starts recording and replaces its node can leave recording active indefinitely. Add an explicit retirement cancellation path and clear ownership on reset. Test removal during down; [LLP 1005:479](/tmp/rv-lang1/llp/1005-plan-and-runner-v1.spec.md:479)’s “always hears the end” claim is currently false.

8. **Must-fix — macOS native buttons never reach these pointer hooks.** The new dispatch is confined to [NodeViewMac.swift:1325](/tmp/rv-lang1/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1325). A `button appearance="auto"` is hit and tracked by [NativeButtonMac](/tmp/rv-lang1/host/apple/Sources/ExactKit/Mac/NativeButtonsMac.swift:13), which forwards activation but no down/up phases. Its pointer handlers therefore remain silent, unlike web and UIKit. Integrate observation with native-control tracking while preserving down → up → press order, and add native-button coverage.

9. **Must-fix — The required cap check fails.** Running `bun scripts/caps.mjs` returned five violations: [tags.rs:571](/tmp/rv-lang1/contract/lower/src/tags.rs:571) and [Apple abi.rs:894](/tmp/rv-lang1/host/apple/src/abi.rs:894) are each **1,503 lines**; [rt.js:795](/tmp/rv-lang1/host/web-js/rt.js:795), [emit.rs:1194](/tmp/rv-lang1/host/web-js/src/emit.rs:1194), and [glue.js:551](/tmp/rv-lang1/host/web/glue.js:551) are each **1,501**. Reduce them below the binding 1,500-line cap and rerun the check.

10. **Should-fix — The badge matcher accepts unrelated tab content.** [NavigationTabsIOS.swift:53](/tmp/rv-lang1/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:53) counts only paragraph children, ignoring other children and the paragraph’s visibility. A filled wrapper containing an icon plus “Chats” becomes a `"Chats"` badge; a hidden text can also supply a visible badge. Require the admitted box shape with a sole visible text child. Add negative fixtures, plus badge removal and hook-set badge preservation tests; the current test covers only appearance of `"1"`.

Verdict: DO NOT LAND

## Round 2, 2026-10-03

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `f8377f0ab`; brief sha256 `45a19361e3e8fcfc19b3715279ca1f93514660cec08186f4fd511f171c52ec90` (the fixes `63e2bd8c7..f8377f0ab` against round 1's findings and dispositions). Blind to grok's round 2.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all five fixed in `6d251386f`:
  1. *Renaming rewrites calls.* Fixed. The renaming apart substitutes values only (`Subst::values`, call heads untouched), as a view's loop variables already are. Test: `renaming_apart_leaves_a_function_call_of_the_same_spelling_alone`, where a parameter `length` and a local `toString` sit beside a `toString(length)` call.
  2. *A release runs a removed node's callback.* Fixed. The web host fires only for a view that is still its id's and is not retired. The JS target fires only for a connected element.
  3. *A disabled div claims the pointer.* Fixed. Both web paths treat the node's own `disabled` attribute as disabling and return before claiming, so the event bubbles to the enabled parent. Test: a disabled `div` inside an enabled pointer parent, in `pointer.test.mjs`.
  4. *Flattened children defeat the badge.* Fixed. A box holding a flat leaf (`FlatLeaves.holdsLeaves`) is not a badge. The fixture's Home pill is now a text beside a dot that flattens.
  5. *Transparent badges.* Fixed. A box or text with opacity 0 does not badge. The fixture adds a transparent count box on Home, and the test asserts no badge there.

---

1. **Must-fix — Renaming locals also renames unrelated function calls.** [tail.rs:127](/tmp/rv-lang2/contract/syntax/src/inline/tail.rs:127) uses substitution that rewrites call names. A valid child action containing `let toString = 0`, then `result = toString(42)`, then `done()` becomes a call to `toString@b…` and fails with `type-unknown-function`. Parameter and match-binding renaming have the same problem. **Fix:** rename references to value bindings while preserving independently resolved function names; add regressions for all three binders.

2. **Must-fix — Document releases execute callbacks belonging to removed nodes.** [pointer.js:16](/tmp/rv-lang2/host/web-js/pointer.js:16) invokes the stored callback without checking its lifetime. If `pointerdown` removes a conditional button, its later release still runs `pointerup` on the JS target—for example, submitting a recording after navigation removed its control. [input-glue.js:125](/tmp/rv-lang2/host/web/input-glue.js:125) likewise bypasses the former retirement guard and dispatches a stale view ID. **Fix:** clear ownership/listeners on retirement and suppress callbacks for retired owners, including nodes retained for exit animations. The revised removal policy is reasonable, but these paths contradict it.

3. **Should-fix — Disabled non-control containers still claim web pointers.** [pointer.js:20](/tmp/rv-lang2/host/web-js/pointer.js:20) and [input-glue.js:130](/tmp/rv-lang2/host/web/input-glue.js:130) only check `:disabled`. A nested `column disabled=true pointerdown=…` becomes a div, which never matches that selector; it fires and claims the event while UIKit/macOS skip it and select the ancestor. This leaves Grok’s non-form-disabled case unresolved. **Fix:** make the authored disabled-container behavior consistent across hosts and test that nested case.

4. **Should-fix — Flattened children defeat the strict badge matcher.** [NavigationTabsIOS.swift:66](/tmp/rv-lang2/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:66) counts only `NodeView` subviews. A filled pill containing text “Chats” plus a plain colored dot has two authored children, but [FlatLeavesIOS.swift:65](/tmp/rv-lang2/host/apple/Sources/ExactKit/IOS/FlatLeavesIOS.swift:65) can render the dot as a layer. The matcher then sees one child and incorrectly creates a “Chats” badge. **Fix:** count logical children, including flattened leaves; add this negative fixture.

5. **Should-fix — Transparent content still produces visible badges.** [NavigationTabsIOS.swift:64](/tmp/rv-lang2/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:64) checks `isHidden`, `display`, and background alpha, but neither the box’s nor the text’s opacity. A count box hidden with `opacity=0` still supplies a visible native badge. **Fix:** exclude fully transparent boxes/text and test changing opacity to zero.

Astra round-1 #1: resolved.  
Astra round-1 #2: resolved.  
Astra round-1 #3: resolved.  
Astra round-1 #4: not resolved completely: disabled-container ownership still differs (#3 above).  
Astra round-1 #5: resolved.  
Astra round-1 #6: resolved.  
Astra round-1 #7: resolved by the documented disposition; the new retirement regression is #2 above.  
Astra round-1 #8: resolved.  
Astra round-1 #9: resolved; `bun scripts/caps.mjs` passes.  
Astra round-1 #10: not resolved completely: flattened children and transparent content still produce false badges.

Source review only apart from the permitted cap check; runtime tests were not run. No files modified.

Verdict: LAND WITH FIXES

## Round 3 (the last), 2026-10-03

- **Method:** `codex exec` as before, `-C` a detached worktree at the round-2 reviews commit; brief sha256 `4d02c715431af3f6df6e20fcb465458e2cc977668c1f488dbf94a2502e52273a` (the round-2 fixes, `6d251386f`). Blind to grok's round 3.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all four fixed in `05b2be7a5` (and the opacity fixup before it). No fourth round (rules/RULES.md, three rounds):
  1. *A focus change ends a hold.* Fixed. `blur` is heard on the window without capture and only when the window is the target. The Chrome test moves focus during a hold and expects only the down.
  2. *Exiting nodes run their up on the JS target.* Fixed. `pointer.js` skips an element inside `[data-exiting]` as well as a removed one.
  3. *Another contact's `pointerout`.* Fixed. Every pointer event must be the held pointer's, `pointerout` included.
  4. *The badge negatives masked each other.* Fixed. The transparent box is alone on Home. The flat-leaf pill sits beside Second's real badge, where its being taken would make two boxes and no "1". Each fails on its own.

---

1. **Must-fix — Changing element focus prematurely ends a hold.** Both [input-glue.js:137](/tmp/rv-lang3/host/web/input-glue.js:137) and [pointer.js:26](/tmp/rv-lang3/host/web-js/pointer.js:26) register `window`’s blur listener in capture mode, so it receives descendant blur events too. Focus the message input, then hold the mic: the input blurs and immediately triggers `pointerup`, stopping recording before release. **Fix:** accept blur only when `event.target === window`, or register that listener without capture.

2. **Must-fix — Exiting nodes still execute their JS-target release callback.** [pointer.js:19](/tmp/rv-lang3/host/web-js/pointer.js:19) checks only `isConnected`, but [rt.js:835](/tmp/rv-lang3/host/web-js/rt.js:835) keeps removed nodes connected during exit animations. If down removes a control or its ancestor with an exit animation, releasing before the animation finishes still executes its up action—for example, submitting a recording after navigation. **Fix:** suppress callbacks for retired owners and descendants of `[data-exiting]`, and clear their hold/listeners on disposal.

3. **Should-fix — Another contact’s `pointerout` cancels the primary hold.** The `pointerout` branches in [input-glue.js:126](/tmp/rv-lang3/host/web/input-glue.js:126) and [pointer.js:16](/tmp/rv-lang3/host/web-js/pointer.js:16) bypass the pointer-ID check. Hold the mic with the primary finger, then touch and lift a second finger: its non-hover `pointerout` with null `relatedTarget` ends the primary recording. **Fix:** require the held pointer ID for every pointer event, including `pointerout`; handle window blur separately.

4. **Should-fix — The badge regression fixture masks both original failures together.** [app.contract:307](/tmp/rv-lang3/apps/native-fixture/app.contract:307) puts the flattened-dot pill and transparent count on the same tab. With the pre-fix matcher, both qualify, so [NavigationTabsIOS.swift:56](/tmp/rv-lang3/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:56) returns nil because there are two candidates. Consequently, [NavigationTabsIOSTests.swift:102](/tmp/rv-lang3/host/apple/tests/ExactKitTests/NavigationTabsIOSTests.swift:102) passes while both bugs remain. **Fix:** exercise each negative case independently, including changing a valid badge’s opacity to zero.

`bun scripts/caps.mjs` passed. In-memory pointer probes confirmed findings 1–3’s handler paths; browser, Rust and UIKit suites were not run. No files modified.

Astra round-2 #1: resolved.  
Astra round-2 #2: not resolved: connected exit-animation nodes still receive JS-target callbacks (finding 2).  
Astra round-2 #3: resolved.  
Astra round-2 #4: resolved.  
Astra round-2 #5: resolved.  
Grok round-2 #1: resolved.  
Grok round-2 #2: resolved; the added termination paths introduce findings 1 and 3.

Verdict: LAND WITH FIXES
