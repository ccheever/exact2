# Code review: role overrides go live (Tamber stage 5, commit 2), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `71d3a11e3cb6539c3a7a34fea239edadcde8b58dcfe27f11418ffb01eca0c4c6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (web events unbound; iOS rich subtitles; hatch title attributes).
- **Disposition:** in the second-round commit.

---

Three findings remain; **P1 blocks approval**.

- **P1 — Ordinary web events still resolve roles unbound.** [host/web/src/host.rs:903](/tmp/rv-s5c2-2/host/web/src/host.rs:903): boot two hosts on one thread, then dispatch an event touching a coloured text node. The runner’s binding ends before `batch_from → view_css → scheme_color`, so `roles::bound()` asserts in debug builds. The new binding in `apply_live` covers only live edits. Bind the common receipt-processing path.

- **P2 — Existing rich navigation subtitles retain their old colour.** [NavigationBarIOS.swift:516](/tmp/rv-s5c2-2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:516): with a pressable title and subtitle, edit `secondary-label` live. `retheme()` updates navigation-bar attributes, but `HeaderTitleView` captured its subtitle colour at construction; unchanged title content skips rebuilding its attributed text. The subtitle stays the old colour until recreated.

- **P2 — Retheming erases appearance set through a native hatch.** [NavigationBarIOS.swift:509](/tmp/rv-s5c2-2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:509): with a `CanvasText` override and a hatch-supplied title font, editing even `AccentColor` replaces both title-attribute dictionaries with only `.foregroundColor`. The custom font, spacing and colour disappear, and the construction-only hatch does not restore them.

The round-1 fixes otherwise look sound, including in-place mutation, borrow lifetimes, keep ordering and generation comparisons.

Four JS tests passed; stripped release `modes.js` is byte-identical to `HEAD~2`. Coverage misses the scenarios above: Apple tests inspect batch JSON, the scope regression checks one pre-existing scope, and the prerendering assertion checks adoption metadata rather than actual adoption. Rust/UIKit execution was unavailable in the read-only sandbox.
