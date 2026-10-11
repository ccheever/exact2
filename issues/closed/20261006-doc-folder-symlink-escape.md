# Keep document-folder operations inside the selected folder

**Status:** Closed
**Resolution:** Document operations are anchored to retained directory handles and refuse descendant symlinks/reparse points; Rust and Hermes regressions cover containment and swaps.
**Systems:** Data documents, Native storage, ibex2 filesystem
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P1
**Related:** LLP 1069.010 D1, data/src/documents.rs, vendor/ibex/crates/ibex2/src/stdlib/fs.rs

A selected folder's `doc:` handle does not constrain filesystem operations to that folder. `documents::resolve` validates the spelling and joins descendants onto the root (data/src/documents.rs:119–139), but does not check links. `run_document` then uses ordinary path-based filesystem operations, which follow them.

Verified through the real `Storage<DataSource>` continuation, not just the resolver: create a temporary `chosen/` folder, a sibling `outside.txt`, and `chosen/link.txt -> ../outside.txt`. Mint a handle for `chosen`; grant only `fs.read doc:/` and `fs.write doc:/`. `fs.readFile(handle + "/link.txt")` returns the sibling's bytes, and `fs.writeFile` changes the sibling to `overwritten-by-doc`. No absolute-path grant or second picker is involved.

This contradicts D1's admission of what the person chose. An ordinary folder containing links can expose or overwrite unrelated files. Directory links also make nested operations reach outside the folder. Canonicalizing the root at mint time does not contain later descendants or eliminate replacement races.

Use operations anchored to the chosen directory's owned handle, with an explicit descendant-link policy and containment enforced during the operation. The repository's owned-directory implementation is a useful starting point; a check followed by an ordinary open is still racy. Preserve support for explicitly choosing a file through a link.

Acceptance: real read, write, append, mkdir and removal tests cover links outside the selected folder, links in intermediate components, and replacement between lookup and execution. In-folder ordinary operations continue to work. Errors retain logical document paths.

Fix verified through activated Rust and Hermes storage continuations. The native table retains the selected directory (or a file’s parent and leaf); Unix uses descriptor-relative operations with `O_NOFOLLOW`/`AT_SYMLINK_NOFOLLOW`, and Windows uses relative NT handles that refuse reparse points. Tests cover all document operations, intermediate links, concurrent swaps, root-path replacement, write-only files, explicit file selections through links, and new saves. The macOS Markdown reader was built and driven to open a normal document. Windows filesystem production code and tests were type-checked in isolation; the full Windows native build requires the unavailable MinGW C compiler, and Windows runtime tests remain unrun.

## Recovery audit (2026-10-08)

Rebased onto current main and repaired two review findings: an explicitly opened file link resolves to its selected target before being placed beneath a folder handle, and selecting a replacement folder at the same pathname no longer reuses the original directory. Directory deduplication compares owned handle identities through `same-file`; regressions cover both cases. Windows runtime qualification remains as stated above.
