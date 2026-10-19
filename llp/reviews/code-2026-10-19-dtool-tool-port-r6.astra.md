# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 6, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `c74152c3a1`.
- **Method:** one brief (sha256 `3ec87a2e998c1c377a1c842ec48d70762887da0e44cb8e1c2e5e7252f000cc4f`), shared by both reviewers, blind to the other review; commit caef755d8d (round 5's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 P2).
- **Disposition:** taken in `0ff71912d0`: test files map into the stage by their role, which app-cli gives from help.mjs's flags (a test verb's operands after its host, `--test`'s value), so a flag's value, an op or a URL that ends in `.test.contract` stays the caller's (Astra P2), and a bare glob operand maps too (Grok). Not taken: the wasm path and the JS dev loop keep their own freshness (both note, unchanged); a clock step over two seconds is outside the grain (both note).

**Request changes: one P2 remains in argument mapping.** I found no new blocker in the JS freshness change.

**[P2] The suffix filter still rewrites non-test arguments — [tamber-tool-port.mjs:83](/tmp/rv-dt-port6/scripts/tamber-tool-port.mjs:83).**

From `apps/design-tool`, run:

```sh
EXACT_TOOL_PORT=17998 bun exact.mjs agent web --storage review.test.contract tree
```

`stageToolTests` rewrites the storage name into an absolute stage path. The driver then rejects its slashes at [agent.mjs:833](/tmp/rv-dt-port6/scripts/agent.mjs:833). Likewise, `"type name app.test.contract"` becomes a filesystem path instead of an operation. A URL ending in `.test.contract` is also rewritten.

A memory-only probe of the actual mapping function reproduced all three. Restrict mapping to test operands and `--test` values, using their argument roles. The suffix alone cannot distinguish them.

The remaining checks:

- **Round-5 interleavings are fixed for new JS markers.** [build.mjs:69](/tmp/rv-dt-port6/host/web/build.mjs:69) captures `inputsAt` before hashing and compilation. [webChanges:836](/tmp/rv-dt-port6/scripts/agent-launch.mjs:836) therefore compares edits made during compilation, including S2 staged before an older S1 build publishes last. The deletion sweep remains independent of timestamps. Memory-only probes detected all three scenarios and accepted unchanged bytes.

- **No perpetual rebuild found for new one-shot JS builds.** The JS builder’s other generated files go into its output stage; module generation uses `target/`; bake intermediates use temporary directories and `OUT_DIR`. The beside-source declaration is excluded. [Expose’s `local.ts`](/tmp/rv-dt-port6/apps/expose/web/build.rs:9) is deterministic for a settled key: creation or changed contents can require one additional build, but rewriting identical bytes stays fresh.

- **Older and alternate markers retain their limitations.** Without `inputsAt`, the marker’s mtime remains the boundary. Wasm still [snapshots after compilation](/tmp/rv-dt-port6/host/web/build.mjs:500), so it does not acquire the new interleaving guarantee. The JS dev loop retains its watchers and supersession checks. One pre-existing caveat merits correcting in the review record: its [inventory-free marker](/tmp/rv-dt-port6/host/web-js/dev.mjs:139) makes `webChanges` report `.exact/backend-origin` even when absent, because [the sentinel is always nominated](/tmp/rv-dt-port6/scripts/agent-launch.mjs:817) and `!built.inputs` makes it changed. That is unchanged by this commit.

- **The two-second margin covers ordinary local timestamp granularity, not arbitrary clock changes.** Nanosecond storage does not imply equally precise timestamp sampling; Linux documents coarse inode timestamps and possible backward wallclock jumps. [Linux timestamp documentation](https://docs.kernel.org/6.15/filesystems/multigrain-ts.html). The comparison is strictly `mtimeMs > inputsAt - 2000`: probes detected a changed file 1,999.5 ms behind, but missed one exactly 2,000 ms behind. Preserved old mtimes or a sufficiently large clock rollback remain outside this guarantee. I found no ordinary APFS/ext4 precision blocker.

- **Cost is modest in this checkout.** The tool has 422 recorded files totaling 6.6 MiB. Warm read-only probes, caching the existing Git lookup, measured about **7.3 ms** with old inputs versus **20.5 ms** when every input was nominated. Files within the build-start window remain candidates on subsequent checks, but matching hashes prevent rebuilding.

No files changed. No builds or test suites ran.
