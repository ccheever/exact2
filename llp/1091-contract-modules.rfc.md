# LLP 1091: Contract modules

**Type:** RFC
**Status:** Accepted by Charlie (r1, 2026-10-04: "Approve recs" on §7). r2 resolves round 1 (Astra max, Grok 4.7 xhigh: both SOUND WITH CHANGES; §9). Stage 1 landed (7cdf080e2), stage 2 (f0a074f32); r3 records the code review (§10)
**Systems:** Contract loader (`contract/cli/src/{sources.rs,symbols.rs,map.rs,rust.rs,lean.rs}`), syntax (`contract/syntax`), the TS bake's capture (`js/bake`), the web build's capture (`host/web-js/build.mjs`), the dev loop and `build.rs` rebuild tracking, `exact new` (`game/new.mjs`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes for Charlie Cheever: stage 1 on 2026-10-05, stage 2 on 2026-10-06 (§5)
**Amends:** LLP 1017 P8 (`use`); LLP 1055.002 D1 (a timeline's name and the merged namespace)
**Unblocks:** LLP 1055.002 D7 (module-scoped keyframes) and D8 (`use Activity from "exact:motion"`); QUEUE "Two `.contract` files cannot be shared between apps" (`apps/markdown` and `apps/llp` copy `Blocks`/`Runs`)
**Related:** LLP 1086 (apps outside the repo; "no npm packages yet"); LLP 1030 (delivery: the plan's sha256 is its identity). Research only: LLP 0480 (exact1's `use` imported TS and was closed), LLP 0520 (content-addressed class names with collision detection).

## Summary

Two problems, which are separable:

1. **Scope.** `use X from "./f.contract"` checks that `f` declares `X`, then
   merges *every* declaration of `f`, and of everything `f` used, into one
   namespace (`sources.rs` `Exports::merge`). Keyframes are global outright.
   Two files that each declare `pulse`, `Button`, `Item` or a timeline
   `Pending` cannot be loaded into one app, and a name nobody imported is
   visible anyway (`diagnostics.rs` calls `Row()` through two files).
2. **Reach.** A `use` path must start `./`, have no `..`, and resolve under
   the root file's directory (`contract-use-path`). Three more mechanisms
   assume the same thing: the TS bake compiles `app.contract` inside a staged
   copy of the app, `Sources::relocate` refuses a source outside that stage,
   and the dev loop and Rust apps' `build.rs` watch `app.contract` only. So a
   library can't live outside the app, and there's no `exact:` module for a
   built-in.

The finding that makes this small: **component, style and fn names never
reach the plan.** They are inlined away; `source_locations.rs` asserts that a
multi-file app's plan is byte-identical to the same app in one file. The
names in plan bytes are shape names (`types.name`), font families
(`families.name`) and keyframes names (`keyframes.name`), plus the
`clock(Name)` string a timeline lowers to. So scope is a loader pass:
resolve each file's references against that file's own scope, give each
declaration a program-unique name, and hand the rest of the compiler the
same flat `File` it gets today. Only names that are also *runtime text*
(keyframes, timelines) need more than that.

| | Decision | Stage |
|---|---|---|
| D1 | A file sees its own declarations and what it names in a `use`, nothing transitive | 1 |
| D2 | `use A, B as C from "…"`: a list, and `as` to rename | 1 |
| D3 | Every top-level declaration is importable; no `export` keyword (yet) | 1 |
| D4 | The loader renames, and only on collision; existing plans stay byte-identical | 1 |
| D5 | Keyframes and timelines are scoped too; their runtime text is rewritten where it is written | 1 |
| D6 | Fonts stay app-global, like `@font-face` | 1 |
| D7 | Identity is the declaration, not its text; the "identical declarations merge" rule goes | 1 |
| D8 | `exact:<name>` built-in modules, shipped in the compiler | 2 |
| D9 | A bare specifier is a package, resolved through `node_modules` | 2 |
| D10 | The compiler reports every source it read; capture, relocation, watching and deploy use that list | 2 |
| D11 | Libraries are Contract only: no resources, data code or native modules | — |

Qualified access (`use * as ui`, `ui.Button(…)`) is §6, not taken now.

## 1. Evidence

- **Collisions refused today.** `contract-use-duplicate` for a component,
  shape, style or fn declared differently in two files;
  `lower-keyframes-duplicate` for any two `keyframes pulse`;
  `contract-use-duplicate` for any two `timeline` of one name, even identical
  (LLP 1055.002 D1, ruling 4).
- **Charlie, 2026-10-04 (LLP 1055.002 rulings):** should Contract take module
  scope now, "since `use` cannot reach a library outside the app directory
  and every imported name lands in one namespace".
- **Copies across apps.** `apps/markdown` and `apps/llp` carry the same
  `Blocks`/`Runs` renderer (QUEUE). Lexy declares its own `timeline Activity`
  because there's no `exact:motion` (LLP 1055.002 D8).
- **Who uses `use` today.** In this repo: `apps/messages`, `apps/messages-legacy`,
  `apps/expose`, `examples/{ios,macos}/calendar`, `contract/corpus/use`.
  Outside it: `signal-exact2`. All of them already name each import on its
  own line (`apps/messages/app.contract` has eleven), so D1 costs them only
  the names they reach transitively.

## 2. Decisions

### D1 — A file sees its own declarations and what it names

A file's scope is its own top-level declarations plus each name its `use`
lines bring. Nothing comes along transitively: if `app` uses `Card` from
`ui`, and `ui`'s `Card` uses `Icon` from `icons`, then `Card` still works,
because `Card` is resolved in `ui`'s scope. But `app` cannot write `Icon()`
unless it uses `Icon` itself.

A name declared in the file and also brought by a `use` is refused
(`contract-use-shadows`), as is one name brought by two `use` lines from
different declarations (`contract-use-duplicate`, reworded). This is ES
modules' rule.

The root is still the first component of the root file. `routes`, `test`,
and resources/mutations/tasks stay root-only.

### D2 — `use A, B as C from "…"`

```
use Button, Card as UiCard from "./ui.contract"
use pulse from "./motion.contract"
```

One line may name several declarations; `as` renames one in this file. `as`
is already reserved (`syntax/src/parser/names.rs`). The one-name form is the
one-element list, so every existing `use` parses unchanged. A renamed
component keeps its declared name in diagnostics, with the local name
alongside it (`UiCard (Card in ui.contract)`).

Any kind of declaration is importable by name: component, shape, style, fn,
keyframes, timeline. Keyframes are new; today they come along implicitly.

### D3 — No `export` keyword

Every top-level declaration can be named by a `use`. Under D1 that is safe:
a helper nobody names is invisible, and two libraries' helpers can't
collide. An `export` keyword buys an API boundary for published packages.
That matters after 1.0 (DEFERRED: no public API stability before then), and
it can be added later without breaking any source that already names only
what it uses. Charlie ruled: none (§7 Q1).

### D4 — Rename on collision only

The loader gives every declaration a program-unique internal name:

- its declared name, if no earlier file declares one in the same
  namespace. Shapes and `fn`s share one namespace, as both are called
  `Name(…)` and `types` refuses a shape and a fn of one name
  (`type-fn-shape-name`); components, styles, keyframes and timelines
  each have their own;
- otherwise `{name}__{stem}`, where `stem` is the declaring file's stem
  sanitized to `[A-Za-z0-9_]`, with `_2`, `_3`… appended until unique.
  Example: `pulse__ui`, `Item__orders`. The root file always keeps its names;
  between other files, the first in load order (depth-first, `use` order)
  keeps its name, so the choice is deterministic.

Each file's references (component tags, `class=`, shape types and
constructors, fn calls, `animation`, `animation-timeline`) are rewritten
through that file's scope before the files are merged into the `File` the
rest of the compiler takes. `types`, `analyze`, `lower`, Lean, and every
executor see the same flat namespace they see today, so they don't change.

Why collision-only: an app with no name in two files keeps byte-identical
plan bytes, and so keeps its plan's sha256 (LLP 1030). That is the only
guarantee. An app that relied on D7's removed rule (two files declaring
one shape alike, merged into one) now has two declarations, refused if one
file names both, and a plan with two types if both are lowered. The mangled form is a valid
CSS ident, keyframes name (`motion/src/animation/parse.rs`), timeline ident
(`kernel/src/timeline.rs`) and Lean string. It is a Rust identifier when
the declared name is; a hyphenated shape name was already not one, and
`contract rust` printing it raw is an existing defect this RFC doesn't
widen (QUEUE). The cost: a name in plan bytes depends on what else is
loaded. That's only visible for keyframes built at runtime (D5).

