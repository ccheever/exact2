# EX10 additional semantic review — Muse

Candidate: `1d7d5bc7e`. Baseline: `38d90ee5f`.

Provenance: `muse-exec.sh --yolo --model muse-spark-1.3-contributor --reasoning-effort high --prompt-file <brief>`. An initial call was still running in its API fallback when the retry was launched; both received independent reviews. The failed plan attempt and launcher envelope are omitted; this additional received response follows verbatim.

READY

No material finding remains in the committed range `38d90ee5f..1d7d5bc7e` (7 commits, 51 files, +1207/−252). Read-only review; no edits or commits made.

Checks I ran (candidate head): focused `cargo test -p contract --test it -- migrate build_warnings expected_refusal` — 5 passed; `cargo test -p exact-grants --lib per_origin` — 2 passed; a live probe of `refusalExpectation`/`refusalsOf` confirming exact-match consumption and preservation of unrelated refusals. All cargo invocations used the required env vars with `timeout`.

Evidence I read (not re-run): check logs under the provided log directory — `final-build` exit 0, `final-clippy` clean, `caps`/`boot`/`fmt`/`difftest`/`web-js` green. The older failing `gates.json`/`build`/`clippy`/`root-tests` entries are intermediate states inside the range (a bad-escaping `replace()` in the refusal test and a private `parsed_with_edits` import); both fixes are in the reviewed head, which compiles and passes the focused suites. Full workspace suite not re-run (prior ~400 s run; its only failure was that same compile error).

Per-area assessment:

- Parser-backed migrate (`contract/syntax/src/migrate.rs`, `contract/syntax/src/parser/migration.rs`, `parser.rs`, `parser/expr.rs`, `parser/view.rs`): retired `list<T>`, free method forms, `when`/`each`/`else when` recover inside the parser while recording source-preserving edits; comments, literals, and user-declared `fn`/shape/action names are untouched (custom-`trim` case tested). Non-retired errors refuse migration with no file write, and the rewritten source is re-parsed before the CLI writes. All-retired reporting bypasses `MAX_REFUSALS`/`MAX_DIAGNOSTICS` truncation only when a retired diagnostic is present (35-retired case tested). `else when` retargets exactly the outer entry (`retired[first]`), so nested `when`s keep their own ids. No `unsafe` anywhere in the diff.
- `--json --errors-only` (`contract/cli/src/build.rs`, `main.rs`, `migrate.rs`, `tests_json.rs`, `build_warnings.rs`): warnings cleared and clock/surface warnings suppressed under the flag; errors keep exit 1, warnings never fail the build and still write the plan. Usage strings and help coverage updated.
- Bakery tickets and dead-claim recovery (`scripts/filesystem.mjs`, spec `llp/1030.002-owned-filesystem.spec.md`): choosing-marker then atomic ticket publish, wait only on live earlier tickets, dead/recycled owners ignored via pid-liveness plus `ps` start-time check, and each caller removes only its own ticket file. Dead-claim takeover renames aside, compares the token, and restores a replaced claim via hard link rather than unlinking on a stale read; malformed claims still fail after 60 s naming the capture override. Crash-leftover empty ticket files are ignored but never reaped — trivial accumulation, not material.
- Expected-refusal filtering (`scripts/verify.mjs`, `scripts/agent-test.mjs`): `refusalsOf` consumes only exact raw matches (multiset), host mirrors dedupe by normalized line, and per-slot cursors cleared on reload; my probe confirmed an unrelated slot's refusal survives. `refusalExpectation` scans for the latest observed refusal plus its preceding transaction line, both sync and async shapes.
- Lock/drift (`scripts/app-cli.mjs`, `scripts/app.mjs`, `game/new.mjs`): new `lock` verb resolves offline-first and reports per-package changes; `update` reports drift without touching the lock; builds never change it. Help text and verb registry match.
- Origin diagnostics (`js/bake/src/typescript.mjs`, `js/bake/src/lib.rs`, `resident.rs`, `host/web-js/build.mjs`, `scripts/agent-test.mjs`): `backendOrigin` kept as a wrapper over `backendOriginSelection`, so existing callers are unbroken; stale-build source is stated explicitly and the saved-selection shadow warning flows through a JSON sidecar into bake warnings and the typecheck.
- Roster/scaffold (`game/new.mjs`, `game/new-app.test.mjs`): missing `insert` rule now counts as deny, per-table bodies (not whole-schema) decide seeding, and a failed seed removes the scaffold directory. Backend default-deny itself is outside this diff and unchanged.
- Auth/keys/manifest/vocab (`grants/src/lib.rs`, `snapback4/ts/snapback.ts`, `snapback4/online/snapback.ts`, `snapback4/README.md`, `scripts/app-manifest.test.mjs`): per-origin secret-key grammar with tests, guest logout-before-signup/login with upgrade tests, manifest shape accept/reject tests, and the vocab table all match the documented APIs (`contract migrate`, `--errors-only`, `lock`, `expect refusal`, refusal slot/`then` semantics).

The temporary uncommitted 0.4.31 source pin was ignored per instructions; the committed tree still references the published receipt version.
