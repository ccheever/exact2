# LLP 1153: A website in Exact — nothing native until asked, and what the web still lacks

**Type:** RFC
**Status:** Draft r2, 2026-10-30. r1 was reviewed blind by GPT-6 Astra (xhigh), Grok 4.7 (xhigh) and Muse Wonder 1.0 (xhigh) (`llp/reviews/rfc-2026-10-30-1153-r1.{astra,grok,muse}.md`): Astra READY WITH CHANGES, Grok READY WITH CHANGES, Muse NOT READY, all three finding the direction right. r2 takes what §R lists. Written at Charlie's request after the Pokédex runs of Nolan Lawson's agent framework benchmark (§1); his request is the approval `rules/RULES.md` asks for a design document, and for the verbs, flags, pattern and recipe named below. Nothing built.
**Systems:** Scaffolding (`game/new.mjs`: what `exact new` writes, the generated native workspace, `--with` and `--without`), the app's command runner (`scripts/app-cli.mjs`: the verbs, and the one place a native verb brings its platform), app resolution (`scripts/app.mjs` `resolveApp`, `checkOutsideLock`), setup (`scripts/exact.mjs` `setup --check`), delivery (`scripts/deploy.mjs` `nativePlatforms`, the web root), the root workspace (`Cargo.toml` members and `exclude`), the web build and its server (`host/web/build.mjs`, `host/web-js/build.mjs`, `host/web/serve.mjs`), the bake (`js/bake/src/typescript.mjs`), the router (`route/`, `runner/src/runner/router.rs`, `host/web-js/router.js`, the Apple and Linux hosts' URL delivery), the kernel (a table's placement), Contract (`contract/lower/src/tags.rs`, `values.rs`, the plan's node kinds), Tamber's importer (`scripts/tamber-import-review.mjs`), the author docs (`docs/start-here.md`, `docs/recipes/`, `patterns/`)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-30
**Implementer:** Claude (Fable 5.1), from 2026-10-30, with Opus 5.5 lanes for the slices §6 names; reviewed by Astra and Grok before each push.
**Related:** LLP 1086 (what an app author is given; its D1 "the app's workspace holds three host crates" is what D1 here replaces); LLP 1036.001 (apps outside the repository; its D4, the private Snapback pin leaving the root workspace, implemented 2026-09-30 and since regressed by Harmony, is D2.4 here); LLP 1046 §5 and `game/app/shells.mjs` (a game's generated workspace, `.shells/`, which D1 reuses for apps); LLP 1071 (the JS target; §7 "Retiring the wasm target on the web" and delivery's bake); LLP 1038 (the router; its Location "never a scheme, host, or fragment" is what D5 amends); LLP 1048.000 (pages at build: `render=build` and `pages=`); LLP 1023 (one URL per app: `serve.mjs`, the envelope, LAN by default); LLP 1121 (Tamber; its D7: tokens as custom properties `--x-<name>` on the web); LLP 1115 (write the web, ship the platform); LLP 1115.002 (the Mac's structure, which owns a table's `NSTableView`); LLP 1113 (settled: when a route's content has arrived); LLP 1152 (the web shootout, whose authoring table these runs preview); LLP 1107 (Android is the Linux crate); LLP 1115.004 (Windows); `docs/agent-pitfalls.md`

## R. Round 1, and what r2 changed

Three reviewers, blind, asked for direction and major oversights only. All three found the
shape right and the web-term answers right in spelling; each found places where the spelling
had lost the meaning. Where two or three raised a point, r2 takes it; where one did and the
repository bears it out, r2 takes it too.

| Change | Raised by | Where |
|---|---|---|
| No host crate ever enters the author's tree: the native workspace is generated on demand, as a game's `.shells/` is, and an authored one is used when the author writes it | Astra, Grok (each as the strongest alternative to r1's D1) | D1 |
| A manifest that names no native platform ships none; today deploy reads it as every stream, and `--without` refuses to remove the last | Grok | D1.5 |
| The capability test is "does this verb need the app's Rust", not "is there a workspace"; `test ios` and its kin bring the platform too; `apple/` is owned by ios and macos together, `linux/` by linux and android | Astra, Grok | D1, D2 |
| Harmony's three crates move together; the lane step that compiles them is named | Muse, Grok | D2.4 |
| The hosting rule is two-way with the route table the build writes; `shell.html` is always written; a single-fallback host serves the shell and the client draws `notfound`; a web-only root has no envelope; `serve` binds loopback | all three | D3 |
| Settings are names the manifest declares, values captured as build inputs, Bun's own `.env` loading held off, a default allowed; `serve` rebuilds or refuses on a changed value | all three | D4 |
| A fragment never mints an entry: identity stays path and query; the fragment is a field of the entry; each host scrolls; target lookup inside the active route; repeated activation; cancellation; focus; `<base href="/">` | all three | D5 |
| `var(--name)` lowers as the token reference `name` does, the web emitting `var(--x-name)`; an unknown name is refused with or without a fallback; the importer rewrites a marked block and refuses what it cannot translate | all three | D6 |
| The plan carries the table: the web emits HTML's elements, native paints the node as a grid for now; a grid is not the table algorithm, and the difference is declared | all three | D7 |
| A parameterised `render=build` route needs `pages=`, so the site pattern's detail renders on the client | Grok | D8 |
| 1079.000 is not finished; LLP 1111, untouched since 2026-10-08, is archived instead | all three | §5 |
| Errors: LLP 1036.001's decision is D4, not D2; LLP 1115 says "names stay CSS"; the bake process reads the environment though the module never runs; `serve.mjs` falls back to `index.html`, not `shell.html` | Grok, Muse | §2, D2, D3, D4 |

Not taken: Muse's alternative of keeping the full scaffold and only skipping Cargo. Charlie's
ask was no files for platforms a website does not use; D1's measurement (§6) is where that
alternative would show it was enough.

## Summary

Five agents built the same Pokédex website with Exact, and five with Next.js. Every Exact
run passed; each cost about twice as much, and the same gaps cost every run (§1). Two
are the scaffold's, three are the web host's, two are Contract's vocabulary, and the
rest are small.

**The scaffold.** `exact new` writes an app for every platform at once: three Rust host
crates, a Cargo workspace, a 392-package lockfile copied from exact2, and a manifest
that ships four platforms. A website never touches them, pays for them in files and
lines, and waits 18 seconds for a setup check of toolchains it will not use. **D1:**
`exact new` writes eight files and no Rust, ever. The first `ios`, `mac`, `linux`,
`android` or `windows` command names that platform in the manifest and builds it in a
generated workspace under the app's `.exact/`, as a game builds in `.shells/`; `--with`
names platforms up front. An author who needs Rust of their own writes crates, and those
are used instead. **D2:** the web verbs run no Cargo in the app, and the root workspace
stops carrying a private git source that every app's compiler build today resolves.

**The web host.** A built site has no verb that serves it or says how to host it (D3);
`app.ts` cannot read a setting at build, so an API origin is hard-coded and rewritten by
hand (D4); a URL's fragment is dropped, so `/pokemon/pikachu#moves` opens at the top
(D5); `var(--ink)` is refused although the app's tokens are custom properties on the
web (D6); `table` and `dl` are refused and rebuilt from ARIA roles on boxes (D7). Each is
decided in the web's own terms, with the web's meaning kept: a declared setting read
through `import.meta.env`, a fragment that scrolls once the page has its content, `var()`
over the tokens the app declares, and HTML's table and description-list elements carried
as what they are.

The smaller snags and the orientation cost are D8. Nothing here adds a blocking check.

## 1. What the runs found

The benchmark asks for a Pokédex: a searchable list, a detail page with sortable move
tables and a `#moves` deep link, an API origin passed as `API_BASE_URL`, colours
supplied as CSS custom properties in `tokens.css`, and `npm start` serving the production
build. Five Exact runs and five Next.js runs, each unattended.

| | Exact | Next.js |
|---|---|---|
| Runs passing the final tests | 5 of 5 | 5 of 5 |
| Cost per run | about $6.67 | about $3.22 |
| Tool calls spent reading exact2's docs or source | 24 to 43 | 0 |

The gaps, each hit by every run:

1. **No way to serve the production build.** Each run wrote its own `server.mjs` so that
   `npm start` serves the build and `/pokemon/pikachu` falls back to the app.
2. **No build-time setting.** Contract and `app.ts` cannot read `API_BASE_URL`; each run
   wrote a script that rewrites the origin literal in `app.ts`.
3. **No fragment.** Each run read `location.hash` by hand: a web native module, a script
   on the page, or `app.ts`. In one run this alone was about 20 calls, most of them
   reading `host/web/navigation.js` and `router.js`.
4. **`var(--…)` refused.** Four of five runs translated `tokens.css` into an `app.tamber`
   by hand.
5. **No `table` or `dl`.** Each run built them from `role="table"`, `row`,
   `columnheader`, `rowheader`, `cell`, `term` and `definition`.

Smaller, seen in one run: a query string in a location
(`` replace(nav, `/?q=${v}`) ``) refused; a `header` drawn as the platform's bar, so the
run used `view role="banner"`; `stack(nav)` changed to `[top(nav)]` so one page shows at
a time; a type error on `aria-hidden`. Before the runs: `aria-sort` was missing (landed as
`f726d13124`), and inside the container exact2's root workspace could not resolve its
private `expo/snapback` git dependency, which a web-only app's build needs resolved.

The benchmark's lines-of-code count includes the `apple/` and `linux/` crates the runs
never touched. D1 makes the count the app.

Two defects sat behind the page-load scores (Charlie, 2026-10-30). Accessibility scored 95
because the `select` tag wrote an explicit `role="combobox"`, which an accessibility
checker then reads as a custom combobox that owes `aria-expanded`; a native `<select>`
has that role on its own (D8). Layout shift of 0.11 to 0.13 and a largest contentful paint
near 2.2 s came from a page that renders only in the browser, so the footer moves when the
API data arrives, as the benchmark's vanilla app does too; the fix is the page arriving
complete, rendered per request (D3.4).

## 2. What `exact new` writes today, measured

`bun scripts/exact.mjs new pokedex` on this Mac, warm, 2026-10-30:

| | |
|---|---|
| Wall clock | 17.9 s, of which `setup --check` is 17.1 s and Cargo's offline resolve of the new workspace 0.5 s |
| Files | 22 |
| Lines, bytes | 4,326 lines, 105 KB |
| Of which `Cargo.lock` | 3,967 lines, 92 KB, 392 packages, copied from exact2 |
| Without the lockfile | 21 files, 359 lines, 13 KB |
| Rust-side files | 13: `apple/`, `linux/`, `web/` (a `Cargo.toml`, `build.rs` and entry each), `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` |
| `app.json` | `host.ios`, `host.macos`, `host.web`; `deploy.store` for web, macos, ios and linux |

What the web uses of it: nothing. The JS target compiles Contract with exact2's own
compiler (`exact-web-js`, kept per machine under `~/.cache/exact/web-modules`) and
captures `app.ts` with Bun; it runs `cargo check` on the app's `web/` crate only when
that crate's `build.rs` is not the scaffold's (`host/web-js/build.mjs:236-240`). So
`web`, `web-build`, `test web`, `agent web` and `verify` never compile the three crates.
What does: the first native build, which compiles the kernel, the runner and a host
again into the app's own `target/` (`bluesky-exact2/target` is 9.0 GB; the euxy port's,
5.6 GB), and `exact run`'s live reload for a native window, whose resident loop is the
web crate's `dev` bin.

What still runs Cargo on the web path is resolution, not compilation. `resolveApp`
(`scripts/app.mjs:490-560`) runs `cargo locate-project` and `cargo metadata --locked
--offline` in the app's workspace on every verb, so a lock that does not resolve stops
`test web`. And exact2's own tools build in the root workspace (`cargo run -p contract`,
`cargo build -p exact-web-js`), whose members include `apps/harmony/app/{apple,linux}`;
those depend on `snapback4/`, whose dependencies are a private git source. `snapback4/`
is excluded from the root for exactly this reason (its `Cargo.toml` says so, and LLP
1036.001 D4 is that decision, implemented 2026-09-30); Harmony's crates bring the source
back. A machine without that git checkout in Cargo's cache cannot build the compiler,
website or not.

