# Vendored ibex2 and ibex2-sqlite — Exact patches

- **Upstream:** `https://github.com/expo/ibex.git`, commit
  `639de62de0ba473417dd85b8f4c61aa08dc07a78` (2026-09-11, on `main`):
  `crates/ibex2` → `vendor/ibex2`, `crates/ibex2-sqlite` → `vendor/ibex2-sqlite`.
- **Why vendored (Charlie, 2026-09-22):** a fresh clone must build without a
  sibling `../ibex` checkout. The compiler includes `src/bindings/storage.d.ts`
  as text, `exact-js` compiles `src/engine/ibex2_jsi.cc` and the binding
  scripts, and seven manifests depend on the crates.
- **Patches:** none. The copy is the commit's tracked tree, byte for byte,
  plus this file.
- **Not vendored:** the Hermes engine and `hermesc` builds. They are
  hand-built outputs in the ibex checkout (`ios/Frameworks-vanilla`,
  `tools/hermes-vanilla`, `linux-vanilla`), needed only by `exact-js`.
- **Update:** from a clean ibex checkout at the new commit,

  ```sh
  rm -rf vendor/ibex2 vendor/ibex2-sqlite
  git -C ../ibex archive --format=tar <commit> crates/ibex2 crates/ibex2-sqlite \
    | tar -x -C vendor --strip-components=1
  ```

  then restore this file with the new commit and date.
