# Code review: Bindings of lengths, numbers and fonts, and the DTCG export of every kind (LLP 1121 stage 6 G), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~3..HEAD, the export, the bindings, the sample host and `prefer bold-text`, and the first round.
- **Method:** one brief (sha256 `ce340d2773c7d4e692f16169797a31528095b92dbb8e35c9f33287ee2cccd1a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (P1s: a reprinted type size losing its text style, a module's dotted system faces drawn proportional; P2s on a leading placeholder and a variable face's weight; a P3 on a shadow's colour reference).
- **Disposition:** applied without re-review (the last round), in the second-round commit.

---

I'll review commit G read-only: the round-1 fixes first, then the four commits from the DTCG export through HEAD, against the RFC. No worktree or index changes.The review stays in this process and read-only. I'll start from the round-1 commit message and the RFC sections those fixes claim to satisfy.Round 1 is in `export.rs`, `export_kinds.rs`, and the binding inputs. I'll read those paths and the host bindings next.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll confirm the round-1 fixes and the two suspected defects against the current source, then report only what the lines support.[P1] A reprinted lossy type size is dropped as soon as any other field is edited — `tamber/src/export_kinds.rs:316`

`read` keeps `$extensions.exact.lossy` (a text style, an `em`, the hairline) only when `v["fontSize"] == base["fontSize"]`. serde_json does not treat `17.0` and `17` as equal, while `alike` does. `same` therefore treats a pure `JSON.stringify` reprint as unchanged, which is what `reprints_unsaid_parts_scheme_shadows_and_units_read_back` checks. That test edits `fontWeight` on the original document, where `fontSize` is still a float, so the keep succeeds. Reprint and edit together — the file Tokens Studio writes back, integers plus `fontWeight: 600` — enters `read`, the `==` fails, and `type.body = -exact-body/1.35 Inter` is proposed as `600 17px/1.35 Inter`. The tuple check accepts it because `canonical` folds `17px`. The size no longer follows Dynamic Type.

[P1] The native module draws every dotted system face as the proportional system font — `host/apple/modules/ExactTamber.swift:103`

`NativeHatches.hatchTamber` (`host/apple/Sources/ExactKit/NativeHatches.swift:193`) sets `system` when `fontName` has prefix `.` or the family number is nil, and `font` then skips the name and calls `systemFont(ofSize:weight:)`. On this Mac the host's own resolution is `.AppleSystemUIFontMonospaced-Semibold` for `ui-monospace` (opens by name and keeps the weight), `.NewYork-Regular` for `ui-serif` (does not open), and `.AppleSystemUIFontRounded-Regular` for `ui-rounded`. All three start with `.`, so a module label bound to `type.code = 14px ui-monospace` draws the proportional system UI face. In-process `TextEngine.font` (`host/apple/Sources/ExactKit/Text.swift:733`) uses `monospacedSystemFont` and `withDesign`. The sample token is `system-ui` and `scripts/smoke-tamber.mjs` only checks that face, so the smoke stays green.

[P2] A non-number line height edited in the placeholder is ignored — `tamber/src/export_kinds.rs:319`

A leading that is not a bare number (`1.5rem`, `24px`) is exported as `lineHeight: 1.2` plus `exact.leading`. On the way back, a said line height with that extension always emits the old source. `type.x = 16px/1.5rem Inter`, with a tool setting `lineHeight` to `1.4`, comes back as `16px/1.5rem Inter`. When that is the only edit, `edited.text == current` (`export_kinds.rs:492`) proposes nothing and the edit vanishes. A weight edit on the same token applies, and the line-height edit is still dropped; the tuple check passes because `want` still has `1.5rem`. Changing only the extension string never enters re-import: `changed` compares `$value`.

[P2] A bundled variable face opened by PostScript name drops the weight the host set — `host/apple/modules/ExactTamber.swift:103`

For `system != true` the library returns `ExactTamberFont(name:size:)` and never reads `weight` or `italic`. The hatch resolved the face with `session.text.font`, which sets the `wght` axis in `declaredFace` (`host/apple/Sources/ExactKit/FontFaces.swift:21`). That axis is not in the JSON. A variable file has one PostScript name, so a module bound to that family at 700 draws the file's default instance. A static face, whose PostScript name already names the weight, is unaffected.

[P3] Editing any shadow field rewrites a color reference as a literal — `tamber/src/export_kinds.rs:70`

Shadow export stores the folded color object and no `$extensions.exact.source`. `0 1px 2px brand` with its blur changed to `6px` is proposed with the folded hex, so a later change to `brand` leaves the shadow behind.

The round-1 fixes for `same`/`alike`, the extension fallback, an unsaid part becoming said, `shadow_sides`, `canonical` (`0.2s` → `200ms`), `inputs` (place font size and device pixel), and an `em` leading on the token's own size hold for the cases they cover. Bold Text, the Swift and Kotlin caches, and the sample smoke show no further defect. The two P1s are blocking.