The setup check runs every row, the Hermes installer through Cargo among them
(`scripts/exact.mjs` `setup`), before a line of the app is written. Three rows are the
web's: the pinned Bun, Rust stable for exact2's own tools, and Chrome for `test` and
`verify`. The nightly, wasm-bindgen, Binaryen, Xcode, the Hermes bundles and the
simulators are a native platform's or a game's.

A probe compiled the same day with the checkout's `contract` binary settles what the
compiler says to each of §1's constructs: `` replace(nav, `/?q=${v}`) `` is refused by
`route-template` with the form to write; `color="var(--ink)"` by `lower-attr-value`
with the list of colour forms; `table`, `thead`, `tr`, `th`, `td` and `tbody` by
`lower-unknown-tag` with "a table or grid is `view display="grid"`"; `dl`, `dt` and
`dd` by the same with no hint. A `header` holding two links, a `link` whose `href`
ends in `#moves`, and `aria-hidden="true"` compile with no diagnostic.

Two rules the scaffold leans on today, which D1 changes: delivery reads a manifest whose
`deploy` names no native platform as shipping every stream (`scripts/deploy.mjs`
`nativePlatforms`: an empty set is `ios, macos, linux`), and `update --without` refuses
to remove the last native key for that reason (`game/new.mjs` `dropPlatform`).

