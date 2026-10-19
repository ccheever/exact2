# Code review: the dev loop's generation identity, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `5862196aa`, `8830a5c32`.
- **Method:** one brief (sha256 `4fd9b864107943e7cbe4c1670c98aaeb45efb58703bb62210ae7b50b2c3635fc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nothing found).
- **Disposition:** none needed.

---

Round 2 does what it says. `validName` in `host/web/dev.js` now admits `plans/`, so an entry plan is no longer rejected before the digest check. `devGenerationDigest` sorts asset cards by UTF-8 name itself, and the hash is only each card’s name, sha256, and length.

That digest matches both clients that actually check `dev.generation`:

- The web page (`host/web/dev.js` `fetchGeneration`) rebuilds the same canonical object and hashes `JSON.stringify` of it. Asset order is UTF-8. Object keys come out in the same order `canonicalBytes` sorts them (`assets`, `module`, `plan`, `rust`; inside a card, `bytes`, `name`, `sha256`).
- Apple (`host/apple/Sources/ExactKit/PlanURL.swift`) builds that same string by hand and compares its SHA-256 to `dev.generation`. Names are UTF-8-sorted. `plans/widget.plan` passes `safeName`. Rust variants are `module`, `receipt`, and `target` only when the kind is not `wasm`.

A direct comparison of those three encodings agreed for a reversed card list that still carried `url`s, a `plans/` entry, a non-ASCII name, a module trio, and wasm plus native plus tiered Rust. Logic-edit cards (no `url`) and disk cards (`assetCards` adds `url: ./…`) hash the same when name, sha256, and length match. That covers the first generation and asset edits (`captureGeneration()` recaptures from disk), logic edits (`captureGeneration(true)` reuses the last cards, then `assetCards` on `plans/`), and mixed apps (module and Rust urls are stripped; `target` is included only for non-wasm).

Linux and Android do not check this identity. Both poll through `host/linux/src/fetch.rs` `fetch_changed` (the Android reader uses the same `poll_development` path). Their skip fingerprint is the plan sha256 and length, plus each Rust card’s sha256, length, and target. It ignores `dev.generation` and ignores assets, so this digest change does not affect them.

Nothing else was keyed on the old bytes. The generation cache directory is a hash of app id, path, and dist. Retained generations are stored by epoch and seq; readers check file bytes against the card sha256, not by recomputing the generation id. Module and Rust receipts do not include it. The other tests that mint a generation id (`host/web/tests/dev-scheduler.test.mjs`, the older case in `scripts/caps.test.mjs`) hash the client canonical directly, which still matches.

The new test is sound for what it claims. It runs the real `fetchGeneration`, which throws unless the digest matches, and the fixture cards carry urls, a `plans/` name, and reversed order. It does not include a Rust section; that projection matches the clients when checked separately.