Tools show the program-unique name, which is the declared name except on
a collision (`Card__ui`); that is enough to tell two apart and costs no
second identity. Because every name is unique after D4, `contract
symbols` no longer lets a later `Button` silently replace an earlier one,
the source map's `component_costs` no longer adds two `Card`s together,
and `contract rust` no longer drops a second `Item`. Showing the declared
name and its file instead is a display change for later, if collisions
turn out to be common.

### D5 — Keyframes and timelines: rewrite where written

Their names are runtime text, so a renamed `pulse` has to be renamed in
the strings that name it.

- **Literal text is resolved lexically.** An `animation`/`animation-name`
  string literal, or the literal parts of a template, is parsed where it's
  written and each keyframes name is rewritten through that file's scope.
  `animation=\`pulse ${d}ms\`` in `ui.contract` becomes `pulse__ui …` if
  `pulse` was renamed. `animation-timeline=Name` and the literal
  `clock(Name)` are rewritten the same way (`syntax/src/clock.rs` already
  rewrites the bare-identifier form).
- **The attributes:** `animation`, `animation-name`, `exit-animation` (and
  their camel spellings), on elements and in `style` and native-control
  rows alike: all of them resolve against the one keyframes table
  (`lower/src/svg.rs` `check_animation_names`).
- **A literal written elsewhere is not followed.** A caller's
  `Spinner(fx="pulse 1s")` is a string where it's written; the inliner then
  puts it in the spinner's `animation`. It is not rewritten in either file.
  When `pulse` was not renamed it works as today. When it was, lowering
  reads the substituted literal and refuses it (`lower-animation-name`),
  at compile time, so the case is loud, not wrong. A component that lets
  its caller choose takes a choice (`kind: string` and a `match`) rather
  than a keyframes name.
