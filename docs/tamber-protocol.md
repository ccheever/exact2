# The Tamber protocol, `tamber/1`

The reference for the design server's wire (LLP 1121 D13), versioned with it. This page
describes **`tamber/1.1`**, the version `hello` reports. The design and the reasons are in LLP 1121
D13 and LLP 1122 D2, D9 and D10; what was built and how it differs from them is in each RFC's §10.
The recipe is `docs/recipes/tamber.md`.

**Development only.** A release build has no endpoint, no token and no server. The server runs
on macOS and Linux.

## Versions

- A **minor** version adds requests or fields. A client ignores fields it doesn't know.
- A **major** version the server doesn't speak is refused.
- `hello` reports the protocol (`"tamber/1.1"`) and the IR's format number (`format`).
- `tamber/1.0` was stage 3. `tamber/1.1` added `history`, `revert`, `import`, `requires`, the
  tool's files, the long poll's `wait`, and the reply fields marked *1.1* below.

## Where it runs

- **In the dev loop.** Both dev loops (`host/web/dev.mjs`, `host/web-js/dev.mjs`) mount the server
  at their own origin when the app has a design. The JS loop's resident `--serve-as` producer
  mounts none.
- **Alone.** `bun exact.mjs tamber serve <app> [--port <n>] [--lan]` runs it with no app running.
  It binds loopback unless `--lan`, and a client reaching it over the LAN still can't get the token.
- **One writer per project.** The server holds `.exact/tamber.lock`. A second server for the same
  app answers every request with 503 and the reason.
- **Layers.** `scripts/tamber-server.mjs` is the HTTP wire. It speaks JSON lines to
  `contract tamber serve --stdio <app>`, which owns the files: the digests, the lock, the journal
  and every write. The wire writes no design file itself.
- **The scripted client.** `bun exact.mjs tamber op <url> '<request JSON>'` sends one request.
  `tamber op <url> subscribe` prints the change stream.

## Transport

No WebSocket (LLP 1121 §10, stage 3 commit 4). Everything is HTTP on the dev loop's origin:

| Path | Method | What |
|---|---|---|
| `/__tamber/token` | GET | the write token, to a local request only |
| `/__tamber` | POST | one request, one reply |
| `/__tamber/subscribe` | POST | the change stream, as server-sent events on the response |
| `/__tamber/events` | POST | the change stream as a long poll, for a client whose `fetch` reads a whole body |
| `/__tamber/tool/…` | GET, HEAD | the design tool's built files, to a local request only (*1.1*) |

- **A body** is one JSON object, at most 1 MiB. A larger one gets 413.
- **A reply** is `{"ok": …}` with 200, or `{"error": {"message": …, …}}` with:
  - 400: malformed, or refused by the checker;
  - 404: an unknown proposal or commit;
  - 409: a conflict;
  - 413: too large;
  - 503: the design server is down;
  - 504: the design server didn't answer within 120 s.
- **A field of the wrong type** is an error, never read as absent.

## Authentication

- **The token** is random, minted in memory when the server starts, and never written to disk.
- **`GET /__tamber/token`** answers `{"token": "…"}` only to a request that the development gate
  calls local: loopback peer, loopback host, no forwarding header (`access.local`,
  `scripts/install-page.mjs`). Any other request gets the install endpoint's not-found.
- **Every POST** carries it as `x-exact-tamber-token`, never in a URL. Reads need it too, since a
  design may be unreleased. A POST without it gets 404.
- **A client follows no redirects** when fetching the token.
- **Remote clients** (pairing, scoped tokens) are LLP 1121 stage 10, not yet built.

## Requests

Each request is `POST /__tamber` with `{"method": "…", …}`.

### Files and digests

- **The design's files** are the app's design (`app.tamber`, or the file `app.json`'s `"tamber"`
  names) and any other `.tamber` file beside it.
- **`files`** in a reply is `{path: digest}`, where a digest is SHA-256 hex of the file's bytes.
- **A read** names a `file`, or the app's design when it names none.
- **A write** carries the digest of every file it edits, as the client last saw it. A different
  digest on disk is a conflict.

