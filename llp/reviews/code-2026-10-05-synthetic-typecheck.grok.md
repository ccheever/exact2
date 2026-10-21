# Code review: synthetic conformance plans skip the data app's type check (host/web-js/build.mjs), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, a detached worktree at `35eb0700a`.
- **Method:** one brief (sha256 `0f68f932fb7fa34be71e4aaf2b1e57f5f70ce096d4981ad7571f6713c7e73f2d`), the same sent to both; one round, sized to a three-line change; blind to the other review. Requested by the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND.
- **Disposition:** no findings to take.

---

I'll review commit `35eb0700a` read-only against the diagnosis, the `--data` skip, and the repo rules.`--data` shows up outside conformance. I'll read those call sites and the type-check path before judging the skip.LAND

No findings.

The diagnosis matches the code. `synthetic-bootpress` is built as `build.mjs realworld --plan <bootpress plan> --data <realworld wasm dist>` (`host/web-js/conform.mjs:220`). RealWorld is the only synthetic data app with `app.ts` and `rust: false`, so `ts` stays true (`host/web-js/build.mjs:80`) and `typecheck()` runs against `gen/app.contract.d.ts` emitted from that plan (`host/web-js/src/main.rs:193`). Bootpress declares only `popularTags`, so `Source` is that one name. `apps/realworld/app.ts:13` (`Result<'currentUser'>` and the rest) is TS2344, and the full `sources` object loses contextual types (TS7006/TS7031). The same function writes those declarations to `apps/realworld/app.contract.d.ts` on any non-production build (`host/web-js/build.mjs:152`). That write and that check arrived with the unconditional `typecheck()`; `b0376720f^` had neither.

Skipping on `--data` is the small fix. A second check against RealWorld’s own plan would need another declaration emit: `gen/app.contract.d.ts` is the plan just compiled. The narrow `Answer` is not a subset check, so it cannot succeed for this `app.ts` and was not catching a real mismatch. Keying off `--plan` would be wrong: the app’s own conformance target and `scripts/deploy.mjs` both pass `--plan` of the baked app plan and must still check.

Nothing a real build catches is dropped. With `ts` still true, the new conjunct only applies to a TypeScript-only app built with `--data`. Callers:

- `host/web-js/conform.mjs:220` passes `--data` only for a synthetic plan. The non-synthetic RealWorld target in the same run (`scripts/async.mjs` `WEB_APPS`) is `--plan <wasm dist>/app.plan` with no `--data`, so it still type-checks and still writes RealWorld’s declarations. Targets are apps first, synthetics after, which is why the old synthetic build left the bad `.d.ts` in place.
- `bun host/web/build.mjs realworld-web` and `host/web-js/dev.mjs` call `build.mjs` with no `--plan` and no `--data`.
- `scripts/agent.mjs:130` and `scripts/agent-playwright.mjs:73` pass `--data` only when `dist/rust/wasm/app.module.wasm` exists. That file is copied only inside `if (rust)` (`host/web-js/build.mjs:215`). Then `ts` is already false (`build.mjs:80`), so this commit does not change those builds. `host/web/build.mjs --wasm` does not write that path.

The comment is accurate for the branch it changes: that plan is not `app.ts`’s, the app’s own build still checks it, and the editor file is only written inside `typecheck()`. `--data` is also the agent’s Rust-module directory, but on that path `ts` is already false, so the new conjunct is idle. No `rules/RULES.md` or `AGENTS.md` breach: one condition, no new check or script, `build.mjs` stays at 550 lines.