- **A computed name is not resolved.** A name built at run time is matched
  against the table, as today, unchecked (`animation=\`nope ${n}ms\``
  compiles). No warning is added: the reviews showed a "first word"
  test misses `1s ${name}` and comma lists, and a correct one is the
  motion grammar's, at run time.
- **Shadowing stays.** `animation-timeline=Name` resolves per file through
  the clock pass, which still lets a binding of the name shadow it
  (`clock.rs`); the pass now takes the file's scope instead of the merged
  file's timelines.

This is CSS Modules' rule (local names in the stylesheet they're written
in; anything built in script is on its own) and LLP 1055.002 D7's proposal,
without a runner change: the plan's keyframes table already holds whatever
name the compiler gives it.

### D6 — Fonts stay global

A `font` declares an app-level face from an asset file, and `font-family`
is matched case-insensitively against families and system names, like
`@font-face`. Two files declaring the same family alike are one; declared
differently, they are refused as today (`lower-font-duplicate`). Canvas
surfaces also name fonts by runtime string (`exact.fontAliases`), which a
rename would break.

A library does not ship font files in stage 2: lowering resolves a face
only under the app's `assets/` (`lower/src/fonts.rs`). A library names a
family and its README says which files the app provides.

### D7 — Identity is the declaration

Two files' `fn clamp` are two fns, whether or not their text matches; a
diamond (two paths to one file) is one declaration, by (canonical path,
index). `same_declaration` and "identical declarations merge" go, which also
removes the one place timelines were special-cased (LLP 1055.002 D1). D1's
ruling, "two timelines need two names", holds within a file's scope, and
across files it's handled by renaming.