### Reads

- **`hello`** answers:
  - `protocol` and `format`;
  - `builds`: the connected development builds, each with an id. This is empty until LLP 1121
    stage 4's build channel;
  - `files`;
  - `app`: the app's directory (*1.1*);
  - `epoch`: this run of the server (*1.1*).

  A `client` given in a hello is not a write's attribution (below).
- **`describe`** `{file?}` answers:
  - `ir`: the IR (kinds, axes, dials, tokens and their `when`s, and the dials each depends on);
  - `files`;
  - `meta`: `{dials: {name: [[key, value], …]}, tokens: {…}}`, the editor metadata as written
    (`control`, `range`, `step`, `label`, `help`, `options`, `group`, `priority`, …). It is keyed
    by IR names, so a role is keyed by its row spelling.
- **`resolve`** `{token, file?}` gives the token's value **as the build stores it**, per app-mode
  tuple:
  - the reply is `{token, tuples: [{modes, value, light, dark, srgb?}], files}`;
  - `modes` is `{axis: value}`;
  - `srgb` is `{light, dark}`, each side clipped to 8-bit sRGB `#rrggbbaa`, which is what a
    contrast ratio is taken on (*1.1*);
  - a residual (a role, a platform colour) is `{modes, value, residual: true, fallback: {light,
    dark}}`. Its fallback is labelled as a fallback, never as the value;
  - `files` gives the digests it read, so a client reading many tokens can tell one revision from
    two (*1.1*).
- **`explain`** `{token, file?}` answers `{token, value, whens: [{when: [{axis, value}], value}],
  dials, names, files}`. `dials` are the dials it depends on, `names` the tokens its values name,
  as the checker resolved them. `files` is *1.1*.
