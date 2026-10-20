# Code review: The params section (LLP 1121 stage 7 A), the round-2 re-check, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `dcf53dd87`, the second round applied without re-review (theme2's sign-off condition: one quick re-check).
- **Method:** one brief (sha256 `ff2696c7494b551f28c7dbeff27fd80a856accd253f58c8ef8a9875cbfe0d24a`), blocking defects only. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** BLOCKING (image rows gated as sources, a mixed image ternary, quoted template holes refused, a symlinked package's identity by route, admission of a design named apart, package diagnostics by identity).
- **Disposition:** all six in `a977d91fc`, each with a test (`tamber_params_tools.rs`): the source gate on `src` and the positional only, per `?:` branch; a quoted hole left as written; identity from the nearest named `package.json` below the app's directory, by canonical path alone; the app built only for its own design; diagnostics by the declaring source's path. A second quick re-check follows.

---

1. **Image styling params are skipped.** [contract/lower/src/params/rewrite.rs:251](/tmp/rv-s7a-r2c/contract/lower/src/params/rewrite.rs:251), also line 298.

   ```contract
   component App
     params
       fill: color = "#ffffff"
     view
       image "https://example.com/photo.png" background-color="param.fill"
   ```

   This produces `tamber-param-unused`: the new `symbolic()` guard applies to every image attribute, including styling rows. Passed through a child prop, the same styling use is incorrectly refused by `reached`.

   **Fix:** apply the symbol-source restriction only to `src` and the positional source, in both walkers.

2. **A conditional image source still corrupts URLs.** [contract/lower/src/params/rewrite.rs:46](/tmp/rv-s7a-r2c/contract/lower/src/params/rewrite.rs:46).

   ```contract
   component App
     params
       fill: color = "#ffffff"
     state show = false
     view
       column background-color="param.fill"
         image (show ? "symbol:star" : "https://example.com/param.fill")
   ```

   One symbolic branch makes the entire expression eligible for rewriting. `texts()` then rewrites the URL branch too, replacing `param.fill` with the internal token. Astra finding 2 remains unfixed for this input.

   **Fix:** classify and rewrite source-producing branches separately, preserving URL branches.

3. **Quoted template holes are incorrectly refused.** [contract/lower/src/params/rewrite.rs:196](/tmp/rv-s7a-r2c/contract/lower/src/params/rewrite.rs:196).

   With `fill: color = "#ffffff"` declared and read by a background row:

   ```contract
   text "x" font-family=`"${"param.fill"}", system-ui`
   ```

   This emits `tamber-param-scope`. Entering the quoted hole changes `value` to false, but its string starts with a fresh quote state; `param.fill` is consequently treated as forbidden computation instead of literal family-name text.

   **Fix:** preserve quoted context when visiting holes; leave original quoted text unchanged while retaining the separate check for internal tokens passed through props.

4. **Symlinked package identity still depends on import order.** [contract/cli/src/sources.rs:210](/tmp/rv-s7a-r2c/contract/cli/src/sources.rs:210).

   Put a package named `pkg`, exporting `./index.contract`, at `vendor/pkg`; let `node_modules/pkg` symlink to it. Its `Card` declares and uses `radius`. Import and instantiate both aliases:

   ```contract
   use Card as A from "./node_modules/pkg/index.contract"
   use Card as B from "pkg"
   ```

   Relative-first loading assigns `vendor/pkg/index.contract#Card`; reversing the imports assigns `pkg/index.contract#Card`. Canonicalization removes the `node_modules` ancestry, leaving ownership dependent on `Origin`; the cache retains the first identity.

   **Fix:** determine canonical package ownership independently of the first import route before assigning and caching identities.

5. **Admission can check a different design from the requested file.** [contract/cli/src/tamber.rs:221](/tmp/rv-s7a-r2c/contract/cli/src/tamber.rs:221).

   Beside a valid `app.contract` with a used literal color param and no `app.tamber`, create `bad.tamber`:

   ```tamber
   tamber t
   axes
     dark-style = black | dim under scheme=dark
   theme
     Canvas = light-dark(#ffffff, #000000)
       when dark-style=dim: light-dark(#ffffff, #161e27)
   ```

   `contract tamber check bad.tamber` passes admission: the new branch ignores `src` and builds the app without this design. It misses the required `tamber-role-axis` refusal.

   **Fix:** compile the app with the explicitly selected design text and path installed for that admission.

6. **Package diagnostics name a nonexistent file.** [contract/cli/src/tamber_params.rs:50](/tmp/rv-s7a-r2c/contract/cli/src/tamber_params.rs:50).

   Import `Card` from package `pkg`, physically at `node_modules/pkg/index.contract`, with:

   ```contract
   component Card
     params
       body: type = "16px/1.4 system-ui"
     view
       text "x" font="param.body"
   ```

   `check` and `describe` locate the fixed-pixel warning at `pkg/index.contract:3`, rather than its declaring file. Component identity is not a filesystem path.

   **Fix:** retain source paths in `AppParams` and relocate using the default span’s `source_id`.

Source review only; no files changed or build/tests run.

BLOCKING