## 3. Design

### D1. `exact new` writes the web; a native platform is named when asked for and built in a generated workspace

`exact new <path>` writes eight files: `app.contract`, `app.ts`, `app.test.contract`,
`app.json`, `exact.mjs`, `AGENTS.md` (and `CLAUDE.md` beside it, as today),
`package.json` and `.gitignore`. No Cargo workspace, no lockfile, no toolchain pin, no
`.cargo/`, no host crate, then or later. `app.json` names `host.web` and
`deploy.store.web` and nothing else. `.vscode/tasks.json` keeps the tasks for the verbs
that exist.

1. **The manifest is the declaration.** A platform the app ships is a `host.<platform>`
   key (`ios`, `macos`, `linux`, `android`, `windows`), with `deploy.store.<platform>:
   "0"` for the three that hold an update store (ios, macos, linux). Nothing else says
   what ships: `icons.rs`'s `targets` already reads these keys, and the crates'
   presence no longer counts for an app with a generated workspace.

2. **A native verb names its platform, then builds.** `bun exact.mjs ios`, `mac`,
   `linux`, `android` or `windows`; `test`, `agent` and `verify` with a native host; and
   `exact run`, `install` and `release` from the checkout: when the manifest does not name
   the platform, the verb first runs that platform's setup rows in report mode (Xcode and
   the simulator's Hermes bundle for iOS, the Android SDK and NDK, and so on) and stops
   before writing anything when a required row is missing, naming its installer; today a
   missing tool is the next build's failure (LLP 1086 §1). Then it writes the manifest
   keys, says so in one line, and builds. `exact new <path> --with ios,mac` writes the
   keys at creation; `update --with <platform>` and `update --without <platform>` edit
   them later and touch nothing else in an app whose workspace is generated. In an app
   that authored its crates (D1.4), `--without linux` or `android` also removes the
   `linux/` crate, its workspace member and its lock entries once neither names it, as
   the euxy port asked (LLP 1086); `--without` any other platform removes keys alone,
   since `apple/` serves iOS and macOS together. A bare `exact run` of an app that names
   no native platform runs `web`.

3. **The generated workspace.** The crates a native build needs are written under the
   app's `.exact/shells/` (already ignored), the way a game's bake writes `.shells/`
   (`game/app/shells.mjs` `gameShells`, `prepareGame`): `Cargo.toml` (the members, the
   `[patch.crates-io]` table), `rust-toolchain.toml`, `.cargo/config.toml`, `Cargo.lock`
   copied from exact2 and re-resolved offline, `web/` always (the bake's producer for
   delivery and the resident loop a native window live-reloads from), and one crate per
   named platform: `apple/` for ios, macos and tvos; `linux/` for linux and android (LLP
   1107); `windows/` in the shape of `apps/shelf/windows`. The crate text is what the
   scaffold writes today, with paths from the shell's directory. The workspace is
   rewritten whenever its inputs change: the templates, the exact2 checkout's path and
   commit, the manifest's platform set and name. `resolveApp` takes it as the workspace
   when the app directory has no authored `Cargo.toml`, as it takes `.shells/` for a game;
   `target/` stays `<app>/target`. `bun exact.mjs lock` re-resolves its lock, as a
   game's. `update` touches no Cargo file.

