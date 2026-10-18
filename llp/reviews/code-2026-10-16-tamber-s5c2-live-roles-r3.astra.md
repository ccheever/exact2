# Code review: role overrides go live (Tamber stage 5, commit 2), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `95a94b7776129d3dd55707bbba53778006d73f4792037fa7aaa5351e34bdc703`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (native placeholders; Linux hosted fields unbound; retired bars' inks).
- **Disposition:** applied without re-review (the last round).

---

Two P2 blockers remain; the four round-2 fixes otherwise look sound.

- **P2 — Native placeholders still miss live edits.** [ThemeRoles.swift:119](/tmp/rv-s5c2-3/host/apple/Sources/ExactKit/ThemeRoles.swift:119): change only `-exact-placeholder` while an ordinary native input is visible. Its serialized style stays unchanged, so no style op arrives; tint refresh skips fields without tint-dependent rows. The retained attributed placeholder keeps its old colour on iOS/macOS until another restyle.

- **P2 — Hosted field colours still resolve unbound.** [hosted.rs:223](/tmp/rv-s5c2-3/host/linux/src/presenter/hosted.rs:223): with native field hosting enabled, boot two presenters on one thread, then call `first.reader_fields(...)`. This path reaches role resolution at lines 383–402 without binding the first session. Debug builds assert; release builds use the newest session’s overrides. The binding around `frame()` has already ended.

- **P3 — Retired navigation bars leave cached colours behind.** [NavigationBarIOS.swift:522](/tmp/rv-s5c2-3/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:522): each new bar adds a strong `UIColor` entry to `titleInks`; neither stack retirement nor reset removes it. Repeated modal presentation retains obsolete entries for the session’s lifetime.

The new web test exercises the missing-bind assertion; the Mac test checks the actual painted layer. Neither covers the gaps above. Four JS tests passed, and stripped release `modes.js` is byte-identical to the baseline. The temporary-file release test was sandbox-blocked; its equivalent passed in memory. Rust/XCTest were not run.
