# Code review: The params section (LLP 1121 stage 7 A), the second re-check, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `dcf53dd87` and `a977d91fc`, the second round and the first re-check's fixes.
- **Method:** one brief (sha256 `462acd38a5f81e710b177835efc92ef76d1736e4370e1df1c6abacbd01616b3d`), blocking defects only. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** BLOCKING (a child's shared derive comparing a passed token, a params-only app's admission, image `match` arms, a design named apart folding the app's params).
- **Disposition:** all four in the second re-check's fixes commit, each with a test (`tamber_params_tools.rs`): a `let`'s value is data; image sources descend `match` arms and `"symbol:" + …`; `check` decides ownership before folding, so a design named apart is folded and admitted alone and a params-only app is built.

---

1. **A generated `let` bypasses the scope check.** [contract/lower/src/params/rewrite.rs:235](/tmp/rv-s7a-r2d/contract/lower/src/params/rewrite.rs:235)

   ```contract
   component App
     params
       fill: color = "#ffffff"
     view
       Child(tint="param.fill")
   component Child
     props
       tint: string
     derive saved = tint
     derive paint = saved == "#ffffff" ? saved : "#000000"
     view
       text "x" color=paint
   ```

   Child expansion shares `saved` using `Expr::Let`. The walker treats its initializer as a row value, then ignores the identifier reads—including the comparison. `reached()` therefore misses the forbidden use. The comparison receives the internal token string and selects black instead of producing `tamber-param-scope`.

   **Fix:** track `let` bindings through their identifier reads and check the bound value in each consuming context.

2. **Apps without a `.tamber` file bypass param admission.** [contract/cli/src/tamber.rs:146](/tmp/rv-s7a-r2d/contract/cli/src/tamber.rs:146)

   With only this `app.contract`, run `contract tamber check app.contract`:

   ```contract
   component App
     params
       fill: color = "#ffffff" control="slider"
     view
       text "x" color="param.fill"
   ```

   `design_path()` returns `None`, making `own` false. Admission compiles the empty dummy app, so `check` reports success despite the required `tamber-param-control` refusal. The initial fold does not check param metadata.

   **Fix:** build the real app when checking its synthesized, params-only design too; distinguish that case from an explicitly selected unrelated design.

3. **Valid image-source `match` arms are skipped.** [contract/lower/src/params/rewrite.rs:53](/tmp/rv-s7a-r2d/contract/lower/src/params/rewrite.rs:53)

   ```contract
   component App
     params
       glyph: icon = "icon(sf \"heart\")"
     view
       image (match some(true) { case some(x) => "symbol:param.glyph", case none => "symbol:star" })
   ```

   `source_parts()` only descends through ternaries. The entire `match` is classified as nonsymbolic, so rewriting and usage collection skip it and incorrectly produce `tamber-param-unused`.

   **Fix:** classify the value-producing arms of `match` separately, as for ternaries, and preserve the admitted value flow through `let` and `+`.

4. **A separately named design still folds the neighbouring app’s params.** [contract/cli/src/tamber.rs:115](/tmp/rv-s7a-r2d/contract/cli/src/tamber.rs:115)

   ```contract
   component App
     params
       fill: color = "accent.fill"
     view
       text "x" color="param.fill"
   ```

   Beside this, let `app.tamber` declare `accent.fill = #ffffff`, and let `other.tamber` contain only `tamber other`. `contract tamber check other.tamber` incorrectly fails because `accent.fill` is missing: the initial fold adds the app’s params before the later `own` check. The disposition’s standalone admission is therefore incomplete.

   **Fix:** determine design ownership before folding and omit neighbouring app params when checking a separately named design.

Source review only; no files changed or builds/tests run.

BLOCKING
