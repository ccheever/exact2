# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 4, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `6da4a13619`.
- **Method:** one brief (sha256 `c2b08f80c0c663b0768744fb7b3f0a4e1f091bf189e7f0f9b422ce8f43fba5f7`), shared by both reviewers, blind to the other review; commit 40dfdd8a77 (round 3's fixes, on the squashed change rebased on main after stage 10 B). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 P2, 1 P3).
- **Disposition:** taken in `9416b2090c`: a web build is stale when an input it read is gone (agent-launch.mjs `webChanges`, for every app), and host/web/build.mjs records the inputs as the JS build starts, so a source changed or gone while it compiles, or a build published over it meanwhile, is a change to the next check; that replaces round 3's stage signature (`builtFrom`), which a build publishing over another could mislabel (Astra P2); test files and globs map into the stage by their real path, a link to the checkout too (Astra P2, Grok P2), and nothing else maps, so a flag's value stays the caller's (Astra P3); `--help` is not staged (Grok).

**Request changes: two P2 issues remain, plus one smaller argument-handling bug.**

1. **[P2] A build’s signature can be attached to another build’s output.** [scripts/tamber-server.mjs:1226](/tmp/rv-dt-port4/scripts/tamber-server.mjs:1226) writes `builtFrom` into the shared dist after the child has published its bundle.

   Concrete interleaving: A compiles old sources S1; an edit produces S2; B stages and builds S2. B’s child publishes S2, but its parent is delayed. A then publishes S1 and stamps S1. Finally, B’s parent stamps S2 onto A’s output. The next invocation accepts the old bundle: its signature says S2, and [input digests collected after compilation](/tmp/rv-dt-port4/host/web/build.mjs:79) can also describe S2.

   This requires no overlapping staging calls, so it is separate from the queued two-stager race. Publish the signature with the bundle it identifies. The new [builtFrom test](/tmp/rv-dt-port4/scripts/tamber-tool-port.test.mjs:79) compares manually written signatures; it does not cover publication interleavings.

2. **[P2] Test paths through symlink aliases still bypass staging.** [scripts/tamber-tool-port.mjs:94](/tmp/rv-dt-port4/scripts/tamber-tool-port.mjs:94) canonicalizes the source root but only lexically resolves each argument.

   On this checkout, Bun resolves the module root to `/private/tmp/rv-dt-port4`. From `apps/design-tool`, this invocation therefore retains the original test file:

   ```sh
   EXACT_TOOL_PORT=17999 bun exact.mjs test web /tmp/rv-dt-port4/apps/design-tool/app.test.contract
   ```

   Its `fail fetch` still targets 7740 while the staged app and stub use 17999, breaking the “no server” test. An absolute glob through `/tmp` has the same problem. The `/private/tmp` spelling maps correctly. Canonicalize existing file arguments and the existing prefix of glob arguments.

3. **[P3] Unrelated flag values are rewritten as source paths.** [scripts/tamber-tool-port.mjs:95](/tmp/rv-dt-port4/scripts/tamber-tool-port.mjs:95) maps every non-flag token. From the tool directory, `bun exact.mjs verify web --shots .` silently changes the requested screenshot destination to the staging directory. Likewise, `--storage app.ts` becomes an absolute staged filename. Parse argument roles and remap test inputs only.

The other taken fixes hold: the [launcher reaches `run`](/tmp/rv-dt-port4/apps/design-tool/exact.mjs:9); ordinary relative and real-path test files, `--test`, globs, `web`, `--size 390x844`, and outside paths map as expected; `EXACT_WEB_DIST` overrides inherited values; stub pruning removes deleted files; 8765 is refused; and both checkout path spellings hash to port 17870. A single obsolete build’s signature is rejected, and a failed rebuild after invalidation stays invalid.

B’s remote handlers correctly select `target/design-tool-web` for 7740 and `target/design-tool-web-17999` for 17999, through both TLS and trusted-tunnel routes. The [shared `TOOL_DIST` import](/tmp/rv-dt-port4/scripts/tamber-tool-files.mjs:7) closes that integration gap. I found no new regression in a person’s default `tamber tool`, another app’s commands, or generated launchers.

The zero-padding disposition is sound under the current five-digit grammar. Concurrent staging and direct script invocations remain the recorded limitations; they do not resolve the findings above.

Reviewed through source inspection and memory-only probes. No files changed; no builds or test suites ran.
