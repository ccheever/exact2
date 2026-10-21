# Review: LLP 1104 step 3 fix delta (4e18bc622..f9c24e8f2), 2026-10-07 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, at `f9c24e8f2`.
- **Method:** a delta brief (sha256 `e163d1d7c56679b667181723d2813587638aeb6ecbc45cd8915c6d254044ae3d`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** new findings 1–7, the partial Astra 2 and 7, and the WebKit bound went to fix lanes BF4k (5, 6, the bound) and BF4a (1, 2, 3, 4, 7).

---

**Verdict: DO NOT LAND.**

**Earlier findings**

- Astra 1 — fixed; `host/apple/src/control_text_tests.rs` exercises live `env()` padding and identical measurement/presentation payloads.
- Astra 2 — partly fixed; outside dismissal and native hide controls work. Custom hide controls remain broken. The test directly calls the recognizer, without dispatching real touches.
- Astra 3 — fixed; `NativeButtonsIOSTests.swift` checks light/dark reconfiguration and title color; it does not verify symbol pixels or contrast changes.
- Astra 4 — fixed; the JS emitter test and conformance visibility toggles exercise bound display.
- Astra 5 — fixed; kernel tests exercise per-size `em` resolution, size changes, clearing and environment changes.
- Astra 6 — fixed; the kernel test exercises the restored projection fallback, but does not invoke an actual menu.
- Astra 7 — partly fixed; focus-owner lookup and scroller detection work. Replacement focus-guide restoration still fails; its test omits that assertion.
- Astra 8 — fixed; browser computed-color and Linux pixel tests exercise authored disabled fills.
- Astra 9 — fixed; payload tests exercise percentage/calc padding; UIKit tests verify reporting of nonuniform-radius stand-ins.
- Astra 10 — fixed; Linux tests activate buttons with both kinds of empty target.
- Grok 1 — fixed; browser tests check native wrapping and alignment under an opposing ancestor style.
- Grok 2 — fixed; browser tests distinguish ordinary wrapping, nowrap ellipsis and positive clamps.
- Grok 3 — fixed; the unit test checks emitted CSS, but neither added test measures the resulting gap.
- Grok 4 — partly fixed; platform-based `em` avoids double scaling, but an authored absolute font followed by child `em` now loses Dynamic Type scaling.
- Grok 5 — fixed; projection eligibility rejects subtitles again. Added coverage is kernel-only.
- Grok 6 — fixed; the recognizer test exercises outside dismissal directly, without real UIKit touch dispatch.
- Grok 7 — fixed for repeated measurement/dirtying; kernel tests count hook calls. They do not cover the new memory cost or trait invalidation.
- Grok 8 — fixed for initial absence; the new test misses—and effectively endorses—the stale-size behavior when an authored size disappears.
- Grok 9 — fixed; the Linux assertion now pins medium-size width and height to text metrics and padding.
- Grok 10 — fixed; the shared-payload tests exercise percentage/calc conversion before UIKit receives insets.

**New findings**

1. **Clearing macOS control size preserves the previous size — P2.** [ButtonConfigurationMac.swift:64](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/ButtonConfigurationMac.swift:64). Switch a native button from a class specifying `large` to one without control size: the retained `NSButton` stays large, while the kernel and fresh measurement probe revert to the default. Restore the pristine platform value on an authored-to-absent transition; test that transition.

2. **Child `em` incorrectly suppresses Dynamic Type — P2.** [button.rs:99](/Users/ccheever/projects/exact2-wt-1104/host/apple/src/button.rs:99). With button `font-size=13` and title `font-size="1em"`, the kernel resolves the child to the unscaled authored 13, yet marks it already scaled. UIKit consequently leaves it at 13 under accessibility sizing. Track whether the actual font basis was scaled, rather than treating every `em` as scaled.

3. **tvOS guides resolve replacement controls before they exist — P2.** [RemoteTVOS.swift:203](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/RemoteTVOS.swift:203). `Presenter.apply` calls `focusGuides.sync()` before `controls.sync()`. A replacement node with the remembered `testId` therefore has no native focus owner yet, so the guide falls back to its container and stays there. Synchronize guides after controls; assert the guide targets the replacement immediately after the batch.

4. **Custom hide buttons remain agent-only — P2; Astra 2 remainder.** [MenusIOS.swift:247](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:247). Open a content popover with a native button, then tap a bare `popovertargetaction="hide"` button inside it. Light dismissal preserves inside touches, `invokeNative` rejects the bare button, and its hide handling remains behind `agentMode`. Route production hide activation through shared handling and test a custom closer.

5. **Kernel measurement reuse ignores platform trait changes — P2.** [buttons.rs:95](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout/buttons.rs:95). A display-scale change with unchanged fonts leaves `ButtonInputs` unchanged. `set_control_fonts` requests layout, but the kernel reuses its answers without consulting the host’s scale-keyed cache, retaining geometry rounded for the previous display. Include a host measurement revision in invalidation and test a scale-only change.

6. **Each cached offer duplicates the entire button face — P2.** [buttons.rs:8](/Users/ccheever/projects/exact2-wt-1104/kernel/src/layout/buttons.rs:8). Sixteen offers retain sixteen complete `ButtonMeasureRequest`s, containing 64 full style records per button, plus the base request and snapshots. Resizing populated button collections can retain roughly 80 KB of style storage per button. Keep one face and compact per-offer geometry keys; test allocation growth.

7. **The fidelity test no longer verifies presented fidelity — P2.** [NativeButtonFidelityIOSTests.swift:404](/Users/ccheever/projects/exact2-wt-1104/host/apple/tests/ExactKitTests/NativeButtonFidelityIOSTests.swift:404). The replacement compares a fresh measurement probe with `sizeThatFits`; it removes both pixel comparison and equality against the presented button’s height. Incorrect rendered colors, corners or actual bounds can now pass. Restore those assertions alongside the measurement check.

The WebKit exception is narrowly scoped and its UA-height rationale is reasonable. Its actual bound is **below 7 px**, including 6.99—not the stated 6 px; tighten the regex or correct the explanation.

`boot`, `caps` and `diff --check` pass. Added tests were inspected, not executed: available binaries predate the delta. No additional main-thread or lifetime violation was identified.
