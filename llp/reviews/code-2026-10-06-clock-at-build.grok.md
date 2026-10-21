# Round 1
I'll review commit `b746c5dae` as a read-only defect check against the LLP 1027 refusal rules, starting with the review skill and the commit itself.This is a defect review of commit `b746c5dae` only — I'll read the diff and the three producers that emit `ambientRefusals`, and I won't change any files.The commit is small. I'll read the refusal walker, the three call sites, and how Rolldown's AST reports positions so the labels and misses can be checked against the actual tree.The refusal walker is syntactic. I'll parse the same Rolldown AST it uses and check shadowing, type positions, optional calls, spread, and whether `start` is a byte offset.The build check misses clock and timer calls that still run, and it rejects some calls that do not.

[P1] Refuse `globalThis`, `self`, and `window` calls of the guarded APIs — `js/bake/src/typescript.mjs:29`

`member()` and the timer check only match a bare identifier (`Date.now()`, `setTimeout()`). `globalThis.Date.now()`, `globalThis.setTimeout()`, `self.setInterval()`, and `window.setTimeout()` are ordinary member calls, so `ambientRefusals` returns nothing. Rolldown's inject rewrites `globalThis`, `self`, and `window` to the guarded `appGlobal`, and the native prelude guards `globalThis.Date` and `globalThis.setTimeout`, so those calls throw on the executors. Bun implements `globalThis` and `self` with a real clock and real timers, so a unit test of that line passes and the device still throws. That is the same hole this commit is closing. Matching a non-computed `globalThis` / `self` / `window` / `global` receiver is a small extension of `member()`.

[P2] Unwrap `!` and a parenthesized optional callee before matching — `js/bake/src/typescript.mjs:34`

`Date.now!()` is a `CallExpression` whose callee is a `TSNonNullExpression`, and `Date!.now()` / `performance!.now()` are member calls whose object is a `TSNonNullExpression`. Neither matches. `(Date?.now)()` has a `ChainExpression` callee and is also missed. `Date.now?.()` is matched, because the callee is still the member expression. The bundler erases `!` and then injects the guarded `Date` / `performance`, so the shipped call is the one the check claims to refuse. Bun erases `!` too, and the test passes. Unwrap `TSNonNullExpression` and `ChainExpression` before the existing checks.

[P2] A local `Date`, `performance`, `Math`, or timer is not the guarded global — `js/bake/src/typescript.mjs:34`

The matcher compares identifier names and does not look at bindings. These are all refused, and none of them reads the clock or schedules a timer:

- `function stamp(performance: { now(): number }) { return performance.now(); }`
- `const Date = { now() { return 1 } }; Date.now()`
- `import { now as Date } from './d'; Date.now()`
- `class Date { now() { return 1 } }` then `new Date()`

Lexical scope on Hermes leaves those bindings alone. The web inject is scope-aware and does not rewrite them either (a parameter or a class named `Date` stays local). The build fails a module that runs on every host. The error text tells the author to pass the clock in, which is the shape of the parameter case above.

The other checks hold. Oxc's `start` is a UTF-16 offset: with `é`, `日`, and `😀` before `Date.now()` it matches the string index, not the UTF-8 byte length, and the column math uses that index. The test's `logic.ts:2:28` is the `D` in `Date.now()`. CRLF does not shift the next line. Type positions (`typeof Date.now`, `as Date`, return types) are not calls. Comments and string literals are not in the tree. `new Date(...xs)` and `new Date(...[])` are spread arguments, so they are not treated as zero-argument `new Date()`. `Date.now?.()` is a real call and is refused.

The transform hook sees the generated entry, imported TypeScript, JSON, and Rolldown's `\0rolldown/runtime.js`. JSON is ignored before `parse` because the id does not end in a script extension. The virtual runtime id is not absolute, so it is ignored. `.d.ts` is excluded. None of those produce a false refusal. All three producers call the same `ambientRefusals` with `this.parse`. One-shot and resident throw from the plugin. The web pass records hits and throws after `generate`.

The empty `catch` in `host/web-js/build.mjs:204` does not drop a refusal already pushed into `why` (that throw is outside the `try`). It does drop a `this.parse` failure for that file. I could not find TypeScript that this filter parses, tsc 7 accepts, and Oxc rejects, so I cannot show that catch letting a clock read through. `--data` still skips the whole pass, because the check sits inside `typecheck()`; the app's own build still runs it.

The new test only covers a bare `Date.now()`, a comment, `new Date(0)`, and `.now()` on another object. It does not cover a shadowed binding, `globalThis`, or `Date.now!()`.

# Round 2
I'll review commits `b746c5dae` and `d9c94db9a` without changing anything, starting with the review instructions and the diffs themselves.This is a read-only defect review of those two commits. I'll read the repo instructions and the diffs, then check the fixes against the surrounding code.The line-number and unwrap cases check out. Next I’ll measure the binding holes and whether the web build’s new error path hides the type check.`bound()` treats erased declarations as real shadows, and the web build now reports a Rolldown failure instead of the type check.

[P2] Erased declarations hide a direct clock read — `js/bake/src/typescript.mjs:72`

