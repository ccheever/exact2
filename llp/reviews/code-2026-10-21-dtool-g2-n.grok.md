# Code review: contract gallery, the variant gallery's G2, narrow verification round of round 3's post-round fixes (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `0fc4a77c5492363a30fd36afd4d51706d6c04d4a25a8b6c00cd0f1b6a20e5943`), shared by both reviewers, each blind to the other's review; HEAD `88c8dabb88`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:**
  All taken post-round, in the commit named in the sign-off request (and a second narrow round on it):
  - Astra 1 (the BLOCKER: git exclusion of one file), Grok 3: git must ignore each of app.contract, app.json and assets (`check-ignore --non-matching --verbose`, every line a matching non-negated rule). A tracked path counts as not ignored (no `--no-index`). Git failing to say refuses. Tested: a repository ignoring only the source.
  - Astra 2, Grok 1 (the re-export walk): every alias a file gives the shape is kept (`Info as Internal, Info as PublicInfo`). A package-self subpath with no `exports` is the file by path, as the loader does, and the package's own `package.json` name counts as itself. Tested: two aliases, and `@acme/lib/card.contract` with no exports.
  - Astra 3, Grok 2 (identity is not the declaration): each probe use must also reach the same canonical declaring file the app's does. Not given a fixture (two installs aliased across roots).
  - Astra 4 (a stale manifest read by the probe): the manifest is written fresh before the probe, and taken away again if the probe refuses and no gallery source is beside it. Tested: a refused cache leaves no app.json, and the next run proceeds.
  - Astra 5 (a name-suffix ownership): gone. Only an app.contract with the generated header makes a destination ours. Tested: a foreign "Photo gallery" manifest is refused.

1. **SHOULD-FIX — `gallery.rs:665`: a package-self subpath is resolved only when `exports` maps it.** `Package::reached` uses the app's specifier as the package name and requires `package_subpath` to return a path. With no `exports` field, the loader maps `@acme/lib/card.contract` to `./card.contract` (`resolve.rs:349`); `package_subpath` returns nothing, so this walk stops. `library.rs:117` also falls back to the subpath itself and follows `package.json`'s `"name"`, which is what a package uses for its own `use` when the app imported it under another name. The relative barrel (`index` → `barrel` → `card`) and an `exports` entry `./card` are followed (`gallery.rs:677`). This case still falls back to the declaration spelling, and every cell that builds the shape is refused.

2. **SHOULD-FIX — `gallery.rs:470`: the same package name at two roots still passes.** Each probe `use` is compared with the app's `params.identity`. That string is `{package.json name}/{relative path}#{Component}` (`sources.rs:322`, `sources.rs:763`). Two installs of `@acme/lib` with the same layout share it. Alias B resolving to root X, already in the app's root set (`gallery.rs:443`) because alias A came from X, while the app resolved B to root Y, still matches. The gallery compiles X's component and keeps Y's variants. Different package names are refused.

3. **NIT — `gallery.rs:320`: if `git` cannot be started, a non-ignored `target/` is still accepted.** Only a process that runs and exits non-zero is refused. The check is also only `app.contract` under `--no-index`, so an ignore rule that matches that file alone does not say `app.json` is excluded, and a tracked path that merely matches a rule still counts as ignored.
