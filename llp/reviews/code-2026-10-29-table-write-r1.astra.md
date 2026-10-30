# Code review: game dev loop: identical table asset not rewritten, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e95fbfa507`.
- **Method:** one brief (sha256 `9fd1a0d186f59bce38ebff2527058d1c1f3af92246216cb334967fb0a6b4b1e2`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [game/app/tables.mjs:56](game/app/tables.mjs:56): **Unchanged tables can still alternate between two serializations.** The watcher serializes declaration order at line 66; the Rust baker uses a sorted `BTreeMap` ([files.rs:33](game/bake/src/files.rs:33)). The validator returns its input unchanged ([lib.rs:569](game/render/src/lib.rs:569)). With declarations ordered `weapons.json`, `labels.json`, each bake and reload rewrites the other’s ordering, preserving the expensive rebuild cycle. Use the same ordering/serialization in both writers, and cover a Rust-produced carrier with two declarations in reverse alphabetical order.

2. [game/app/tables.mjs:56](game/app/tables.mjs:56): **Decoded-string equality does not meet the byte-equality condition.** UTF-8 decoding replaces malformed bytes: an existing invalid byte can compare equal to a legitimate U+FFFD in `text`, leaving different, malformed bytes on disk while publishing valid bytes. Compare `readFileSync(path).equals(Buffer.from(text))`; retain the ENOENT-only exception and atomic replacement.

**Non-blocking**

1. [host/web/tests/tables.test.mjs:85](host/web/tests/tables.test.mjs:85): Backdating the directory avoids timestamp-granularity problems, and the inode check supports detecting replacement. However, an in-place rewrite preserves both checked values while changing the file’s mtime, which Cargo also watches. Backdate and assert the file’s mtime too; include the dist copy if claiming coverage of both destinations.

2. [host/web/tests/tables.test.mjs:182](host/web/tests/tables.test.mjs:182): The assertion sees the app-wasm Cargo output: its build has no `-q`, inherits stderr, and the dev wrappers relay it. However, inherited quiet/color settings can make this negative substring check pass vacuously. Force nonquiet, uncolored output and assert a positive build-completion marker. Also narrow line 177’s “compiles nothing of the app’s own” to **app wasm**: the resident producer uses `run -q` ([app.mjs:1255](scripts/app.mjs:1255)) and finishes after this assertion. Removing the old release-profile comment is correct.

3. The commit’s audit needs qualification. [.shells/shaders is written](game/app/shells.mjs:180) and [watched by render builds](game/games/wind-fixture/render/build.rs:10); its unchanged writes are already suppressed. [Linux proofs:520](game/proof.mjs:520) unconditionally copy `surfaces.json` and `keys.json`, although that path is outside this web loop. The loop also writes [.exact/status.json](game/status.mjs:35), so “target/ and dist/ only” is too broad. Document these distinctions; no additional unconditional write into the listed watched paths surfaced in the ordinary web loop.

4. [QUEUE.md:3](QUEUE.md:3): Distinguish the two wasm inputs. `wasm-opt` processes the unchanged app wasm, but [wasm-bindgen:415](scripts/app.mjs:415) processes the GPU wasm, which gameplay changes can alter. Any bindgen reuse must depend on that artifact’s own inputs, not the app wasm remaining unchanged.

The remaining skip behavior is sound: non-ENOENT errors refuse the flush, changed content still uses atomic rename, and [publication remains unconditional after successful validation](game/app/tables.mjs:69). [Generation publication](host/web/dev.mjs:1221) uses explicit bytes and announcements, so unchanged dist mtimes do not prevent live swaps.

Static review only; no tests, builds, or network access.
