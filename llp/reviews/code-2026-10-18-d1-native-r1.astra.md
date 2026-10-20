# Code review: LLP 1129 D1 on native hosts, and a formatted live time's width without its fallback, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b17567500`.
- **Method:** one brief (sha256 `91dc0426ebbbd8dac0f0eb9bd08f1c344f74070353b6223b889524bc0f8b2776`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; a nit: the message called native press the kernel's run-time dim (each host has its own press feedback, none picked by a compile-time fill mark).
- **Disposition:** message reworded (88bd78446a); the test is confirmed to discriminate.

Nothing is blocking. One wording issue:

1. **NIT — Correct HEAD’s native-press explanation.** For a custom button with a bound fill, macOS uses a host-side tint over captured layers ([PressLookMac.swift:74](/tmp/rv-d1-1/host/apple/Sources/ExactKit/Mac/PressLookMac.swift:74)); iOS applies a fill-independent opacity dim ([PressFeedback.swift:417](/tmp/rv-d1-1/host/apple/Sources/ExactKit/PressFeedback.swift:417)). Linux/Android’s shared custom press path requires authored scale or paint ([contact.rs:590](/tmp/rv-d1-1/host/linux/src/presenter/contact.rs:590)). These are not a shared “kernel run-time dim.” **Fix:** describe the host-specific feedback and state that none selects it using a compile-time fill mark. The no-bound-fill-gap conclusion remains sound.

The remaining claims check out:

- D1 explicitly scopes its hover to the web ([RFC:63](/tmp/rv-d1-1/llp/1129-what-the-brand-probe-asks.rfc.md:63)). Linux’s filled hover is for platform controls; row washes use fixed neutral colors. Mac custom hover requires an authored look. iOS supports authored hover and UIKit pointer effects, but no D1 default. No examined native path needs the JS bound-fill mark.
- The [test:278](/tmp/rv-d1-1/contract/cli/tests/it/live_time.rs:278) discriminates: `flex-start` prevents equal stretched widths, and the fallback exceeds the relative-time reservation. Including it in measured text would widen the first sample and fail equality. The current [text-run branch:725](/tmp/rv-d1-1/kernel/src/arena.rs:725) excludes children when formatted text exists.
- The CSS scoping and obsolete queue-item removal raise no findings.

Static review only; no files changed, builds, or tests run.
