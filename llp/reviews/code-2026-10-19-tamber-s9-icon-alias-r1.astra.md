# Review round 1, LLP 1121 stage 9 icon-alias fix, Astra (gpt-6-astra, max)

Reviewed: 3fac27b365 (before the amend that changed the corpus line; see round 2).

- **BLOCKING — [refuse-icons.tamber:22](/tmp/rv-rl-alias/tamber/tests/corpus/refuse-icons.tamber:22):** HEAD does **not** contain the described `star` → `sparkle` change. Resolution now removes the `22:15` error still expected by [refuse-icons.err:10](/tmp/rv-rl-alias/tamber/tests/corpus/refuse-icons.err:10), so the corpus test will fail. The intended replacement preserves unknown-target coverage. Resolving `star` is otherwise correct: its invalid declaration still rejects the entire design at `23:3`; another error on its alias is unnecessary.

- **SHOULD-FIX — [tamber_cli.rs:242](/tmp/rv-rl-alias/contract/cli/tests/it/tamber_cli.rs:242):** Add regression coverage for the previously silent role-variance guard: a design containing an icon alias must reject a newly varying role with `tamber-role-axis` during verified scrub/propose. The added tests exercise folding and CLI reads, but not this write guard.

- **NIT — [lib.rs:141](/tmp/rv-rl-alias/tamber/src/lib.rs:141):** `Folding.design` is no longer simply “the design as parsed”: icon values contain resolved literals. Document that distinction.

What checks out: every listed fold reaches the shared resolution step. Valid chains resolve; unknown targets, theme-token targets and cycles remain refused. A target declared as both a theme token and an icon still produces `tamber-duplicate`. Resolution preserves declaration/value coordinates, and diagnostics remain sorted by position.

I found no consumer regression in the requested lowering, editing, session, import, scrub or export paths. Theme values remain untouched; sessions compare resolved icon sources; DTCG omits icons; explain/uses recover written aliases from source.

The unit and no-params CLI tests detect the original bug; the params build test preserves existing behavior. Remaining coverage gaps include forward references, theme/name collisions, diagnostic positions and alias IR output.

Static review only: Rust tests were not run because this checkout lacks build artifacts and compiling would write files. `git diff --check HEAD^ HEAD` passed.

After round 1: the corpus line was already `sparkle` in the amended commit (the reviewers had the sha before it); the write-guard test and the `Folding.design` doc are in round 2's sha, 4ed1b22ed4.