`bound()` records every `VariableDeclarator`, every class id, and every import local name. That includes bindings TypeScript erases. With this repo's compiler options (`ES2023`, `WebWorker`), `tsc` accepts all three of these and emits a bare global call:

- `declare const performance: { now(): number }` is a `VariableDeclaration` with `declare: true` (line 72).
- `declare class Date { static now(): number }` is a `ClassDeclaration` with `declare: true` (line 73). The emit is `export const s = () => Date.now()`.
- `import type { Date }` and `import type Date from '...'` are type-only. On the specifier, Oxc still sets `importKind` to `"value"`; the `"type"` flag is on the `ImportDeclaration`. `import { type Date }` is the opposite: the specifier is `"type"` and the declaration is `"value"`. Line 76 records both.

`declare function Date()` is a `TSDeclareFunction` and is still refused, so this is not the module-wide skip. A Bun test of that module runs the global; the build check stays quiet. Skip a declarator only when its declaration is `declare`, and skip an import only when the declaration's `importKind` or the specifier's `importKind` is `"type"`.

[P2] A value namespace named `Date` is refused as the global — `js/bake/src/typescript.mjs:73`

The same walk records functions and classes and does not record `TSModuleDeclaration`. This typechecks and emits a real binding whose `now` returns `1`:

```ts
namespace Date { export function now() { return 1 } }
export const s = () => Date.now();
```

The bake reports `Date.now() is unavailable`. A type-only namespace, a `declare namespace`, and a `const enum` are erased and must still be refused; only a namespace that actually emits a value binding should be skipped. `TSImportEqualsDeclaration` is the same hole, though under `module: ESNext` that form already fails as TS1202.

[P2] An angle-bracket assertion still hides the call — `js/bake/src/typescript.mjs:32`

`bare()` unwraps `TSAsExpression` and `TSSatisfiesExpression`, so `(Date as any).now()` and `(Date.now as any)()` are refused. `TSTypeAssertion` has the same `expression` field and is not in that list. In a `.ts` module these three are direct calls and come back with no refusal: `(<any>Date).now()`, `(<any>Date)()`, `new (<any>Date)()`, `(<any>setTimeout)(() => {}, 1)`.

[P2] A Rolldown failure on the web skips the type check — `host/web-js/build.mjs:202`

`generate()` sits outside the old catch, and `check()` runs only after it returns. A syntax error rejects `generate()` even with `logLevel: 'silent'` (I got Rolldown's `PARSE_ERROR`), so `check()` never runs. The one-shot native bake runs `tsc` first and reports the type checker's diagnostics for that program. A missing relative import still does not reject `generate()`, so that case still reaches `check()`. The resident compiler already waits for both and joins the messages; this pass should do that too, otherwise a file with a parse error and a type error is refused with only Rolldown's text.

Line endings match the spans. `CRLF` is one break, and a raw U+2028 or U+2029 (including inside a string or comment) advances the line. Offsets are UTF-16, so a line with `é` or `🙂` before `Date.now()` still lands on the right column. The module-wide value skip is what the commit describes: a parameter named `Date` anywhere in the file suppresses every `Date.now()` in it, including one that really is the global. I scanned the app sources; the hits are host scripts, smoke fixtures, and `js/tests/fixtures`, which the `app.ts` graph does not reach. A 20,000-line module takes about 50 ms, parse included.

# Round 3
I'll review the three commits as they stand, with the last one as the focus, and report only concrete defects. First I'm reading the review instructions and the diffs.This is a read-only review of those three commits, centered on the last one. I'll inspect the diffs and the surrounding checker before deciding whether anything is still wrong.The shadowing rules and the web build's joined diagnostics are the parts that can still be wrong. I'll parse the same trees the checker sees and check which bindings actually survive compilation.`bound()` treats every namespace that has a body as a runtime binding. A type-only or empty one is erased, so a direct call of the global still typechecks, is emitted, and passes the build.

[P2] A type-only or empty namespace suppresses the build refusal — `js/bake/src/typescript.mjs:80`

`TSModuleDeclaration` is recorded whenever `node.body` is set. That is right for `namespace Date { export function now() { return 1 } }`, which emits a `var Date`. It is wrong for a namespace that holds only types, interfaces, or nothing:

```ts
namespace Date { export type Stamp = number }
export const stamp = () => Date.now();
```

The native `tsc` this bake runs accepts that, and the emit is `export const stamp = () => Date.now()`. `ambientRefusals` reports nothing, so the one-shot compile, the resident compile, and the web build all succeed. The same is true of `namespace Math { export type R = number }` with `Math.random()`, `namespace performance {}` with `performance.now()`, and `namespace setTimeout { export type Handle = number }` with `setTimeout(...)`. On device the runtime guard still throws; a Bun test of the module does not. The new bake test only covers a namespace that contains a function.

The other round-3 checks hold. `declare` nodes and type-only imports, including `import type Date = require(...)` and inline `type` specifiers, do not shadow. A value `import Date = require(...)` does. `<any>Date`, `<any>Date.now`, and `new (<any>Date)()` are still seen. The web build starts the type check and the bundle together and joins a bundle failure with the refusal lines and the type diagnostic.

