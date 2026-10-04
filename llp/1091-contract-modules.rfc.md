# LLP 1091: Contract modules

**Type:** RFC
**Status:** Accepted by Charlie (r1, 2026-10-04: "Approve recs" on §7); in review (Grok, Astra) while stage 1 is built
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
what it uses. Charlie to rule (§7 Q1).

### D4 — Rename on collision only

The loader gives every declaration a program-unique internal name:

- its declared name, if no other loaded file declares one of the same kind
  and name;
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

Why collision-only: every app that compiles today keeps byte-identical plan
bytes, and so keeps its compat id (LLP 1030). The mangled form is a valid
CSS ident, keyframes name (`motion/src/animation/parse.rs`), timeline ident
(`kernel/src/timeline.rs`), Lean string, and Rust identifier for
`contract rust`. The cost: a name in plan bytes depends on what else is
loaded. That's only visible for keyframes built at runtime (D5).

Tools that show names show the declared one plus its file:
- `contract symbols` keys its index by (file, name) and stops letting a
  later `Button` silently replace an earlier one;
- the source map's `component` label and `component_costs`;
- `agent-inspect`;
- diagnostics.

`contract rust` stops dropping a second same-named shape.

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
- **A fully computed name is not resolved.** If the name itself comes from a
  prop, fn result, or slot, it's still matched at runtime against the
  table, as today. Today that's unchecked anyway (`animation=\`nope ${n}ms\``
  compiles). So a component that wants a caller to choose its animation
  takes the keyframes through a `use` in the caller and a literal there, and
  this RFC adds `lower-animation-computed-name`, a warning when an
  `animation` value's first word is not literal in a program that renamed
  any keyframes.

This is CSS Modules' rule (local names in the stylesheet they're written
in; anything built in script is on its own) and LLP 1055.002 D7's proposal,
without a runner change: the plan's keyframes table already holds whatever
name the compiler gives it.

### D6 — Fonts stay global

A `font` declares an app-level face from an asset file, and `font-family`
is matched case-insensitively against families and system names, like
`@font-face`. Two files declaring the same family differently are refused
as today (`lower-font-duplicate`). Canvas surfaces also name fonts by
runtime string (`exact.fontAliases`), which a rename would break. A library
that ships a face documents its family name.

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
like any other file under the source path `exact:motion`. The first two:

- `exact:motion` — `timeline Activity` (LLP 1055.002 D8), and the shared
  keyframes the corpus re-declares (`spin`, `pulse`, `fade-in`, `shimmer`
  if they're the same everywhere; to confirm when implementing).
- Nothing else until a consumer asks. Every built-in is a source file an
  author can read, not compiler magic.

Built-ins version with the compiler; there's nothing to install.

### D9 — Packages through `node_modules`

A specifier that isn't `./…` or `exact:…` is a package:
`use Button from "@acme/ui"` or `"@acme/ui/button.contract"`.

- Resolution follows Node: walk up from the using file to
  `node_modules/@acme/ui/package.json`. The bare name maps to the package's
  `exports["."]` if it ends `.contract`, else `index.contract`. A subpath maps
  through `exports` when present, else to the file at that path.
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

- **TS bake and web build capture:** stage each package source under a
  `node_modules` mirror in the stage, so compile-in-stage still works. The
  Rolldown and `tsc` refusals of anything outside the stage are unchanged,
  because only `.contract` files are captured this way.
- **`relocate`** maps each staged path back to its original path, not only
  the app's.
- **Dev loop and `build.rs`** watch every listed path (`rerun-if-changed`
  per source), which fixes QUEUE's "the dev loop watches the app file only"
  for multi-file apps too.
- **Deploy** records each package's name, version, and the sha256 of its
  sources in the snapshot. The plan's sha256 is still the compat id; a
  library change that changes the plan ships as any plan change does.

### D11 — Libraries are Contract only

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
- **types / analyze / lower / plan / runner / kernel / hosts.** No change,
  apart from new diagnostic wording and `lower-animation-computed-name`.
- **Lean / difftest.** No change: `lean.rs` emits the merged file's names,
  which are unique.
- **`symbols.rs`, `map.rs`, `rust.rs`.** Keyed by declaration identity; show
  declared names.
- **js/bake, host/web-js/build.mjs, host/web/dev.mjs, `apps/*/web/build.rs`,
  scripts/deploy.mjs, game/new.mjs.** Stage 2 (D10, D9).
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
2. **Stage 2, reach (2026-10-06).** `exact:` built-ins with `exact:motion`;
   package resolution; the source list and its consumers; `exact new` writes
   `package.json`; `Blocks` shared between markdown and llp as the consumer.

## 6. Considered and not taken

- **Qualified access (`use * as ui from "@acme/ui"`, `ui.Button(…)`,
  `class=ui.Card`, `x: ui.Item`).** Renaming solves collisions. Qualification
  costs four grammar positions: `Ui.Button(` fails at `.` in tag position
  today, `ns.fn(x)` is refused as a method call, and `class=` takes only
  bare names. It can be added later without breaking anything here. Charlie
  to rule (§7 Q3).
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
