# LLP 1086: What an app author is given

**Type:** RFC
**Status:** Draft r2, 2026-10-04; implemented the same day (§3 as built). Reviewed blind by Astra (max) and Grok (xhigh): both SOUND WITH CHANGES; §6 lists what r2 changed. Charlie asked for it to be implemented after review (2026-10-04); his request is the approval RULES asks for the build script (D3) and the DEFERRED bullet (D9).
**Systems:** Scaffolding (`game/new.mjs`: the files `exact new` writes and `--update` rewrites), the app's command runner (the generated `exact.mjs`), setup (`scripts/exact.mjs` `setup`), the app manifest schema (`scripts/app.schema.json`, `scripts/app.mjs` validation), the Contract CLI (`contract/cli`: a `vocab` command), the author docs (`docs/contract-for-humans.md`, `docs/contract-for-agents.md`, `docs/contract-grammar.md`, `docs/reference.md`), `README.md`, `AGENTS.md`/`CLAUDE.md`, `rules/DEFERRED.md`, LLP 1000
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Related:** LLP 1036.001 (apps outside the repository: what `exact new` writes); LLP 1035.005 (Contract authoring ergonomics: `fmt`, `symbols`, maps); LLP 1081 (moves the style-name table out of `tags.rs`, which would make D3's enumeration direct); `docs/agent-pitfalls.md` (its rule: an entry is deleted by the change that fixes the footgun); the authoring diaries: `signal-exact2/DIARY.md` (2026-10-02 to 10-03), `bluesky-exact2/DIARY.md`, the dice-tray diary (2026-10-04, fixes on `origin/tuft/diary-fixes`)

## Summary

Charlie asked what the repository still needs so that people, and the agents
working for them, can build with exact2 as easily as possible.

A lot is already there. The README has a quick start and a coding-agent prompt
that a fresh session finished in about 23 minutes. Three guides exist: one for
humans, one for agents, and a grammar reference. Their 16 complete Contract
programs all compile on today's compiler. The pitfalls list is kept honest by
its own deletion rule. The compiler's diagnostics teach well: stable ids, exact
ranges, "did you mean", and JavaScript idioms translated into Contract.

The gaps are at the edges an author meets first. Each was hit in a recorded
trial or is visible on `main`:

1. **A new app can't find the docs.** `exact new` writes no `AGENTS.md`. An
   agent working in `../hello` never learns the guides exist. This was the
   dice-tray diary's first complaint.
2. **The data module is undocumented.** No guide, and nothing in
   `reference.md`, shows a line of `app.ts`.
3. **No list of what can be written.** The grammar reference declines to list
   style properties and points at `schema.json` and `tags.rs`. Agents guess
   CSS and are refused.
4. **The guides' commands assume the exact2 root.** They use
   `cargo run -p contract -- … apps/caltrain/…`, which doesn't work from an
   outside app. Three of the dice-tray fixes are still unmerged.
5. **`app.json` has no author-facing description**, and the scaffold gives
   editors no schema.
6. **Setup reports missing prerequisites one at a time.** `exact new` checks
   none of them.
7. **Nothing tells a learner which example apps to read.** Caltrain, the
   reference app, has no README.
8. **Some pointers are stale.** `AGENTS.md` names Weird Castle, which no
   longer exists. LLP 1000 predates the JS target.

Two process gaps sit underneath these. Policy says "sparse prose", so docs
drift is nobody's job. And nothing fails when a guide's example stops
compiling.

This RFC has eleven decisions. D1–D8 fix the eight gaps above, D9–D10 fix the
two process gaps, and D11 is the trial practice that found them. None adds a
blocking check. D10 is a test in an existing suite.

## 1. The author's path today

What an author meets, in order, with what happens at each step:

| Step | Today |
|---|---|
| Read the README | Good: principles, quick start, the agent prompt, what works. |
| Install the tools | Five bullets. `exact setup` covers Rust, wasm-bindgen, Binaryen and `bun install`. Xcode, Chrome and Hermes stay manual, and a missing one shows up as the next build's failure. |
| `exact new ../hello` | Smooth when Cargo's cache is warm. Writes `app.contract`, `app.ts`, `app.json`, the host crates and `exact.mjs`. **No README, no AGENTS.md, no `$schema`.** |
| Learn Contract | `docs/contract-for-agents.md` (the trial agent called it excellent), the human guide, the grammar. **Reachable only from exact2's README.** |
| Write the view | Diagnostics are good. **No list of style properties.** CSS shorthands such as `padding="12px 40px"` are refused (fixed on `tuft/diary-fixes`). |
| Write `app.ts` | **Nothing shows how.** The scaffold's 10-line `greeting` source is the only example short of reading Weatherlight or Fieldnotes. |
| Compile, inspect | **Every guide command is the exact2-root form.** |
| Test and drive | `bun exact.mjs test web` and `agent web …`. Good once the app is built. Before the first build it gives a root-form hint (fixed on `tuft/diary-fixes`). |

## 2. Design

### D1. `exact new` writes the app's agent instructions

`exact new` writes `AGENTS.md` and an identical `CLAUDE.md` into the app. They
are identical because exact2 itself keeps its two copies that way: two
harnesses, one text. The file is short and says only what an agent in the
app's directory can't discover:

- **What this is.** An Exact app. The view is in `app.contract`, the data in
  `app.ts`, the manifest in `app.json`.
- **Read first.** The three guides and the pitfalls list, as paths relative
  to the app, computed the same way `exact.mjs` computes its `EXACT2` default.
- **The app's commands.** Every `bun exact.mjs` verb, including D4's
  `contract`.
- **The loop.** `bun exact.mjs contract types app.contract -o
  app.contract.d.ts` first, because `app.ts` imports the file it writes and
  nothing else creates it before a host build. Then edit, `contract build
  --json`, `test web`, drive with `agent web`, and finally the native hosts.
- **Don't edit the generated parts.** The `[patch.crates-io]` table,
  `rust-toolchain.toml`, forced-offline `.cargo/config.toml`, and `exact.mjs`
  are rewritten by `bun exact.mjs update`.

The generated text sits between `<!-- exact:begin -->` and `<!-- exact:end -->`
markers. `--update` rewrites only what is between the markers, and appends the
block when they are missing, so an author's own notes survive. An app that has
neither file gets both. An app with only one gets the block in that one only.

A game gets the same (2026-10-04, the platformer's diary R1: a game had no
`exact.mjs`, `AGENTS.md` or test file). `exact new <path> --game`, or
`game/new.mjs` with a path outside the checkout, writes them beside the
template's files; its `exact.mjs` adds `test-rust` (the hostless Rust tests)
and `prove` (the proof's baseline), its notes point at `game/README.md`, and
`--update` rewrites only those two, since a game's Cargo workspace is the
bake's (`.shells/`).

The template's root is full-bleed (2026-10-07, the authoring bench's iOS
diaries: the status-bar strip was black and finding the fix cost about five
minutes): `viewport-fit="cover"` on `main`, and padding of
`calc(env(safe-area-inset-*) + 24px)` on each side, so the background fills
behind the status bar and the content keeps clear of it. On the web the insets
are 0 and the page is unchanged.

Not taken:

- **A skill directory in the app.** Skills are discovered per harness, and a
  plain file reaches every harness.
- **Copying the guides into the app.** Copies go stale. Paths follow the
  checkout because `--update` rewrites them.

### D2. The data module gets a worked example

The README already shows a complete `app.ts`: the todo list, which is in
memory. `docs/contract-for-humans.md` gains a section, "Writing the data
module", that continues that example instead of starting over. It contains one
complete, compiling `app.ts` beside the Contract that uses it:

- a synchronous source;
- an async source that uses `fetch`, with the `net.fetch` grant it needs;
- a source that reads and writes `storage` (SQLite, with its grants), plus a
  mutation over it that `refreshes` the list;
- the `answer` dispatcher, `appId` and `grants`.

The section also states the rules an author otherwise learns by failing:

- Baking runs with no network or storage, and catching `Unavailable` gives a
  placeholder.
- SQLite integers are `bigint` and must be converted before they are returned.
- `app.ts` imports only local files. There are no npm packages yet.
- Grant verbs are a closed list (`grants/src/lib.rs`), and the section names
  all of them.
- A Rust data crate is the alternative. It is shown in a paragraph pointing at
  `contract rust` and Caltrain's `data/`, not taught.
- **How to verify it.** Generate the types, build, then drive with
  `--storage <name>`, which an ordinary agent drive refuses
  (`host/web/storage-environment.js`). Without that flag a drive exercises
  only the storage-unavailable path. Authored tests took no `--storage` at all
  when this was written; main's `agent-test.mjs` (`af670d6b9`, the same day)
  now gives each test an empty store of its own, so `test web` needs no flag. On the web, every drive starts a new
  browser profile, so persistence across launches is shown in the dev loop or on
  a native host, not by a second drive.

`docs/contract-for-agents.md` gains the same example in condensed form under
"Data requests and side effects".

`docs/reference.md`'s TypeScript section keeps its reference prose. It loses
the run log in its second half: the iPhone 17 Pro Max sweeps, the gesture
mitigations, "manual verification remains owed", and the Safari delay. That
material is history, and git and LLP 1027 keep it. What a reader needs from it
moves into one "Limits" paragraph: no Linux native TypeScript, no npm capture,
no signed module updates.

The example is checked by D10, and also built and driven as a real outside app
before it lands.

### D3. `contract vocab`: the compiler lists what it accepts

The new command is `contract vocab [--json] [<name>]`. It prints what the
compiler admits:

- every built-in tag, with its family and the kernel node it makes;
- every attribute, with its kind:
  - a **style** attribute, with the rows it sets, each row's codec
    (`dimension`, `color`, an enum's values and so on) and its default;
  - a **prop**, with its type;
  - a **handler**;
- the renamed spellings, each with its replacement.

With a name, it prints just that entry. If the name isn't admitted, it prints
the same "did you mean" the compiler would.

**The list comes from the lookups themselves.** `tags.rs`'s `tag()` and
`attr()` are `match name` expressions with no table to iterate. `renamed()`
and `html_tag()` are the same.

1. **Scan.** A build script in `contract-lower` scans those four functions'
   `match name` arms, and only the patterns left of `=>`. Value literals
   inside arm bodies (`"flex"`, `"pre-wrap"`) are never candidates. It follows
   or-patterns across lines.
2. **Fail on anything else.** The script fails the build if an arm in those
   matches is anything other than string-literal patterns or `_`. That makes
   the scan complete rather than sampled: a new arm is either seen, or it
   breaks the build with a message saying why.
3. **Filter and classify.** `vocab` keeps the candidates the live lookup
   admits, and classifies each by its `AttrTarget`: `Styles`, `Prop`,
   `InvertedBoolProp`, `Handler`, `Flex` or `Surface`.
4. **Describe.** It attaches each style row's codec, enum values and default
   from the kernel's schema. The lookup's return can't supply those.
5. **Rules no table lists.** It prints them from the compiler's own constants:
   - the `head` fields;
   - the attributes admitted only on certain tags (`lib.rs`'s contextual
     refusals);
   - the two open sets that bypass the lookups: a hyphenated tag is a native
     module declared in `app.json` (`native.rs`), and `data-*` must be
     declared too (`dataset.rs`).

The generated list is a build artefact, not a committed file. It can't drift:
the scan is total over the patterns, and admission is decided by the same
function the compiler calls. A unit test checks that every scanned candidate
resolves or is a refusal (`renamed`, `html_tag`).

If LLP 1081 lands its iterable `style_names.rs` table, the style half of the
scan is replaced by that table, as Astra recommends.

`docs/contract-grammar.md`'s "Style and prop names come from `schema.json` and
`tags.rs`… This document does not duplicate their changing property tables"
becomes "run `contract vocab`". The agent guide's inventory says the same.

Not taken:

- **A generated Markdown table committed to `docs/`.** Generated files are
  built, not committed.
- **Rewriting `tags.rs` as a table.** That is LLP 1081's job, and `tags.rs`
  is at 1,499 of its 1,500 lines.

### D4. One command form that works from the app

The generated `exact.mjs` gains a `contract` verb. It runs exact2's compiler
as `cargo +<pinned stable> run -q --manifest-path <exact2>/Cargo.toml -p
contract -- …` with the remaining arguments.

- **Debug, not `--release`.** The guides already use the debug build, and a
  release build would add a cold kernel-and-runner compile to the author's
  first command.
- **From the app directory**, so relative paths resolve against it.
- **The environment:**
  - The toolchain is named explicitly, and an ambient `RUSTUP_TOOLCHAIN` is
    dropped. rustup picks the toolchain from the cwd and the environment, not
    from `--manifest-path`, and `scripts/app.mjs` already guards against
    mise's override.
  - An inherited `CARGO_TARGET_DIR` is dropped, so the compiler builds into
    exact2's own `target/` and never into the app's or another checkout's.

So `bun exact.mjs contract build app.contract --json` and `bun exact.mjs
contract vocab padding` work from inside the app.

Every command in the three guides is shown in the app form. The root form
(`cargo run -q -p contract -- … apps/<name>/…`) is kept once, as "inside the
exact2 checkout".

`origin/tuft/diary-fixes` lands first: `cc0b33e65` and `5ad4813de`, the three
dice-tray fixes and an `exact.mjs` that builds a stale web app before driving
it. They were reviewed with the diary and are independent of this RFC.

### D5. `app.json` describes itself

- The scaffold writes `"$schema"` as the path to `scripts/app.schema.json`,
  relative to the app. `--update` rewrites it when the checkout moves. Editors
  that read JSON Schema then complete keys and show descriptions.
- `app.schema.json` already admits a top-level `$schema`, and both web
  builders whitelist manifest keys, so it never reaches a web manifest. No
  validator change is needed.
- Each key that an author sets, or that `exact new` writes, gets a
  `description` that says what it does in a sentence. The LLP citation follows
  the sentence instead of replacing it. Those keys are:
  - `name`, `short_name`, `id`
  - `app.id`, `app.name`, `app.command`
  - `host.ios`, `host.macos`, `host.web`
  - `deploy.store`
  - `modules`, `data`, `typescript.placement`, `rust.placement`
  - icons
  - `file_handlers`
  
  Internal keys keep their current text.

### D6. Setup reports everything missing, once

- `exact setup --check` collects every problem and reports them together,
  instead of throwing at the first. It exits non-zero only after printing the
  whole table.
- It gains rows for:
  - exact2's `node_modules`;
  - Chrome, found through the agent's own `chromium()` lookup
    (`agent-launch.mjs`). It is needed for the web loop, which is how every
    app is driven.
  - on macOS, an Xcode `xcode-select -p`, not the Command Line Tools;
  - the digest-pinned Ibex **host Hermes bundle**, including its paired
    `hermesc`, lean archive and receipt. This row is required: `setup --check`
    fails when it is absent, and `setup` runs the install-once bundle installer
    for the host target;
  - on macOS, optional rows for the exact iOS Simulator target and
    `aarch64-apple-ios` device target. Each prints its complete installer
    command. `host/apple/build.mjs` preflights that selected target and repeats
    the command rather than acquiring during Cargo. tvOS TypeScript refuses
    until E2's v4 bundles.

  Rows that depend on the app are reported as *needed for …*, not as
  failures: a Rust app needs no Hermes.
- `exact new` runs the check in report mode **before** it touches Cargo. A
  missing `cargo` today fails at `cargo metadata` and deletes the half-written
  app, and a report printed afterwards can't help with that. It then writes
  the app and prints anything missing below its command list.

`exact new` also writes the root's forced-offline Hermes Cargo configuration
into the outside workspace and names the one-time installer in generated
`AGENTS.md`. Its generated command runner exports the offline refusal too, so
an older or edited workspace cannot turn an ordinary app build into engine
acquisition. Setup does not install Xcode or the optional iOS bundles.

### D7. Say which apps to learn from

The README's "Example apps" section opens with four apps to read, and what
each teaches:

| App | What it teaches |
|---|---|
| Caltrain | Rust data, authored tests, routes, a GPU module |
| Weatherlight | TypeScript with `fetch` and grants |
| Fieldnotes | storage: SQLite, files, backup |
| Recorder | strings, native modules |

Caltrain gets a README of about 30 lines: what it is, how to run it on each
host, and which files to read first. Fixtures and stress apps get no README.
Their names already say what they are.

### D8. Stale pointers

- `AGENTS.md` and `CLAUDE.md` gain three lines at the top. If you are building
  an app rather than working on exact2, read the agent guide and the pitfalls
  list, and use `exact new`.
- The Weird Castle line becomes `exact new`, in `AGENTS.md` and in
  `game/README.md`.
- LLP 1000's map gains `host/web-js` (LLP 1071) and `host/render`, the render
  host (LLP 1048). Its web dev-loop description is brought up to date: the JS
  target is the default build, and the ~20 ms resident loop is `--wasm`. Its
  "`apps/caltrain/gpu` is … the line map" is corrected: the map left the GPU
  module for Canvas 2D (`apps/caltrain/gpu/src/lib.rs:1-3`).

### D9. The docs under `docs/` are part of the product

`rules/DEFERRED.md`'s "Sparse prose. The code and the checks are the authority"
and the 20-document cap were written against a governance corpus. Author docs
are something else. DEFERRED gains one bullet beside them:

> The guides under `docs/` describe the shipped product to its authors. A
> change that alters what an author writes or runs updates them in the same
> change. That is not apparatus, and it needs no approval.

The enforced document caps are on `llp/current/` and `llp/foundation/`. The
20-document sentence is about a governance corpus, and `docs/` is outside it.

RULES' "agents add no design doc" is unchanged. It is about adding documents,
and this is about keeping four existing ones true. New files under `docs/`
still need a person to say so. This RFC is that for the files it names.

### D10. Every example in the guides compiles, in `cargo test`

Add a test to the Contract CLI's existing integration suite
(`contract/cli/tests/it/docs.rs`). It does two things:

- Every fenced `contract` block in `docs/*.md` and `README.md` must compile with
  no diagnostics. The test compiles in-process and reports every failure in one
  run. A block that is deliberately partial is fenced as `text`. The README's
  todo app is fenced bare today, so it is labeled `contract`, `ts` and
  `contract-test`.
- Every `contract-test` block must parse.
- Paths come from `CARGO_MANIFEST_DIR/../../`, as `corpus.rs` already does.

The TypeScript examples are taken from the docs themselves, not from a fixture
copy. Each `ts` block that follows a `contract` block in the same section is
type-checked with `tsc` against that Contract's `contract types` output. The
check runs in the existing async-lane `tsc` test (`typescript.rs`), not in the
blocking gate.

This is not DEFERRED's refused "executable `contract` assertions". Those were
assertions inside language sections, which LLP 1006 refused in favor of `test`
blocks. Here, examples compile.

Cost: about 20 small compiles, well under a second. It isn't a new check. It
runs inside `cargo test`, one of the five. The one block that fails today, the
native `list` fragment at `contract-for-humans.md:754`, is refenced as `text`.

### D11. The trial is the measurement

The README's agent prompt and the authoring diaries found every gap above. No
new tooling comes from this, and it is not a gate: a 23-minute run cannot
block anything under RULES. When a change alters what an author meets first
(the scaffold, setup, the guides' structure), someone re-runs the README
prompt in a fresh session soon after. They record the elapsed time and the
machine's provisioning state (warm Cargo cache, Hermes present). The README
tip is updated to match, and anything rough goes in `docs/agent-pitfalls.md`
or `issues/`. A storage-backed task (D2's example) is a good second prompt.

## 3. Implementation order

1. Land `origin/tuft/diary-fixes` (D4's prerequisite).
2. D8, D9: text only.
3. D5, D1, D4: the scaffold and its update path, with `game/new-app.test.mjs`
   covering the generated files and the marker-preserving update.
4. D6: setup's report.
5. D3: `contract vocab`, its build script and tests.
6. D2, D10: the data-module section, its fixture, and the docs test.
7. D7: the README table and Caltrain's README.
8. Verify end to end: `exact new` into a temporary directory, paste D2's
   example, then build, test and drive it on the web from inside the app,
   using only the app's `AGENTS.md`.

`llp/current/` is at its cap of 15, so linking this RFC means archiving one
document. The candidate is the one whose work has most fully landed; see §5.

## 4. Costs

- **Two generated files in every new app.** They are short, and `--update`
  owns them.
- **A build script in `contract-lower`.** It scans one file and is about 40
  lines.
- **A docs test that fails when an author changes Contract without updating
  the guides.** That is the point of D9. The failure names the file and line.
- **D2 adds about 120 lines to the human guide. D2 and D7 delete more than
  that from `reference.md`.**

## 5. Questions settled for implementation

Charlie asked for this to be implemented after review without stopping on
details. These were settled by the author on the recommendation, and are open
to reversal:

1. **CLAUDE.md is a copy**, as the repo's own is. It works in every harness.
2. **LLP 1075.003 leaves `llp/current/`.** It is the native platform control
   plan, merged per its title. This RFC takes its place.
3. **D9's wording is as written.** It is the one change here that moves a
   binding document. Charlie should read it.

## 6. Revisions

**r2 (2026-10-04)**, after blind reviews by Astra (max) and Grok (xhigh). Both
returned SOUND WITH CHANGES. Their findings are in
`llp/reviews/rfc-2026-10-04-1086.{astra,grok}.md`.

- **D3:** scan only arm patterns, fail on a non-literal arm, take codecs from
  the schema, and print the contextual rules and the open sets. (Both
  reviewers.)
- **D4:** debug rather than release, an explicit toolchain, no inherited
  `CARGO_TARGET_DIR`, and the app as cwd. (Both.)
- **D6:** corrected the Hermes orders, separate `hermesc` and engine rows,
  Chrome through `chromium()`, and the check before Cargo. (Both.)
- **D1:** `contract types` comes first in the loop. **D2:** grant verbs, and
  the `--storage` verification. (Grok; Astra.)
- **D2:** continues the README's todo example. **D10:** README fences labeled,
  TypeScript taken from the docs, scope against DEFERRED stated. (Astra;
  Grok.)
- **D8:** Grok's #5 said the "line map is Canvas 2D" correction was a misread.
  It was checked and kept: `apps/caltrain/gpu/src/lib.rs:1-3` says the map
  left the GPU module for Canvas 2D (LLP 1056). Grok's `game/README.md` find
  is taken.
- **D11:** not a gate. (Astra.)
- **D5:** `$schema` is already admitted. (Astra.)
