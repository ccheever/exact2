# Code review: LLP 1121 stage 3, commit 2 (edit operations and inverses), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `885dee64ffe5a0eccb48f38867ec1b3b6eedef2396a2d94e2bc3abf8570e10e5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five high, four medium, two low; Grok: three high, one medium, three low.
- **Disposition:** fixed in round 1 (the inverse became a restore of the changed lines)

---

## High

**1. A removed token or `when` line is put back against the next anchor, so comments and blank lines in between move or disappear — `tamber/src/edit.rs:253`**

`SetToken` inserts a new `when` immediately after the token line (`at` 0) or the previous `when` (`edit.rs:253-258`). `UnsetTokenWhen` records only that index (`edit.rs:274-280`). `tamber fmt` keeps a comment or a blank line in that gap (`tamber/src/fmt.rs:7-9`, and the stable fixture in `tamber/tests/fmt.rs:82-86`):

```text
  ink = #ffffff
    // darker
    when tone=hard: #000000
```

Undoing the unset writes the `when` above `// darker`. The same thing happens with a blank line between the token and the `when`, which fmt also keeps.

`RemoveToken` has the same hole in two places (`edit.rs:349-359`):

- Lines inside the drained span that are not the token line, a `when` line, or a trailing `//` comment are deleted and not stored. A `// darker` between `ink` and its `when` is gone after undo.
- Anything between that span and the next token stays put, and the inverse `AddToken` inserts flush against `before` (or at `block_end`). On the formatted clone, `scrim` is followed by a blank line and the "Pairs that are the same in Dim and Dark" comments (`tamber/tests/fmt/bluesky-clone.tamber:108-113`). Undo places `scrim` under those comments, not above the blank line. The edit test only removes `contrast_25_light`, the last token, which has nothing after it (`tamber/tests/edit.rs:152-157`), so the gate never sees this.

Store the raw gap. For a `when`, store how many physical lines lie between it and the token or the previous `when`, and insert that far down, not flush after the anchor. For a token, keep the in-span comment and blank lines in the inverse, and insert that many lines before the next token or the block end, not against the anchor.

**2. Undoing the first `add-token` on a design with no `theme` block leaves an empty `theme` header — `tamber/src/edit.rs:322`**

If `block_end("theme")` is `None`, the operation pops trailing blank lines, appends a blank line and `theme`, and forces a trailing newline (`edit.rs:324-332`). The inverse is only `RemoveToken` (`edit.rs:338`), which deletes the token lines and leaves:

```text
tamber t

axes
  tone = soft | hard

theme
```

A formatted axes-only file is legal, and an importer's first `add-token` is this path. Remember that this call created the block, and have that inverse remove the header and the blank line it added.

**3. `rename-token` replaces whole words, and only one spelling — `tamber/src/edit.rs:383`**

`rename_words` (`edit.rs:518-553`) walks `theme` and `exports` and replaces any word bounded by characters other than a letter, digit, `-`, `_`, or `.`. The checker does not resolve names that way.

- A role override answers to several spellings (`tamber/src/check.rs:437-440`): `-exact-secondary-label`, `secondary-label`, and any case. The rename looks for the declared text only, and `is_name` rejects a role spelling as the new name (`edit.rs:363-367`), so the new name cannot be a role either. After renaming the declaration to `muted`, a value that still says `secondary-label` checks as the platform role, not as `muted`. The edit is accepted and the color changes. Exports written with the other spelling fail `tamber-unknown-token` instead, so a legal rename is refused.
- The same whole-word match rewrites non-references in the theme block: `when dark-style=black`, and `scheme=fixed` if the token is named `dark-style`, `black`, `scheme`, or `fixed`. That is either a false refusal or a silent change to the condition or the mark.
- Quotes do not follow the lexer. A `\` does not escape, so `label="Lift \"up\""` is left early and `up` is renamed inside the string. An apostrophe opens a string; `lex` does not treat `People's` as one.

