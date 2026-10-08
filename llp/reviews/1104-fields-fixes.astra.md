# Review: LLP 1104 steps 1–2 fix delta (aa2f4e057..ed36af6d0), 2026-10-07 (astra)

- **Family:** OpenAI — `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one session, at `ed36af6d0`.
- **Method:** a delta brief (sha256 `16419c08d08f917ff1f124b51e71bc65dfd3b20ee22e99a26addb4f12e15b12d`) asking per earlier finding whether it is fixed and whether the fixes add bugs; blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** the partly fixed items (Astra 8, Grok 1, Grok 5) and new findings 1–3 went to fix lane F4 (b8ba774dc).

---

**Verdict: LAND WITH FIXES.**

**Earlier findings**

- Astra 1 — fixed; [NativeFieldsIOSTests.swift:15](/Users/ccheever/projects/exact2-wt-1104/host/apple/tests/ExactKitTests/NativeFieldsIOSTests.swift:15) exercises selected-plan launch and replacement without fields. No tvOS test.
- Astra 2 — fixed; [native_fields.rs:559](/Users/ccheever/projects/exact2-wt-1104/kernel/tests/it/native_fields.rs:559) changes only the chrome minimum, including shrinking it again.
- Astra 3 — fixed; [region tests:208](/Users/ccheever/projects/exact2-wt-1104/kernel/tests/it/content_region/native_fields.rs:208) exercise cold pending fields; the following test covers candidate and retained provisional flags. Neither tests Apple presentation.
- Astra 4 — fixed; [Linux test:141](/Users/ccheever/projects/exact2-wt-1104/host/linux/tests/it/native_fields.rs:141) checks actual text pixels beneath a white-text ancestor.
- Astra 5 — fixed; [native_fields.rs:584](/Users/ccheever/projects/exact2-wt-1104/kernel/tests/it/native_fields.rs:584) compares hypothetical auto height against an auto-sized textarea and checks restoration.
- Astra 6 — fixed; [native_field_resize.rs:29](/Users/ccheever/projects/exact2-wt-1104/runner/tests/it/native_field_resize.rs:29) exercises chrome, authored padding, and size feedback without growth.
- Astra 7 — fixed; [content_region.rs:720](/Users/ccheever/projects/exact2-wt-1104/host/apple/tests/it/content_region.rs:720) checks emitted `fieldContent`, unchanged-layout suppression, and clearing.
- Astra 8 — partly fixed; [kernel test:622](/Users/ccheever/projects/exact2-wt-1104/kernel/tests/it/native_fields.rs:622) exercises negative centering, but [Linux paint.rs:1078](/Users/ccheever/projects/exact2-wt-1104/host/linux/src/paint.rs:1078) still clamps it. Remove that clamp; the 80-point line in 40-point content paints 20 points below its reported baseline.
- Grok 1 — partly fixed; [web:155](/Users/ccheever/projects/exact2-wt-1104/host/web/tests/focus.test.mjs:155), [Linux:132](/Users/ccheever/projects/exact2-wt-1104/host/linux/tests/it/native_fields.rs:132), and [macOS:151](/Users/ccheever/projects/exact2-wt-1104/host/apple/tests/ExactKitTests/NativeFieldsMacTests.swift:151) cover the restored paths, but macOS covers inputs only. Bare `TextArea` still returns its superclass’s empty mask at [TextAreaMac.swift:25](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:25); forward the node’s mask from the actual first responder. [AppKit focus-ring contract](https://developer.apple.com/library/archive/qa/qa1785/_index.html).
- Grok 2 — fixed; [focus.test.mjs:155](/Users/ccheever/projects/exact2-wt-1104/host/web/tests/focus.test.mjs:155) compares disabled input/textarea colors against UA controls in both schemes and checks authored color.
- Grok 3 — fixed for the reported rows; [native_fields.rs:650](/Users/ccheever/projects/exact2-wt-1104/kernel/tests/it/native_fields.rs:650) exercises resets, inheritance, overrides, and clearing. It misses the relative-font regression below.
- Grok 4 — fixed; [Linux tests:57](/Users/ccheever/projects/exact2-wt-1104/host/linux/tests/it/native_fields.rs:57) exercise typography and disabled paint; [terminal test:110](/Users/ccheever/projects/exact2-wt-1104/host/terminal/tests/cli.rs:110) checks computed typography and rendered colors.
- Grok 5 — partly fixed; [NativeFieldsIOSTests.swift:36](/Users/ccheever/projects/exact2-wt-1104/host/apple/tests/ExactKitTests/NativeFieldsIOSTests.swift:36) exercises counting and preserved operations, but explicitly requires provisional geometry to be applied.

**New findings**

1. **Web native fields resolve relative fonts against 16px — material.** [control_text.rs:91](/Users/ccheever/projects/exact2-wt-1104/kernel/src/arena/control_text.rs:91). Returning `Some(WEB_START)` makes [relative.rs:59](/Users/ccheever/projects/exact2-wt-1104/kernel/src/txn/relative.rs:59) treat the four-row boundary as a control font. Inside `font-size=32`, an input with `font-size="2em"` resolves to 32px instead of 64px; the wasm host emits those incorrect pixels. Root-font invalidation also skips these fields at line 32. **Fix:** test whether the boundary masks `FontSize`, rather than whether it exists; cover relative fonts and root-font changes without platform styles.

2. **iOS now applies geometry from failed provisional layouts — material.** [PresenterIOS.swift:721](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:721). Removing the guard restores counting but also applies every geometry operation. A running host’s three-pass failure preserves and serializes its batch; native-region promotion can already have staged frames before that failure. Thus Rust’s error does not guarantee that provisional geometry cannot reach `apply`. **Fix:** preserve semantic operations while withholding provisional geometry at the publication boundary. Test failure during a running session; replace the assertion that requires the guessed frame to appear.

3. **Textarea restyling allocates a disposable TextKit stack — minor.** [FieldChromeMac.swift:84](/Users/ccheever/projects/exact2-wt-1104/host/apple/Sources/ExactKit/Mac/FieldChromeMac.swift:84). The computed `textareaInset` constructs an `NSTextView` on every access: once per native textarea restyle and twice per chrome measurement. **Fix:** measure once on main and cache the inset with the relevant platform traits.

Repository tests were inspected, not executed. An in-process AppKit probe confirmed `NSTextView` inherits the empty mask implementation. Repository unchanged.
