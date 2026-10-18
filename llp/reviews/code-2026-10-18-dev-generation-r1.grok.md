# Code review: the dev loop's generation identity, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `5862196aa`.
- **Method:** one brief (sha256 `b12254251206a7dc3fe87e3a7c2dc5d7d041fe822f8b7b8bac44bf8cc14f8271`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nothing blocking; entry plans refused by the web page; the helper's asset order).
- **Disposition:** fixed in `8830a5c32`.

---

I'll review the HEAD commit read-only: how the new generation digest is computed, whether every client matches it, and whether anything still depends on the old digest.The digest change is small. Next I’ll compare it with every client that checks generation identity, and with every path that still names or caches a generation.The host and the web client build the digest from different JSON writers. I’ll compare those bytes, including Apple’s canonical form, on the paths this commit claims to cover.Nothing in this commit is blocking. The new digest matches the web client and the Apple client for every generation the dev loop actually publishes. Linux and Android do not check that identity.

`devGenerationDigest` hashes the same canonical JSON those two clients rebuild: plan, assets, module, and Rust cards, each as name, sha256, and byte length, with urls left out. Key order, escapes, Rust `target` (present except on `wasm`), and UTF-8 asset order all agree. I checked that against `host/web/dev.js` `fetchGeneration` and a port of `PlanURL.swift`'s hand-built canonical string, including a module, wasm/native/tiered Rust, `plans/*.plan` cards, and names with quotes, backslashes, newlines, and non-ASCII. `captureGeneration` sorts asset names with `Buffer.compare` before it calls the digest, on the first generation, asset edits, and logic edits.

Linux (`host/linux/src/fetch.rs`) and Android (that same poller inside the Linux host) never read `dev.generation`. They fingerprint the plan card plus Rust receipt/module bytes for their own refresh check. This change does not affect them.

Nothing else was using the old url-inclusive digest. The on-disk cache is keyed by app, path, and dist, and retained generations are stored by epoch and seq. The epoch is random per process. Receipts and `developmentCandidate` still see the asset objects; only the generation id dropped `url`. Logic edits that already reused cards without a url keep the same id. The dev-scheduler test and the Apple reconnect fixture hash an empty asset list, which still matches.

The new test is sound for what it claims. It passes `assetCards` (those objects include `url`) through `devGenerationDigest` and requires the real `fetchGeneration` to accept the id. Putting `url` back into the hashed object makes that call throw.

1. **should-fix** — `host/web/dev.js:107` (`validName` at `host/web/dev.js:57`). Pre-existing, and outside this diff, but an entry plan still never applies on web. `captureGeneration` puts `plans/*.plan` on the asset roster (first generation, asset edit, and logic edit). `fetchGeneration` rejects any name that is not under `assets/`, `deck/`, or `shaders/` before it checks the digest, so the page keeps the build it booted. Apple allows those names, and the new digest matches Apple. The new test only uses `assets/0.txt` through `assets/2.txt`, so it stays green. Admit `plans/` in `validName`. The loader already fetches every asset card, and the digest already covers those cards.

2. **nit** — `host/web/serve.mjs:700`. The digest keeps the caller's asset order. Both clients sort by UTF-8 name, and `host/web/dev.mjs:512` sorts before the only call, so a published generation matches. The helper itself does not, and the test's names are already sorted, so dropping that sort would not fail the test and every client would report that the digest does not bind the manifest. Sort by UTF-8 name inside `devGenerationDigest`.