4. **An authored workspace wins.** An app that needs Rust of its own (a Rust data crate
   as Caltrain's, a dependency as Harmony's, a render entry) writes its crates and a
   `Cargo.toml`, as apps do today; `resolveApp` finds and checks that workspace as it
   does now, and no shell is generated. The generated crates are plain text under
   `.exact/shells/`, so an author who needs to start from them copies them up. Nothing
   converts one shape to the other: there are two, and the directory says which.

5. **A manifest that names no native platform ships none.** `nativePlatforms` reads an
   empty native set as empty; `--without` may remove the last native key. Delivery of
   such an app is D3.4.

6. **The setup check at creation** is the web's three rows (§2). The check that took 17
   of the 18 seconds moves to the platform that needs it.

The scaffold's test file, `game/new-app.test.mjs`, covers: the eight files and nothing
else; `ios` on a fresh app writing `host.ios` and `deploy.store.ios`, generating
`.exact/shells/` with `web/` and `apple/`, and a second `ios` regenerating nothing;
`linux` after `ios` adding `host.linux` and `linux/` to the shell; `--with`; `--without
ios` leaving `host.macos` and the crate; an authored `Cargo.toml` taking precedence; a
manifest naming no native platform deploying the web root alone.

Not taken:

- **Writing the crates into the author's tree on first use** (r1's D1). Two of three
  reviewers proposed the generated workspace instead, and it is the repository's own
  answer for games. r1's shape would commit a 4,000-line lock and three crates at the
  first `ios`, and every later script would branch on whether they exist. Its one
  advantage, that an author sees the crates, is kept by step 4.
- **Asking before naming a platform.** The author ran `ios`; the manifest key is the only
  way to answer. A question here is the dialogue `exact new` was built to end (LLP 1086).
- **One lockfile shared with exact2.** The scripts refuse a shared `target/` for a
  reason (`AGENTS.md`); a shared lock has the same owners problem. The copy stays, in
  the shell.

### D2. The web verbs run no Cargo in the app, and the root carries no private source

1. **`resolveApp` is told whether the verb needs the app's Rust.** The web verbs (`web`,
   `web-build`, `test web`, `agent web`, `verify`, `serve`, `contract`) resolve with
   `rust: false`: no `cargo locate-project`, no lock check, no shell generation, whether
   or not a workspace exists. The JS build asks for Rust in two cases only: an authored
   `web/build.rs` that is not the scaffold's (`host/web-js/build.mjs:236-240`), and
   `--render rust`. A native verb resolves with `rust: true` after D1.2 named its
   platform, so the lock check still guards every build that compiles. In-repository apps
   are root members and are unchanged.
2. **The dev loop's Cargo reads** (`host/web/dev.mjs`: `cargo metadata` in the app's
   workspace, for a game's runtime inputs and for the resident compiler) run only for
   `--wasm` and a game. A native client that opens the JS loop's URL
   (`host/web-js/dev.mjs`) starts the resident loop's producers from the generated
   workspace's `web/` crate, which `resolveApp` writes when a build first asks for the
   workspace: every build helper reads `app.workspace`, and that read is where a
   generated workspace is written and a lock is checked, so a build script run directly
   (`host/apple/build.mjs`, delivery) needs no verb to have run first.
3. **`bun exact.mjs lock`** on an app with no workspace and no shell says "no Rust here
   yet: a native verb brings it" and exits 0.
4. **Harmony leaves the root workspace.** `apps/harmony/app/{apple,linux,web}` become the
   members of `apps/harmony/app/Cargo.toml`, a workspace of its own beside `snapback4/`'s,
   as `game/` is; the root's `exclude` names it. LLP 1036.001 D4 is restored. The sixth
   check no longer compiles Harmony; `scripts/async.mjs` gains the step that does
   (`harmony-check`: `cargo check --manifest-path apps/harmony/app/Cargo.toml
   --all-targets`, beside its existing `harmony` backend step), on the same trigger as
   the lane's other Rust steps. `resolveApp` finds the workspace as it finds any
   authored one (D1.4).

With 1 to 4, a website's loop is Bun, Chrome and exact2's two prebuilt tools. Rust is
needed once per checkout to build those tools, and `exact new`'s check says so.

### D3. A built site is a directory any static host serves

`bun exact.mjs web-build` already writes the JS target into `target/web-dist`: the
shell, the module, the stylesheet, and for each `render=build` route without parameters
(LLP 1048.000; a parameterised one needs `pages=`, `contract/lower/src/routes.rs`) its
page at `<route>/index.html`, with `404.html` for the `notfound` route
(`host/web-js/build.mjs:815-829`). A `render=build` home page overwrites `index.html`,
and `shell.html` is the copy taken before it, written only when such pages exist. What is
missing is the verb that serves the directory, and the rule that says how anyone else can.

1. **`web-build --out <dir>`** names the directory (`dist/` is the convention the runs
   reached for, and the scaffold's `.gitignore` already lists it; the default stays
   `target/web-dist`). The build always writes `shell.html`, and writes `routes.json`:
   the route table (each pattern, its render policy, the `notfound` route) in the
   plan's order, so a host outside Exact can apply the rule below. The build's last
   lines name the directory and the rule.
