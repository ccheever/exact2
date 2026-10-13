---
name: 20261011-code-highlighting-languages
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-code-highlighting-languages
pr_url: https://github.com/ccheever/exact2/pull/466
verified_commit: 1f813789b
---

# Every language the reference highlights is highlighted

## Outcome

The user decided on 2026-10-11 that this is built now, matching the reference T3 Code, and decided U15 (left open by
[shiki-residuals](20261005-shiki-residuals.md)): take the reference's whole list and accept the size cost.

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

**Review round (2026-10-12): a retried line always finishes.** The review found the limit could loop: an end or while
regex made from a begin match (markdown `**` and `*`, rust `r#"…"#`) was a fresh entry each time, so a retry compiled it
again, and a line longer than what was left of the turn (limit below zero) allowed one compile per try and stopped at
the second; such a line never finished and every slice tokenized it again. Now such a regex is cached by its resolved
source (as Shiki's JavaScript engine caches compiled patterns) and never stops a line, though its compile still counts
as work; only a grammar regex stops a line, and those stay compiled, so each try gets further. A line that starts its
turn may also do half a turn of one-time work whatever its own cost (`FIRST_LINE_WORK`), so a long line on a grammar's
first use takes a few turns, not one per regex (a 3,618-character JavaScript line: 141 turns before, 6 now; Hermes
slices at most 40 ms). The 3,165-character markdown emphasis line, a 1,200-character Korean one and a 2,880-character
rust raw-string line, which never finished, finish in 2, 3 and 1 turns.

**Languages.** `shikiLanguage` resolves a fence word through the ids and aliases as written (Shiki's `bundledLanguages`
keys are lower case: ```` ```Python ```` and ```` ```Dockerfile ```` are plain, as in the reference; review round) and
`gitignore` as `ini` (`extractFenceLanguage`); `fileLanguage` is @pierre/diffs' `getFiletypeFromFileName` on the path as
the reference passes it: the whole path as a name first (a root `Dockerfile`, `CMakeLists.txt`, `CODEOWNERS`; a nested
one has no language, review round), then a compound and a simple extension, over the whole table
(`r12-render-filetypes.ts`, 345 entries; `x.h` is objective-cpp, `notes.1c` bsl). The Files panel, diffs, the attachment
code preview and search lines resolve a path with `fileLanguage`. A word or name Shiki does not know paints plain, as the
reference falls back to "text": the heuristic tokenizer now only paints a text whose Shiki tokens are still being computed
(before, ```` ```h ```` and other words Shiki lacks got its colours).

**Long lines in Files and diffs (review round).** @pierre/diffs renders Files and diffs with `tokenizeMaxLineLength`
1,000 (its default, and what `DiffWorkerPoolProvider.tsx` sets): Shiki paints a line that long or longer plain and starts
the next line in the state before it. `lineTokens` (Files, diffs, attachment previews) passes
`TOKENIZE_MAX_LINE_LENGTH`; a chat code block and a search line keep no limit, as there.

Licences: `LICENSE-T3` lists each grammar's upstream licence: tm-grammars 1.31.15's records, and where those say
NOASSERTION (11) or nothing (21) the upstream repository's LICENSE, package.json or README, read on 2026-10-12 (review
round): MIT 186, Apache-2.0 22, llvm Apache-2.0 with LLVM exceptions, BSD-2-Clause apache and matlab, BSD-3-Clause apex
and haskell, ISC apl and emacs-lisp, the TextMate bundles' permission notice for nine (abap, applescript, erb, logo,
mipsasm, po, ssh-config, toml, yaml), MPL-2.0 bird2, hcl and terraform, GPL-3.0 ada, gnuplot, nginx and racket, and no
licence found for dax, dream-maker, glsl and tcl.

**Round 3 (2026-10-12): the coordinator's decisions.** (1) Licences: the generator leaves out the four GPL-3.0 grammars
and the four with no licence (`LICENCE_HELD`: ada, gnuplot, nginx, racket; dax, dream-maker, glsl, tcl). glsl was looked
for again and has none: `polym0rph/GLSL.tmbundle` (tm-grammars' source) has no LICENSE file, no GitHub licence, nothing
in its README or `info.plist`, no header in the grammar, and no issue about one. A held language is no fence word or file
language, so it paints as plain text; a held grammar is an empty grammar of its scope name (an include of it is a rule
with no patterns, a key of its repository is missing), so the rule that embeds one stays and its content paints plain:
cpp's `R"glsl(…)glsl"` (the delimiters keep their string colour), elm's `[glsl| … |]`, ts-tags'
`/* glsl */` templates and latex's gnuplot code. `shiki-compare` gives Shiki the same empty grammars, so the comparison
stays one registry against another. bird2, hcl and terraform (MPL-2.0) are back: `LICENSE-T3` carries the licence's
Exhibit A notice, their repositories and commits and where their Source Code Form is, and each of their chunks carries
the notice (the generator writes it). The data holds 245 grammars and 227 languages. (2) Load order: kept, every grammar
from the start. Both are declared in `EXACT2-GAPS.md` ("Code colours") and `STATUS.md` "Known differences", the
coordinator's decision; reversible (`LICENCE_HELD`).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| CH-1 languages | pass but the eight held for their licences (round 3, the coordinator's decision of 2026-10-12): 227 languages and 97 aliases; the clone's 324 fence words are Shiki 4.2's `bundledLanguages` keys less the held eight, which have no aliases (`shiki-compare --list`, which fails on any other difference; before 25 grammars). Round 3, on `0660c0dab`'s content, with the held grammars empty on both sides: the samples 223 languages, 170,646 characters, 0 differing; the 4 unsampled 0; the real files 214,594 0; the TypeScript corpus 169,771 0; Unicode 396 0; long lines 12,717 0; `--max-line 1000` (the app path) on the long lines and the 223 samples 0; and a held-grammar corpus (cpp `R"glsl(…)glsl"`, elm `[glsl| |]`, nim `glsl"""…"""`, ts-tags `/* glsl */` and `glsl` templates, latex `minted{gnuplot}`, the terraform, hcl and bird2 samples) 3,552 characters 0 ([round 3 record](https://raw.githubusercontent.com/ccheever/exact2/720c3cc2638b5df03960a1e77ec419b74be59d25/code-highlighting-languages/ch1-round3-record.txt)). Review round (ten held then): the samples corpus less the held 221 languages, 170,522 characters, 0 differing; the 4 unsampled 0; the real files 214,594 characters 0; the TypeScript corpus 169,771 0; Korean markdown, `u.ts`, julia `2π` 0; long lines (≥ 1,000 and 999 characters, minified JavaScript, empty lines inside blocks) 0; and the app path itself (`highlight()` with the Files line limit) against Shiki with `tokenizeMaxLineLength` 1,000, by synColor class: the 221 samples 170,522 characters 0, the long-line files 12,717 0 ([review record](https://raw.githubusercontent.com/ccheever/exact2/359acbb648dd0cddc78f1599a16d8b3c7650f320/code-highlighting-languages/ch1-review-record.txt)). First round: Against real Shiki (Oniguruma), light, dark and italic of every non-blank character: the samples corpus 231 languages, 177,168 characters, 0 differing; the 4 without a sample (git-commit, git-rebase, html-derivative, nextflow-groovy) 0; real files for 12 languages (cpp 5 files, objective-c 4, objective-cpp 4, haskell 4, julia 4, sql, perl 2, php, vue, astro 2, proto, scss) 214,594 characters, 0; the 40-file TypeScript corpus 169,771 characters, 0 (regression guard, as in shiki-residuals); Korean markdown, Unicode identifiers, julia `2π` 0 (with ASCII `\b` only: 6 and 2) | [ch1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/93dd5a4fb1d8e4144d44d31a25db6fcf5c21ee0c/code-highlighting-languages/ch1-record.txt), [samples log](https://raw.githubusercontent.com/ccheever/exact2/873840eaaed21297f18fd0f3ee0c264f0322e8c6/code-highlighting-languages/ch1-samples-harness.log), [real-files log](https://raw.githubusercontent.com/ccheever/exact2/5b15b11b20845233ac2efaf7a28c80d9d586827d/code-highlighting-languages/ch1-real-files-harness.log) |
| CH-1 Hermes | pass (round 3: the 21,333 regexes, 28,637 forms of the new data compile in the pinned `hermesc` (exit 0) and in Hermes (0 failures); the review's blocking loop confirmed fixed: the test's loop, `tokenizeLineWithin(…, -1)` retried, does not finish in 500 tries on the reviewed head `ac712a39b` and finishes in 79 here; a template-dense cpp line is not bounded by the work limit, which covers one-time work only: in Files and diffs the 1,000-character line limit bounds a line at 675 ms (962 characters) and a slice at about 1.4 s (two such lines), and a chat fence has no bound (2,960 characters 6.2 s; Oniguruma 151 ms), declared in STATUS "Known differences" ([round 3 record](https://raw.githubusercontent.com/ccheever/exact2/720c3cc2638b5df03960a1e77ec419b74be59d25/code-highlighting-languages/ch1-round3-record.txt)). Review round: long lines under slicing in Hermes, the reviewed head against this branch: the markdown emphasis, Korean markdown and rust raw-string lines never finished (dropped after 300 turns), now 2, 3 and 1 turns; the 3,618-character JavaScript line 141 turns, now 6, slices at most 40 ms; the record's per-language probe again: TypeScript 211 KB max 52 ms, cpp 55, asm with Korean 40, same turn counts; [review record](https://raw.githubusercontent.com/ccheever/exact2/359acbb648dd0cddc78f1599a16d8b3c7650f320/code-highlighting-languages/ch1-review-record.txt)). First round: 21,920 regexes, 29,488 distinct forms; the pinned `hermesc` compiles all as literals (exit 0) and vanilla Hermes 1.0.0 builds all with `new RegExp(s, f + 'dg')` (0 failures); no form has the `v` flag; slices of the app's slicing stay near 50 ms (TypeScript 211 KB max 52 ms, cpp 59, Korean-commented TypeScript median 40, asm with Korean max 41; before the work limit up to 427 ms in one line) | ch1-record.txt |
| CH-1 sizes (U15) | measured: grammar source 1,130,900 → 11,738,851 bytes; `app.js` 4,991,714 → 15,595,454; `app.hbc` 6,174,159 → 13,222,347; the bake's `app.plan` 28,854,201 → 28,854,110 (the plan holds no grammar data); `T3 Code (Exact).app` 156,944 → 163,856 KB. Review round (ten grammars held, merged with the feature tip `8fa9446cc`): grammar source 11,340,593; `app.js` 15,214,893; `app.hbc` 12,883,176; bake `app.plan` 29,242,423 (the merged tip's); `.app` 164,140 KB; `contract build` in Checks. Round 3 (eight held, bird2, hcl and terraform back, merged with `ca4f28e35`): grammar source 11,427,102; `contract build` 29,208,699 (the plan holds no grammar data); `app.js` 15,300,293; `app.hbc` 12,952,920; bake `app.plan` 29,242,515 (+92 bytes, the merged tip's); `.app` 164,204 KB | ch1-record.txt, [round 3 record](https://raw.githubusercontent.com/ccheever/exact2/720c3cc2638b5df03960a1e77ec419b74be59d25/code-highlighting-languages/ch1-round3-record.txt) |
| CH-1 in the app | pass. Round 3 not driven again: it changes which grammars the data holds, not how the app paints them, and `shiki-compare --max-line 1000` runs `highlight()` itself (the function Files, diffs and chat fences go through) against Shiki on the 223 samples and the long lines, and the held-grammar corpus through the engine (0 differing); the tests cover the fence words and file names. Review round (one agent drive of this branch's bundle on the merged head, light, 1280×840; before on `t3-code-evidence-base` `6db5cadea`; the reference on the same lane): chat fences through `fences.md` rendered in Files, which is ChatMarkdown in the reference (`FileMarkdownPreview`) and `messageCodeBlocks` in the clone, the function a thread message's fences use: before, cpp in the heuristic colours, haskell and julia plain, ```` ```Python ```` and ```` ```Dockerfile ```` coloured; after, every block as the reference, those two plain ([fences](https://raw.githubusercontent.com/ccheever/exact2/3ac1d14b18649858c67fa61c60a56ee8ae26d5d9/code-highlighting-languages/ch1-review-fences.png)); `long.ts`: before, the 1,016- and 1,000-character lines coloured and the next line still in the comment the second opens; after, both plain and the next line code, as the reference ([long lines](https://raw.githubusercontent.com/ccheever/exact2/f039e6692dcbe9a23fa784aa135fd943895eb964/code-highlighting-languages/ch1-review-long.png)); `docker/Dockerfile`: before coloured, after plain, as the reference ([nested Dockerfile](https://raw.githubusercontent.com/ccheever/exact2/2994040633cc2fa2d41cba1eaf900e52856fe1c7/code-highlighting-languages/ch1-review-dockerfile.png)). A message in a thread is not driven: it needs a provider turn, which the lane rules forbid. First round (agent drive, Files surface, 1280×840, light and dark): before, cpp and the C-like heuristic colours, haskell, julia, php and sql plain, the Korean markdown's `__밑줄__` bolded; after, each file's colours equal the reference's; the one difference is the declared one (the markdown's ` ```sh ` fence is coloured in the clone, plain in a fresh reference window) | [cpp](https://raw.githubusercontent.com/ccheever/exact2/763f19bdfc24f25dc27c1aad9317ab394762bca4/code-highlighting-languages/ch1-cpp.png), [haskell](https://raw.githubusercontent.com/ccheever/exact2/0786c1ec915d4c166d0e61f53a28c2135244f130/code-highlighting-languages/ch1-haskell.png), [julia](https://raw.githubusercontent.com/ccheever/exact2/216dbfde58c4969341ec422c51a89bca5964f774/code-highlighting-languages/ch1-julia.png), [php](https://raw.githubusercontent.com/ccheever/exact2/c7ac86395f0286168648b40ef7c754e074febbdd/code-highlighting-languages/ch1-php.png), [sql](https://raw.githubusercontent.com/ccheever/exact2/4b9ac6b018feacb5d6290bec0e5c594c02c27b2b/code-highlighting-languages/ch1-sql.png), [markdown](https://raw.githubusercontent.com/ccheever/exact2/5945c0b014d68f84e590cf3379101e280aee94d9/code-highlighting-languages/ch1-markdown.png) |

Drives: before on `t3-code-evidence-base` at `6db5cadea` (lane `code-highlighting-languages-before`), after on this
branch's bundle (the app code of `60a281a74`; lane `code-highlighting-languages`), the reference T3 Code `1e2ecbd975` on the
same lane (CDP 16541), each with the Verification fixture project pointed at a lane-private copy of
`verify-runtime/project` holding the six files at its root (a tree id with a slash reads as an agent part), both lanes'
Codex and Claude at missing binaries. Exit 0 each (the before drive's first two tries stopped at the drive script's ids,
before any after drive). The one retry re-drove the merged head's bundle (`7272f6ab8`, main adoption round 9 merged):
exit 0, and the code area of all twelve images is pixel-identical to the first after drive's.
Review round: `fences.md`, `long.ts` and `docker/Dockerfile` added to both lanes' project copies; one after drive of the
bundle built on `a9dcd3803` (lane `code-highlighting-languages`, storage `audit-chl6-after`), exit 0; the before drive
on the evidence base ran three times to settle the steps (a wrapped 1,000-character line kept `long.ts` from showing
within the wait, so word wrap is turned off before the Dockerfile and `long.ts` shots, in all three apps); the reference
drive is `lanes/code-highlighting-languages/ref-round2.mjs` (it turns word wrap back on at the end).

## Real-input batch steps

None: the rows are colours, which the agent drive's `screenshot … window` reads as the screen shows them.

## Tests

- Round 3, `code-highlighting-languages.test.ts` (+6, 27 in all): the counts with the eight held (227 languages, 97
  aliases, 324 fence words, 245 chunks) and bird, bird2, hcl, terraform, tf, tfvars resolving; terraform, hcl and bird2 as
  Shiki paints them; cpp `R"glsl(…)glsl"` and elm `[glsl| … |]` as Shiki paints them with an empty glsl grammar (the
  delimiters keep their colours, the GLSL plain); every held fence word paints plain; held file names (`.frag`, `.vert`,
  `.glsl`, `nginx.conf`, `.adb`, `.tcl`, `.rkt`) plain, `.tf`, `.tfvars`, `.hcl` coloured.
- Review round, `code-highlighting-languages.test.ts` (+8): a line retried under a work limit below zero finishes
  though each try makes its back-reference regexes again (fails on the reviewed head: 500 tries); markdown emphasis,
  Korean markdown, rust raw strings and reStructuredText on its first use, each one line longer than half a turn, finish
  under slicing in under 10 slices with the unsliced tokens; Files and diffs leave a 1,000-character line plain and start
  the next in the state before it (999 characters tokenized; a chat block has no limit); fence words as written
  (```` ```Python ````, ```` ```Dockerfile ```` plain) and whole file names only as the whole path; the language counts
  with the held ten. `r12-render.test.ts`: file paths through `fileLanguage` (`docker/Dockerfile` has none).
- `code-highlighting-languages.test.ts` (14, first round): the language list (235, 97 aliases, 253 chunks; no embedded-only grammar or
  `Object.prototype` name is a language); cpp, haskell, julia, php in html, sql, lua and vue with its injections as Shiki
  paints them; file names as @pierre/diffs picks them, every table value a language; a fence word Shiki lacks paints plain,
  `gitignore` as ini; every regex form has ES2018 flags and compiles; Unicode `\b` on non-ASCII lines (`__밑줄__과`, `2π`)
  and the ASCII form on ASCII ones; a chunk parses on first use and counts as work; cpp weighs double.
- `r12-render.test.ts`, `r12-render-residuals.test.ts`: `cobol` and `x.h` now have grammars (the plain-fence case uses a
  word Shiki lacks); both reset slicing before each test (an app test may leave it on, and a first use then defers).
- `tools/grammar/gen-grammar.test.ts`: the committed data is the generator's output byte for byte (`--check` too); the six
  pins.

## Checks

Round 3: run once on `0660c0dab` (the coordinator's decisions, on the merge `1fd790341` of `origin/feat(example)/t3-code`
`ca4f28e35`). `b4b1e731e` after it changes a generator comment (so the data headers' hash) and notes: `gen-grammar
--check`, `bun test` (4,737 pass), strict `tsc` and caps ran again on it, exit 0 each; the record commit after that is
docs only. The coordinator asked for these checks; the cargo checks were not rerun (no Rust or Swift changed in this
round; the merged feature tip's one Swift change, the real-input test `real-key-chords`, was checked in #482), and no
live drive (data only).

| Check | Exit | Note |
| --- | --- | --- |
| `gen-grammar.mjs --check` | 0 | the committed data is the generator's, byte for byte |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,737 pass, 0 fail (319 files) |
| strict `tsc` (README command) | 0 | 424 files of the clone |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,208,699 bytes; `app.contract` 1,418 lines |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `EXACT_APP_DIR=… bun host/apple/build.mjs t3-code-macos --bundle` | 0 | on `0660c0dab`'s content: `app.js` 15,300,293 B, `app.hbc` 12,952,920 B, bake `app.plan` 29,242,515 B, `.app` 164,204 KB |
| `shiki-compare` (list, samples, extra, real, ts, unicode, long lines, `--max-line 1000` on the long lines and the samples, the held-grammar corpus) | 0 each | 0 differing everywhere ([round 3 record](https://raw.githubusercontent.com/ccheever/exact2/720c3cc2638b5df03960a1e77ec419b74be59d25/code-highlighting-languages/ch1-round3-record.txt)) |
| `hermesc` on every regex form; Hermes `new RegExp` on every form | 0; 0 failures | 28,637 forms |

Review round: run once on the merged head `a9dcd3803` (`d0d3a1ba4`, the review fixes, with `origin/feat(example)/t3-code`
`8fa9446cc`); the record and PR text after it changed docs only.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,727 pass, 0 fail (318 files) |
| strict `tsc` (README command) | 0 | |
| `gen-grammar.mjs --check` | 0 | the committed data is the generator's, byte for byte (with `LICENCE_HELD`) |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,208,607 bytes; `app.contract` 1,418 lines (the merged tip's) |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust or Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (95 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `EXACT_APP_DIR=… bun host/apple/build.mjs t3-code-macos --bundle` | 0 | on `a9dcd3803`, before the drive: `app.js` 15,214,893 B, `app.hbc` 12,883,176 B, bake `app.plan` 29,242,423 B |
| `shiki-compare` (list, samples, extra, real, ts, unicode, long lines, `--max-line 1000` on the long lines and the samples) | 0 each | 0 differing everywhere ([review record](https://raw.githubusercontent.com/ccheever/exact2/359acbb648dd0cddc78f1599a16d8b3c7650f320/code-highlighting-languages/ch1-review-record.txt)) |

First round (merged head `7272f6ab8`): every check 0 but `bun test` (1 then 0 after the slicing reset in the highlight
tests) and `cargo test` (101: main's `button_migration` test on `apps/color-gallery`, inherited, since fixed on main).

## Not done / not verified

None. (A fence inside a thread message is not driven: that needs a provider turn, which the lane rules forbid; the
function it goes through, `messageCodeBlocks`, is driven through Files' rendered markdown, which the reference renders
with the same ChatMarkdown.) Round 3: none; not driven live, since it changes which grammars the data holds, which
`shiki-compare` checks on the app path (`highlight()`); the template-dense cpp line's cost is a declared known difference.

## Decisions (decided 2026-10-12)

1. **Grammar licences** (the coordinator): keep out the four GPL-3.0 grammars (ada, gnuplot, nginx, racket) and the ones
   with no licence found (dax, dream-maker, tcl, and glsl, which has none either); bring back the three MPL-2.0 grammars
   (bird2, hcl, terraform) with their notice in `LICENSE-T3`. Every held language renders as plain text, and a language
   that embeds glsl leaves that part plain. Declared in `EXACT2-GAPS.md` and `STATUS.md` "Known differences";
   reversible (`LICENCE_HELD` in `tools/grammar/gen-grammar.mjs`). Built in round 3.
2. **Load order** (the coordinator): keep loading every grammar at the start; declared that the clone colours a lazily
   embedded language (a Markdown fence) before a fresh reference window does (`EXACT2-GAPS.md`, `STATUS.md`).

Not decided, for whoever takes it: a template-dense cpp line costs far more in Hermes than its slice charge (see
"Seen while building"). Keeping the reference's colours exactly, the clone cannot bound one line's time without a clock;
stopping a line after a count of regex work (as Shiki stops one after 500 ms, the rest of the line in its current
scope) would bound it but paint such lines differently from the reference, which finishes them. Declared as a known
difference, not changed.

## Seen while building, not a finding row

- (Built in the review round: the 1,000-character line limit in Files and diffs, fence words as written, whole file names
  only as the whole path.)
- Template-dense cpp lines are super-linear in the JavaScript regex engine: cpp's function-definition regexes (11,813 and
  3,135 characters) run once per template and scan the rest of the line each time. The per-line work limit does not
  bound them (round 3, asked by the coordinator): it covers one-time work (chunk parses, regex compiles) and stops a line
  only before a compile, never during execution, and a line is never split; a line's execution is charged by its length
  (2 units a cpp character) and not measured. Measured in Hermes on the app's path (round 3, grammar warm), one line of
  `template <typename T> std::vector<T> twice(const T& x) { return {x, x}; } ` cut at n characters: 370 111 ms, 740 414,
  962 675, 999 175 (a line ending on a whole template costs about four times one ending mid-template). In Files and diffs
  what bounds it is the 1,000-character line limit, a length, not a time: for this text one line costs at most 675 ms (962
  characters), and a slice, which starts lines while budget is left, takes two such lines (ten 962-character lines: five
  slices of 1.34 to 1.37 s). A chat fence or search line has no line
  limit, as in the reference, so no bound: 1,480 characters 1.6 s, 2,960 6.2 s, 3,700 9.6 s. The reference (Oniguruma,
  Bun): 962 characters 26 ms, 2,960 151 ms, and Shiki stops a line after 500 ms (`tokenizeTimeLimit`), which a data
  module without a clock cannot do. Such a slice passes the runner's 100 ms answer budget (the answer is refused; the
  slice's progress stays). Real cpp files (the corpus's 47,530 characters) stay at 55 ms a slice. Declared in `STATUS.md`
  "Known differences" ([round 3 record](https://raw.githubusercontent.com/ccheever/exact2/720c3cc2638b5df03960a1e77ec419b74be59d25/code-highlighting-languages/ch1-round3-record.txt)).
- Chrome slants Korean in an italic run; the clone's Korean italic text stays upright (the font has no italic).
- Dark images: the file tree's labels and the sidebar title keep the light raster after the live switch (X82, known).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

This task has no real-input steps (its rows are colours, read by the agent's `screenshot … window`). G4-02, the Diff's colours on the bundle, was not run: the lane's Diff listed no file (fixture); the session's agent pre-check on `1f813789b` stands. The eight grammars held for their licences and the template-dense cpp line stay in `STATUS.md` "Known differences" (the coordinator's decisions of 2026-10-12).

## Delivery

Merged on 2026-10-12 as `63d1c410c` (#466, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; what stays open is listed in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