- **A design that doesn't check** answers `{"message": "the design is refused", "diagnostics":
  [{file, line, col, id, message}]}`.

### Operations

`ops` is a list of objects, each `{"op": "…", "file"?, …}`, where `file` defaults to the app's
design.

| `op` | Fields | Edits |
|---|---|---|
| `set-dial` | `name`, `value` | a dial's value |
| `set-token` | `name`, `when?: [{axis, value}]`, `value`, `comment?` | a token's default, or one `when` line |
| `unset-token-when` | `name`, `when` | removes one `when` line |
| `add-token` | `name`, `kind?`, `value`, `meta?: ["key=value"]`, `whens?: [{when, value, comment?}]`, `comment?`, `before?` | structure |
| `rename-token` | `from`, `to` | structure |
| `remove-token` | `name` | structure |
| `add-axis` | `name`, `values`, `under?: "scheme=dark"` | structure |
| `add-axis-value` | `axis`, `value`, `before?` | structure |
| `remove-axis-value` | `axis`, `value` | structure |

- **Values are source text**, as the file writes them: `"light-dark(#fff, #000)"`, `"accent"`,
  `"oklch(from accent calc(l + 0.1) c h)"`.
- **Every written file is formatted** (`tamber fmt`), so a value edit is a one-line diff.
- **Not yet built:** the component-param operations (`set-param`, `unset-param`; stage 7), state
  parts (`set-state`; stage 8), `set-row` (LLP 1122 T5), and `preview` (stage 4, as the build
  channel's `pin`).

### `propose`

`{ops, files?, requires?}` plans without writing. It answers:
- `diff`;
- `changed: [{file, token}]`;
- `class`, `"live"` or `"rebuild"`: what the edit does to running development builds (LLP 1121
  D13). In `tamber/1.1` this is judged from the design's shape. LLP 1121 stage 4 judges it by the
  whole closure;
- `files`: the digests it planned against.

A digest it carries is checked as a transaction's is. `requires` is `{path: digest}`, for files it
was planned on but doesn't edit, checked alongside (*1.1*).

### `transaction`

`{client, ops, files, requires?}` writes.
- **`client`** is required, and names who writes: `design-tool:<person>`, `agent:<name>`,
  `import:<kind>`. Clients share one server, so a hello's name is no write's.
- **Under the lock:** the digests are checked, the checker runs, and the files are committed as a
  set (temporary files fsynced, an intent record in `.exact/tamber.journal`, the renames, the
  record removed). A server that starts and finds a record completes or withdraws it. A refused
  transaction changes nothing.
- **The reply:** `{files, changed, class, diff, client, written, before, inverse}`.
  - `files` is the digests after the write;
  - `written` lists the files it wrote;
  - `before` gives their digests as it read them;
  - `inverse` is an id, or null when nothing changed on disk.
- **A conflict** is 409, `{conflict: true, message, files, stale: [path], by: {path: who}}`. `by`
  names who last changed each stale file: a client, or `"changed on disk"` for a hand edit or a
  checkout.
- **Refused by the checker:** `{message: "refused", diagnostics: [{file, line, col, id,
  message}]}`.
- **An editor that ignores the lock** and saves between the server's last read and its rename is
  overwritten. The digests narrow that window but don't close it.

### `undo`

`{client, inverse}` applies an inverse this run issued, while each file it restores is as that
write left it. Otherwise it's a conflict, with `by`.
- The server keeps the last 256 inverses. An id from another run, or one dropped, is refused, and
  the message says so.
- A client never sends restore operations itself.
- An inverse restores absence where the write created something.

### `commit` (LLP 1122 D9)

`{client, message?}` commits to git the design files this session wrote. It never pushes.
- **Refused, naming each file and why** (`refused: [{file, why}]`), when any of these holds:
  - the file was dirty or staged when the session first wrote it;
  - HEAD has moved since;
  - a change the server didn't make was seen, even one a later write overwrote;
  - the file differs from the server's last write.

  It is also refused while a write is in flight, or when there is nothing to commit.
- **How it commits:** through a temporary index (HEAD plus exactly those files). It then updates
  the real index's entries for those paths only, so other staging is untouched. HEAD moves only
  from the HEAD the guard checked.
- **The message** is the one given, or a composed one, followed by each edit and its author.
- **The reply** is `{commit, files}`, announced as a `committed` event.

### Held proposals (LLP 1122 D10)

- **`propose`** with `hold: true` and a `client` keeps the plan for 15 minutes, 64 at most (the
  oldest goes), and announces it.
  - The reply carries the plan plus `{proposal, client, diff, changed, class, expires, ops:
    [{index, op}], review?}`.
  - `review` is free-form for the tool, but its `offers[].items` must be operation indices and
    its `tokens` a count. `review.revert` is the server's own, and refused from a client.
- **`accept`** `{proposal, client, items?}` applies the proposal against the digests it was planned
  with.
  - A file moved since, or `items` naming only some operations, recomputes it on the current
    files as a **new** proposal. The reply is `{recomputed: true, replaces, proposal, …}`, for a
    fresh accept. The old operations are never replayed onto new bases.
  - An accepted write is attributed to the proposer, with `acceptedBy` on its event.
- **`drop`** `{proposal, client}` forgets one.
- **The open list** rides `subscribed` and every long poll's answer, as `proposals`.

### `history` (*1.1*, LLP 1122 D9)

`{limit?, skip?}` reads the git log of the design's files, newest first.
- `limit` is 1–100 (default 20). `skip` is at most 10,000.
- The reply is `{commits: [{commit, parent, author, email, date, message, files: [{file, status,
  diff, edits, refused?}]}], more}`.
- Each edit is `{what, name, when: [{axis, value}], before, after}`, derived by comparing the two
  versions' IR. It says "`radius.md` 8 → 12px", not only the raw diff.
- A version that doesn't check, or is over 1 MiB, has `edits: null` and `refused`.

### `revert` (*1.1*)

`{commit, client}` holds, as a proposal, the inverse of a past commit's design-file changes.
- **Planned on today's files** as semantic operations, never a byte restore. Its
  `review.revert` is `{commit, subject, moved, already, only?}`.
- **Refused (409)** when the files have moved on its edits, with `refused: [{file, edit, now,
  why}]`. Also refused when everything is undone already, or when the commit has no inverse: it
  created or removed a design file, added or removed a dial, reordered or removed an axis, or
  changed a kind, range or metadata.
- **Accepted after the files move,** it is planned again from the commit. A partial accept keeps
  its selection when re-planned.

### `import` (*1.1*, LLP 1122 T1a)

`{kind, source, name?, prefix?, client}`, or `{kind, sources: [{name, text}], prefix?, client}` for
multi-file DTCG such as a Figma per-mode export.
- **`kind`** is `shadcn`, `tailwind` or `dtcg`. Astryx is refused here and keeps its CLI (`bun
  exact.mjs tamber import astryx …`), since resolving its package needs a Bun install.
- **The import is held as a proposal** whose `review` is `{importer, counted, unmapped, offers:
  [{role, token, items, note}], tokens}`. The first `tokens` operations are the mapped tokens.
  Each offer's `items` are its role overrides, off until accepted.
- **An import with nothing new** answers `{proposal: null, review, message}`.

## The change stream

Every change is an event, whoever made it:
- a client's write;
- a hand edit or a `git checkout`, seen by the server's file watcher, with `client: null`;
- a commit;
- a proposal held or ended.

Every event carries `seq` (rising, per run) and `epoch` (this run). The server keeps the last 256.

| `event` | Fields |
|---|---|
| `changed` | `files` (all digests now), `edited` (paths whose digest moved), `changed: [{file, token}]` (null for a hand edit), `client`, `acceptedBy?`, `proposal?` |
| `committed` | `client`, `commit`, `files` |
| `proposal` | the held proposal, as `propose {hold}` answered it |
| `proposal-dropped` | `proposal`, `client`, `reason`: `accepted`, `dropped`, `replaced` (with `replacedBy`), `stale`, `expired` or `too many held` |
| `down` | `message`: the design server stopped or never started |

- **A write is announced once,** from its reply. A hand edit made just before a write is announced
  first, as its own event.
- **A file that appears or disappears** is announced.
- **`POST /__tamber/subscribe`** opens with `{event: "subscribed", seq, epoch, files, proposals}`,
  then sends one `data:` line per event.
- **`POST /__tamber/events`** `{after, epoch?, wait?}` answers `{events, seq, epoch, missed, files,
  proposals}`:
  - with the events after `after`, once there is one, or after 25 s with none;
  - at once with `wait: false` (*1.1*), for a client's first ask.
  - **`missed`:** an `after` from another run (a different `epoch`), or older than the events
    kept, is answered `missed: true` with no events and the current `files`. The client starts
    again from those.
  - **The design server down:** the `down` event if it hasn't been seen yet, then 503, never an
    empty answer that would be asked again at once (*1.1*).

## The design tool's files (*1.1*)

- **`GET /__tamber/tool/…`** serves the built tool (`target/design-tool-web`) to local requests
  only, through no symlink and no dot file.
- **`bun exact.mjs tamber tool [<app>] [--via <dev url>]`** serves it at the fixed origin
  `http://127.0.0.1:7740`, which the tool's grant names. So one tool runs per machine, on one
  project. With `--via`, port 7740 forwards `/__tamber` to the dev loop that holds the design.
  See `apps/design-tool/README.md`.

## Not yet in the protocol

- **LLP 1121 stage 4:**
  - the live table (a `tamber-batch` event on the dev stream, and snapshots);
  - the build channel (registration, `status`, `unsupported` and `selected` reports, the `pin`
    command, `builds` in `hello`);
  - scrub sessions;
  - `uses`;
  - `propose`'s answer from the whole closure.
- **Later stages:**
  - `resolve-on` (LLP 1122 T2b);
  - component params (stage 7) and state parts (stage 8);
  - `set-row` (LLP 1122 T5);
  - pairing and remote clients (stage 10).
