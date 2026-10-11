---
name: 20261011-code-highlighting-languages
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: draft-pr
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

## Cause and fix

**The data.** `tools/grammar/gen-grammar.mjs` now compiles every grammar of `@shikijs/langs` 4.2.0 (253: the 235
languages of Shiki's bundle and 18 grammars Shiki packages only to embed or inject, `EMBEDDED_ONLY`) into one rule space,
as a vscode-textmate registry with every grammar loaded resolves an include: a grammar's context is made when it is first
compiled or included, so an include of another language (`source.js#expression`) resolves to that grammar's rule. Rules
are numbered grammar by grammar, and each grammar's rules, scope names and regexes are one chunk with global indices;
the index (`r12-render-grammar.ts`: languages, aliases, per-language injections, chunk ranges, the themes) imports the
chunk files `r12-render-grammar-1.ts` … `-9.ts` (each under 1,500 lines, a function per chunk returning its JSON).
`r12-render-textmate.ts` parses a chunk only when a rule of it is first needed (`ruleOf`, a binary search over the
ranges), so a language costs nothing until it is highlighted (all 253 parse in 29 ms in Hermes; the largest 2 ms).
Injections per root language follow vscode-textmate's `Grammar.getInjections` and Shiki's `Registry` (the grammar's own,
then each grammar its module loads whose `injectTo` names the root scope or a dotted prefix of it).

**vscode-textmate's rule kinds, now exact.** Running all 231 sample languages against real Shiki found three generator
differences, each fixed as vscode-textmate's `RuleFactory` does it: an include that resolves to nothing is left out of a
pattern list, and a begin/end, begin/while or include-only rule whose list lost an entry that way and kept none is left
out too (`dropMissingPatterns`; asm's strings include keys asm does not define, so Shiki never matches them); an empty
`end` never matches (`'\uFFFF'`; bicep's decorator); a falsy `match` makes no match rule and `patterns` win over
`include` (haxe, hxml). Captures that are not objects (racket's strings, wikitext's nulls) name nothing.

**ES2018 for Hermes.** 65 regexes (haskell 41, stata 16, purescript 5, julia, matlab, racket) use Oniguruma class
intersection or a nested class oniguruma-to-es writes only with the ES2024 `v` flag. Such a class matches one character,
so the generator rewrites it as lookarounds over plain classes (`[A&&B]` → `(?:(?=B)A)`, `[^A&&B]` →
`(?:(?!(?=B)A)[\s\S])`, `[a[B]]` → `(?:[a]|B)`), converts each class alone and splices it into the converted pattern at a
placeholder character (Oniguruma's parser refuses a lookaround inside a lookbehind; JavaScript allows it, and a lookahead
there still tests the class's one character). Every form compiles in the pinned `hermesc` and in Hermes.

**Unicode word boundaries.** Shiki's JavaScript engine reads `\b` as ASCII; the reference runs Oniguruma, whose `\b` is
Unicode: `__밑줄__과` is no emphasis there and `2π` is one number. The Unicode form costs about twice the time in Hermes
(asm and fortran 5 to 7 times) and on an ASCII line reads every `\b` as the ASCII form does, so a regex whose Unicode
form differs carries it as `u` (6,603 of them) and the engine uses it only on a line with a character outside ASCII.

**Slices.** shiki-residuals counts a turn's budget in characters (2,500, about TypeScript's 36 ms in Hermes). Now cpp's
characters weigh 2 (37,000 a second in Hermes on real files), a non-ASCII line's 2 (asm 7, d 6, fortran 5 and 6, surrealql
4, codeql, cpp, llvm, swift and system-verilog 3: `UNICODE_SLOW_LANGUAGES`, measured over every language's sample with
Korean text on each line), a chunk parse and each first-use regex compile are charged as work (a Unicode property in a
pattern costs four units), and a line whose compiles would pass what is left of the turn stops before the compile and is
tokenized again from its start next turn (`tokenizeLineWithin`; a line only reads its start state), so a line that first
meets a big list (asm's 280 Unicode forms, 427 ms in one line before) spreads over turns.

**Languages.** `shikiLanguage` resolves a fence word through the 332 ids and aliases and a file name through
@pierre/diffs' whole table (`r12-render-filetypes.ts`, 345 entries, every value a language here; `x.h` is objective-cpp,
`notes.1c` bsl), and `gitignore` as `ini` (`extractFenceLanguage`). A word or name Shiki does not know paints plain, as the
reference falls back to "text": the heuristic tokenizer now only paints a text whose Shiki tokens are still being computed
(before, ```` ```h ```` and other words Shiki lacks got its colours).

Licences: `LICENSE-T3` lists each grammar's upstream licence (tm-grammars 1.31.15's records): MIT 176, Apache-2.0 18,
BSD-3-Clause haskell, ISC emacs-lisp, MPL-2.0 bird2, hcl and terraform, GPL-3.0 ada, gnuplot, nginx and racket,
NOASSERTION 11, none stated 21; see "Decision needed".

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| CH-1 languages | pass: 235 languages and 97 aliases, the clone's 332 fence words equal Shiki 4.2's `bundledLanguages` keys (`shiki-compare --list`; before 25 grammars). Against real Shiki (Oniguruma), light, dark and italic of every non-blank character: the samples corpus 231 languages, 177,168 characters, 0 differing; the 4 without a sample (git-commit, git-rebase, html-derivative, nextflow-groovy) 0; real files for 12 languages (cpp 5 files, objective-c 4, objective-cpp 4, haskell 4, julia 4, sql, perl 2, php, vue, astro 2, proto, scss) 214,594 characters, 0; the 40-file TypeScript corpus 169,771 characters, 0 (regression guard, as in shiki-residuals); Korean markdown, Unicode identifiers, julia `2π` 0 (with ASCII `\b` only: 6 and 2) | [ch1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/93dd5a4fb1d8e4144d44d31a25db6fcf5c21ee0c/code-highlighting-languages/ch1-record.txt), [samples log](https://raw.githubusercontent.com/ccheever/exact2/873840eaaed21297f18fd0f3ee0c264f0322e8c6/code-highlighting-languages/ch1-samples-harness.log), [real-files log](https://raw.githubusercontent.com/ccheever/exact2/5b15b11b20845233ac2efaf7a28c80d9d586827d/code-highlighting-languages/ch1-real-files-harness.log) |
| CH-1 Hermes | pass: 21,920 regexes, 29,488 distinct forms; the pinned `hermesc` compiles all as literals (exit 0) and vanilla Hermes 1.0.0 builds all with `new RegExp(s, f + 'dg')` (0 failures); no form has the `v` flag; slices of the app's slicing stay near 50 ms (TypeScript 211 KB max 52 ms, cpp 59, Korean-commented TypeScript median 40, asm with Korean max 41; before the work limit up to 427 ms in one line) | ch1-record.txt |
| CH-1 sizes (U15) | measured: grammar source 1,130,900 → 11,738,851 bytes; `app.js` 4,991,714 → 15,595,454; `app.hbc` 6,174,159 → 13,222,347; the bake's `app.plan` 28,854,201 → 28,854,110 (the plan holds no grammar data); `T3 Code (Exact).app` 156,944 → 163,856 KB; `contract build` in Checks | ch1-record.txt |
| CH-1 in the app | pass (agent drive, Files surface, 1280×840, light and dark): before, cpp and the C-like heuristic colours, haskell, julia, php and sql plain, the Korean markdown's `__밑줄__` bolded; after, each file's colours equal the reference's; the one difference is the declared one (the markdown's ` ```sh ` fence is coloured in the clone, plain in a fresh reference window) | [cpp](https://raw.githubusercontent.com/ccheever/exact2/763f19bdfc24f25dc27c1aad9317ab394762bca4/code-highlighting-languages/ch1-cpp.png), [haskell](https://raw.githubusercontent.com/ccheever/exact2/0786c1ec915d4c166d0e61f53a28c2135244f130/code-highlighting-languages/ch1-haskell.png), [julia](https://raw.githubusercontent.com/ccheever/exact2/216dbfde58c4969341ec422c51a89bca5964f774/code-highlighting-languages/ch1-julia.png), [php](https://raw.githubusercontent.com/ccheever/exact2/c7ac86395f0286168648b40ef7c754e074febbdd/code-highlighting-languages/ch1-php.png), [sql](https://raw.githubusercontent.com/ccheever/exact2/4b9ac6b018feacb5d6290bec0e5c594c02c27b2b/code-highlighting-languages/ch1-sql.png), [markdown](https://raw.githubusercontent.com/ccheever/exact2/5945c0b014d68f84e590cf3379101e280aee94d9/code-highlighting-languages/ch1-markdown.png) |

Drives: before on `t3-code-evidence-base` at `6db5cadea` (lane `code-highlighting-languages-before`), after on this
branch's bundle (the app code of `60a281a74`; lane `code-highlighting-languages`), the reference T3 Code `1e2ecbd975` on the
same lane (CDP 16541), each with the Verification fixture project pointed at a lane-private copy of
`verify-runtime/project` holding the six files at its root (a tree id with a slash reads as an agent part), both lanes'
Codex and Claude at missing binaries. Exit 0 each; one run each (the before drive's first two tries stopped at the drive
script's ids, before any after drive).

## Real-input batch steps

None: the rows are colours, which the agent drive's `screenshot … window` reads as the screen shows them.

## Tests

- `code-highlighting-languages.test.ts` (14): the language list (235, 97 aliases, 253 chunks; no embedded-only grammar or
  `Object.prototype` name is a language); cpp, haskell, julia, php in html, sql, lua and vue with its injections as Shiki
  paints them; file names as @pierre/diffs picks them, every table value a language; a fence word Shiki lacks paints plain,
  `gitignore` as ini; every regex form has ES2018 flags and compiles; Unicode `\b` on non-ASCII lines (`__밑줄__과`, `2π`)
  and the ASCII form on ASCII ones; a chunk parses on first use and counts as work; cpp weighs double.
- `r12-render.test.ts`, `r12-render-residuals.test.ts`: `cobol` and `x.h` now have grammars (the plain-fence case uses a
  word Shiki lacks); both reset slicing before each test (an app test may leave it on, and a first use then defers).
- `tools/grammar/gen-grammar.test.ts`: the committed data is the generator's output byte for byte (`--check` too); the six
  pins.

## Checks

(filled in below after the final run)

## Not done / not verified

None.

## Decision needed

1. **Grammar licences.** The full list brings 4 GPL-3.0 grammars (ada, gnuplot, nginx, racket), 3 MPL-2.0 (bird2, hcl,
   terraform) and 32 with no licence stated or NOASSERTION into the clone's data (`LICENSE-T3` lists them). The reference
   ships the same grammars inside Shiki. Keep them, or leave some out (a list in `gen-grammar.mjs`, like `EMBEDDED_ONLY`)?
2. **Load order.** The reference colours a lazily embedded language (a markdown file's fences) only once its window has
   loaded that language, and applies an injecting grammar (vue, ts-tags, angular) to other roots only after it has been
   opened; the clone holds every grammar from the start (EXACT2-GAPS "Code colours: every grammar loaded from the start";
   visible in the markdown image). Keep, or model the window's loaded set?

## Seen while building, not a finding row

- @pierre/diffs does not tokenize a line over 1,000 characters in Files and diffs (`tokenizeMaxLineLength = 1e3`,
  `DiffWorkerPoolProvider.tsx`); the clone tokenizes it.
- The reference looks a fence word up as written (```` ```Python ```` is plain there); the clone also tries it lowercased.
- A nested `docker/Dockerfile` has no language in the reference (@pierre/diffs matches a whole name only at the root); the
  clone matches a capitalized base name at any depth.
- Chrome slants Korean in an italic run; the clone's Korean italic text stays upright (the font has no italic).
- Dark images: the file tree's labels and the sidebar title keep the light raster after the live switch (X82, known).

## Delivery

Draft PR (below).

## Next action

Review; the two decisions above.
