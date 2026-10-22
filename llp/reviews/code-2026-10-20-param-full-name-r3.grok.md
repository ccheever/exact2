# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `9725e28c85`.
- **Method:** one brief per round (sha256 `3dfedcfb6f4b58d8476694bfffa97791957c4d33c8d9fc7746b8c7990ed9ae96`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings
- **Disposition:** none needed

No findings.

The respell is consistent with the build rule and with undo. `param_names` writes an exported library param as `p.full` and sets `respell` only when the spelling it will edit is not already that full name. `set_param` renames that line in place, then sets the value or the `when` line. The inverse is still the ordinary line `restore`, so undo puts the old bytes back, including a short line, its comment, and its `when` lines.

`earlier.or(line)` does not change non-library params. A non-library op records the snapshot line when there is one, and that string is what the next op reads back, so preferring the earlier spelling over the snapshot line is the same result. `rfind` matters once a library unset has recorded the short name and a later set in the same transaction has recorded the full name: the next op follows the full name. A set then an unset looks the line up under the respelled name after the rename. An unset then a set still carries `respell: Some(short)`, finds no line, and writes a new full one.

`respell` with no line is unused. The lookup misses, the rename is skipped, and the new line is `name`, which is already `p.full`.

The slice is safe on every line the params parser keeps. Indent is removed before the name is stored and before `parts` returns `code`. A trailing comment is split off and put back. A `name: kind` line is rejected (`kind.is_some()` returns no token, and the file does not parse), so `set_param` never slices it. An accepted name is `lhs.trim()` with no `:`, and that string is a prefix of the trimmed code, so `code[name.len()..]` stays on a character boundary.

Scrub frames call the same `param_names` against the base snapshot, so each frame respells from the file the gesture opened on. Import's `op_json` still emits `name`, `when`, and `value`; `parse_ops` sets `respell: None` and `param_names` fills it again. Import does not emit `set-param`.

The new test covers a new full line, a later short-name set, a short line with a comment and a `when` line (and its undo), and unset plus set in one transaction. A set of only the `when` line, and a set followed by an unset in one transaction, are not in that test; both follow the same rename and `restore` paths.
