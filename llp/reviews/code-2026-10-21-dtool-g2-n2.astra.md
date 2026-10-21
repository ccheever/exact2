# Code review: contract gallery, the variant gallery's G2, second narrow verification round (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `468ca88f8214298a125672ce4d98d08f95df261da2d76383a90ae97ba7dfbf56`), shared by both reviewers, each blind to the other's review; HEAD `1c83745b08`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX; the rest closed).
- **Disposition:**
  Nothing blocks, both reviewers. Astra 1 (a SHOULD-FIX: a rule file whose path holds `:` misread) taken post-round, without another round: git's answer is read with `--stdin -z` (NUL-separated fields, no `:` or tab misread). The rest are closed.

**Nothing blocks.** Reviewed HEAD `1c83745b08`. The reported scenarios are fixed; one exclusion edge case remains.

1. **SHOULD-FIX — A1/B3: reported cases closed, but ignore parsing has a remaining gap.** [gallery.rs:364](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:364) checks all three outputs without `--no-index`; failure to start Git is refused. However, [gallery.rs:376](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:376) misreads negations when the ignore file’s path contains `:`. For example, `cache:rules/.gitignore:2:!target/g/app.json` makes `nth(2)` return `2`, accepting the explicitly unignored manifest. Use `--stdin -z` and parse the NUL-separated fields.

2. **SHOULD-FIX — A2/B1: closed.** [gallery.rs:739](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:739) propagates every matching alias through the barrel chain. [gallery.rs:687](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:687) recognizes the package’s own name, and [gallery.rs:703](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:703) supplies the no-`exports` self-subpath fallback.

3. **SHOULD-FIX — A3/B2: closed.** [gallery.rs:487](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:487) compares canonical declaring paths as well as logical identity. Two installations with identical package names and layouts no longer pass that comparison.

4. **SHOULD-FIX — A4: closed.** [gallery.rs:163](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:163) refreshes the manifest before the probe reads it. A failed fresh probe removes that manifest, preserving retryability.

5. **SHOULD-FIX — A5: closed.** [gallery.rs:339](/tmp/rv-dt-g2n2/contract/cli/src/gallery.rs:339) requires the generated source header for ownership. A lone `{"name":"Photo gallery"}` manifest is refused.

The previously closed dependency-retention and validation-before-creation fixes remain intact. I found no new behavioral regression beyond the remaining exclusion gap above.

Read-only review; no files changed, builds run, or test suites executed.
