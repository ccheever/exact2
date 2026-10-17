# The Tamber protocol, `tamber/1`

The reference for the design server's wire (LLP 1121 D13), versioned with it. This page
describes **`tamber/1.9`**, the version `hello` reports. The design and the reasons are in LLP 1121
D13 and LLP 1122 D2, D9 and D10; what was built and how it differs from them is in each RFC's §10.
The recipe is `docs/recipes/tamber.md`. Each change to the wire changes this page in the same
commit.

**Development only.** A release build has no endpoint, no token and no server. The server runs
on macOS and Linux.

## Versions

- `hello` reports the protocol (`"tamber/1.9"`) and the IR's format number (`format`).
- **A minor version** adds requests or fields. A client ignores fields it doesn't know.
- **No request carries a client's version.** A client checks `hello`'s `protocol` itself and stops
  on a major version it doesn't speak.
- `tamber/1.0` was LLP 1121 stage 3. `tamber/1.1` added:
  - `history`, `revert`, `import` and `requires`;
  - the tool's files;
  - the long poll's `wait`;
  - the reply fields marked *1.1* below.
- `tamber/1.2` (LLP 1121 stage 4, with LLP 1122 D4) added:
  - `class` judged by the edit's whole closure, and `synthetic` on `propose` and transactions;
  - `hello`'s `live` and `builds`;
  - `pin` and `preview`, and `{client}` on the subscription and the long poll;
  - the `builds`, `build-status`, `unsupported` and `selected` events;
  - the live table's batch and snapshot frames, and the dev loop's endpoints (`/__dev/design`,
    `/__dev/report`, `/__dev/hello`, `/__dev/commands`);
  - `uses`, scrub sessions (`scrub`, `scrub-release`, `scrub-cancel`, the `scrub` event) and
    `propose`'s `texts` (stage 4, commit 6);
  - the items marked *1.2* below.
- `tamber/1.3` (LLP 1122 T2) added:
  - `hello`'s `page`;
  - `frame-ancestors` on a dev loop's HTML;
  - a build's `pinRevision` and `unsupported`;
  - `inspect` (the request and the command) and the `selected` report's shape;
  - the `selection` event, `outline` (the request and the command), and the source map's
    `sources` (each source's SHA-256 as compiled);
  - the items marked *1.3* below.
- `tamber/1.4` (LLP 1121 stage 5, commit 6) added `export`, and `import`'s reading of a Tamber export
  against its baseline.
- `tamber/1.5` (LLP 1121 stage 6, commit B) added length and number tokens: each IR token's real
  `kind` (`color`, `length`, `number`), `resolve`'s length and number tuples, and `uses` over the rows
  of a length or number token's kind.
- `tamber/1.6` (LLP 1122 T2b) added the `inspect` event: inspect mode's owners as they move.
- `tamber/1.7` (LLP 1122 T2b) added a click's token on `selected`, which the `outline` answering it
  names.
- `tamber/1.8` (LLP 1121 stage 6, commit D) added shadow tokens: the IR's `box-shadow` and
  `text-shadow` kinds, and `resolve`'s shadow tuples.
- `tamber/1.9` (LLP 1121 stage 6, commit E) added motion tokens: the IR's `duration` and `easing`
  kinds, `resolve`'s motion tuples, and `uses`' source-literal sites for one. A motion token's name
  is dotted (`tamber-motion-name`). A transaction or `propose` that changes one is `rebuild`.
  The 1.9 window also takes LLP 1122 T2c's optional fields, marked *1.9*: a build's `planDigest`
  (on `register`, `status` and its record), and source maps by plan, each build joined and
  outlined from its own plan's map.
  Stage 6 C added under the same version (the window's one version, theme2's ruling): the IR's
  `type` and `family` kinds and `resolve`'s type and family tuples. A transaction that changes the
  longhands a type token writes, or a type token's or family token's family, is `rebuild`; one that
  changes only a type token's style, weight, size or line height is `live`.
  Stage 6 F added under the same version: the IR's `icons` and `resolve`'s icon tuples. A
  transaction or `propose` that changes an icon is `rebuild`; an operation does not write one.

## Where it runs

- **In the dev loop.** Both dev loops (`host/web/dev.mjs`, `host/web-js/dev.mjs`) mount the server
  at their own origin when the app has a design. The JS loop's resident `--serve-as` producer
  mounts none: it relays the main loop's batches and commands to its devices (see the build
  channel).
- **Framed by the design tool.** Every HTML response a dev loop sends (the page, its install
  pages, the auth callback, `/__dev/open`) carries
  `Content-Security-Policy: frame-ancestors 'self' http://127.0.0.1:7740` (*1.3*): the tool's
  origin and the page's own may frame it, no other. `'self'` lets a page the canvas shows frame
  its own pages. No `X-Frame-Options`. Development only; the tool never scripts the frame.
- **Alone.** `bun exact.mjs tamber serve <app> [--port <n>] [--lan]` runs it with no app running.
  It binds loopback unless `--lan`, and a client reaching it over the LAN still can't get the token.
- **One writer per project.** The design process holds `.exact/tamber.lock`. When a second server
  for the same app can't take the lock, its design process exits. Its `POST /__tamber` and
  `/__tamber/subscribe` then answer 503 with the lock error, and its long poll answers as in
  "The change stream".
- **Layers.** `scripts/tamber-server.mjs` is the HTTP wire. It speaks JSON lines to
  `contract tamber serve --stdio <app>`, which owns the files: the digests, the lock, the journal
  and every write. The wire writes no design file itself.
- **The scripted client.** `bun exact.mjs tamber op <url> '<request JSON>'` sends one request.
  `tamber op <url> subscribe` prints the change stream.

## Transport

No WebSocket (LLP 1121 §10, stage 3 commit 4). Everything is HTTP on the server's origin.

| Path | Method | What |
|---|---|---|
| `/__tamber/token` | GET | the write token, to a local request only |
| `/__tamber` | POST | one request, one reply |
| `/__tamber/subscribe` | POST | the change stream, as server-sent events on the response |
| `/__tamber/events` | POST | the change stream as a long poll, for a client whose `fetch` reads a whole body |
| `/__tamber/tool/…` | GET, HEAD | the design tool's built files, to a local request only (*1.1*) |

- **The development gate runs first.** It answers 421 with an empty body to an unknown host, or to
  a browser POST whose `Origin` isn't the server's own (`scripts/install-page.mjs`).
- **A body** is one JSON object, at most 1 MiB less 64 bytes (room for the id the wire adds). A
  larger one gets 413, or the connection is cut while it is being read.
- **A field of the wrong type** is an error. An optional field given as `null` reads as absent.

**Replies.** A reply is `{"ok": …}` with 200, or `{"error": {"message": …, …}}` with one of these
statuses:

| Status | When |
|---|---|
| 400 | malformed; refused by the checker; an unknown inverse; a `commit` refused |
| 404 | an unknown proposal or commit |
| 409 | a conflict (`conflict: true`); a revert refused; a commit while a write is in flight (no `conflict` field); a commit whose `scope` is stale (`stale: true`) |
| 413 | too large |
| 500 | reading git failed (`history`, `revert`, a write's baseline) |
| 503 | the design server is down |
| 504 | the design server didn't answer within 120 s |

- **A 504 doesn't cancel a write.** A `transaction`, `undo` or `accept` the server answers later is
  still applied and announced on the change stream, and a held proposal still ends. Before trying
  again, a client checks the stream or re-reads `hello`.
- **A request without the right token,** or with the wrong method on these paths, gets 404 with
  the bare body `{"message": "Not found."}`.

## Authentication

- **The token** is random, minted in memory when the server starts, and never written to disk.
  Each run of the server has its own.
- **`GET /__tamber/token`** answers `{"token": "…"}` only to a request the development gate calls
  local: loopback peer, loopback host, no forwarding header (`access.local`,
  `scripts/install-page.mjs`). Any other request gets the install endpoint's not-found.
- **Every POST** carries the token as `x-exact-tamber-token`, never in a URL. Reads need it too,
  since a design may be unreleased.
- **A client follows no redirects** when fetching the token.
- **Remote clients** (pairing, scoped tokens) are LLP 1121 stage 10, not yet built.

## Requests

Each request is `POST /__tamber` with `{"method": "…", …}`.

### Files and digests

- **The design's files** are the app's design (`app.tamber`, or the file `app.json`'s `"tamber"`
  names) and every other `*.tamber` file directly in the app's directory. Paths are relative to
  the app's directory.
- **`files`** in a read or write reply is `{path: digest}`, where a digest is SHA-256 hex of the
  file's bytes. It covers every design file. `commit` is the exception: there, `files` is a list
  of paths.
- **A read** names a `file`, or the app's design when it names none.
- **A write** carries the digest of every file it edits, as the client last saw it. A different
  digest on disk is a conflict.

### Reads

**`hello`** answers:
- `protocol` and `format`;
- `builds` (*1.2*): the registered development builds with an open stream, each `{id, host,
  capabilities, inspector, plan, planDigest, design, status, pin, pinRevision, unsupported}`
  (`pinRevision` and `unsupported` *1.3*; `inspector`, the client whose inspect mode it is in or
  null, *1.6*; see [The build channel](#the-build-channel-llp-1122-d4-12));
- `files`;
- `app`: the app's directory, absolute (*1.1*);
- `epoch`: this run of the server, the same value its events carry (*1.1*);
- `live` (*1.2*): true when a dev loop serves the wire, whose running builds take a value-only edit in
  place, with no reload (LLP 1121 stage 4; see [The live table](#the-live-table-llp-1121-stage-4)).
  False from a server no dev loop runs.
- `design`: the app's design file, the one an operation with no `file` edits (*1.1*);
- `page` (*1.3*): the dev loop's own origin (`http://127.0.0.1:<port>`), as it prints it, or null
  from a server no dev loop runs (`tamber serve`, `tamber tool` without `--via`). Through
  `tamber tool --via`, the forwarded `hello` is the loop's, so `page` is the loop's. The design
  tool opens it in a tab as given, and frames it as `http://localhost:<port>` (the dev loops answer
  on [::1] too): another site than the tool's, so a browser with site isolation runs it in its own
  process. That frame's cookies and storage are its own, apart from a tab at `page`.

A `client` given in a hello is not a write's attribution (see `transaction`).

**`describe`** `{file?}` answers:
- `ir`: the folded IR (LLP 1121 D2):
  - `format`, `digest`, `name`, `tuples`;
  - the axes and the dials (with each dial's `range`);
  - the tokens, each with its `kind` (`color`, `length` or `number`, *1.5*: written, or inferred from
    its value; `box-shadow` or `text-shadow`, *1.8*, always written; `duration` or `easing`, *1.9*, written or inferred; `type` or `family`, *1.9*, written or inferred), its folded `values` per tuple, `entries`, the `dials` it depends on and its
    `fallback` (a length's or number's is its default tuple's value, twice);
  - `exports`;
  - `icons` (*1.9*), when the design has any: each `{name, sf, material, svg}`, its sources as
    written, null where unwritten (D10).

  The IR has no `when` lines. A token's source lines come from `explain`.
- `files`;
- `meta`: `{dials: {name: [[key, value], …]}, tokens: {…}}`, the `key=value` metadata words as
  written (`control`, `step`, `label`, `help`, `options`, `group`, `priority`, `pair`, …). It is
  keyed by IR names, so a role is keyed by its row spelling.
- `pairs`: `{token: [{name, token, fallback}]}`, what each token says it is drawn on (`pair=`,
  LLP 1121 D3): `token` is whether `name` is a token of the design; a role no token overrides
  has its `fallback` `{light, dark}` (*1.1*).
- `warnings`: `[{file, line, col, id, message}]`, the design's `tamber-contrast` warnings, a pair
  below 4.5:1 at its lowest tuple and scheme. They never refuse the design (*1.1*).

**`resolve`** `{token, file?}` gives the token's value **as the build stores it**, per app-mode
tuple:
- **The reply** is `{token, tuples, files}`.
  - `files` gives the digests it read, so a client reading many tokens can tell one revision from
    two (*1.1*).
- **A constant tuple** is `{modes, value, light, dark, srgb}`:
  - `modes` is `{axis: value}`;
  - `value` is the folded CSS (`#rrggbbaa`, `light-dark(…)`, or a wide colour's serialization);
  - `light` and `dark` are each side's CSS, equal for a one-scheme colour;
  - `srgb` is `{light, dark}`, each side clipped to 8-bit sRGB `#rrggbbaa`, which is what a
    contrast ratio is taken on (*1.1*).
- **A residual** (a role, a platform colour) is `{modes, value, residual: true, fallback}`, with no
  `light`, `dark` or `srgb`.
  - `fallback` is `{light, dark}`, labelled as a fallback, never as the value.
  - `fallback` is null when the vocabulary has none.
- **A length** (*1.5*) is `{modes, value, kind: "length", unit, amount}`: `value` is the stored text;
  `unit` is `px`, `rem`, `em` or `named` (a text style, `-exact-hairline`), and `amount` its number
  (null for a named length). A `rem`, `em` or named length's px is the build's, which depends on its
  text size: `resolve-on` (LLP 1122 T2b) gives it.
- **A number** (*1.5*) is `{modes, value, kind: "number", amount}`.
- **A duration** (*1.9*) is `{modes, value, kind: "duration", ms}`; **an easing** is
  `{modes, value, kind: "easing"}`, its `value` the easing as written. A motion token has one tuple's
  value in every tuple (no `when`).
- **A type** (*1.9*) is `{modes, value, kind: "type", style, weight, size, leading, family}`: each
  longhand as the token writes it, null for one it leaves unsaid (`size` always written). **A
  family** is `{modes, value, kind: "family", family}`, the stack as CSS writes it.
- **An icon** (*1.9*) is `{modes, kind: "icon", sf, material, svg}`, the same in every tuple, a
  source unwritten null.
- **A shadow** (*1.8*) is `{modes, value, kind: "box-shadow" | "text-shadow", shadows}`: each shadow
  `{inset, lengths, color}`, its lengths in px (offset x, offset y, then blur and spread when
  written) and its colour's folded CSS (null for a text shadow's `currentcolor`).

**`explain`** `{token, file?}` answers `{token, value, whens, dials, names, files}`:
- `value` and each `whens[].value` are source text, as written;
- `whens` is `[{when: [{axis, value}], value}]`;
- `dials` lists the dials it depends on, and `names` the tokens its values name, as the checker
  resolved them;
- `files` gives the digests it read (*1.1*).

`resolve` gives a residual's stored form and its fallback, labelled; its concrete value on a
given build (`resolve-on`) comes with LLP 1122 T2b, from that build's `selected` report, as its
runner resolves it.

**`uses`** `{token, file?}` (*1.2*, stage 4 commit 6) says where the app reads a token,
statically: every row of the plan of the token's kind (one that takes a colour for a colour token,
one that takes none for a length or number token, *1.5*; and a row of the kind of a token whose
value names it, so a `box-shadow` or `text-shadow` row for a length or colour a shadow token folds
over, *1.8*) whose expression names it, directly,
through a token whose value names it, through a synthetic token (a row's expression over
tokens) that reads it, or through a slot or derive that may hold it (a slot whose initial
value, or a value an action stores in it, names it). A value compared (`mode == "brand"`) is no
read, nor an element id (`url(#brand)`). It answers `{token, plan, sites, aliases, files}`:
- `sites: [{site, row, file, line, col, end_col, component, via?}]`: the plan node (`site`), the
  row, and its source span from the plan's source map. `via` is absent for a direct read, `{token}`
  through a token naming it, `{synthetic, through}` through a synthetic token's expression
  (`through` null, or the `{token}` it reads it by), and `{slot}` or `{derive}` through one;
- `plan`: the digest of the plan the sites index, compiled from the sources as they are, before
  any bake. A build joins the spans through its own source map (LLP 1122 T2b's `outline`);
- `aliases`: the tokens whose values name it, directly or through another;
- `files`: the digests it read.

A duration or easing token (*1.9*) is written into its rows at build, so the plan has no node
for it: `plan` is null, and each site is a source literal, `{site: null, row, file, line, col,
end_col, argument?, via?}`. These are the literals of a motion row the build wrote it into
(`row` is that row's name, `-` as `_`) in every source the app loads, a package's too, and the
component arguments whose literals name it (`row` null, `argument: {component, name}`), which an
expansion writes into a row. `via: {token}` says when the literal names the token through
another token. A comment is no site.

A `file` that is not the design the app builds has no sites. The JS target's plan and a native
bake of the same sources have the same sites. How many instances of each site are mounted on a
build is not answered: that needs a report the
build channel does not carry (a LLP 1121 §9 candidate). A design token no row reads has no sites.
The app's build failing is an error (`the app does not build: …`).

**A design that doesn't check** answers `{"message": "the design is refused", "diagnostics":
[{file, line, col, id, message}]}`.

### Operations

`ops` is a list of objects, each `{"op": "…", "file"?, …}`, where `file` defaults to the app's
design.

| `op` | Fields | Edits |
|---|---|---|
| `set-dial` | `name`, `value` | a dial's value |
| `set-token` | `name`, `when?: [{axis, value}]`, `value`, `comment?` | a token's default, or one `when` line (`comment` only on a new line) |
| `unset-token-when` | `name`, `when` | removes one `when` line |
| `add-token` | `name`, `kind?`, `value`, `meta?: ["key=value"]`, `whens?: [{when, value, comment?}]`, `comment?`, `before?` | structure |
| `rename-token` | `from`, `to` | structure |
| `remove-token` | `name` | structure |
| `add-axis` | `name`, `values`, `under?: "scheme=dark"` | structure |
| `add-axis-value` | `axis`, `value`, `before?` | structure |
| `remove-axis-value` | `axis`, `value` | structure |

- **Values are source text**, as the file writes them: `"light-dark(#fff, #000)"`, `"accent"`,
  `"oklch(from accent calc(l + 0.1) c h)"`.
- **An edit splices only the lines it touches,** laid out as `tamber fmt` would, and leaves every
  other byte alone. So a value edit is a one-line diff, even in a file that wasn't formatted.
- **The limits are the checker's:**
  - a token's `kind` is `color` until LLP 1121 stage 6;
  - the axes may make at most 64 tuples;
  - naming a dial, token, `when` or axis value the design doesn't have is refused.
- **Not yet built:**
  - the component-param operations (`set-param`, `unset-param`; stage 7);
  - state parts (`set-state`; stage 8);
  - `set-row` (LLP 1122 T5);
  - `preview` as an operation (it is a request instead: see the build channel).

### `propose`

`{ops, files?, requires?}` plans without writing. It answers:
- `diff`;
- `changed: [{file, token}]`: every token whose folded value differs, the dependents of a changed
  dial included, plus tokens added or removed;
- `class`, `"live"` or `"rebuild"`: what the edit would do to running development builds (LLP
  1121 D13), judged by the edit's whole closure (stage 4, commit 2). It is `live` when the plan's
  retained shape is unchanged (the axes, the dials, and each token the plan keeps by D7's retention
  with its kind, its role and whether it varies by an app axis; see
  [The live table](#the-live-table-llp-1121-stage-4-12)) and the app's synthetic tokens (its Contract rows' expressions over
  tokens) fold on both sides; a role override's value, or a token one reads, is a value like any
  other (stage 5). It is `rebuild` for structure (a role override gained or lost among it), a
  synthetic token gained, lost or left unfoldable, a profile colour, or an unknown closure (the
  app's build stops before its design). A `live` edit reaches running development builds as a batch, with
  no reload (see [The live table](#the-live-table-llp-1121-stage-4-12)); a `rebuild` one reloads;
- `synthetic: [{expr, values}]` (*1.2*): each synthetic token whose value the edit changes, its new
  value as CSS text per app tuple, for the live batch;
- `files`: the digests it planned against;
- `texts` (*1.2*): with `texts: true` in the request, the new text of each file it would write, by
  path; a scrub's preview compiles them.

A digest it carries is checked as a transaction's is, and a stale one is 409 (with no `by`).
`requires` is `{path: digest}`, for files it was planned on but doesn't edit, checked alongside
(*1.1*).

### Scrub sessions (*1.2*, LLP 1121 D13, LLP 1122 D6)

A token or dial dragged in the tool previews on every running build as a live batch, is written
to no file while the gesture lasts, and is written once, on release. The wire runs it
(`scripts/tamber-scrub.mjs`); the dev loop that mounts the wire previews it.

- **`scrub`** `{client, ops, files, scrub?}` opens a scrub, or updates the client's open one
  (`scrub` its id). It needs `write` scope and a present client (a subscription or long poll that
  names it), and the dev loop's wire: a server no dev loop runs answers 501.
  - **One at a time:** another client's scrub open is 409.
  - **Each update is the whole gesture's edit,** planned (`propose` with `texts`) on the files as
    the scrub opened on them (`files`, the digests its client read, given when it opens). The dev
    loop compiles the texts in place of the design file and sends every build the batch from the
    design it runs, as a value-only edit sends one; the snapshot it serves (`/__dev/design`) is the
    scrub's while it lasts.
  - **Updates that come while one previews** wait; only the latest is previewed, and the others
    are answered `{scrub, superseded: true}`. An update back to the committed value previews the
    committed design, and the scrub stays open.
  - **The dev loop's builds and a preview.** Any write other than the scrub's own release, and
    any hand edit, ends the scrub first (below); that write is never previewed over. What the
    loop does with a value-only build that lands while a preview is still showing is only about
    ordering: the build of such a write when it lands before the end's restore reaches the loop,
    and a previous scrub's release build when it lands after the next scrub has opened. Both
    leave the builds on the preview and change only what the end restores (the restore, or the
    next scrub's end). A build that reloads the builds ends the preview there.
  - It answers `{scrub, class: "live", changed, synthetic, files}`.
  - **A rebuild-class edit has no scrub** (409): it applies on release, as a `transaction`.
  - **A token the JS target writes inline has no scrub either** (*1.5*): a number a `font-weight`
    names, or a length or number an SVG attribute row names, has no property the page sets in
    place, so its edit rebuilds and the JS dev loop refuses its scrub, the refusal naming why (the
    design entry says `inline`).
    So does every type and family token (stage 6 C, under *1.9*): the JS target writes each as
    the kernel resolves its longhands, a literal, so an edit to one rebuilds there and has no
    scrub, whatever `propose` classes it for the native and wasm builds (`live` for a value-only
    edit is no promise for the JS target); and one that varies by an app axis is refused at build
    when the app ships the JS target.
- **`scrub-release`** `{client, scrub}` writes once: one transaction checked against the digests
  the scrub opened with, its reply a transaction's. A file moved since is a conflict naming who
  moved it (`by`): nothing is written, and the builds get the committed design back. While a
  release is written no scrub opens (409). The builds keep the preview until the build of what
  it wrote, whose batch goes from it.
- **`scrub-cancel`** `{client, scrub}` ends it without a write.
- **It ends without a write** on a cancel, a competing write (any write, or a hand edit the
  watcher sees, cancels it first), its client no longer present, the design server's loss, and
  30 s with no update (counted from each update's arrival). On each, every build gets the **latest committed** design, not the
  gesture's first, and a build that connects later gets the committed snapshot.
- **The `scrub` event** `{scrub, client, reason}` says each opening (`opened`, at its first
  preview; one that ends before it previews is never announced) and each end
  (`released`, `refused`, `cancelled`, `competing write`, `disconnected`, `server lost`, `idle`).
  While one is open the files and the running builds differ: the tool says "previewing".
- **Scope:** the local tool's token writes. A paired client's scoped token (LLP 1122 D6) names
  its scope, and a `propose`-scope client's scrub is 403.

### `transaction`

`{client, ops, files, requires?}` writes.
- **`client`** is required, any non-empty string. By convention it is `design-tool:<person>`,
  `agent:<name>` or `import:<kind>`. Clients share one server, so a hello's name is no write's.
- **Answered when done.** A write (a transaction, `undo`, `accept`) is held until the server
  answers, however long it takes; it never answers 504 and then lands. A server that exits fails
  every request in flight. Reads still time out at 120 seconds (504).
- **Under the lock:** the digests are checked, the checker runs, and the files are committed as a
  set (temporary files fsynced, an intent record in `.exact/tamber.journal`, the renames, the
  record removed). A refused transaction changes nothing.
- **A server that starts and finds a record** completes it, or withdraws it if none of its files
  landed. If it can't judge the record (a partial landing it can't account for, a temporary
  file with the wrong digest, an unreadable record), it keeps it and refuses to start. The wire
  then reports the server down.
- **The reply** is `{files, changed, class, diff, synthetic, client, written, before, inverse, write}`
  (`class` and `synthetic` as `propose` answers them):
  - `files` gives every design file's digest after the write;
  - `write` numbers the write (rising, per run), the same number its `changed` event and the
    session's edits carry, so a client can tell its own write's event from another's that left
    the same bytes (*1.1*);
  - `written` lists the files it wrote, and `before` their digests as it read them;
  - `inverse` is an id, or null when nothing changed on disk.
- **A conflict** is 409, `{conflict: true, message, files, stale: [path], by: {path: who}}`. `by`
  names who last changed each stale file: a client, or `"changed on disk"` for a hand edit or a
  checkout.
- **Refused by the checker:** `{message: "refused", diagnostics: [{file, line, col, id,
  message}]}`. A write (or a propose) that would make a role override vary by an app axis is
  refused the same way, `tamber-role-axis`, since the build would refuse it (*1.1*).
- **An editor that ignores the lock** and saves between the server's last read and its rename is
  overwritten. The digests narrow that window but don't close it.

### `undo`

`{client, inverse}` applies an inverse this run issued, while each file it restores is still as
that write left it. Otherwise it is a conflict, with `by`.
- **An inverse restores exactly** the lines the write replaced, as they were, so the bytes come
  back, even in a file that wasn't formatted. Where the write created something, the inverse
  restores its absence.
- **The reply** is a transaction's, with a new `inverse` that redoes the write. Undo is itself
  undoable.
- **An inverse id is single-use.** The server keeps the last 256. An id already used, dropped, or
  from another run is 400, and the message says so.
- A client never sends restore operations itself.

### `commit` (LLP 1122 D9)

`{client, message?, dry?, scope?}` commits to git the design files this session wrote. It never
pushes.
- **`dry: true`** runs the same checks and writes nothing. It answers `{files, edits, scope}`:
  what a commit would take, so a client can compose the message from the commit's own scope
  (writes from before it connected among them). Files that are HEAD's again (an edit and its
  undo) answer "nothing to commit" (*1.1*).
- **`scope`**, a dry run's given back, refuses the commit (409, `stale: true`) if the session has
  written since, so a write that lands in between is never committed under a message that leaves
  it out (*1.1*).
- **`edits`** (on a dry run and a commit) is `[{file, client, tokens, write?}]`, each session edit
  with its author, the tokens it changed and its write's number (*1.1*).
- **Refused (400), naming each file and why** (`refused: [{file, why}]`), when any of these holds:
  - the file was dirty or staged when the session first wrote it;
  - HEAD has moved since;
  - a change the server didn't make was seen, even one a later write overwrote;
  - the file differs from the server's last write.

  Also refused (400, no `refused`): nothing to commit, no git repository, or a git failure
  (`commit: …`). While a write is in flight it is 409.
- **How it commits:** through a temporary index (HEAD plus exactly those files). It then updates
  the real index's entries for those paths only, so other staging is untouched. HEAD moves only
  from the HEAD the guard checked. If it can't move, those index entries are put back.
- **The message** is the one given, or a composed one, followed by each edit and its author.
- **The reply** is `{commit, files: [path], edits}`, announced as a `committed` event.

### Held proposals (LLP 1122 D10)

**`propose` with `hold: true`** and a `client` keeps the plan for 15 minutes, 64 at most (the
oldest goes), and announces it.
- **The reply** is the plan (as `propose`) plus `{proposal, client, expires, ops: [{index, op}],
  review?}`. `expires` is an ISO-8601 UTC time.
- **`review`** is free-form for the tool, except that its `offers[].items` must be operation
  indices and its `tokens` a count. `review.revert` is the server's own, and refused from a
  client.
- **Guards.** The files a held propose `requires`, and any file in `files` that no operation
  edits, are checked when it is planned and again at `accept`.

**`accept`** `{proposal, client, items?}` applies the proposal against the digests it was planned
with.
- **With no `items`,** or with every index, it applies **every** operation, an import's role
  offers included. To leave offers off, a client names the indices it keeps.
- **Applied,** the reply is a transaction's. The write is attributed to the proposer (`client`),
  and its `changed` event carries `acceptedBy` and `proposal`.
- **A file moved since, or `items` naming only some operations,** puts the kept operations up again
  on the current files as a **new** proposal. The reply is `{recomputed: true, replaces,
  proposal, …}`, and the person accepts that one; nothing is written. A held revert is instead
  planned again from its commit (see `revert`).

**`drop`** `{proposal, client}` forgets one, and answers `{dropped: id}`.

**The open list** rides `subscribed` and every long poll's answer, as `proposals`, each as the
`proposal` event shows it.

### `history` (*1.1*, LLP 1122 D9)

`{limit?, skip?}` reads the git log of the design's files, newest first and first-parent only.
- `limit` is 1–100 (default 20). `skip` is at most 10,000.
- **The reply** is `{commits, more}`. Each commit is `{commit, parent, author, email, date,
  message, files}`:
  - `parent` is null on a root commit;
  - `date` is the author date;
  - each file is `{file, status, diff, edits, refused?}`, and a rename shows as a deletion plus an
    addition.
- **Each edit** is `{what, name, when: [{axis, value}], before, after}`:
  - `what` is `token`, `dial`, `axis` or `other`;
  - `before` and `after` are source text, or null;
  - they come from comparing the two versions' parsed declarations, so a version the checker
    would refuse still gives edits.
- **`edits` is null, with `refused`,** when a side doesn't parse, declares a name or `when` twice,
  or the two versions together are over 1 MiB.

### `revert` (*1.1*)

`{commit, client}` holds, as a proposal, the inverse of a past commit's design-file changes.
- **Planned on today's files** as operations, never a byte restore. Its `review.revert` is
  `{commit, subject, moved, already, only?}`. `moved` lists the files whose bytes differ from
  the commit's result.
- **Refused (409)** with `refused: [{file, edit, now, why}]` when an edit can't be inverted:
  - the file has moved on it since;
  - the commit created or removed a design file;
  - it added or removed a dial;
  - it added an axis, reordered an axis's values, or changed an axis's `under`;
  - it changed a kind, a range, metadata, the design's name or its exports.

  A removed axis is restored with `add-axis`.
- **Other refusals:**
  - everything is undone already: 409 with `already`;
  - a commit that touched no design file: 400;
  - an unknown commit: 404.
- **Accepted after the files move,** it is planned again from the commit. A partial accept keeps
  its selection when planned again.

### `import` (*1.1*, LLP 1122 T1a)

`{kind, source, name?, prefix?, client}`, or `{kind, sources: [{name, text}], prefix?, client}`.
- **The source.** `source` is the file's text. `name` is its file name; a Figma export's
  `<Collection>.<Mode>.json` names its modes. `sources` takes 1 to 256 files, and only for
  `dtcg`: a stylesheet's importer reads one sheet, and several are refused (400, *1.1*).
- **`kind`** is `shadcn`, `tailwind` or `dtcg`. `prefix` is the token prefix, by default
  `shadcn`, `tw` or `dtcg`.
- **Astryx is refused here** and keeps its CLI (`bun exact.mjs tamber import astryx …`), since
  resolving its package needs a Bun install.
- **The import is held as a proposal** whose `review` is `{importer, counted, unmapped, offers,
  tokens}`:
  - `counted` is `{source, mapped, reported}`;
  - `unmapped` is `[{block, name, value, why, token}]`;
  - `offers` is `[{role, token, items, note}]`;
  - the first `tokens` operations are the importer's own (tokens and any `add-axis`). Each
    offer's `items` are its role overrides, which a person leaves out unless ticked (see
    `accept`).
- **A source that doesn't parse** is refused, `{message: "the source is refused", diagnostics:
  [{file, line, col, id, message}]}` as a checker's refusal is. With several files a
  diagnostic is `{id, message}`, its message naming the file (*1.1*).
- **An import with nothing new** answers 200, `{proposal: null, review, message}`.

### `export` (*1.4*, LLP 1121 D14)

`{kind: "dtcg"}`, read-only for the design: the app's design as DTCG Format 2025.10 with the Resolver
Module 2025.10, as `bun exact.mjs tamber export dtcg <app>` writes it, from the same serializer.
- **The reply** is `{kind, text, baseline}`: `text` is the file, `baseline` where it is kept, by
  path from the app (`.exact/tamber-dtcg/<digest>.json`, the design's digest). Each export keeps
  what it wrote there, the newest 16, in a directory whose own `.gitignore` (`*`) keeps them out of
  every commit. A link in a baseline's place is replaced, never written through.
- **The document.** One modifier, `mode`, whose contexts are the full tuples (every app-axis tuple,
  then `light` and `dark`, named `axis=value, …, light`), each a complete set of the colour tokens,
  checked to read back as the colour a host renders before it is written. The header is the root
  `$extensions["exact"]` of the one token document of a set `exact`, first in `resolutionOrder`
  (the resolver's own root takes no `$extensions`): `export` (`"tamber-dtcg/1"`), `design`,
  `digest`, `conversions`, `axes`, `contexts` (each context's `modes` and `scheme`) and `dials`.
  The root `description` names the conversions too.
- **Each token** is `$type: color` with its value as a 2025.10 colour object (`hex` the 8-bit sRGB
  fallback), and in `$extensions["exact"]`: `source`, the line the context takes when it is not a
  plain colour; `role` or `platform`, for a value at that role's or platform colour's fallback;
  `overrides`, for a role override, exported as a token named for the role's row (`AccentColor`);
  `lossy`, the CSS of a colour past Oklab's range (an HDR one), clipped.
- **A design that does not check** is refused as a read is.
- **Importing it back** (`import` with `kind: "dtcg"` and the export as its one source). The file is
  read as the export when it has the header, or the export's shape (a set `exact`, then the one
  modifier `mode` with its contexts named for their tuples) where a tool dropped `$extensions`.
  - **The baseline** is the kept one the header's digest names, or a fresh export of the design
    when that is the digest. With no header, it is the one candidate (a kept export, or the
    design's own) with the file's contexts; with none, or more than one (the design changed since),
    the import is refused: comparing with another export would propose reverting what nobody
    edited.
  - **Each token's values** are compared with the baseline's, as colours: a tool that drops `hex`
    or reprints the numbers changes nothing. In a context with several sources the last that
    declares a token wins, and a source may be a reference to a set of the same document, as the
    resolver says. An unchanged token proposes nothing, its source kept
    (an expression, a role); so does one the design has already.
  - **A change edits the line that wins for its tuple** (LLP 1121 D6, as `explain` names it): the
    default or the `when` it is. A line whose contexts on a side all changed, to one value, takes
    it, its other side kept as written (`light-dark(new, kept)`). A line some of whose contexts did
    not change cannot move without moving them: its changed tuples get the narrowest `when` that
    holds for them and for no tuple that should keep another value, never one that ties with an
    existing `when` as narrow. An axis `under scheme=dark` keeps the light side of the line without
    it, so a `when` naming one follows that line's new light; a light value edited only in such a
    tuple is reported. Each side the file did not change is the design's as it is.
  - **Checked as the design would take it.** Each token's operations are applied and folded: a
    tuple they reach that the file left alone is pinned to the design's value, until every context
    renders as wanted. A token no lines can give that, or that the design would refuse, alone or
    with the tokens before it in one transaction, is reported with the reason or the checker's id.
  - **Reported in `unmapped`, not added or removed:** a token the export did not have, one the file
    drops or the design no longer has, one missing from a context, a context the export did not
    have, and a value no colour is read from.

## The change stream

Every change is an event, whoever made it:
- a client's write;
- a hand edit or a `git checkout`, seen by the server's file watcher, with `client: null`;
- a commit;
- a proposal held or ended.

Every event carries `seq` (rising, per run) and `epoch` (this run). The server keeps the last 256.

| `event` | Fields |
|---|---|
| `changed` | `files` (every digest now), `edited` (paths whose digest moved, and paths gone, which `files` no longer lists), `changed: [{file, token}]` (null for a hand edit), `client`, `write?` (the write's number, *1.1*), `acceptedBy?`, `proposal?` |
| `committed` | `client`, `commit`, `files: [path]`, `edits` (the commit's reply's, *1.1*) |
| `proposal` | the held proposal: `proposal`, `client`, `diff`, `changed`, `class`, `expires`, `ops`, `replaces?`, `review?` |
| `proposal-dropped` | `proposal`, `client` (null when it `expired`, went `stale` or was one `too many held`), `reason`: `accepted`, `dropped`, `replaced` (with `replacedBy`), `stale`, `expired` or `too many held` |
| `down` | `message`: the design server stopped or never started |
| `builds` | `builds`: the registered builds, as `hello` lists them, after one registers, goes, or its pins change |
| `build-status` | `build`: one build, after its `status` report |
| `unsupported` | `build`, `what` (a command's kind, `pin.mode`, `pin.contrast`, `batch`), `detail` |
| `selected` | `build`, `selected` (LLP 1122 D5; *1.3*: the wasm web page in inspect mode, see the build channel) |
| `selection` | `build`, `node`, `site`, `instance`, `tuple`, `source`, `rows`, `readOnly`, `trimmed` (*1.3*, LLP 1122 D5: a current `selected` report joined to source; see the build channel) |
| `inspect` | `client`, `on`, `builds` (*1.6*, LLP 1122 D4: inspect mode's owners as they move; each build record names its owner too, `inspector`, a client or null. `on: false` lists the builds a client no longer inspects, with `to` the client that took them or null where inspect mode ended, whatever ended it: its own command, another client taking them, a page's own exit, the client's presence lapsing, the build forgotten; `on: true` lists the builds it took. Nothing is said when nothing moved.) |
| `scrub` | `scrub`, `client`, `reason` (*1.2*; see [Scrub sessions](#scrub-sessions-12-llp-1121-d13-llp-1122-d6)) |

- **A write is announced once,** from its reply. A hand edit made just before a write is announced
  first, as its own event.
- **`POST /__tamber/subscribe`** opens with `{event: "subscribed", seq, epoch, files, proposals,
  builds}`, then sends one `data:` line per later event. It replays nothing kept. Catching up is
  the long poll's job. Its body may be `{client}` (*1.2*), which makes that client present while
  it is open (see the build channel's ownership).
- **`POST /__tamber/events`** `{after, epoch?, wait?, client?}` answers `{events, seq, epoch, missed, files,
  proposals}`:
  - with the events after `after`, once there is one, or after 25 s with none;
  - at once with `wait: false` (*1.1*), for a client's first ask.
- **`missed`.** These are answered `missed: true`, with no events and the current `files`, and the
  client starts again from those files:
  - an `epoch` that isn't this run's;
  - an `after` beyond the current `seq`;
  - an `after` older than the events kept.

  The epoch is checked only when the client sends one, so a client sends it.
- **The design server down:** the long poll answers the events kept after `after`, its `down`
  event among them. Once those are seen it answers 503, never an empty answer that would be
  asked again at once (*1.1*).

## The live table (LLP 1121 stage 4, *1.2*)

A write reaches every running development build at once. Release builds take none of this.

- **A value-only edit publishes no plan.** When only `.tamber` files changed and the new plan's
  structure is the published one's, the dev loop sends the changed tokens as a batch instead, and
  nothing reloads. This is decided by content, so a hand edit and a write through the wire are the
  same case. The plan file is still written, for a host that watches the file instead of the
  stream.
  - **The structure** is the plan without its design values: every `theme` row's `value` and
    every slice's, a role override's included (stage 5), except a profile colour's, which hosts
    take at boot. The dev loop compares it; it does not go on the wire.
  - **Anything else** still publishes a plan and reloads: a Contract edit, a token the plan keeps
    added or gone, or one starting to vary, a role override gained or lost, a synthetic token
    gained or lost. Which tokens the plan keeps is LLP 1121 D7's retention (`tamber::Retention`):
    every role override, what the views may name, and in an app that can name a token outside its
    views (a data or native module) every token, bounded by `exports`. `propose` and a
    transaction's `class` ask the same retention, as the build last found it, so a plain app's
    token no view names is `live` to add, and the dev loops publish nothing for it.
- **The batch** is an event on the dev stream the builds already hold (`/__dev`; the JS target's
  `/__dev/page`): `{kind: "tamber-batch", structure, base, design, tokens: [{name, value,
  slices}]}`.
  - `structure` is the digest of the design's **shape** (`design::shape`): the axes, and each
    token's name, role and slice count (which tokens override which role, not with what). The JS
    target's build and a wasm or native build of the same sources share it, though their plans
    differ.
  - `value` is the default tuple's wire text (a colour as CSS text), and `slices` one text per full
    app tuple where the token varies. A synthetic token's name is its expression, and its new value
    rides along.
  - **A role override** (stage 5) rides in `tokens` too, as the entry named by its role
    (`AccentColor`, `-exact-label`, `CanvasText`): its `value` is the colour the role now takes,
    one for every tuple (a role does not vary by an app axis, so it has no `slices`). There is no
    second field: the role is the entry's name, and a host knows which entries are roles from the
    plan it runs. A build sets its session's overrides from those entries, and re-presents what
    it resolved from the role: Apple sends the presenter its role table again (`{"op": "roles"}`;
    `AccentColor` is the app's tint) and each node's rows; the web rewrites its role sheet
    (`{"op": "theme"}`) in the batch's one style update; Linux re-targets its paint and paints
    again; the JS target's development build sets the role's sheet property (below).
  - `base` is the design digest it applies to, and `design` the new one. The JS target's batches
    add each token's custom property and its CSS per tuple (`prop`, `css`). A role override's
    `prop` is its role's property in the theme's sheet (`--exact-accent`, `--exact-canvas-text`,
    `--exact-label`), which a development build's rows and the kernel's own role colours read
    (`var(--exact-label, #…)`); a release build writes the values.
- **Applied only on its base.** A build applies a batch whose `base` is the design it runs, in one
  commit with no layout for colours. A build on another design answers stale and fetches the
  **snapshot**: `GET /__dev/design` answers `{kind: "tamber-snapshot", structure, base: null,
  design, tokens}` with every token, the role overrides among them, or 204 when no value-only
  edit came after the published plan. A build re-presents roles only where an override moved.
  A build applies a batch or a snapshot only when its `structure` equals the shape of the plan it
  runs (`design::shape`); on another it answers `structure`, as a new plan is on its way.
- **A notice** is a batch with no tokens whose `base` and `design` are the current design. A build
  on that design applies nothing; one on another fetches the snapshot. The stream sends one on
  connect, after value-only edits, and in place of a batch too large for a frame (32 KiB). A
  native build fetches the snapshot after each plan it applies, and Linux polls it with its plan.
- **One source.** The loop that mounts the server makes every batch and snapshot. A build on a
  polled stream (Linux, and the JS loop's resident producer, which relays them to its devices)
  takes them from `/__dev/commands`; the resident producer makes none of its own.
- **A full wide colour table** refuses a colour with a journal line. The row keeps its value, and
  the build reports it `unsupported` (see the build channel).

## The build channel (LLP 1122 D4, *1.2*)

The dev loop's running development builds register with it, report up and take commands down.
The tool drives no input into an app; a command changes only what `prefer` would.

- **Registration.** A build's dev stream hello (`/__dev`; the JS target's `/__dev/page`) carries
  a `nonce` when the loop has a design. The build answers with one `POST /__dev/report`:
  `{nonce, kind: "register", id, host, capabilities, plan, planDigest?, design}`. `planDigest`
  (*1.9*) is the SHA-256 of the plan the build runs, the digest its source map names; the server
  joins and outlines that build from that plan's map.
  - `id` is the build's own, stable across its reconnects (a tab's, kept across its reloads but
    not shared with a duplicated tab; a process's), so its pin is replayed when it registers
    again. Every registration is told the pin the server holds for it, or none.
  - `plan` is the generation the build runs (the envelope's identity; the JS target, which runs
    no plan, sends none), and `design` its design digest. Before accepting a dev generation,
    Apple reports the digest of its running embedded or cached plan.
  - The nonce is 128 random bits. A stream is one build: another `id` registered on it replaces
    the build it was, and that build's pin.
  - **Caps.** At most 256 streams are open, stream and polled together, so at most 256 builds
    are connected; polled streams at most 64, the oldest giving way to a new one. Past the cap a
    stream's hello has no nonce, so its build can't register, and `GET /__dev/hello` is 503.
    Builds a registration names are kept, at most 256, the oldest disconnected forgotten first.
  - A report whose nonce is no open stream's is refused (403): the dev loop takes other POSTs
    with no token. A report before a registration is 409. A report's body is at most 64 KiB,
    read within 10 s. Its fields are typed and bounded (strings strings, `mode` an object of
    strings, numbers numbers); any other is 400, and one the loop fails to handle 500.
  - A build with no stream opens a polled one: `GET /__dev/hello` gives a nonce, and `GET
    /__dev/commands?nonce=<n>&ack=<k>` answers `{"seq": s, "frames": [...]}`. Linux does this,
    and so does the JS loop's resident `--serve-as` producer, which relays the frames onto its
    devices' streams; their reports go to the main loop directly.
    - **Acknowledged.** `k` is the `seq` of the last answer the client read whole: 0 at first,
      and a missing `ack` is 0. The loop keeps its last answer until a poll acknowledges it. A
      poll whose `ack` is below that answer's `seq` gets it again (the same `seq` and frames), so
      a response lost on the way loses no command; one that acknowledges it gets the next frames
      under `seq + 1`. The first answer's `seq` is 1.
    - An answer holds at most 32 KiB of frames (the rest wait for the next poll, and a batch
      larger alone goes as its notice).
    - **Bounded.** Only the latest `pin` waits in a stream's queue. Past 64 frames or 128 KiB
      queued, the live-table frames waiting collapse into one notice of the latest design, and
      the build fetches the snapshot.
    - A nonce no stream holds is 404 with the body `null`: the stream is gone (the loop
      restarted, or it went unpolled 5 seconds), and the build says hello again.
- **Reports up** (`POST /__dev/report`, each with the nonce), told to every client as events:
  - `status`: the effective `scheme`, `contrast`, `mode` (`{axis: value}`) and `rootFontSize`,
    the facts that paint, the `plan` and `design` digests (and `planDigest`, *1.9*, as it moves),
    and the pin `revision` it applied.
    Sent on connect and on any change, the app's own `setScheme`, `setMode` and
    `setRootFontSize` included (at most once a turn).
  - `unsupported`: a command or batch it can't apply, by name (`what`), with the pin frame's
    `revision` when it is one (*1.3*, optional).
  - `selected` (*1.3*, LLP 1122 D5; the wasm web page and the iOS and Mac hosts send it in inspect mode):
    `{selected, inspect?, click?, plan, design, revision}`, the number of the `inspect` command it
    was made under (*1.6*; one since replaced is refused), a click's token (*1.7*, 1 to 64
    characters: a selection a press made, re-sent when it is reported again), the digests and pin
    revision the build runs, and
    `selected` `{node, type, testId, site, instance, planDigest, tuple, rows, trimmed}`: the
    node a press in inspect mode hit (the browser's hit test, as a real tap's), its plan site
    and instance path, the plan digest its map is for, the modes it is in (`tuple`: the
    window's, overridden by the pins above it; the runner's `layout` answer now says them as
    `modes`), and its rows as that answer gives them (`{name, value, source, from?, token?}`).
    Trimmed to the report's room (64 KiB, in bytes), and said so (`trimmed`). `selected: null`
    with `inspect` (the `seq` of the inspect command it left) says the page left inspect mode
    itself (Escape, its close control): the client that owned it there no longer does, unless a
    newer command (another `seq`) gave it to another.
- **Commands down**, each a frame whose `kind` the host dispatches before any generation:
  - `pin` `{revision, pin}`: `pin` is `{scheme?: "light"|"dark", contrast?:
    "more"|"less"|"no-preference", mode?: {axis: value}, rootFontSize?: px}`, the facts `prefer`
    pins, or null to hand them back. Each pin replaces the whole set: a fact it doesn't name is
    what the first pin replaced.
  - `inspect` (*1.3*) `{on, seq}` or `{refresh: true}`: inspect mode on or off (`seq` numbers
    the command), or the selection
    reported again under the digests now (found again by its site and instance path after a
    new generation, or said gone). In inspect mode the page shows a banner with a close control
    (and leaves on Escape), takes every input itself (pointer, mouse, touch, wheel, keys,
    gestures; guest frames take no pointer input) so nothing reaches the app, outlines the node
    under the pointer and selects the one a press is released on. The iOS and Mac hosts (T2c,
    development builds only: Swift's `EXACT_INSPECT`, which a production bake has not) do the
    same. On iOS the window takes every touch and press but those on the banner's close, and a
    touch begun in inspect mode stays taken to its end; on entry editing ends (the keyboard goes),
    gestures and controls under way are cancelled, and a status-bar tap scrolls nothing. On the
    Mac an event monitor takes every mouse, wheel and key event in the app's windows but the
    banner's close; Escape leaves. A tap or click selects the nearest view, up from the window's
    own hit view, that stands for a plan node, a walk that stops at a system bar or the title bar
    (a plan node the bar hosts, its title, a bar or toolbar item, a tab, is selected; UIKit's and
    AppKit's own parts, Back, a window button, the bar, select nothing, and the banner says so);
    a finger down (the pointer over a view, on the Mac) highlights what it would select. tvOS,
    the Linux host and a development JS page (until the JS target names the capability) answer
    `inspect` unsupported. A build that registers again
    is told inspect mode as the server holds it. A build that can says
    `inspect` among its `capabilities` (the wasm web page, and the iOS and Mac hosts; the JS
    target later). Development builds only.
  - `outline` (*1.3*) `{sites, plan, click?}`: the sites of plan `plan` (its digest) to outline while
    inspect mode is on, every copy of a node; an empty list clears the outlines, and so does
    `{clear: true}`. Each build is sent its own plan's sites, from that plan's map (*1.9*: by the
    `planDigest` it says; the running generation's for a build that says none); a build whose plan
    has no map at the server is sent none. A host never reads a source map: only the server
    resolves sites. A frame for another plan than its own still leaves a build's outlines as they
    are. The outline
    answering a selection names its `click` (*1.7*) when the report carried one: the host that
    made that click does not scroll to the first copy for it (the copy clicked is under the
    pointer); every other outline scrolls its first copy into view.
  - `hold` is reserved (stage 8). A host answers it `unsupported`.
- **A selection joined to source** (*1.3*, LLP 1122 D5). The server takes a `selected` report
  only while the build is in someone's inspect mode (*1.6*: one sent as it ended, or naming an
  `inspect` command since replaced, is refused, 409, and not published; a page's exit, `selected: null`, is published only for the build's
  current inspect command), and only when it is the build's latest: its `plan`, `design` and `revision` those of its latest
  status. One from before (a batch or a pin since) is refused (409) and the build is asked again
  (`inspect {refresh: true, status: true}`: its status first, then the selection), once per
  status it has. A current one is joined to the source map of the plan it names
  (the dev loop's `app.plan.map.json`, verified by that digest) and published as the `selection`
  event: `source` `{file, line, col, end_col, component, chain}` (the component call sites above
  it), and each row with its map `origin` (`own`, `class:<Name>`, `tag`) and `editable`: `{rung:
  "token", token, modes}` only for a row the node itself sets (`authored`, `own`) whose whole
  value is one token, else `{rung: null, reason, link?}` (a row reached by inheritance, a dynamic
  expression, a boundary, or an expression over tokens links its token, never this node's edit;
  a style's or the tag's row comes with template editing, T5). Each file the join reads is
  checked against the digest the map says it was compiled from (`sources`): one that moved since
  makes the selection `readOnly`, with its reason, and every row read-only. The app's design file
  likewise (the one the server serves, which the map must name), unless the build runs the
  design the dev loop serves now and the file's bytes are the ones that design was made from (a
  value edit it took, not a later save it kept the last good design over). Then every build that can
  inspect is sent `outline` with its own plan's sites at the selection's source location (*1.9*: the
  span from the selection's plan map, resolved through each build's own; a build whose map is
  missing or stale is sent none, and loses an outline this client drew). A selection that names no
  plan joins with no map, and is read-only.
- **`outline`** `{client, at: [{file, line, col}]}` (*1.3*, write scope, a present client;
  journaled; the outlines are its client's, cleared when it stops being present): every build
  that can inspect outlines its views at those source locations (a token's `uses` sites, a
  changeset entry's), each by its own plan's map (*1.9*; the running generation's for a build that
  says no plan). Answered `{sites, builds}`: the running plan's sites, and the builds sent one. A
  build whose map is missing, or compiled a file that has changed since (a fresh span would land
  on another node), is sent none; refused (409, or 503 with no map) only when no build could be.
- **`inspect`** `{client, build?, on | refresh: true}` (*1.3*, write scope and a present client)
  sends
  `inspect` to a build, or to every build that can. Answered `{builds, unsupported}`: the builds
  it went to, and those that can't inspect. Inspect mode is its client's: when the client stops
  being present, it is turned off where it turned it on. Turned off with no `build`, it is off
  on that client's builds that are away too (not in `builds`): one that registers again is told
  off, not on.
- **`pin`** `{client, build?, pin}` (write scope, as the wire's POSTs are) sends one to a build, or
  to every registered build without `build`. `preview` `{client, build, axis, value}` is the
  same as pinning that build's mode for that axis (it replaces that build's pin, as any pin
  does). Answered `{builds, revision}`.
  - **Caught up** (*1.3*). A build in `builds` (hello's and the events') names `pinRevision`, the
    revision of the last `pin` frame it was sent: a pin, a hand-back, or its pin again when it
    registers (revision 0 when it has none), whoever sent it. Its status has caught up when its
    `revision` reaches that. A status older than the one kept is dropped (it can't rewind the
    row), and one naming a revision this registration was never sent (a report from before a
    restart) is kept with `revision` null: it acknowledges nothing. A pin to no build sends
    nothing and leaves the revision as it is.
  - Its `unsupported` `{what, detail}` is the last command it could not apply, the frame's whose
    `revision` the report names (the web, Apple and Linux hosts name a refused pin's), or, from a
    host that names none, the one whose status it reports next (a host answers a frame before its
    status for it). It is kept while the status stays at that revision and null once it moves to
    another (or the build registers again); one for a revision the status has passed is dropped.
  - **Ownership.** A pin is its client's while the client is present: it holds an open
    subscription naming it, or a long poll naming it is open or was answered in the last 35
    seconds (a client whose fetch reads a body whole holds the wire by the long poll). A pin from
    a client that is not present is refused (409). When a client stops being present, its pins
    are handed back on every build.
  - Each is journaled with its client on the dev loop's output (LLP 1122 D4).
- **On each host:**
  - the web page and the JS target set the mode and the root font size as `prefer` does (the
    root's size and `--exact-host-font-size`, which an app's percentage is of). A pinned scheme
    is the system's, beneath the app's: a `:root { color-scheme }` rule over the page's `light
    dark`, which the app's `setScheme` (the root's own style) overrides, and the system scheme
    the runtimes read for `prefers-color-scheme`. They answer a contrast pin `unsupported`,
    since a page can't override the browser's `prefers-contrast`;
  - a mode a pin names that the build refuses leaves the modes from before the first pin, and is
    reported `unsupported` (`pin.mode`);
  - Apple and Linux pin what the agent's `prefer` pins, on every session, and set a held pin
    again on a session or generation that boots under it.

## The design tool's files (*1.1*)

- **`GET /__tamber/tool/…`** serves the built tool (`target/design-tool-web`) to local requests
  only. It serves no dot file, and follows no symlink out of the build.
- **`bun exact.mjs tamber tool [<app>] [--via <dev url>]`** serves the tool at the fixed origin
  `http://127.0.0.1:7740`, which the tool's grant names. So one tool runs per machine, on one
  project. With `--via`, port 7740 forwards `/__tamber` to the dev loop that holds the design.
  See `apps/design-tool/README.md`.

## Not yet in the protocol

- **`uses`' mounted-instance count** needs a report outside the build channel's (a LLP 1121 §9
  candidate).
- **LLP 1121 stage 4's gate also names `resolve-on`.** LLP 1122 defers it to T2b, where a build's
  `selected` report carries the value it resolved.
- **Later stages:**
  - component params (stage 7) and state parts (stage 8);
  - `set-row` (LLP 1122 T5);
  - pairing and remote clients (stage 10).
