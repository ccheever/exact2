# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `828e32f657`.
- **Method:** one brief per round (sha256 `26cdeb279ea9452fa440fced4fee9d0c8cafb60acc2b9303fa75606a38d52935`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no findings (would keep the short spelling; writing the full name beside it would add a second line)
- **Disposition:** the respell renames in place, so no second line

No findings.

`Described.library` matches the build's short-name check for every param `set-param` can resolve. `described` sets that flag from `file.library.contains(identity)` (`contract/lower/src/anatomy.rs:745`), and `param_names` requires the same identity (`contract/cli/src/tamber_ops.rs:183`). `file.library` is `library::read`: entry components plus re-exports, including `use X from "./x.contract"`, under `home()#DeclaredName` (an alias does not change the identity). The loader stamps that same string before scope (`contract/cli/src/sources.rs:731`). A design-declared component (`component` in a `.tamber`) is in neither set: `library::read` only walks the contract entry, and `param_lines` refuses a resolved short name only when `file.library` contains `p.identity` (`anatomy.rs:487`). `Map.pin` stays short, and the build accepts it.

New library lines go out in full on every path that writes a design `params` line. `when` lines sit under that rewritten name. A second op in the same transaction takes `earlier`, which the first op recorded as `p.full`. Scrub frames and the resting transaction both call `param_names`. `rename-token` rewrites param values only. `set-row`, `add-param`, and promote write a component's own `params` section. Import does not emit `set-param`.

Accepting `short` and writing `p.full` is the right contract. `describe`'s `short` is still the unambiguous address (`null` only when two components share the name). The file spelling is the server's.

The test fails without the fix: the line would be `Card.inset = 20px`, so both the spelling assert and `compile_path` fail. `propose` and `transaction` diffs are unified diffs of that file text, so their equality checks the spelling, and the follow-up set checks the same line is updated once.

An existing short line keeps its spelling, including an unset and set in one transaction (`line` is the snapshot, and the unset stores that spelling in `earlier`). That should stay. The editor finds the line by the name string; forcing `p.full` while the short line is still there would add a second line and `tamber-param-twice` would refuse the write. The file is already unbuildable. A later unset, then a set in its own transaction, writes the full name.
