# Code review: `tamber gen`, the TS Token union, Rust constants (Tamber stage 5, commit 4), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `a10690d4b9aa3e0a90f6a87f08042701fc1895cb581e3b067b000ecc497118d4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (role constants unresolved; the retention test's misses; union ignored exports; TS clashes; CSS collisions; Exact roles in CSS; output directory).
- **Disposition:** in the first-round commit.

---

Blocking correctness issues remain:

- **P2 — Generated role constants cannot resolve.** [tamber_gen.rs:102](/tmp/rv-s5c4-1/contract/cli/src/tamber_gen.rs:102): `secondary-label = #123456` generates `Token::new("-exact-secondary-label")`, but the plan stores `secondary-label`. Runtime lookup compares names exactly, so `get()` returns `None`. The export filter also silently drops valid case-insensitive role exports such as `canvastext`.

- **P2 — Module detection misses supported apps.** [lower/src/lib.rs:1138](/tmp/rv-s5c4-1/contract/lower/src/lib.rs:1138): an `app.ts` or Rust data module without resource/send sources does not trigger retention. Neither does a native module used only through hatches. Unnamed tokens are pruned despite generated bindings naming them.

- **P2 — Runner-owned sources incorrectly disable pruning.** [lower/src/lib.rs:1138](/tmp/rv-s5c4-1/contract/lower/src/lib.rs:1138): `root.sources` includes `exactViewport()` and `exactTime()`. A pure Contract app using either now retains every design token, violating the ruling that apps without data/native modules remain unchanged.

- **P2 — `ColorToken` ignores `exports`.** [typescript.rs:145](/tmp/rv-s5c4-1/contract/cli/src/typescript.rs:145): it includes every retained nonsynthetic token. With only `badge.ink` exported, a view referencing private `brand` causes `"brand"` to enter the union and pass `tsc`, although `tamber gen` excludes it.

- **P2 — Accepted names generate invalid TypeScript.** [tamber_gen.rs:295](/tmp/rv-s5c4-1/contract/cli/src/tamber_gen.rs:295): `tokens` and `tuples` collide with generated declarations; `await` and `arguments` escape keyword validation. In-memory TypeScript checks confirmed redeclaration and reserved-identifier errors.

- **P2 — CSS names silently collide.** [tamber_gen.rs:210](/tmp/rv-s5c4-1/contract/cli/src/tamber_gen.rs:210): `_ink` and `ink` pass both identifier collision checks but both emit `--ink`; the latter overwrites the former. `a__b`/`a--b` also collide, and `_` emits the invalid property name `--`.

- **P2 — Standalone CSS contains unresolved Exact roles.** [tamber_gen.rs:104](/tmp/rv-s5c4-1/contract/cli/src/tamber_gen.rs:104): theme-fixture’s `text.label = -exact-label` emits `--text-label:-exact-label`. Outside Exact, `color:var(--text-label)` is invalid; these exports need browser-valid role values or fallbacks.

- **P3 — Default output follows the design directory.** [tamber_gen.rs:365](/tmp/rv-s5c4-1/contract/cli/src/tamber_gen.rs:365): manifest `"tamber":"design/theme.tamber"` writes into `design/.exact/tamber`, rather than the documented app-root directory. In-repo, that also misses the `apps/*/.exact/` ignore rule.

The tests miss these boundaries: the exports test checks only generated Rust text, and the `tsc` fixture uses tokens already referenced by views. Full Rust tests were not run because the sandbox forbids build/temp writes. Worktree and index remain unchanged.
