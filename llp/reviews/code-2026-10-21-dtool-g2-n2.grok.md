# Code review: contract gallery, the variant gallery's G2, second narrow verification round (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `468ca88f8214298a125672ce4d98d08f95df261da2d76383a90ae97ba7dfbf56`), shared by both reviewers, each blind to the other's review; HEAD `1c83745b08`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; every finding closed.
- **Disposition:**
  Nothing blocks, both reviewers. Astra 1 (a SHOULD-FIX: a rule file whose path holds `:` misread) taken post-round, without another round: git's answer is read with `--stdin -z` (NUL-separated fields, no `:` or tab misread). The rest are closed.

1. **A1 (was BLOCKER) — closed.** `all_ignored` (`gallery.rs:360`) runs `git check-ignore --non-matching --verbose` on `app.contract`, `app.json`, and `assets`. A non-match is `::` before the tab and fails the check (`gallery.rs:375`). A negation (`!…`) fails it too. If `git` cannot be started, or its stdout is not UTF-8, the function returns `None` and `destination` refuses (`gallery.rs:328`). A rule that ignores only `app.contract` leaves `::` lines for the other two paths, so the destination is refused.

2. **A2 / B1 (was SHOULD-FIX) — closed.** `exported_by` (`gallery.rs:740`) keeps every alias an inner file gives a shape, and the caller matches any of them. `Info as Internal, Info as PublicInfo` with an upstream import of only `PublicInfo` now resolves to `PublicInfo`. The `.next()` at `gallery.rs:673` only picks among the names the outermost file actually exports. `Package::reached` (`gallery.rs:703`) treats a missing `exports` field like the loader: the subpath is `./{sub}`. `names` (`gallery.rs:687`) includes both the app's specifier and `package.json`'s `"name"`.

3. **A3 / B2 (was SHOULD-FIX) — closed.** Each placed component stores the canonical path of the file that declares it (`gallery.rs:576`). The probe must match that path as well as the identity string (`gallery.rs:487`). Two installs that share `{name}/{relative}#{Component}` no longer compare equal.

4. **A4 (was SHOULD-FIX) — closed.** The gallery manifest is written before the probe (`gallery.rs:163`), so the loader reads the current design path (`sources.rs:369`) instead of a stale `tamber`. If the probe refuses and `app.contract` is not there yet, that manifest is removed (`gallery.rs:169`). A later run writes a fresh manifest before it probes again.

5. **A5 (was SHOULD-FIX) — closed.** Ownership is only an `app.contract` that starts with the generated header (`gallery.rs:339`). `{"name":"Photo gallery"}` is not owned, and the name-suffix check is gone.

6. **B3 (was NIT) — closed** with A1. Without `--no-index`, a tracked path is not ignored: `check-ignore` prints `::` for it (a matching rule under `--no-index` still would). That fails `all_ignored`. A symlink at `assets` is still judged by its path, so a `target/` rule covers it on a later run.
