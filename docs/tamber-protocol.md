# The Tamber protocol, `tamber/1`

The reference for the design server's wire (LLP 1121 D13), versioned with it. This page
describes **`tamber/1.1`**, the version `hello` reports. The design and the reasons are in LLP 1121
D13 and LLP 1122 D2, D9 and D10; what was built and how it differs from them is in each RFC's §10.
The recipe is `docs/recipes/tamber.md`. Each change to the wire changes this page in the same
commit.

**Development only.** A release build has no endpoint, no token and no server. The server runs
on macOS and Linux.

## Versions

- `hello` reports the protocol (`"tamber/1.1"`) and the IR's format number (`format`).
- **A minor version** adds requests or fields. A client ignores fields it doesn't know.
- **No request carries a client's version.** A client checks `hello`'s `protocol` itself and stops
  on a major version it doesn't speak.
- `tamber/1.0` was LLP 1121 stage 3. `tamber/1.1` added:
  - `history`, `revert`, `import` and `requires`;
  - the tool's files;
  - the long poll's `wait`;
  - the reply fields marked *1.1* below.

## Where it runs

- **In the dev loop.** Both dev loops (`host/web/dev.mjs`, `host/web-js/dev.mjs`) mount the server
  at their own origin when the app has a design. The JS loop's resident `--serve-as` producer
  mounts none.
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
| 409 | a conflict (`conflict: true`); a revert refused; a commit while a write is in flight (no `conflict` field) |
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
- `builds`: the registered development builds with an open stream, each `{id, host,
  capabilities, plan, design, status, pin}` (see [The build channel](#the-build-channel-llp-1122-d4));
- `files`;
- `app`: the app's directory, absolute (*1.1*);
- `epoch`: this run of the server, the same value its events carry (*1.1*);
- `live`: true when a dev loop serves the wire, whose running builds take a value-only edit in
  place, with no reload (LLP 1121 stage 4; see [The live table](#the-live-table-llp-1121-stage-4)).
  False from a server no dev loop runs.

A `client` given in a hello is not a write's attribution (see `transaction`).

**`describe`** `{file?}` answers:
- `ir`: the folded IR (LLP 1121 D2):
  - `format`, `digest`, `name`, `tuples`;
  - the axes and the dials (with each dial's `range`);
  - the tokens, each with its folded `values` per tuple, `entries`, the `dials` it depends on and
    its `fallback`;
  - `exports`.

  The IR has no `when` lines. A token's source lines come from `explain`.
- `files`;
- `meta`: `{dials: {name: [[key, value], …]}, tokens: {…}}`, the `key=value` metadata words as
  written (`control`, `step`, `label`, `help`, `options`, `group`, `priority`, `pair`, …). It is
  keyed by IR names, so a role is keyed by its row spelling.

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

**`explain`** `{token, file?}` answers `{token, value, whens, dials, names, files}`:
- `value` and each `whens[].value` are source text, as written;
- `whens` is `[{when: [{axis, value}], value}]`;
- `dials` lists the dials it depends on, and `names` the tokens its values name, as the checker
  resolved them;
- `files` gives the digests it read (*1.1*).

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
  1121 D13), judged by the edit's whole closure (stage 4, commit 2). It is `live` when the
  design's shape is unchanged, no role override or anything one reads changes, and the app's
  synthetic tokens (its Contract rows' expressions over tokens) fold on both sides. It is
  `rebuild` for structure, a role override (hosts take role tables at boot), a synthetic token
  gained, lost or left unfoldable, a profile colour, or an unknown closure (the app's build
  stops before its design). A `live` edit reaches running development builds as a batch, with
  no reload (see [The live table](#the-live-table-llp-1121-stage-4)); a `rebuild` one reloads;
- `synthetic: [{expr, values}]`: each synthetic token whose value the edit changes, its new
  value as CSS text per app tuple, for the live batch;
- `files`: the digests it planned against.

A digest it carries is checked as a transaction's is, and a stale one is 409 (with no `by`).
`requires` is `{path: digest}`, for files it was planned on but doesn't edit, checked alongside
(*1.1*).

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
- **The reply** is `{files, changed, class, diff, synthetic, client, written, before, inverse}`
  (`class` and `synthetic` as `propose` answers them):
  - `files` gives every design file's digest after the write;
  - `written` lists the files it wrote, and `before` their digests as it read them;
  - `inverse` is an id, or null when nothing changed on disk.
- **A conflict** is 409, `{conflict: true, message, files, stale: [path], by: {path: who}}`. `by`
  names who last changed each stale file: a client, or `"changed on disk"` for a hand edit or a
  checkout.
- **Refused by the checker:** `{message: "refused", diagnostics: [{file, line, col, id,
  message}]}`.
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

`{client, message?}` commits to git the design files this session wrote. It never pushes.
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
- **The reply** is `{commit, files: [path]}`, announced as a `committed` event.

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
  `<Collection>.<Mode>.json` names its modes. `sources` takes 1 to 256 files.
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
- **A source that doesn't parse** is refused with string `diagnostics`.
- **An import with nothing new** answers 200, `{proposal: null, review, message}`.

## The change stream

Every change is an event, whoever made it:
- a client's write;
- a hand edit or a `git checkout`, seen by the server's file watcher, with `client: null`;
- a commit;
- a proposal held or ended.

Every event carries `seq` (rising, per run) and `epoch` (this run). The server keeps the last 256.

| `event` | Fields |
|---|---|
| `changed` | `files` (every digest now), `edited` (paths whose digest moved, and paths gone, which `files` no longer lists), `changed: [{file, token}]` (null for a hand edit), `client`, `acceptedBy?`, `proposal?` |
| `committed` | `client`, `commit`, `files: [path]` |
| `proposal` | the held proposal: `proposal`, `client`, `diff`, `changed`, `class`, `expires`, `ops`, `replaces?`, `review?` |
| `proposal-dropped` | `proposal`, `client` (null when it `expired`, went `stale` or was one `too many held`), `reason`: `accepted`, `dropped`, `replaced` (with `replacedBy`), `stale`, `expired` or `too many held` |
| `down` | `message`: the design server stopped or never started |
| `builds` | `builds`: the registered builds, as `hello` lists them, after one registers, goes, or its pins change |
| `build-status` | `build`: one build, after its `status` report |
| `unsupported` | `build`, `what` (a command's kind, `pin.mode`, `pin.contrast`, `batch`), `detail` |
| `selected` | `build`, `selected` (LLP 1122 D5; its sender is T2b's) |

- **A write is announced once,** from its reply. A hand edit made just before a write is announced
  first, as its own event.
- **`POST /__tamber/subscribe`** opens with `{event: "subscribed", seq, epoch, files, proposals,
  builds}`, then sends one `data:` line per later event. It replays nothing kept. Catching up is
  the long poll's job. Its body may be `{client}`: the client's commands (`pin`) last as long as
  a subscription naming it.
- **`POST /__tamber/events`** `{after, epoch?, wait?}` answers `{events, seq, epoch, missed, files,
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

## The live table (LLP 1121 stage 4)

A write reaches every running development build at once. Release builds take none of this.

- **A value-only edit publishes no plan.** When only `.tamber` files changed and the new plan's
  structure is the published one's, the dev loop sends the changed tokens as a batch instead, and
  nothing reloads. This is decided by content, so a hand edit and a write through the wire are the
  same case. The plan file is still written, for a host that watches the file instead of the
  stream.
  - **The structure** is the plan without its design values: every `theme` row's `value` and
    every slice's, except a role override's or a profile colour's, which hosts take at boot.
  - **Anything else** still publishes a plan and reloads: a Contract edit, a token added, gone or
    starting to vary, a role override's value, a synthetic token gained or lost.
- **The batch** is an event on the dev stream the builds already hold (`/__dev`; the JS target's
  `/__dev/page`): `{kind: "tamber-batch", structure, base, design, tokens: [{name, value,
  slices}]}`.
  - `value` is the default tuple's wire text (a colour as CSS text), and `slices` one text per full
    app tuple where the token varies. A synthetic token's name is its expression, and its new value
    rides along.
  - `base` is the design digest it applies to, and `design` the new one. The JS target's batches
    add each token's custom property and its CSS per tuple (`prop`, `css`).
- **Applied only on its base.** A build applies a batch whose `base` is the design it runs, in one
  commit with no layout for colours. A build on another design answers stale and fetches the
  **snapshot**: `GET /__dev/design` answers `{kind: "tamber-snapshot", structure, base: null,
  design, tokens}` with every token, or 204 when no value-only edit came after the published plan.
  A build applies a snapshot when its structure is the plan's it runs.
- **A notice** is a batch with no tokens whose `base` and `design` are the current design. A build
  on that design applies nothing; one on another fetches the snapshot. The stream sends one on
  connect, after value-only edits, and in place of a batch too large for a frame (32 KiB). A
  native build fetches the snapshot after each plan it applies, and Linux polls it with its plan.
- **A full wide colour table** refuses a colour with a journal line. The row keeps its value, and
  the build reports it `unsupported` (see the build channel).

## The build channel (LLP 1122 D4)

The dev loop's running development builds register with it, report up and take commands down.
The tool drives no input into an app; a command changes only what `prefer` would.

- **Registration.** A build's dev stream hello (`/__dev`; the JS target's `/__dev/page`) carries
  a `nonce` when the loop has a design. The build answers with one `POST /__dev/report`:
  `{nonce, kind: "register", id, host, capabilities, plan, design}`.
  - `id` is the build's own, stable across its reconnects (a tab's, a process's), so its pins
    are replayed when it registers again.
  - A report whose nonce is no open stream's is refused (403): the dev loop takes other POSTs
    with no token. A report before a registration is 409.
  - A build with no stream opens a polled one: `GET /__dev/hello` gives a nonce, and `GET
    /__dev/commands?nonce=` answers the frames since its last poll (a nonce unpolled for 30
    seconds is forgotten). Linux does this, and so does the JS loop's resident `--serve-as`
    producer, which relays the frames onto its devices' streams; their reports go to the main
    loop directly.
- **Reports up** (`POST /__dev/report`, each with the nonce), told to every client as events:
  - `status`: the effective `scheme`, `contrast`, `mode` (`{axis: value}`) and `rootFontSize`,
    the `plan` and `design` digests, and the pin `revision` it applied. Sent on connect and on
    any change.
  - `unsupported`: a command or batch it can't apply, by name (`what`).
  - `selected`: LLP 1122 D5's; its route is here, its sender comes with T2b.
- **Commands down**, each a frame whose `kind` the host dispatches before any generation:
  - `pin` `{revision, pin}`: `pin` is `{scheme?: "light"|"dark", contrast?:
    "more"|"less"|"no-preference", mode?: {axis: value}, rootFontSize?: px}`, the facts `prefer`
    pins, or null to hand them back. Each pin replaces the whole set: a fact it doesn't name is
    what the first pin replaced.
  - `inspect`, `outline` and `hold` are reserved (T2b, stage 8). A host answers them
    `unsupported`.
- **`pin`** `{client, build?, pin}` (write scope, as the wire's POSTs are) sends one to a build, or
  to every registered build without `build`. `preview` `{client, build, axis, value}` is the
  same as pinning that build's mode for that axis. Answered `{builds, revision}`.
  - The client must hold an open subscription naming it, or it is refused (400). When its last
    subscription closes, its pins are handed back on every build.
- **On each host:**
  - the web page and the JS target set the mode and the root font size as `prefer` does, and the
    scheme through the root's `color-scheme`. They answer a contrast pin `unsupported`, since a
    page can't override the browser's `prefers-contrast`;
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

- **LLP 1121 stage 4, with LLP 1122 D4:**
  - scrub sessions;
  - `uses`.
- **LLP 1121 stage 4's gate also names `resolve-on`.** LLP 1122 defers it to T2b, where a build's
  `selected` report carries the value it resolved.
- **Later stages:**
  - component params (stage 7) and state parts (stage 8);
  - `set-row` (LLP 1122 T5);
  - pairing and remote clients (stage 10).
