# Code review: contract gallery, the variant gallery's G2, round 1 of 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `6b6f490ed3dda32afea4fe32aafaf938752c42a8d8f5879b00310a61fb30e02b`), shared by both reviewers, each blind to the other's review; HEAD `43be997c0f`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 4 BLOCKER, 4 SHOULD-FIX.
- **Disposition:**
  All taken, in the round 1 commit:
  - Astra 1 (output can overwrite the app): `-o` is refused when it is the app's directory or holds it. Inside the app it must be under node_modules or target. An app.contract without the generated header is never overwritten. Tested, with the app's sources unchanged.
  - Astra 2, Grok 1-2 (discovery by spelling): components come from the loader's own resolved imports (`Sources::imports`, kind component) in the app's own files. Each `use` is spelled as the app spells it. A re-export's rename, an exports subpath, an installed alias and a file use are all the declaration the app reaches. Tested: `Fancy` re-exporting `Card`, `Pill` by subpath, two packages' `Card`.
  - Astra 3, Grok 5 (refusals outside the gallery's lines): when a refusal isn't placed in a cell or a shape's use, each remaining cell is compiled alone and refused with its own first refusal. A gallery only fails when it fails with no cells. Tested: a string prop that the library uses as `text-align` is refused at the library's span, and only that cell.
  - Astra 4 (recursive records): the compiler refuses any shape cycle (`type-shape-recursive`), even through an option or list, so such an app doesn't build either. The gallery brings only components with a placeable cell, and refuses such cells when it walks the placeholder. The comments now say so, rather than claiming option/list break a compiler cycle.
  - Astra 5, Grok 4 (renamed shapes): each shape is brought by the name its declaring file gives it (read from that file at the declaration's span), under a gallery-unique local name. Tested: the app's own `Info` makes the loader rename the package's to `Info__index`, and the gallery writes `use Info from "@acme/lib"` and places the cells.
  - Astra 6, Grok 3 (name collisions): one taken-name set, seeded with `Gallery` and `noop`, gives each component and shape the next free `_n`. Unit and integration tests cover it (`Gallery_2`, `Card_3` after `Card_2`).
  - Astra 7 (resolution context): before compiling, the gallery's component uses are loaded from `-o`. Each package must resolve at the app's own root, otherwise the destination is refused ("can't reach the app's packages", or "reaches X at another root"). A relative specifier is rewritten from the gallery. Tested with a cache no node_modules reaches.
  - Astra 8 (manifest): the app's identity comes from `Manifest::read` (its defaults included, errors propagated). Tested with an app that has no app.json.
  - Grok 6 (tests): the cases above are now tested.

This round has blockers.

1. **BLOCKER — [contract/cli/src/gallery.rs:87](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:87): Output can overwrite the original app.** Nothing checks the canonical destination before writing `app.json` and `app.contract`. Passing the app directory—or a symlink to it—as `-o` destroys those inputs. Other destinations can put generated files in git-visible source directories. The integration test only chooses a safe cache; it does not test rejection or git exclusion.

2. **BLOCKER — [contract/cli/src/gallery.rs:219](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:219): Discovery confuses import spellings with declaration identities.** A package re-exporting `Card as Fancy` has identity `…#Card`, so an app importing `Fancy` is missed. Installed package aliases and mapped export subpaths also fail the prefix test. Conversely, accepting `.` after the prefix lets `@acme/lib` match `@acme/lib.extra`. Use the loader’s resolved imports and retain the public export spelling.

3. **BLOCKER — [contract/cli/src/gallery.rs:143](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:143): Cell failures can abort the entire gallery.** Diagnostics need not point into generated cell lines. For example, a string prop `align` used as `text-align=align` receives `"align"`; lowering rejects it at the library attribute’s span. Likewise, a missing inject in a nested helper points into the library. Both take the fatal branch at line 169 instead of producing a refused cell. The direct-inject test misses these cases.

4. **BLOCKER — [contract/cli/src/gallery.rs:319](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:319): Recursive records still prevent a usable gallery.** Although placeholder generation marks the cell refused, every component remains imported. The compiler checks all loaded shape declarations and emits `type-shape-recursive` at the library field, causing generation to fail. The recursion unit test exercises only `args`, not `generate`, so it does not establish the approved labelled-cell behavior.

5. **SHOULD-FIX — [contract/cli/src/gallery.rs:335](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:335): Shape imports expose compiler-internal names.** When two libraries export `Info`, or the app declares its own `Info`, scoping can rename the library shape to `Info__index`. The generator writes `use Info__index from "…"`, although the public export remains `Info`, and wrongly refuses otherwise placeable cells. Shape constructors need an export-to-gallery alias mapping.

6. **SHOULD-FIX — [contract/cli/src/gallery.rs:236](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:236): Generated component names can collide.** Importing `Card_2` before `Card` assigns both the gallery name `Card_2`: counting matching prefixes does not find an unused suffix. A library component named `Gallery` also conflicts with the generated root. Both failures occur on imports and abort generation. Reserve generated names and check candidate aliases for actual uniqueness.

7. **SHOULD-FIX — [contract/cli/src/gallery.rs:321](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:321): Moving imports changes package resolution.** Bare specifiers are copied unchanged, then resolved from `out`. A cache outside the app’s dependency ancestry cannot find its local `node_modules`; another cache may resolve a different installation. Preserve the original resolution context or validate the destination requirement before writing. Both current integration destinations happen to retain dependency visibility.

8. **SHOULD-FIX — [contract/cli/src/gallery.rs:260](/tmp/rv-dt-g2r1/contract/cli/src/gallery.rs:260): Manifest handling bypasses established identity rules.** Missing manifests produce the shared identity `app.gallery`, instead of deriving from `Manifest::read`’s directory-based defaults. Malformed manifests are silently treated the same way, and `.shells/app.json` is ignored. Reuse the existing manifest reader and propagate its errors.

Static review only; no files changed, builds run, or test suites executed.
