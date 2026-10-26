# Code review: contract gallery, the variant gallery's G2, round 3 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `0d873f2943a93a8c0c5964c9f743639f20b75f4cb5b27636135e685e53e22131`), shared by both reviewers, each blind to the other's review; HEAD `22e71c0235`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 2 BLOCKER, 3 SHOULD-FIX.
- **Disposition:**
  All taken post-round, in the commit named in the sign-off request (with a narrow verification round, since BLOCKERs were fixed):
  - Astra 1 (an ancestor named target): the destination is judged within its git repository (else within the app). A node_modules or target must be below that root, and git must ignore what the gallery writes there (`git check-ignore`). Tested: a checkout under /tmp/.../target refuses the app's src; a target git doesn't ignore is refused.
  - Astra 2 (a refused cell dropped its package's design): every placed component is brought, refused or not (an unused use compiles), so its package's design stays. Tested: an app whose only library use is a refused cell, and whose design names the package's token.
  - Astra 3, Grok 2 (re-export chains): a shape's exported name is followed through each re-export, by a relative path or the package's own name and subpath, at most 8 files deep. Tested: a barrel, and `@acme/lib/card`.
  - Astra 4 (root membership isn't identity): each probe `use` must reach the declaration the app's does (by identity), or the destination is refused. Not given a fixture: it needs two installs aliased across roots.
  - Astra 5, Grok 1 (a lone manifest after a failed probe): app.json is written only after the probe passes, and a manifest a gallery writes is taken as ours. Tested.
  - Grok 3 (made then refused): the destination is checked as it will be, before any directory is made. Tested: the app's src is not created.

**Two blockers remain.** I cannot mark every round 2 finding closed.

I checked these dispositions against HEAD~1; links use current HEAD line numbers.

| Round 2 finding | Disposition |
|---|---|
| 1. Destination safety | **Partial.** Both output files and symlinks are guarded at [gallery.rs:306](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:306), but source/git exclusion remains incomplete; finding 1 below. |
| 2. Design assets | **Closed for the reported case.** [gallery.rs:323](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:323) links the app’s assets. |
| 3. Public shape aliases | **Partial.** [gallery.rs:506](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:506) handles direct re-exports; chains remain broken, finding 3. |
| 4. App inside its package | **Closed.** Discovery uses recorded `Origin::App` at [gallery.rs:408](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:408). |
| 5. Installed-alias shadowing | **Partial.** Unknown roots are rejected at [gallery.rs:368](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:368), but resolution identity is still unchecked; finding 4. |
| 6. Relative imports aborting generation | **Closed for the abort.** They become explicit cell refusals at [gallery.rs:136](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:136) and are excluded from the probe. They remain unrendered. |
| 7. Lowercase component exports | **Closed.** [gallery.rs:442](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:442) supplies an uppercase local alias. |
| 8. Compiler records | **Closed.** [gallery.rs:718](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:718) reads the admitted constructors’ field types from the checker; this is line 716 in HEAD~1. |

1. **BLOCKER — [gallery.rs:294](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:294): An ancestor named `target` still permits source-tree output.** The check scans the entire absolute path. With a checkout at `/tmp/target/repo`, an app at `apps/x` accepts `-o apps/x/src/gallery`: the ancestor satisfies the check, package resolution still works, and the generated files are inside the app’s sources and visible to that repository’s git. Directory spelling also does not establish actual git exclusion. Validate the destination relative to the app/repository boundaries and verify exclusion of the generated files. The destination test does not cover this layout.

2. **BLOCKER — [gallery.rs:619](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:619): Refusing a cell can remove design dependencies and abort the whole gallery.** Consider a library exporting `Align(align: string)` with `text-align=align`, plus a design token `lib.pad`. The app validly calls `Align(align="left")`, and its design defines `app.pad = lib.pad`. The gallery’s `"align"` placeholder fails; removing the last live component import then removes that package’s design. The unchanged app design now fails on missing `lib.pad`, so generation returns an error instead of producing the refusal cell. Package designs are loaded from the remaining imports at [packages.rs:118](/tmp/rv-dt-g2r3/contract/cli/src/sources/packages.rs:118). Preserve the original design dependencies independently of live component imports. The existing `Align` test keeps other components from the same package, masking this case.

3. **SHOULD-FIX — [gallery.rs:538](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:538): Shape export recovery follows only one re-export hop.** A valid `index.contract → barrel.contract → card.contract` chain can expose the leaf’s `Info` as `PublicInfo`. This function only accepts an entry import pointing directly to the declaring file; otherwise it falls back to `Info` and emits an unavailable export. All affected cells are falsely refused. Resolve the public name through the loader’s complete export chain. The new alias fixture covers only a direct re-export.

4. **SHOULD-FIX — [gallery.rs:368](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:368): Membership in the app’s root set does not prove the same component was resolved.** Suppose the app imports a shape through alias A from package root X and `Card` through alias B from root Y. Both packages export a compatible `Card`. If B beneath the cache points to X, this check accepts it because X is already in the app’s root set. The gallery renders X’s component while reporting Y’s identity and variants. Compare each generated import’s resolved declaration with its original declaration, rather than accepting any previously seen root.

5. **SHOULD-FIX — [gallery.rs:109](/tmp/rv-dt-g2r3/contract/cli/src/gallery.rs:109): A failed first generation leaves the cache unusable on retry.** `app.json` is written before the reachability probe at line 164, while `app.contract` is written afterward. If the cache cannot reach a package, generation leaves a lone manifest. After fixing package resolution, retrying fails the ownership check at line 305 because no generated `app.contract` exists. Make failure leave a safely retryable destination, and cover failure followed by a successful retry.

The shared cells comparison is present. I found no additional issue with HEAD’s manifest-name addition, literal escaping, the inert `noop`, or recursion termination. Recursive generation itself remains untested; its unit test exercises only `args`.

Static review only: no files changed, builds run, or test suites executed. The commit messages’ reported 46-app parity gate was not rerun.
