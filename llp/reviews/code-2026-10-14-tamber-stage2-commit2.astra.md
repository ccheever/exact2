# Code review: LLP 1121 stage 2, commit 2 (the compiler reads designs), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `7aa7231a51f8e27dc8544c3eb36cbb31ec12d2bb8872ffe90723af392ee45d3d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit (d4ed14f26)

---

1. **MAJOR — manifest-selected designs cannot use the normal build tools.** [sources.rs:142](/tmp/rv-s2c2/contract/cli/src/sources.rs:142) accepts `"tamber"`, but `scripts/app.schema.json` omits it and forbids unknown keys. The existing manifest validator returns **`tamber: not a known key`**, blocking build/dev commands. Add the schema entry and test through the manifest reader.

2. **MAJOR — JS bindings cannot resolve the new token forms.** [host/web-js/src/style.rs:457](/tmp/rv-s2c2/host/web-js/src/style.rs:457) copies colour functions unchanged and splits identifiers at dots. Even with the appropriate table entries, both `surface.card` and `color-mix(in srgb, brand 50%, #ffffff)` remain unresolved. This affects computed rows, gradient stops and shadows; the browser receives invalid colours. I reproduced both failures with the repository’s mapper.

3. **MAJOR — profile stand-ins can silently change colours.** [theme.rs:148](/tmp/rv-s2c2/contract/lower/src/theme.rs:148) checks profile usage **before** adding synthetic tokens, so a row’s `color-mix()` can fold the sentinel instead of refusing profile arithmetic. Aliases also bypass the direct-name check. Additionally, [theme.rs:298](/tmp/rv-s2c2/contract/lower/src/theme.rs:298) uses valid authored colours as sentinels: a first profiled token and an unrelated `faint="#00000101"` cause `faint` to become that profile colour. Profiles need an opaque representation, including through aliases.

4. **MAJOR — literals inside calls never become synthetic tokens.** [scan.rs:164](/tmp/rv-s2c2/contract/lower/src/theme/scan.rs:164) returns without traversing call arguments, lists and other expression forms; `let` initializers and action bodies are also missed. For example, `color=echoColor("light-dark(brand, card)")`, with an identity function, retains the ordinary tokens but never creates the required synthetic entry. The runtime cannot resolve that expression. Traverse reachable literal subexpressions.

5. **MAJOR — unrelated strings can make an app fail compilation.** [scan.rs:126](/tmp/rv-s2c2/contract/lower/src/theme/scan.rs:126) synthesizes colours from every collected string, regardless of its use. With a `brand` token, a text-only state such as `state label = "light-dark(brand, CanvasText)"` generates `tamber-residual`. Conservative name retention is reasonable; evaluating arbitrary text as a colour is not.

6. **MINOR — design diagnostics lose their actual source location.** [theme.rs:370](/tmp/rv-s2c2/contract/lower/src/theme.rs:370) returns a default span for file designs. Compiler-owned refusals such as reserved `top`, invalid `LinkText` overrides and profile misuse consequently point at `app.contract:0:0`, without the design’s location. Crate refusals put the location only in the message; their structured file/span remain wrong.

7. **MINOR — diagnostic remapping misses conditional literals.** [values.rs:497](/tmp/rv-s2c2/contract/lower/src/values.rs:497) passes the original attribute to `token_error`. Thus `color=flag ? "brnad" : "brand"` retains `lower-attr-value`, because the wrapper sees a ternary rather than the offending string. Apply the mapping where each literal is checked.

8. **MINOR — retention invalidates an existing JS test fixture.** [style.rs:977](/tmp/rv-s2c2/host/web-js/src/style.rs:977) declares `brand` and `ink` without any colour usage, then expects their mappings. They are now pruned. Give the fixture a computed colour reader and extend it to dotted and synthetic names.

By inspection, the native row, gradient-stop, shadow and border parsers preserve the required function substring. `theme::resolved` adds no allocation itself, but its additional linear searches are not covered by allocation counts.

Bun reproduced findings 1–2. Cargo tests could not start because the read-only sandbox rejected build-directory creation; app regressions and allocation tests remain unverified. `git diff --check` passed. No files were changed.

VERDICT: fix