Shapes are nominal (`Ty::Record(name)`), so two libraries' `Item`s become two
types. That's the right answer, and the first time it's been expressible.

### D8 — `exact:` built-in modules

`use Activity from "exact:motion"` resolves to a `.contract` file compiled
into the compiler (`contract/lib/motion.contract`, `include_str!`), loaded
like any other file under the source path `exact:motion`. The first:

- `exact:motion` — `timeline Activity` (LLP 1055.002 D8). As built it
  carries no keyframes: the corpus's spinners don't agree on one `spin`,
  and no consumer asked for a shared one.
- Nothing else until a consumer asks. Every built-in is a source file an
  author can read, not compiler magic.

Built-ins version with the compiler; there's nothing to install.

### D9 — Packages through `node_modules`

A specifier that isn't `./…` or `exact:…` is a package:
`use Button from "@acme/ui"` or `"@acme/ui/button.contract"`.

- Resolution follows Node: walk up from the using file to
  `node_modules/@acme/ui/package.json`. `exports` maps `.` and `./sub` to a
  string, or to an object's `contract` or else `default` condition; as in
  Node, a package with `exports` offers only what it lists. Without
  `exports`, the bare name is `index.contract` and a subpath the file at
  that path. A bare `card.contract` is a mistaken relative path, refused
  with `contract-use-path`, not looked up as a package.
- A package's own relative `use`s must stay inside the package's canonical
  root, by the same rule that keeps an app's inside the app today. Symlinks
  are followed first, so `bun link` and `"file:../ui"` work.
- Local sharing *is* this. A sibling directory with a `package.json` becomes
  a dependency through `"@me/ui": "file:../ui"` or a Bun workspace, with no
  second mechanism. Publishing to npm is `npm publish` of that directory.
- Versioning is npm's. Two versions of one package in one app are two sets
  of files, so they're two sets of declarations (D7) and don't collide.
- A relative `use` gets one relaxation: `..` is allowed as long as the
  result stays inside the same root (app or package). `contract-use-path`
  still refuses leaving it.

The app needs a `package.json` for this, which `exact new` doesn't write
today (LLP 1086: "no npm packages yet"). It writes one with no dependencies
from stage 2 on.

### D10 — One source list, from the compiler

The loader already knows every file it read. `compile_path_output` returns
it (path, kind: app | package | builtin, package name and version), and its
consumers use it instead of their own walks:

The list is a resolution graph, not only paths (Astra 3): each source's
logical specifier and resolved path, the package root and the
`package.json` it consulted, and the edges between them. It is reported
on failure as well as success, so a watcher still sees a library that
broke the build (Astra 5).

- **TS bake capture.** The bake compiles a staged copy
  (`js/bake/src/lib.rs`), so it stages each package's consulted
  `package.json` and `.contract` files, byte copies (symlinks are
  refused there), under a `node_modules` mirror in the stage. Rolldown's
  and `tsc`'s refusals of anything outside the stage are unchanged.
  `relocate` maps each staged path back through the graph rather than
  one prefix strip; an `exact:` source has no path to relocate and is
  skipped.
- **Web build.** It compiles the original `app.contract`
  (`host/web-js/build.mjs`) and stages only TS/JSON, so it needs nothing
  but the list for its watch.
- **Watching.** The JS dev loop already rebuilds on any app file and drops
  `node_modules` (`host/web-js/dev.mjs`); it adds each listed package
  root. The wasm session (`host/web/src/dev.rs`) and the Rust apps'
  `build.rs` (`rerun-if-changed=../app.contract`) watch every listed
  path.
