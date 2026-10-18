# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ebd279c80`.
- **Method:** one brief (sha256 `c226f06af73fa1857c291c7b24e0372b5b94e6933e96d20c4300f662621cc68a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high, 2 medium).
- **Disposition:** all taken in `12bf3ad2f`: the published `selected` report is scrubbed as the join is (the builds channel publishes what the join hands back); IR names are found anywhere in a name, so a synthetic token's expression is named too, and its test fixture is now the whole expression; a file change empties the map until `describe` answers, a failed read retries at the next change, and a new plan reads again; a param with `when` lines aliases no token, a role override aliases by its role's spelling; a row reached through a param links it by its full name; the internal unread mark never leaves the join. `modes` is the copy's tuple, as the token rung's (the tool reads the branch by `explain`): the protocol now says so rather than 'the winning branch'. A package component's change re-reads at the next plan (the map is kept meanwhile). Not taken: the describe cost is one fold per file change, measured at about 118 ms on the clone before rt1's cache.

I would block this commit. The new rung works for a simple direct param, but the name-scrubbing and branch guarantees are not met.

1. **[P1] Every selection also publishes an unsanitized `selected` event.**  
   [tamber-server.mjs:219](/tmp/rv-dt-t3s1/scripts/tamber-server.mjs:219) publishes the joined copy, then [tamber-builds.mjs:327](/tmp/rv-dt-t3s1/scripts/tamber-builds.mjs:327) publishes the original report. I reproduced a single click producing `selection` without the IR name followed by `selected` containing it. Both reach subscribers and the event history, including when the join is read-only. Scrubbing the joined copy alone cannot satisfy the client guarantee.

2. **[P1] Synthetic token names bypass the scrubber.**  
   [tamber-inspect.mjs:131](/tmp/rv-dt-t3s1/scripts/tamber-inspect.mjs:131) only recognizes names *starting* with `param.f…`. A row such as `color-mix(in oklab, param.fill 50%, black)` is represented by a synthetic token whose name is the entire lowered expression; [theme.rs:315](/tmp/rv-dt-t3s1/kernel/src/style/theme.rs:315) confirms that producer behavior. Consequently, the IR name survives in `token.names`, `token.text`, and the expression’s `editable.reason`. I reproduced this with known and unread params, inherited and dynamic rows, and missing/stale source maps. The new test’s expression fixture incorrectly supplies `[paramToken]` instead of `[wholeExpression]`.

3. **[P1] Refreshes retain editable, stale param metadata; failed reads become sticky.**  
   [tamber-server.mjs:360](/tmp/rv-dt-t3s1/scripts/tamber-server.mjs:360) advances `params.key` without invalidating `params.map`. Change `Card.fill` from aliasing `ink` to `accent`: while `describe` is pending, a selection still offers `token: "ink"`. Once the build has taken the new live design, the file checks allow that stale offer. If `describe` fails or times out, the old map remains and subsequent refreshes with identical digests never retry. I reproduced both behaviors. Invalidate editability while refreshing, validate the returned snapshot, and allow retry after failure.

4. **[P1] Neither the winning branch nor its alias is computed.**  
   [tamber-inspect.mjs:169](/tmp/rv-dt-t3s1/scripts/tamber-inspect.mjs:169) copies `row.token.modes`, but [mode.rs:230](/tmp/rv-dt-t3s1/runner/src/runner/mode.rs:230) produces the **complete active tuple**, as does the JS host. It does not report the param’s winning `when`. Meanwhile, [tamber-inspect.mjs:121](/tmp/rv-dt-t3s1/scripts/tamber-inspect.mjs:121) always chooses the base `set.value` or default. For a param aliasing `ink` normally and `accent` under `appearance=dark`, a dark/compact selection reports `token: "ink"` and both axes. The target should be `accent` and the existing `appearance=dark` branch. A default-only param should identify the default branch. The protocol’s “winning branch” promise is therefore inaccurate.

5. **[P2] The cache key excludes imported component dependencies.**  
   [tamber-server.mjs:361](/tmp/rv-dt-t3s1/scripts/tamber-server.mjs:361) keys exclusively on `hello.files`. Those files exclude package sources—see [tamber_contract.rs:75](/tmp/rv-dt-t3s1/contract/cli/src/tamber_contract.rs:75). Updating a linked/package component’s param default can rebuild the app without changing that key. The selection then retains the old alias; newly added params remain unread until an owned file changes or the server restarts. Cache invalidation needs the component dependency generation, not just writable-file digests.

6. **[P2] Declared role aliases are missed.**  
   [tamber-inspect.mjs:117](/tmp/rv-dt-t3s1/scripts/tamber-inspect.mjs:117) compares source spelling only with `ir.tokens[].name`. A declared `label` override appears in the IR as `name: "-exact-label", role: "label"`; a param default of `"label"` incorrectly gets no alias. I reproduced this. Match declared role spellings and their supported casing; an unoverridden platform role should still offer no token edit.

Smaller points:

- **The existing client does not ignore the unknown rung completely.** [inspector.ts:70](/tmp/rv-dt-t3s1/apps/design-tool/inspector.ts:70) retains its alias, and [app.contract:1063](/tmp/rv-dt-t3s1/apps/design-tool/app.contract:1063) offers “Open ink”, invoking the token editor. That contradicts the stated old-client assumption.
- **Inherited/dynamic direct params lose their links.** After normalization, full names contain `#`, so `names.find(declared)` at [tamber-inspect.mjs:150](/tmp/rv-dt-t3s1/scripts/tamber-inspect.mjs:150) yields null.
- **The tests overstate coverage.** The server test never changes its files, waits with `Bun.sleep(20)`, and checks only the filtered `selection` event ([tamber-server.test.mjs:76](/tmp/rv-dt-t3s1/scripts/tamber-server.test.mjs:76), [98](/tmp/rv-dt-t3s1/scripts/tamber-server.test.mjs:98)). Neither test derives branch metadata from real producer output.
- **Describe cost is unmeasured here.** Each changed key adds a full `describe` to the serialized Rust request queue. That folds the design and folds it again for `components`; design changes also invalidate the cached app params. This deserves a large-app measurement, especially alongside the tool’s own describe requests.

Other paths: `resolve`/`explain`/`uses` already pass through Rust’s `present()` normalization; outline commands carry sites and locations. Direct state-part rows scrub correctly. Existing agent `node` output and live snapshots still carry IR identifiers outside this join; snapshots use them to address plan tokens, so the guarantee must distinguish host transport from tool-facing names. The different-key late-response guard works, and a fresh server starts with an empty map.

Read-only throughout. I ran in-memory Bun reproductions; I did not run suites or builds that create files.