`a.b` versus `a.bc` is handled, because `.` is a name character. Dial names are not rewritten: dials live in another block, and a token may not reuse a dial's name. The axes block is skipped, so the axis line itself is safe.

Replace only idents the fold would resolve as that token, including every role spelling and case, in value position, the declaration, and exports. Leave `when` conditions, metadata, function names, and strings alone. Allow a role spelling as the new name.

## Medium

**4. The inverse re-serializes metadata and drops quoting — `tamber/src/edit.rs:505`**

`RemoveToken` stores `meta_word` of the parsed, unquoted value (`edit.rs:355`). `meta_word` (`edit.rs:505-512`) writes quotes only when a character is outside `A-Za-z0-9-_.%#`. Fmt keeps metadata as written (`tamber/src/fmt.rs:256-261`). A formatted `label="Accent"` or `step="0.1"` comes back as `label=Accent` and `step=0.1`. A value the lexer decoded (`label="a\nb"`, `label="Lift \"up\""` is fine; a newline is not) is written with a raw newline, so the undo splits the line and can fail to parse. `scheme=fixed` on the clone survives only because that value needs no quotes.

Keep the metadata words from the source line, and put those strings back on the inverse `add-token`.

## Low

**5. An edited line loses its CR — `tamber/src/edit.rs:171`**

`Doc` splits on `\n` and keeps a `\r` on the line. `parts` strips that `\r` (`edit.rs:171`) and `line` writes the line back without it (`edit.rs:475-482`). One edited line in a CRLF file becomes LF, and the inverse writes that same LF line, so the bytes do not come back. Formatted files are LF (`tamber/tests/fmt.rs:100-104`). Rename does not go through `parts`, so it keeps the `\r`.

Strip `\r` when splitting, and join with `\r\n` when the file used CRLF.

**6. Every `tamber-edit` diagnostic is `1:1` — `tamber/src/edit.rs:140`**

`refuse` always uses line 1, column 1. "No such token" has nowhere else to point. "A token is declared already", "no such `when`", and "the axis already has this value" all name a line the design already has, and the checker's diagnostics for a refused result do carry that line. Point those at the existing dial, token, `when`, or axis.

**7. `when` conditions are anonymous tuples — `tamber/src/edit.rs:23`**

`WhenLine` is `(Vec<(String, String)>, String, Option<String>)`, and `SetToken.when` is `Vec<(String, String)>`. A JSON wire for commit 3 would be arrays with no `axis`, `value`, or `comment` fields. `at` and `comment` on `SetToken` are the right extra fields for a replayable inverse. Give conditions a named struct before that wire freezes.

## Checked, no defect

- `apply_all` reparses and builds a new `Doc` before each operation (`edit.rs:128-132`), so `parts` does not read a stale line after an earlier insert.
- `same_conds` (`edit.rs:500-502`) treats a `when` as the same in any order. The inverse stores the file's order, and applying it writes that order back.
- On a formatted file, a new `when` indents at the sibling `when`'s indent, or two spaces under the token. That matches fmt.
- `add-token` at the end of an existing `theme` includes indented comments in the block and inserts after them (`edit.rs:446-460`). Undo removes only the lines it added, so the bytes come back. A top-level comment is treated as the next block and stays after the new token.
- Removing a token another token or an export names is refused by `check` (`tamber-unknown-token`), and `apply` returns those diagnostics. A role override is different: after it is gone, a use spelled as the role is still a legal role, so the checker accepts it and the use follows the platform again.
- `add-axis-value` that would pass 64 tuples is refused by the checker's `tamber-tuples` cap. No separate edit cap is missing.
- Dial names, the `a.b` / `a.bc` boundary, and ordinary double-quoted strings are not rewritten.

The edit tests never hit these. `round_trip` uses the clone and one token whose `when` is flush against it. The dial test appends a space that `set-dial` trims off (`tamber/tests/edit.rs:197`), so the value does not change. Nothing covers a comment or blank between a token and its `when`, `remove-token` of `scrim` on the clone, a design with no `theme`, quoted metadata, a role rename, `apply_all`, or CRLF.
