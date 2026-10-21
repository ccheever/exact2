# Round 1
Found four defects:

- **[P2] Shadowed bindings are rejected** — [typescript.mjs:29](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:29). Matching uses identifier spelling without scope resolution. Valid code such as `function stamp(Date: { now(): number }) { return Date.now(); }` fails the build despite reading no ambient clock. The same applies to local `performance`, `Math`, and timer bindings.

- **[P2] The web scan silently accepts an incomplete module graph** — [build.mjs:204](/Users/admin/projects/exact2-wt-ft/host/web-js/build.mjs:204). Reproduced with an imported `helper.js` containing `Date.now()` and matching `helper.d.ts`: capture omits the JavaScript file, Rolldown’s resolution error is swallowed, and TypeScript succeeds using the declaration. The actual page bundle includes the unchecked helper. Native bundling rejects this graph; the web build succeeds.

- **[P2] Several direct calls escape refusal** — [typescript.mjs:29](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:29), [typescript.mjs:40](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:40). `globalThis.Date.now()`, `Date.now!()`, and `new Date(...([] as []))` all type-check and produce no refusal. Qualified timers such as `window.setTimeout(...)` are also unmatched. These preserve the Bun-passes/device-throws problem without introducing aliases, making the documentation’s direct-use guarantee too broad.

- **[P3] Non-LF line terminators produce incorrect locations** — [typescript.mjs:50](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:50). Only LF advances the line counter. `const s=1;\rDate.now();` reports `1:12` instead of `2:1`; U+2028/U+2029 have the same problem.

Optional calls, comments/strings, ordinary type references, nonempty date spreads, UTF-16 offsets, mounted TypeScript, and JSON filtering behaved correctly in probes.

Verified with in-memory Bun/Rolldown and TypeScript probes. No files changed.
# Round 2
Three defects remain:

1. **[P2] TypeScript value bindings still produce false positives** — [typescript.mjs:72](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:72). `bound()` misses `TSModuleDeclaration` and `TSImportEqualsDeclaration`. For example, `namespace Date { export function now() { return 1; } } export const x = Date.now();` is refused despite calling a local function. Rolldown bundles it correctly without the refusal pass.

2. **[P2] Web bundle failures discard other diagnostics** — [build.mjs:202](/Users/admin/projects/exact2-wt-ft/host/web-js/build.mjs:202). When `generate()` rejects, collected ambient refusals are lost and `check()` never runs. A module containing a missing export, `Date.now()`, and an independent type error reports only the missing export. Preserve the bundle error while collecting the other diagnostics, as the resident compiler does.

3. **[P2] Angle-bracket type assertions bypass detection** — [typescript.mjs:32](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:32). `bare()` omits `TSTypeAssertion`. `(<typeof Date.now>Date.now)()` returns no refusals and bundles directly to `Date.now()`, retaining the Bun-test/device-failure problem.

Read-only Rolldown probes confirmed these. The listed wrapper/receiver and line-terminator fixes passed. No false positives appeared in the ten existing app graphs that bundled; two required generated inputs. No new performance defect was established on flat modules through 2.5 MB. No files changed.
# Round 3
Two concrete defects remain:

- **[P2] Erased namespaces suppress refusal** — [js/bake/src/typescript.mjs:80](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:80). `namespace Date {}; export const stamp = () => Date.now();` type-checks and produces no ambient diagnostic. Rolldown erases the namespace, leaving the global clock call. Namespaces containing only types behave identically. Introduced by `ac9399b38`.

- **[P2] Bindings suppress diagnostics outside their scope** — [js/bake/src/typescript.mjs:104](/Users/admin/projects/exact2-wt-ft/js/bake/src/typescript.mjs:104). `bound(ast)` supplies one module-wide set, so a helper’s parameter named `Date` suppresses refusal of a separate, module-level `() => Date.now()`. That call still reaches the guarded global and fails at runtime. Introduced by `d9c94db9a`, still present.

Confirmed with in-memory TypeScript and Rolldown probes. The other requested fixes, including joining bundle, ambient, and type diagnostics, passed focused probes. No files changed.
# Disposition

Fixed in d9c94db9a, ac9399b38 and 7394ae946, except one kept on purpose: bindings are module-wide, not scoped, so a module that binds `Date` (a parameter, say) anywhere is not checked for `Date` at all. The check is an early warning a Bun test cannot give, not the guard; the runtime refusal still holds on every executor, and a false refusal of working code would be worse than a missed warning. Round 3's type-only-namespace finding was fixed after the last round and is not re-reviewed.
