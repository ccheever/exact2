---
name: 20261011-code-highlighting-languages
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-code-highlighting-languages
pr_url: null
verified_commit: null
---

# Every language the reference highlights is highlighted

## Outcome

The user decided on 2026-10-11 that this is built now, matching the reference T3 Code, and decided U15 (left open by
[shiki-residuals](closed/20261005-shiki-residuals.md)): take the reference's whole list and accept the size cost.

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| CH-1 | 25 grammars (`r12-render-grammar.ts`: typescript, tsx, javascript, jsx, json, jsonc, shellscript, python, css, html, markdown, yaml, toml, rust, go, swift; `r12-render-grammar-more.ts`: c, java, kotlin, csharp, xml, diff, docker, make, ruby). Every other language (cpp, haskell, julia, php, sql, …) paints with the heuristic tokenizer or plain text. haskell and julia were left out because their regexes need ES2024 class intersection and the pinned Hermes targets ES2018; cpp alone was measured at about 735 KB | `getSharedHighlighter` (`@pierre/diffs`, Shiki 4.2, the Oniguruma engine) loads any language of Shiki's bundle (`bundledLanguages`, its ids and aliases); a file's language comes from `getFiletypeFromFileName`; a fence word Shiki does not know paints as plain text, and `gitignore` is read as `ini` |

U15 sizes: measure the plan and the bundle (the data module's JavaScript and Hermes bytecode) before and after, and
report them. Hermes targets ES2018: every grammar must load there (transpile what does not convert).

## Steps

1. Reference first: the language list (`shiki` 4.2 `bundledLanguagesInfo`: ids and aliases), how a file name picks a
   language (`@pierre/diffs` `getFiletypeFromFileName`), how a fence word does (`ChatMarkdown.tsx`
   `extractFenceLanguage`), which embedded grammars and injections a language brings (`@shikijs/langs` module arrays,
   `injectTo`).
2. Generator (`tools/grammar/gen-grammar.mjs`): every grammar of `@shikijs/langs` 4.2.0 in one rule space (an include
   of another language resolves, as a registry that has it loaded resolves it), one chunk per grammar, parsed by the
   app only when a language first needs it; the ES2024 class intersections rewritten to ES2018 lookarounds before
   conversion; per language, the injections its own module loads.
3. Engine (`r12-render-textmate.ts`) and language resolution (`r12-render-highlight.ts`): the chunked data, the full
   file-name table, `gitignore` as `ini`.
4. Proof: `shiki-compare.mjs` against real Shiki (Oniguruma) for every language on the grammar samples corpus, real
   files for cpp, haskell, julia and a few others; every converted regex compiled by the pinned `hermesc`; tokenize
   speed and chunk load time in Hermes; sizes before and after.
5. Tests; one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| CH-1 languages | the language list equals Shiki 4.2's bundled list (ids and aliases); `shiki-compare.mjs` 0 differing characters per language, light, dark and italic; tests | before / after text records (counts, harness log) |
| CH-1 Hermes | every converted regex (all anchor variants) compiles in the pinned `hermesc`; tokenizing in Hermes | log |
| CH-1 sizes (U15) | plan bytes, `app.js` and `app.hbc` bytes, before and after | text record |
| CH-1 in the app | agent drive: chat fences and Files for cpp, haskell, julia and others, light and dark | before / after / reference images |

## Reference

- `apps/web/src/lib/syntaxHighlighting.ts` (`getSyntaxHighlighterPromise`, `PREFERRED_HIGHLIGHTER` "shiki-wasm", the
  fallback to "text"); `apps/web/src/components/ChatMarkdown.tsx:559-564` (`extractFenceLanguage`), `:1267-1298`;
  `apps/web/src/components/search/HighlightedSearchLine.tsx:176`; `apps/web/src/components/files/FilePreviewPanel.tsx:792`.
- `@pierre/diffs` 1.3.0-beta.10: `dist/highlighter/languages/resolveLanguage.js` (`bundledLanguages` from `shiki`),
  `dist/utils/getFiletypeFromFileName.js` (`EXTENSION_TO_FILE_FORMAT`).
- `shiki` 4.2.0 `bundledLanguagesInfo`; `@shikijs/langs` 4.2.0 (the grammars and each language's module array);
  `@shikijs/core` `Registry` (`injectTo`, scope-prefix injections); `vscode-textmate` `Grammar.getInjections`.
