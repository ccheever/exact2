# Code review: contract gallery, the variant gallery's G2, narrow verification round of round 3's post-round fixes (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `0fc4a77c5492363a30fd36afd4d51706d6c04d4a25a8b6c00cd0f1b6a20e5943`), shared by both reviewers, each blind to the other's review; HEAD `88c8dabb88`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 4 SHOULD-FIX.
- **Disposition:**
  All taken post-round, in the commit named in the sign-off request (and a second narrow round on it):
  - Astra 1 (the BLOCKER: git exclusion of one file), Grok 3: git must ignore each of app.contract, app.json and assets (`check-ignore --non-matching --verbose`, every line a matching non-negated rule). A tracked path counts as not ignored (no `--no-index`). Git failing to say refuses. Tested: a repository ignoring only the source.
  - Astra 2, Grok 1 (the re-export walk): every alias a file gives the shape is kept (`Info as Internal, Info as PublicInfo`). A package-self subpath with no `exports` is the file by path, as the loader does, and the package's own `package.json` name counts as itself. Tested: two aliases, and `@acme/lib/card.contract` with no exports.
  - Astra 3, Grok 2 (identity is not the declaration): each probe use must also reach the same canonical declaring file the app's does. Not given a fixture (two installs aliased across roots).
  - Astra 4 (a stale manifest read by the probe): the manifest is written fresh before the probe, and taken away again if the probe refuses and no gallery source is beside it. Tested: a refused cache leaves no app.json, and the next run proceeds.
  - Astra 5 (a name-suffix ownership): gone. Only an app.contract with the generated header makes a destination ours. Tested: a foreign "Photo gallery" manifest is refused.

**One blocker remains.** Reviewed HEAD `88c8dabb88`.

1. **BLOCKER — A1 partially closed: Git exclusion covers only one output.** Repository-relative validation fixes the ancestor-`target` case, but [gallery.rs:316](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:316) checks only `app.contract`. A rule ignoring `apps/x/target/g/app.contract` admits that destination while leaving the generated `app.json` and assets link visible to Git. Check every generated path. Git execution errors also pass through at line 320.

2. **SHOULD-FIX — A3/B2 partially closed: the re-export walk still misses valid exports.** The reported straight barrel chain and explicitly mapped self-subpath now work. However, [gallery.rs:693](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:693) returns only the first alias from an inner file. If a barrel imports `Info as Internal, Info as PublicInfo`, while its caller imports only `PublicInfo`, recovery selects `Internal`, fails upstream, and falsely refuses the cells. Also, [gallery.rs:668](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:668) rejects self-subpaths without an `exports` map, although the loader supports that fallback at [resolve.rs:348](/tmp/rv-dt-g2n/contract/cli/src/resolve.rs:348).

3. **SHOULD-FIX — A4 partially closed: logical identity does not establish declaration equality.** [gallery.rs:470](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:470) compares identities built from package name, relative filename and component name—not the canonical declaring file ([sources.rs:322](/tmp/rv-dt-g2n/contract/cli/src/sources.rs:322)). Two installed copies can therefore compare equal. If X is already admitted through a shape-only import, shadowing B’s component from Y with X still passes. This can be valid when X lacks a design and Y supplies one. Compare canonical declaration paths as well.

4. **SHOULD-FIX — A5/B1’s fresh-failure case is closed, but manifest timing introduces a retry regression.** The manifest now follows the probe at [gallery.rs:159](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:159). After one successful generation, rename the app’s design file and update its manifest. The next probe reads the gallery’s **old** manifest, fails opening the old design, and returns before refreshing it. Every retry repeats that failure. The loader reads this cached reference at [sources.rs:369](/tmp/rv-dt-g2n/contract/cli/src/sources.rs:369).

5. **SHOULD-FIX — New ownership regression: an unrelated manifest can be overwritten.** [gallery.rs:378](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:378) accepts even `{"name":"Photo gallery"}` as gallery-owned, although the generator always writes an `app` object too. With no `app.contract`, this permits overwriting a foreign manifest. Use a reliable ownership marker; a name suffix is insufficient.

A2’s reported dependency-loss blocker **is closed**: [gallery.rs:771](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:771) retains refused components’ package imports. B3 **is closed**: destination validation precedes directory creation at [gallery.rs:286](/tmp/rv-dt-g2n/contract/cli/src/gallery.rs:286). I found no additional regression in `to_be` or retaining those imports.

Static review only. No files changed, builds run, or test suites executed.
