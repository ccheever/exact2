# Code review: The params section (LLP 1121 stage 7 A), the third re-check, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `dcf53dd87`, `a977d91fc` and `cc7f4da74` (landed as `1d119e9b8`, `d2f0c6ffe`, `c2aea5bbc`).
- **Method:** one brief (sha256 `4fe80ec5bb7ac5e320ff76d7cab925869c633875278ea64ed41606db7d582d23`), blocking defects only. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** BLOCKING (a symbol source behind a shared derive refused, a failed load silencing check, unnamed external packages by basename, image-source conditions unchecked).
- **Disposition:** theme2 ruled A pushed before this check ended (16:38) and its findings landed as a follow-up: all four in the third re-check's fixes commit, each with a test (`tamber_params_tools.rs`).

---

1. **Valid image sources behind a generated `let` are refused.** [rewrite.rs:72](/tmp/rv-s7a-r2e/contract/lower/src/params/rewrite.rs:72)

   ```contract
   component App
     params
       glyph: icon = "icon(sf \"heart\")"
     state show = true
     view
       Child(glyph="param.glyph", show=show)
   component Child
     props
       glyph: string
       show: bool
     derive flag = show
     derive picture = flag ? `symbol:${glyph}` : (flag ? "symbol:star" : "symbol:heart")
     view
       image picture
   ```

   Expansion shares `flag` through `Expr::Let`. `source_parts()` classifies that entire expression as nonsymbolic, so `reached()` incorrectly reports `tamber-param-scope` for `glyph`. The initializer contains no param; the param supplies a symbol source through the body.

   **Fix:** handle `Let` explicitly: inspect its initializer as data, then classify its body’s source-producing branches.

2. **A loading refusal makes `check` silently omit every param.** [tamber.rs:152](/tmp/rv-s7a-r2e/contract/cli/src/tamber.rs:152), [tamber_params.rs:53](/tmp/rv-s7a-r2e/contract/cli/src/tamber_params.rs:53)

   Beside `app.tamber` containing `tamber t`, use:

   ```contract
   component App
     params
       fill: color = "#ffffff" control="slider"
       spare: color = "#000000"
     view
       text "x" color="param.fill"
   ```

   `contract tamber check app.contract` reports success. Loading encounters `tamber-param-unused` for `spare`; `params_of()` replaces that failure with empty params. Admission then builds the dummy app, missing both the unused param and the invalid slider control.

   **Fix:** propagate loading failures to the CLI. A failed param load must not select standalone admission.

3. **Distinct unnamed external packages collapse to one identity.** [sources.rs:228](/tmp/rv-s7a-r2e/contract/cli/src/sources.rs:228)

   Put packages at `/tmp/libs/a` and `/tmp/libs/b`, each with:

   ```json
   {"exports":{".":"./index.contract"}}
   ```

   Each `index.contract` declares `Card.radius`, uses it in `border-radius`, and defaults it to `"4px"` and `"8px"` respectively. Link them as `/tmp/app/node_modules/a` and `b`, then build:

   ```contract
   use Card as A from "a"
   use Card as B from "b"
   component App
     view
       column
         A()
         B()
   ```

   With no named ancestor manifest, both canonical files fall through to the basename `index.contract`. Both params become `index.contract#Card.radius`, producing duplicate tokens and rejecting otherwise valid imports.

   **Fix:** retain a distinct, route-independent canonical path for unnamed external packages; never reduce it to the basename.

4. **Image-source conditions bypass the required scope refusal.** [rewrite.rs:296](/tmp/rv-s7a-r2e/contract/lower/src/params/rewrite.rs:296)

   ```contract
   component App
     params
       fill: color = "#ffffff"
     view
       column background-color="param.fill"
         image ("param.fill" == "#ffffff" ? "symbol:heart" : "symbol:star")
   ```

   This is accepted instead of producing `tamber-param-scope`. `source_parts()` visits the condition with `false`, but `view()` discards that visit. The condition retains raw `"param.fill"`, which `reached()` cannot recognize because it checks internal token names. Match subjects have the same gap.

   **Fix:** scope-check image conditions and match subjects as computations, while preserving nonsymbol value branches such as URLs.

Source review only; no files changed or builds/tests run.

BLOCKING
