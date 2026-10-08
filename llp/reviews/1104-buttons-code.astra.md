# Review: LLP 1104 step 3 code (4e18bc622), 2026-10-07 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, at `4e18bc622` (`git diff 3af60bc8b..4e18bc622`).
- **Method:** a code-review brief (sha256 `85a06a764a293bfc73bee373bdce5dd10b2766bca5dc1e23c359ac64def97c45`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** all ten findings went to fix lanes BF1 (kernel: 5, 6), BF2 (Apple: 1, 2, 3, 5, 7, 9) and BF3 (web and Linux: 4, 8, 10).

---

**Verdict: DO NOT LAND.**

**Findings**

1. **Measurement and drawing resolve different lengths — blocking.** [host/apple/src/control_text.rs:140](/Users/ccheever/projects/exact2-wt-1104/host/apple/src/control_text.rs:140). Measurement serializes with `Env::default()`, while presentation uses the kernel’s environment. Thus `padding-top="env(safe-area-inset-top)"` measures with zero inset and draws with the real inset. The cache subsequently declares this wrong geometry exact. **Fix:** resolve dimensions against the live environment before constructing the shared measurement/presentation payload.

2. **iOS native popovers lack ordinary touch dismissal — material.** [MenusIOS.swift:248](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:248). Native invokers now open `agentShow` outside agent mode, but outside-tap dismissal and ordinary hide-button handling remain inside agent-only `agentTap`. Open the fixture’s popover and tap elsewhere: it stays open. **Fix:** connect the lifted presentation to production pointer, close-command, and dismissal handling; test actual touches.

3. **Authored iOS colors remain in the previous appearance — material.** [NativeButtonsIOS.swift:160](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/NativeButtonsIOS.swift:160). `Written.traits` includes category, legibility and scale, but excludes appearance and contrast. With `color="light-dark(black, white)"`, switching to dark mode leaves the face equal and skips configuration; the previously resolved, fixed `UIColor` remains black. Symbol tints have the same problem. **Fix:** include color-affecting traits in configuration invalidation or retain dynamically resolving colors.

4. **Bound display breaks the JS target’s native face layout — material.** [host/web/src/element.rs:235](/Users/ccheever/projects/exact2-wt-1104/host/web/src/element.rs:235), [host/web-js/src/rows.rs:268](/Users/ccheever/projects/exact2-wt-1104/host/web-js/src/rows.rs:268). Static emission substitutes `display:grid`, but `display=(visible ? "flex" : "none")` writes literal `flex` through the generic dynamic mapper. Titles, subtitles and symbols consequently lose their grid arrangement, including on initial evaluation. **Fix:** map native-button display bindings to `grid`/`none`, matching static emission.

5. **Button-relative lengths use the field font — material.** [kernel/src/arena/control_text.rs:105](/Users/ccheever/projects/exact2-wt-1104/kernel/src/arena/control_text.rs:105). Apple still supplies the same field font as `styles.button`, with no control-size selection. On this Mac, regular is 13pt and mini is 9pt: a mini button with `width="10em"` therefore lays out at 130pt while drawing 9pt text. **Fix:** supply and select the actual button font per control size before resolving relative lengths; invalidate those lengths when size changes.

6. **Existing custom menu rows lose their second text — material.** [kernel/src/arena/button.rs:71](/Users/ccheever/projects/exact2-wt-1104/kernel/src/arena/button.rs:71). Changing shared `PressFace.fits` to admit two texts also changes custom buttons. A menu row containing “Account” and “alice@example.com” previously fell through to concatenated text; now menu projections accept the face and use `shown`, which returns only the title. **Fix:** preserve existing custom projection behavior or explicitly handle subtitles in each consumer.

7. **tvOS focus ownership changes leave node-based bookkeeping behind — material.** [RemoteTVOS.swift:21](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/RemoteTVOS.swift:21). Native button nodes now return false from `canBecomeFocused`, but `focusReturn`, remembered focus guides, and `holdsFocusableNode` still test that node property. Replacing a focused native button loses restoration; a scroller containing only native buttons incorrectly becomes an additional focus stop. **Fix:** make these paths resolve the same native focus owner.

8. **Disabled web/Linux buttons still dim authored accents — material.** [host/web/index.html:86](/Users/ccheever/projects/exact2-wt-1104/host/web/index.html:86), [host/linux/src/paint/button.rs:251](/Users/ccheever/projects/exact2-wt-1104/host/linux/src/paint/button.rs:251). A disabled filled button with explicit `accent-color="#ff0000"` receives a 45% fill on both hosts. D16 requires authored colors to survive, with authored opacity controlling dimming. **Fix:** distinguish authored accents from platform defaults when applying the disabled look.

9. **UIKit silently drops admitted padding/radius values — material.** [ButtonConfigurationIOS.swift:100](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/ButtonConfigurationIOS.swift:100). Lowering admits percentage/calc padding and individual corner radii, but UIKit reads only numeric padding and the top-left radius. `padding-left="10%"` retains factory padding; `border-top-right-radius=18` changes nothing. **Fix:** resolve supported dimensions before configuration and refuse or explicitly report shapes UIKit cannot represent.

10. **Empty invoker targets suppress Linux activation — material.** [host/linux/src/navigation.rs:22](/Users/ccheever/projects/exact2-wt-1104/host/linux/src/navigation.rs:22), [host/linux/src/host.rs:839](/Users/ccheever/projects/exact2-wt-1104/host/linux/src/host.rs:839). Newly admitted native buttons with `popovertarget=""` or `commandfor=""` hit unsupported-presentation refusals before their press handler runs. D13 defines an empty target as no target. **Fix:** ignore empty targets before applying Linux’s presentation refusal.

**What is right**

The shared face serialization, synchronous main-thread measurement, copied FFI payloads, and scoped probe lifetime are good foundations. Bare buttons remain the default.

`boot`, `caps`, and `git diff --check` pass. Runtime suites were not rebuilt in the read-only checkout.