- **Deploy.** Deploy excludes `node_modules` and freezes every byte the
  bake reads (`scripts/deploy.mjs`), so the snapshot captures each
  package's consulted files themselves (registry, `file:`, workspace and
  linked alike) and the bake reads those, not the live tree. The
  package's name, version and content hash are part of the snapshot's
  identity. A library change that changes the plan ships as any plan
  change does.

### D11 — Libraries are Contract only

`provide`/`inject` names stay one program-wide channel, matched by name
across every component (`inline.rs`): two libraries that both `inject
theme` read the same nearest `provide theme`. That is the context model
(React's is keyed by object, Contract's by name), and libraries name their
injects accordingly (`acmeTheme`). Not renamed by this RFC.

A library can declare components, shapes, styles, fns, keyframes, and
timelines. It cannot own resources, mutations, or tasks (root-only already),
carry TypeScript or Rust, or declare native modules (`app.json`'s roster).
Data reaches a library component through props, `inject`, and slots. Library
data code is a separate design with its own consumer.

## 3. Effect on each implementation

- **Syntax.** `UseDecl` becomes `{ names: Vec<(name, alias)>, path, span }`.
  The parser accepts the list and `as`.
- **Loader (`sources.rs`).** Per-file scope tables, the rename map (D4), the
  rewrite pass, specifier resolution (D8, D9), and the source list (D10).
  `Exports::merge`, `merge_declarations`, `same_declaration` and the
  timeline special case are deleted. This is most of the code.
- **`syntax/src/clock.rs`.** Timeline rewriting moves into the per-file pass.
- **types / analyze / lower / plan / runner / kernel / hosts.** No change.
- **Lean / difftest.** No change: `lean.rs` emits the merged file's names,
  which are unique.
- **`symbols.rs`.** A `use` name navigates to the declaration it means, by
  namespace; `map.rs` and `rust.rs` need no change (D4).
- **js/bake, host/web-js/build.mjs, host/web/dev.mjs, `apps/*/web/build.rs`,
  scripts/deploy.mjs, game/new.mjs.** Stage 2 (D10, D9).
- **The inliner's constructor set.** `record_constructors` (`inline.rs`)
  is the merged file's shapes, so a shape named like another component's
  action prop still stops that prop's substitution. That is today's
  behaviour with one file too; D4 doesn't change it.
- **Apps.** Add `use` lines for transitively reached names in
  `apps/messages`, `apps/messages-legacy`, `apps/expose`,
  `examples/*/calendar`; signal-exact2 is told. Lexy moves to
  `exact:motion`'s `Activity`. `apps/markdown`/`apps/llp` share `Blocks` from
  a package (the QUEUE item), as stage 2's consumer.
- **Docs.** `contract-for-agents.md`, `contract-for-humans.md`, and
  `contract-grammar.md` get the `use` forms and the scope rule.

## 4. Tests

Stage 1, in `contract/cli/tests/it/use.rs` and `source_locations.rs`:
- two files each declaring `Button`, `pulse`, `Item`, `clamp`, and a
  timeline `Pending` load together, and each file's uses hit its own;
- a transitively reachable name is refused with `contract-unknown-*` and a
  hint naming the file that declares it;
- `as` renames, list form, and `contract-use-shadows`;
- a diamond is one declaration;
- every corpus and app plan is byte-identical before and after (D4);
- a renamed keyframes in a literal and a template plays (the runner), and
  shows in the web build's CSS as `pulse__ui`;
- `contract symbols` jumps to the right `Button` of two;
- difftest quick.

Stage 2:
- `exact:motion`'s `Activity` shared by two components in two files;
- a package in a temp `node_modules` (scoped name, `exports`, subpath,
  symlinked via `file:`), and a package's `use` leaving its root refused;
- the bake and the web build compile an app whose library lives outside it;
- touching a library file rebuilds in the dev loop.

## 5. Implementation plan

1. **Stage 1, scope (2026-10-05).** One commit: syntax, loader, symbols, map,
   rust, diagnostics, app migrations, docs. No executor or plan change.
2. **Stage 2, reach (built 2026-10-04).** As built:
   `contract/cli/src/resolve.rs` (specifiers), `contract/lib/motion.contract`,
   `contract::source_graph` and `contract sources` (the graph, on failure
   too), `contract::rerun_if_changed` in every app `build.rs`, the wasm dev
   session's watch of every used file, the JS dev loop's watch of each
   package root (`.gen/dev-sources.json`, written by `exact-web-js
   --dev-reload`), the TS bake staging packages under `node_modules/<name>`
   (two copies of one name refused) and relocating through each, deploy
   capturing the repository of a package linked from outside the app and
   Exact (a registry package is pinned by the captured `bun.lock`), `exact
   new` writing `package.json`, and `@exact/reading` (`packages/reading`, a
   Bun workspace) shared by `apps/markdown` and `apps/llp`: the block model,
   `Runs` and `SchemeButton`. Their `Blocks` had diverged in layout and stay
   each app's. Lexy is not in this repository; its move to `exact:motion` is
   its owner's. Planned as:
   **Stage 2, reach (2026-10-06).** `exact:` built-ins with `exact:motion`;
   package resolution; the source list and its consumers; `exact new` writes
   `package.json`; `Blocks` shared between markdown and llp as the consumer.

## 6. Considered and not taken

- **Qualified access (`use * as ui from "@acme/ui"`, `ui.Button(…)`,
  `class=ui.Card`, `x: ui.Item`).** Renaming solves collisions. Qualification
  costs four grammar positions: `Ui.Button(` fails at `.` in tag position
  today, `ns.fn(x)` is refused as a method call, and `class=` takes only
  bare names. It can be added later without breaking anything here. Charlie
  ruled: later (§7 Q3).
- **Always qualify internal names** (`ui/Button`). Cleaner, but it changes
  every multi-file app's plan bytes and compat id, and puts characters into
  CSS idents and Rust names that would need escaping again. Collision-only
  renaming gives the same guarantees.
- **An `app.json` `libraries` map** instead of `node_modules`. It works
  locally but invents a second package system, and an npm publish would then
  need both.
- **Keyframes stay global** (CSS's own rule). Two libraries' `pulse` would
  still collide, which is the problem this RFC exists for.
- **Runtime name qualification for computed keyframes** (LLP 1055.002 D7 as
  written: a file-qualified id in the table and a runner lookup in the
  caller's file). It needs the runner to know which file a string came from,
  for a case that's unchecked today.

## 7. Open questions for Charlie

1. **`export`?** D3 recommends none for now: everything top-level is
   importable, and nothing is visible unless named. The alternative is
   private-unless-`export`ed from day one, which costs every existing
   library file an `export` per declaration.
2. **`node_modules` and a `package.json` in every app (D9)?** This is what
   makes npm publishing free. It means generated apps get a `package.json`,
   which LLP 1086 deliberately left out.
3. **Qualified access now or later?** §6 recommends later.
4. **Process.** Grok and Astra review this before stage 1, or stage 1 lands
   on this draft (it changes no executor, and every existing plan stays
   byte-identical) and review covers stage 2?

**Rulings (Charlie, 2026-10-04): "Approve recs".** Q1: no `export` keyword
(D3). Q2: packages through `node_modules`, and `exact new` writes a
`package.json` (D9). Q3: qualified access later (§6). Q4: Grok and Astra
review this RFC while stage 1 is built; their findings are folded in before
stage 1 lands, and stage 2 waits on them.

## 8. Revisions

- r0 (2026-10-04): draft.
- r1 (2026-10-04): Charlie's rulings on §7.
- r2 (2026-10-04): the round-1 reviews (`llp/reviews/rfc-2026-10-04-1091.{astra,grok}.md`,
  both SOUND WITH CHANGES), disposed below; stage 1 built against this text.
- r3 (2026-10-04): stage 2 as built (§5) and the code review (§10).

## 9. Review dispositions (round 1)

| Finding | Disposition |
|---|---|
| Astra 1 / Grok 4: D4's collision domain misses the shared `Name(…)` namespace and the inliner's constructor set | Taken for the namespace: shapes and fns are one namespace in D4, as built. The constructor set's shadowing of an action prop is today's single-file behaviour; recorded in §3, not changed |
| Astra 2 / Grok 2: D5 misses `exit-animation`; caller literals through props; the first-word warning is wrong; keep clock shadowing | Taken: D5 lists every attribute, narrows the caller-literal promise (a compile-time refusal, not a rewrite), drops the warning, and keeps the clock pass's shadowing per file |
| Astra 3 / Grok 5: D10 needs a resolution graph incl. `package.json`, and the bake and web build differ | Taken into D10 |
| Astra 4 / Grok 5: deploy needs the package bytes, not hashes | Taken into D10 |
| Astra 5 / Grok 5: watcher scope (wasm session, `build.rs`; JS loop drops `node_modules`); expose sources on failure | Taken into D10 |
| Astra 6 / Grok 1: byte identity is conditional on no D7 split; "compat id" is the wrong term | Taken: D4 narrows the claim and says the plan's sha256. Measured: all 122 plans in the repo (apps, examples, corpus, conformance) are byte-identical before and after stage 1 |
| Astra 7 / Grok 3: a hyphenated name is not a Rust identifier | Pre-existing in `contract rust`; recorded in D4, not widened |
| Astra 8: library fonts have no delivery | Taken: D6 says the app supplies a library's faces in stage 2 |
| Grok 6: `provide`/`inject` stay one string channel | Taken: said in D11 |

## 10. Code review dispositions (stages 1 and 2)

Astra and Grok reviewed the landed code (`llp/reviews/code-2026-10-04-1091.{astra,grok}.md`), both
UNSOUND. Every finding is taken, each with a case in `contract/cli/tests/it/scope_review.rs`, and
every plan in the repository stays byte-identical to stage 2's.

| Finding | Fix |
|---|---|
| Astra 4 / Grok 1: every word of an `animation` literal was read as a keyframes name | The rewrite reads the shorthand as CSS does: per comma-separated animation, the first word that is not an animation keyword, a number or a function; in `animation-name`, each item. A computed part glued to a unit is a time |
| Astra 3: a `clock(Name)` literal inside an inline `match` was not rewritten | `match` arms are rewritten as ternary arms are |
| Astra 6 / Grok 2: a local action's curried call, a primitive type, or a roster call was resolved as another file's top-level name | The rewrite tracks bindings (members, parameters, `each`/`match`/arrow/`let` binders); primitives and roster calls are never refused; a used file's shape named like a roster function is renamed |
| Astra 2 / Grok 3: the package was the nearest `package.json` above the file, not the one whose `exports` were read | The package is the directory of the consulted manifest's real path; an exported file that leads out of it is refused |
| Astra 5: one library installed under two names kept only the first in the graph, so the bake staged one | `SourceGraph::packages` lists every name a package was reached by; the bake stages each |
| Astra 8: a manifest whose `exports` refused was not watched | `SourceGraph::consulted` lists every `package.json` read; `build.rs`, the wasm session and the JS loop watch them |
| Astra 7: a generated name (`Helper__ui`) bypassed `use` | Generated names are refused like declared ones |
| Grok 4: alike fonts on different lines did not merge | Fonts compare by family and faces, not position |
| Grok 5: an `exports` condition that is not a path hid `default` | Conditions fall through |
| Astra 1: deploy reinstalled an absolute `file:` or a `link:` from the live tree | Deploy refuses them (a relative `file:` moves with the snapshot) and, after the materialized install, refuses any Contract source outside the captured tree |