2. **The rule.** For a request: a file at that path is served; else, for an HTML
   navigation whose path matches a declared route, `shell.html` with 200; else
   `404.html` with 404 and its body. A host whose only switch is a single fallback page
   serves `shell.html` for every HTML miss, and the client draws its `notfound` route;
   the build says so. `host/web/serve.mjs` today falls back to `index.html` (which a
   rendered home page replaces) and answers a miss with an empty body; it takes the
   two-way rule, reading `routes.json`.
3. **`bun exact.mjs serve [<dir>] [--port <n>] [--lan]`** serves that directory with that
   rule: `serve.mjs` behind a verb, as `web` is `dev.mjs` behind one. It binds loopback by
   default and prints the URL; `--lan` binds every interface, as the dev server does by
   default (LLP 1023 D8 ruled for the dev loop, which a phone must reach; a finished site
   carries D4's inlined settings and need not). The scaffold's `package.json` gets
   `"scripts": { "start": "bun exact.mjs serve" }`, so `npm start` and `bun start` are
   the verb.
4. **Routes a static directory cannot honour.** A route declared `render=request` or
   `cached` is a render server's (LLP 1048.002, `host/render`); a static root serves its
   shell, and `web-build --out` says so once per such route. `serve` renders them: in the
   two-way rule, a request whose path such a route matches gets its page rendered under Bun
   (`host/web-js/render.mjs` `pageServer`: a `cached` page kept 60 s, brotli, a navigation's
   head sent first), not the shell, so the page arrives with its data rather than drawn after
   it in the browser (the benchmark's page loads measured a CLS of 0.11 to 0.13 and an LCP
   near 2.2 s from that). Files, client routes and misses are served as the rule says. A
   render server elsewhere stays a deployment the author chooses (`scripts/deploy.mjs`).
5. **Pages for a scaffolded app render under Bun.** `--render rust` needs an
   `<app>-render` entry that only an authored workspace has; `web-build` defaults to
   `--render js` when there is none, and says so once.
6. **A web-only deploy publishes the build and no envelope.** `scripts/deploy.mjs` today
   bakes the `<app>-web` crate for every stream, requires its `exact.json`, and publishes
   the JS build of that bake's plan as the web root (LLP 1071 §7, item 4). An app whose
   manifest names no native platform (D1.5) has no stream and no envelope producer: deploy
   publishes `web-build`'s output as the root, bakes nothing, and writes no `exact.json`;
   the shell omits its `rel="alternate"` link. A native client has nothing to run until a
   platform is named, and the first native deploy bakes as today.

Not taken: a Node-compatible `server.mjs` written into the app, which is what the runs
did. It is apparatus the author then owns, and `serve.mjs` has the generations, the
content types and the dot-path refusals right already.

### D4. A setting read at build: `import.meta.env.NAME`, declared in the manifest

The bake never runs `app.ts`, so a value a module reads from `process.env` at run time
does not exist; the bake process itself reads the environment, and already replaces one
constant: a top-level literal `const origin` and the `net.fetch` grant written over it,
from `EXACT_BACKEND_ORIGIN` or `.exact/backend-origin` (`js/bake/src/typescript.mjs:26-32`).
It is named for Snapback's backend, documented only in `reference.md` and the Snapback
recipe, and a spec that says `API_BASE_URL` gives an agent nothing to find it by.

The web's spelling for a value fixed at build is `import.meta.env.NAME` (Vite's; Next's
is `process.env.NEXT_PUBLIC_NAME`). Both expose only names marked public, because an
inlined value ships to every client. Exact's mark is the manifest:

1. **Declared names.** `app.json` `"env": ["API_BASE_URL"]`. A name is
   `[A-Z][A-Z0-9_]*`. A read of an undeclared name is refused at bake, naming the key; so
   is a declared name no module reads. Every declared value is public by construction, and
   `app-json.md` says so.
2. **The sources**, in order: the process environment, then `.env.local`, then `.env`
   beside `app.json` (the dotenv line grammar: `NAME=value`, `#` comments, quoted values).
   `.env.local` goes in the scaffold's `.gitignore`. Bun loads `.env` files from the
   working directory into `process.env` on its own, which would make a file's value look
   like the environment's; the bake reads the files itself and runs its capture with
   Bun's loading off (`--no-env-file`), so the order above is the order.
3. **The read.** `import.meta.env.NAME` in a top-level `const`, optionally with a default:
   `const API = import.meta.env.API_BASE_URL ?? 'http://127.0.0.1:8080'`. The static
   pass that today evaluates `appId` and `grants` (`staticFacts`, which reads exported
   names only) is extended to evaluate these constants, and the value is inlined where
   the constant is read. A grant template over it (`` `net.fetch ${API}` ``) follows the
   constant, as it follows `origin` today. A read anywhere else is refused: the value is a
   constant of the build, not a variable of the program.
4. **Unset.** A declared name with no value and no default fails the build, naming the
   three sources; a build must not guess an origin.
5. **A build input.** Each declared name's value and source are recorded in the web
   build's input digest (`webInputDigests`, which today records `EXACT_BACKEND_ORIGIN` and
   `PORT`) and in the bake's receipt, so a changed value rebuilds every target, a stale
   native build is seen as stale, and a release records what it inlined. Delivery bakes
   with the same sources and refuses an unset name as the build does.
6. **At serve.** `API_BASE_URL=… npm start` against a finished build is how hosts run:
   `serve` compares each declared name in its environment with the build's record; a
   difference rebuilds when the sources are beside the build, and refuses, naming the
   name and both values, when they are not (a `dist/` copied elsewhere).
