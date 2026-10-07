# Round 1
Found two issues. The Linux cleanup gaps predate HEAD~1; clearing `controls` itself is correct.

1. **[P2] Replacement still retains other per-node interaction state** — [presenter.rs:1341](/Users/admin/projects/exact2-wt-ctl/host/linux/src/presenter.rs:1341).

   Neither replacement path clears `edited`, `fields`, `menu`, `pointer_held`/`pointer_buttons`, or `hovered`. Concrete consequences:
   - Type into an unbound field, reload, then blur: the surviving `edited` flag makes [commit_text](/Users/admin/projects/exact2-wt-ctl/host/linux/src/presenter/typing.rs:489) dispatch `change` with the fresh field’s reset value.
   - An old selection survives when the reused ID’s text matches; an open select menu survives and attaches to the reused ID.
   - Pointer-down → reload → pointer-up can deliver the release to a new node: fresh kernels reuse both slot indices and generations, and [pointer_lifted](/Users/admin/projects/exact2-wt-ctl/host/linux/src/presenter/pointer.rs:156) accepts that key. Stale `hovered` IDs likewise suppress enters or misdirect leaves.

   These need retirement alongside checked state. Apple resets these form states, and web recreates the DOM nodes.

2. **[P3] `declared` reads manifests without recording them in `consulted`** — [resolve.rs:206](/Users/admin/projects/exact2-wt-ctl/contract/cli/src/resolve.rs:206).

   Ancestor and workspace-member manifests affect the diagnostic but never enter the watch list. After a failed compile, editing an ancestor declaration or workspace member’s name can leave the dev error unchanged: [Session::poll](/Users/admin/projects/exact2-wt-ctl/host/web/src/dev.rs:133) suppresses recompilation when the source and recorded dependencies are unchanged. Record the attempted manifest paths, including relevant symlink identities, as the existing resolver does.

Otherwise, I found no defects in the requested checks: both production replacement paths call `replaced`; bound checks and radio groups remain correct; surface/game controls use separate state. The declaration walk chooses the nearest matching ancestor, supports workspace object form, tolerates malformed JSON without panicking, follows symlinks, and runs only after all ancestor install locations fail. Its install message is appropriate for the intended fresh-checkout case.

Three existing prebuilt Linux tests passed: reload and controlled/uncontrolled radio groups. No files modified.
# Round 2
One concrete defect:

- **[P2] Ancestor manifests become recursive package-watch roots** — [contract/cli/src/resolve.rs:210](/Users/admin/projects/exact2-wt-ctl/contract/cli/src/resolve.rs:210). Recording these manifests is correct, but [host/web/dev.mjs:481](/Users/admin/projects/exact2-wt-ctl/host/web/dev.mjs:481) treats every consulted manifest’s directory as a package root. A missing import now makes the TypeScript native/wasm dev loop recursively watch the app or workspace, excluding only `node_modules`. I reproduced the repository-root scan failing with `module watcher source graph exceeds 4096 entries`. In a smaller in-memory fixture, changing only generated `target/module-dev/.../app.plan.map.json` triggered a rebuild callback, allowing build output to retrigger compilation. Watch ancestor manifests individually without promoting their directories to package roots.

No further defects found in the Linux reset or dependency-only hint. Focus restoration remains intact; 18 focused Linux tests passed. No files were modified.
# Disposition

Round 1: `replaced` now also clears `edited`, `fields`, `menu`, `hovered` and `pointer_held`; the workspace-member branch of `declared` is gone (a member with no dependency entry is not linked by `bun install`). Round 2: recording the ancestor manifests in `consulted` (round 1's P3) is reverted, because the dev loops watch a consulted manifest's directory recursively, which for the repository root is everything; the comment on `declared` says so. The install the message names is still watched through the `node_modules` paths `package` records.
