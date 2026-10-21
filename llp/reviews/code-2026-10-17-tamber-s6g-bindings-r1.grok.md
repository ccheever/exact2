# Code review: Bindings of lengths, numbers and fonts, and the DTCG export of every kind (LLP 1121 stage 6 G), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the export and the bindings.
- **Method:** one brief (sha256 `2c265923b543f12b10432c2a771874a79c882759c057d3dc0907edea1d0dccdf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (a P1: a reprinted number replacing a lossy source; P2s on scheme shadows, extensions taken from the baseline, cached lengths).
- **Disposition:** in the first-round commit (the system font in the sample-host commit before it).

---

I'll review HEAD~1..HEAD read-only against the stage 6 commit G spec, starting with the review instructions and the two commits.The review stays read-only and covers both commits against D12, D14, D6, and the stage 6 G brief. I'll pull the diffs and the spec next.The as-built section is in place. Next I’ll read the export and the host bindings against the round-trip and resolution rules.The export’s read-back path looks uneven. I’ll check vocabulary sizes and the binding code against the cases the brief calls out.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open. I'll confirm the remaining export and binding hypotheses against the spec and the unread call sites, then report only defects I can pin to a line and a failure.The skill is a defect-first read of the two commits. Next I’ll pin the export read-back and the binding cache paths to the lines that would actually fail.The byte-identical export still re-imports with no operations. Reprinted numbers and a few binding inputs do not.

[P1] A reprinted number is treated as an edit and replaces a lossy source — `tamber/src/export.rs:638`

`same` is JSON equality except for colours. `serde_json` 1.0.151 does not treat `16` and `16.0` as equal (`number.rs:38`). DTCG tools reserialize whole floats as integers (`JSON.stringify(16.0)` is `16`). The protocol says a reprint changes nothing. For a non-colour it marks the token changed, and the read path then drops what the export had marked `lossy`:

- A length ignores `exact.lossy` (`export_kinds.rs:232`). `-exact-title1` exported as `27.0px` comes back as `27px`.
- A type keeps its text style only when `fontSize` is JSON-equal (`export_kinds.rs:314`), so the same reprint drops it.
- An easing is always read as `cubic-bezier(...)` (`export_kinds.rs:286`). `ease-out`, and a `steps()` / spring stored as `linear`'s curve, become that cubic bezier.

`alike` (`export_kinds.rs:500`) then accepts the proposal, because `27` and `27.0` match numerically. The unedited byte-for-byte file still proposes nothing; a reserialize of the same file does not.

[P2] A shadow whose colours differ by scheme cannot be edited — `tamber/src/export_kinds.rs:397`

`shadows` writes `colour_side` per scheme (`export_kinds.rs:64`), so `0 1px 2px light-dark(#00000033, #ffffff22)` or a role such as `CanvasText` is two different `$value`s. Re-import of an unedited file still proposes nothing. Change the blur in both contexts and re-import stops with "a token of this kind has one value in light and dark" before any line is proposed. The design has one line that could take the new lengths; the edit is discarded.

[P2] Typography extensions are taken from the baseline, not the file — `tamber/src/export_kinds.rs:301`

`read` restores `unsaid`, `fontStyle`, and `leading` from the baseline `exact`. The incoming closure only passes `$type` and `$value` (`export.rs:886`). `fontStyle` is not in `$value`, so deleting italic proposes nothing and the italic comes back. On `type.body`, which leaves the weight unsaid and exports `fontWeight: 400`, setting that weight to `600` is ignored because the baseline still lists it unsaid; the apply check then fails ("no lines this design takes give every context its value") and the token is unmapped.

[P2] Cached lengths miss the em reference and the display scale — `runner/src/runner/tamber.rs:382`

`points` resolves `em` from the node's font size and the hairline from `device_pixel()` (`tamber.rs:260`). `inputs` hashes the tuple, design edits, scheme, contrast, root size, and `preferences.bold_text`, not those two. Hosts refresh only when that string changes (`Tamber.swift:333`, `Tamber.kt:115`). Move the window from a 2x display to a 3x display: `set_display_scale` commits layout hairlines to one third of a point, and `ExactTamber.length` for `-exact-hairline` stays at the cached half point. The same stickiness hits `1em` after a font-size commit that is not a token edit, a root-size change, or a mode change. Bold Text and Dynamic Type do move `inputs` on the host path (`set_preferences` / root size).

Nothing else in this range is blocking. Tuple selection, live edits, Bold Text on a named family versus `system-ui`, the native-module `{size, name}` font, and the per-kind unions (exports-bounded, synthetic names dropped) match the as-built notes. The round-trip test only asserts a byte-identical file.