7. **On every host.** The value is baked into the module, so iOS and Linux read what the
   web reads. The agent's `logs` show each name and its source at bake, as the backend
   selection prints its source today.

`EXACT_BACKEND_ORIGIN` and `.exact/backend-origin` keep their job, the driver's selection
of a development backend, and their names (Grok, Muse: a driver's override must not look
like the author's `.env`). The evaluator is one: the literal-`origin` rewrite becomes a
case of the constant evaluation above, so there is one reader of build inputs; whether the
rewrite's spelling then goes is §5's question 2.

### D5. The fragment: a field of the entry, scrolled to by each host when the route settles

LLP 1038 §3 defines a Location as `path[?query]`, "never a scheme, host, or fragment";
canonical identity is `pathname + search`, and two locations with equal canonical forms
are the same screen. The web router reads `location.pathname + location.search` at launch
and on every link (`host/web-js/router.js:160`), and leaves a same-document fragment link
to the browser (`:161`). The fragment is dropped before the app sees it.

On the web a fragment means: after the document loads, find the element the fragment
names and scroll to it (HTML, "scroll to the fragment": the fragment as given, then
percent-decoded; `top` when nothing matches; the target becomes the sequential focus
navigation starting point). An Exact page's content arrives after load, when its
resources answer, so the browser's step finds nothing. The runtime does the browser's
step at the right time, and the identity of a screen does not change:

1. **Identity stays path and query.** The canonical form is unchanged; `match`, `go`'s
   dedup and `chain` never see a fragment. The Location grammar becomes
   `path[?query][#fragment]`: a location with a fragment names the same screen as one
   without, and the fragment rides on the entry: `Entry.fragment` (a string, `""` when
   none), kept as written after `#` with the fragment percent-encode set applied, beside
   `url`. `fragment(entry)` reads it. `path("pokemon", name) + "#moves"` writes one;
   `location_of` and a protocol handler's location keep it (the handler location today
   carries the original URL as data, fragment included).
2. **A fragment-only change is not a new entry.** `go`, `push` or `replace` to a location
   whose path and query equal the top entry's updates that entry's fragment in place; on
   the web this is one browser history entry, mirrored as the entry's url with the hash,
   so the browser's Back restores the fragment and position it had, as `hashchange` does.
   A same-document link (`href="#moves"` in the source; the shell's `<base href="/">`
   makes the resolved URL `/#moves`, so the runtime reads the attribute, not the resolved
   URL) is such a change, handled by the runtime rather than left to the browser.
3. **Each host scrolls.** The JS target has no runner at run time and the runner draws no
   UIKit, so the scroll is each host's: when an entry whose fragment is non-empty becomes
   the top of its stack, or its fragment changes while it is, and its route's content has
   settled (LLP 1113), the host finds the element with that `id` inside that entry's
   route subtree (a retained entry beneath may hold the same `id`), and scrolls every
   scroller above it until it shows, as `scrollIntoView(id)` would with `block="start"`;
   `top`, or no match, scrolls that route's scroller to its top; the target becomes the
   sequential focus starting point. A scroll by the person before settle cancels it. A
   return to the entry (Back, a tab switch) restores its retained position and does not
   scroll again; a new activation with a fragment (A to B to A again, `go` with `#moves`
   on an entry already showing `#stats`) scrolls again. An author's own `scrollIntoView`
   in a `task` wins, as anything written does.
4. **On native hosts** a universal link or a custom scheme's URL arrives through the same
   location (`app.json` `protocol_handlers`, LLP 1038 D8), fragment included, and step 3
   runs in that host.

The agent reads the fragment in `state` as it reads the entry's url today. A test's
launch line `at "/pokemon/pikachu#moves"`, new beside `size` and `epoch`
(`docs/recipes/tests.md`), opens there; the test asserts the entry's fragment, then the
scroller's `scrollTop`. LLP 1038 §3's Location paragraph and its `location_of` sentence
are amended to say this.

### D6. `var(--name)` is the token `name`, resolved as the token's own spelling is

