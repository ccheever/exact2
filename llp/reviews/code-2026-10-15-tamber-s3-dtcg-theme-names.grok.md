# Code review: DTCG: a Tokens Studio theme group with a nameless or repeated theme is refused, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `2baa503579b27c28b5ce0bb6583c818f6d441196f306f512c831216edf4fd8f4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (both).
- **Disposition:** landed as reviewed; Astra's optional test additions not taken

---

I'll review the HEAD commit read-only, starting with the change itself and the Tokens Studio theme-group checks you listed.The request is a read-only review of one commit with a specific checklist, so I’ll follow that review procedure and inspect the change before judging it.Nothing blocking. No findings.

The new check in `studio` refuses a multi-theme group before it assigns a mode, and that refusal is what `import` returns.

1. **Both group kinds, and the ungrouped group.** At `tamber/src/import/dtcg.rs:550-560` the check runs for every group with more than one theme, before the scope match at `:563-566`. A blank or repeated name is refused whether that group would have been the scheme (`scheme` is light/dark after trim and ASCII case-fold, `:370-377`), a proposed axis (`out.axis`), or the ungrouped bucket. A missing `group` and `""` are the same bucket (`:532`); an empty bucket is reported as `the ungrouped themes` (`:545-547`). A real light/dark pair (`Light`/`Dark`, including surrounding whitespace) is neither blank nor an exact repeat, so it still becomes the scheme.

2. **`ident()` folding and whitespace-only names.** Exact repeats are refused here, on the raw string (`:554-559`). Names that differ as written and meet only after `ident()` still go through `axis` (`:100-119`), which records every spelling in `written`; `import` then refuses them as `tamber-import-axis` (`:1040-1054`). That is the right split: identical raw names would be one `BTreeSet` entry and would fold quietly, which is the case this check adds. Whitespace-only names are covered by `trim().is_empty()` (`:551`), so `" "` in a group of several is "a theme with no name". A single whitespace-only theme stays the base, same as a theme with no name.

3. **A single theme stays the base.** The new check is under `members.len() > 1`. One theme still takes `(1, _) => Scope::Base` (`:564`), named or not.

4. **The importer diagnostic.** `studio` returns `Err(String)`, `read` returns that (`:437`), and `import` maps it to one `Diagnostic` with id `tamber-import-source` (`:1034-1035`). The test reads `import(...).err()` and that diagnostic's message (`tamber/tests/import.rs:871-878`).

5. **The test.** `dtcg_refuses_a_theme_group_with_a_nameless_or_repeated_theme` (`tamber/tests/import.rs:864-894`) locks a nameless pair in group `g` and a repeated name in the ungrouped bucket, and locks that one unnamed theme and two distinct names still import. The success cases only require `Ok`. The lone theme being the base is the unchanged `Scope::Base` arm, not an assertion on the imported text.

6. **Nothing else.** A non-string `name` becomes `""` and is refused in a group of several. The first bad group fails the import; later groups are not applied.
