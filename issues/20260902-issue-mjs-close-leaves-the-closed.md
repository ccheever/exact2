# issue.mjs close leaves the closed header out of the index

**Status:** Open
**Systems:** Issue tooling, cdcstack
**Severity:** P2
**Author:** Claude (Opus 5) for Charlie Cheever
**Date:** 2026-09-02
**Related:** cdcstack scripts/issue.mjs (upstream owner); exact2 285be27 / 2f3ec66

`scripts/issue.mjs:846-848` writes the reshaped, closed issue to the **old**
path and then runs `git mv` to the new one:

```js
writeFileSync(absolute, text);                       // absolute = the OLD path
mkdirSync(dirname(targetAbsolute), { recursive: true });
const moved = spawnSync('git', ['mv', path, target], …);
```

`git mv` moves the **index entry**; it does not stage the working tree. So the
index gets the file's last-committed bytes under the new name — `Status: Open`,
no `Resolution:` — while the close, and any body the author appended before
running the command, stay unstaged as an ordinary worktree modification.
`git status` shows this as `RM`, which reads like "renamed and modified" and
looks exactly like a close that worked.

Reproduced standalone:

```
git init; echo 'Status: Open' > a.md; git add a.md; git commit -m one
echo 'Status: Closed' > a.md          # what issue.mjs does before the mv
mkdir closed; git mv a.md closed/a.md
git show :closed/a.md                 # → Status: Open
cat closed/a.md                       # → Status: Closed
```

**Nothing catches it.** `runFormatCheck` (`scripts/issue.mjs:665-685`) reads
each tracked path from the working tree, so `issue.mjs check` validates content
the commit will not contain and passes. The command's own closing advice —
"Commit the move together with the fix (cdcstack: close-with-fix is atomic)" —
then walks the author into the bad commit.

**It only bites `git add <path>`.** `git add -A` or `git commit -a` sweep the
worktree change back in and hide it. Staging by path is the required habit in
any repo where a second session shares the worktree, which is where this landed:
exact2 `285be27` moved two issues under `issues/closed/` still carrying
`Status: Open` and no `Resolution:`, and `2f3ec66` had to finish the close.
(That commit message describes the mechanism as a rewrite *after* the `mv`. It
is the other way round, as above — the write is first and `git mv` discards it.)

**Fix**, one line at the call site: write the file at `targetAbsolute` after the
move, or `git add target` after `git mv`, and fall back to `git add` when
`git mv` declines. Worth considering alongside it: `check` reporting when a
tracked issue's index content differs from the worktree, since that is the state
this produces and the one nothing currently names.

**Upstream.** `scripts/issue.mjs` is adopted from cdcstack by copy, and
`~/projects/cdcstack/scripts/issue.mjs` is byte-identical to this one — so the
fix belongs there first and then travels to every repo that adopted it (exact2,
ibex, weird-castle).