Tamber is the design system (LLP 1121): an `app.tamber` declares tokens, the build folds
them, and on the web a token that varies by an app axis is the custom property
`--x-<name>`, unregistered, while one that does not is folded inline (its D7). The
compiler refuses `color="var(--ink)"` with the list of colour forms (`lower-attr-value`,
§2's probe), so an app whose colours arrive as `tokens.css` is translated by hand.

The web's `var()` is substituted by the browser at computed-value time, which is what
lets a page's stylesheet redefine a property. Exact's tokens must be drawn by every host,
most of which have no stylesheet, so `var()` here is the build's: a reference to a
declared token, in the spelling the web taught. That is a narrower meaning than CSS's,
and this RFC says so rather than imitating the wider one.

1. **`var(--name)` is the token `name`**, wherever a style attribute takes a value of the
   token's kind: a colour token in `color`, a length token in `padding`, a font token in
   `font`. The compiler lowers it exactly as writing the token's name does: folded, or on
   the web `var(--x-name)` where the token varies, so scheme and mode resolve as they do
   for the token. An undeclared `--name` is refused, naming the declared tokens of that
   kind, whether or not a fallback is written: a typo must not ship the fallback
   silently. `var(--name, fallback)` with a declared name is the token; the fallback is
   ignored and warned once, since Exact never leaves a token undefined.
2. **`bun exact.mjs tamber import css <file>`** reads a stylesheet's custom properties
   into `app.tamber`, between markers (`// tamber:import <file>` … `// tamber:end`, in
   the file's own comment syntax) that a
   later import of the same file rewrites, so an edited `tokens.css` is re-imported, not
   re-translated; the author's own tokens stay outside the markers. The subset it
   translates: each `--name: value` on `:root` becomes a token; a redeclaration under
   `@media (prefers-color-scheme: dark)` makes the pair a `light-dark()`, which is what
   that query means; `--a: var(--b)` becomes a reference to the token `b`. What it does
   not translate it refuses by name, and writes nothing for: a `.dark` or
   `[data-theme="dark"]` scope (an author-controlled switch, which is a Tamber axis the
   author declares), a value composed around a `var()` (`hsl(var(--h) var(--s) var(--l))`),
   a value the evaluator cannot type. Today's `tamber import <kind> <source> <server url>`
   is the design tool's wire and stays.

Not taken: reading `tokens.css` at build without an import. A stylesheet is a file the
author owns; the tokens it declares become the app's visibly, in `app.tamber`, where
`contract vocab` and the design tool can see them.

### D7. `table` and `dl`: HTML's elements, carried as what they are

The compiler refuses `table`, `thead`, `tbody`, `tr`, `th` and `td` with "a table or grid
is `view display="grid"`", and `dl`, `dt`, `dd` with no hint. The guide's answer is ARIA's
roles on boxes, which every run reached after the lookups. The names are the web's, and
LLP 1115 says names stay CSS's; HTML's element names are the same rule.

r1 lowered a table to a grid at compile time, on every host. All three reviewers refused
that: a grid of `auto` tracks is not CSS 2.1 §17.5's automatic table layout (an auto-width
table shrinks to its columns, a block grid fills its container; spans, captions and
column minima differ), a compiled-away table leaves the Mac nothing to map to
`NSTableView` (LLP 1115.002), and `dt dt dd` groups are not two columns. So:

1. **The plan carries the table.** New node kinds, with HTML's semantics: `table`,
   `caption`, `thead`, `tbody`, `tfoot` (row groups), `tr`, `th` with `scope` (`col`,
   `row`) and `aria-sort`, `td`; `colspan` and `rowspan` on cells. And `dl`, `dt`, `dd`,
   grouped as HTML groups them (one or more `dt` then one or more `dd`). The agent's
   `tree` shows them by these names.
2. **On the web** they are those elements: the browser's table layout and semantics,
   `aria-sort` the attribute it is since `f726d13124`.
3. **On native hosts, for now, the kernel paints the node as a grid.** At layout, over
   the real rows, it runs HTML's "forming a table" placement (so `colspan` and `rowspan`
   land where the table algorithm puts them), sizes columns as `auto` grid tracks, and
   sizes the table box to its columns (shrink-to-fit), not its container. ARIA's roles
   are exposed from the node kinds. The declared deviation from the web is column
   widths: grid tracks, not §17.5's minima and maxima. `dl` is HTML's own layout on
   every host: block, `dd` indented by the browser's default margin.
4. **The Mac** maps the same node to `NSTableView` when LLP 1115.002 takes it; a data
   table with sortable headers is that control to anyone who knows the platform. iOS has
   no data table control; the grid is what a hand-built screen would do. Neither refuses
   `table`, which would stop every cross-platform app for one platform's tell.

`contract vocab table` and `dl` answer with the tag; the guide's ARIA paragraph goes. The
conformance run compares the web's table to the kernel's on fixtures with spans, so the
deviation is measured, not assumed.

### D8. The smaller snags, and orientation

- **A query in a computed location.** `` replace(nav, `/?q=${v}`) `` is refused with
  "write `` path("home") + `?q=${v}` ``" (`route-template`). The diagnostic is right and
  complete; no change. `docs/start-here.md`'s navigation line gains that form.
- **A `header` on the web is the web's app bar** (`docs/recipes/web-look.md`): sticky,
  styled. A site header holds a `nav` of links, a logo and a search field, and the run
  that met the bar fell back to `view role="banner"`. The plan's node is unchanged; the
  web host's presentation rule is: a `header` that holds a `nav`, or more than one
  `link`, is a site header, drawn as written; the bar look stays for the header that would
  be iOS's bar (one heading, buttons). `appearance="none"` on a header, which iOS already
  honours, does the same on the web and is the spelling the recipe teaches.
- **One page at a time.** The web already shows only the selected route of each stack:
  the others are `visibility: hidden` and `inert` (`host/web-js/document.js`
  `markRoutes`), so `[top(nav)]` was a change for nothing and lost retained scroll.
  `start-here.md` says so in its navigation line; the agent's `tree` marks a hidden
  route as such, so the next agent sees it rather than infers the opposite.
- **`aria-hidden="true"`** compiles since `f726d13124` (an ARIA word state is no longer
  a bool). The probe in §2 confirmed it. No change.
- **A native element's own role is not restated** on the web. The wasm host already left
  off a button's `button` and a link's `link` (ARIA in HTML: authors should not restate
  an implicit role); the rule moves to one place both web hosts use
  (`host/web/src/roles.rs`), the JS target applies it to the attributes it writes ahead
  of time, and a single `select`'s `combobox` joins the table, so the checker no longer
  demands an `aria-expanded` the native control never carries.
- **Orientation.** The runs spent 24 to 43 calls reading. D3 to D7 remove the reasons for
  most of them: the fragment, the table and the setting no longer need a lookup. What
  remains is one tested starting point: a `site` pattern (`bun exact.mjs pattern site`)
  with a list route that renders at build, a detail route with a parameter that renders
  on the client (a parameterised `render=build` route needs `pages=`, which a site whose
  data is an API cannot list) and is served by the shell, a site header with a `nav`, a
  `table` with `aria-sort`, a `dl`, the detail opened at a fragment, a declared
  `import.meta.env` origin with its grant and a `.env`, and `serve` as `package.json`'s
  `start`. `start-here.md` gains one row in its "Not in a pattern" table: *a website:
  serving, a setting, a deep link* → `recipes/website.md`, a page of one screen.

## 4. Costs

- **Two workspace shapes.** `resolveApp` already has a generated-workspace path for
  games; an app takes it when it has no authored `Cargo.toml`. One branch, in one place.
- **A verb (`serve`), three flags (`--with`, `--without` for every platform, `--out`,
  `--lan`), a manifest key (`env`), a pattern, a recipe, an importer kind, two build
  files (`routes.json`, `shell.html` always).** Each is named above, and this RFC is the
  approval RULES asks for.
- **Delivery's rule inverts** (D1.5): a manifest naming no native platform ships none.
  Every in-repository app names its platforms already.
- **The bake reads `.env` files and records values.** Two files, one line grammar, in the
  capture that already reads `app.ts` statically; the digest and the receipt gain the
  names.
- **A fragment field on the entry** touches `route/`, the runner's router, both web
  routers, and each host's scroll. LLP 1038 §3 changes in two sentences.
- **Eleven node kinds** in the plan's tables and the kernel's placement for a table.
  `aria-sort`'s commit shows the shape of a kind's traversal: ten files, 69 lines; the
  placement is new.
- **`var()` resolution** is a lookup in the evaluator Tamber already runs at build.
- **Harmony's lane step** is one row in `scripts/async.mjs`.

## 5. Questions for Charlie

Each has a recommendation; above about 80 the implementation proceeds on it and this
section is where to veto.

1. **D1's generated workspace instead of crates in the tree.** Recommended as written,
   80: two reviewers proposed it independently, games already do it, and the author's
   tree holds the app and nothing else. The cost is that an author who wants to read the
   crates looks under `.exact/shells/`. The measurement in §6 is where r1's shape would
   show it was better.
2. **D4 and the backend origin.** Fold `.exact/backend-origin` and `EXACT_BACKEND_ORIGIN`
   into `import.meta.env` underneath, or leave them as the driver's own switch?
   Recommended: leave the names and the switch (two reviewers of three); the evaluator
   is shared now, and the literal rewrite's spelling goes when Snapback's recipes are
   next edited.
3. **D5's scroll.** Once per activation with a fragment, including a fragment-only
   change on the same entry, never on a return that restores position. Recommended as
   written, 85: it is the browser's `hashchange` and load behaviour.
4. **D7 on the Mac.** Paint the grid now, map the node to `NSTableView` under LLP
   1115.002 later. Recommended, 80: the node survives, and a refusal would stop every
   cross-platform app for a Mac tell.
5. **`serve`'s bind.** Loopback by default, `--lan` to widen. Recommended, 75 (two
   reviewers of three): `npm start` is a local convention, the dev loop keeps LAN.
6. **The working set.** `llp/current/` is at its cap of 15. Linking this RFC archives
   `1111-apps-as-tools-for-agents.rfc.md`, the working set's stalest draft (untouched
   since 2026-10-08, r3, NOT READY from both families); r1 proposed 1079.000, which all
   three reviewers found unfinished. Swap if that is the wrong one.

## 6. Order of work

Each slice lands on `origin/main` after Astra and Grok review it, with its tests.

1. **D1, D2.1 to 2.3, D1.5**: the eight-file scaffold, the manifest as declaration,
   the generated workspace and `resolveApp`'s `rust` flag, `--with` and `--without`,
   the web-only setup rows, `nativePlatforms`. Verified by `exact new` into a temporary
   directory, `test web` and `verify` with no Cargo run in the app (the diary's command
   log shows none), then `ios --run` naming the platform, generating the shell and
   building; `--without ios` leaving macOS; an authored `Cargo.toml` taking precedence.
2. **D2.4**: Harmony's workspace and lane step. Verified by `cargo metadata --locked
   --offline` in the root with the git source absent from Cargo's cache, and the lane
   step green.
3. **D3**: `--out`, `routes.json`, `shell.html` always, the two-way rule in `serve.mjs`,
   the `serve` verb, `--render js` by default, the web-only deploy. Verified by serving
   `dist/` and requesting a rendered page, a parameterised route, a missing file and an
   undeclared path, and by a dry-run deploy of a web-only app.
4. **D4**: the `env` key, `import.meta.env`, the sources, the digest and receipt,
   `serve`'s comparison. Verified by the bench's own spec: `API_BASE_URL` in `.env`, the
   grant following it, a build with it unset refused, `serve` with a changed value
   rebuilding.
5. **D5**: the fragment. Verified on the web (a launch at `#moves`, a same-document
   link, Back) and on iOS through a universal link.
6. **D6 and D7**: `var()`, the importer, the eleven node kinds, the kernel's placement.
   D7's conformance run compares the web's table to the kernel's on span fixtures.
7. **D8**: the `site` pattern, the recipe, the three lines in `start-here.md`. Then the
   measurement LLP 1086 D11 asks for: the Pokédex spec once more in a fresh session, and
   once more with "now add iOS" as a second prompt, counting calls that read or edit
   anything under `.exact/shells/` or a Cargo file (which should be none), the time from
   `ios --run` to the first screen, and cost beside §1's. If the web run's cost falls
   mostly by the 17 s setup check, r1's alternative (Muse's: the full scaffold with the
   web-only check) would have been enough, and this section says so.
