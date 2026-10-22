# The Tamber protocol, `tamber/1`

The reference for the design server's wire (LLP 1121 D13), versioned with it. This page
describes **`tamber/1.16`**, the version `hello` reports. The design and the reasons are in LLP 1121
D13 and LLP 1122 D2, D9 and D10; what was built and how it differs from them is in each RFC's §10.
The recipe is `docs/recipes/tamber.md`. Each change to the wire changes this page in the same
commit.

**Development only.** A release build has no endpoint, no token and no server. The server runs
on macOS and Linux.

## Versions

- `hello` reports the protocol (`"tamber/1.16"`) and the IR's format number (`format`).
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
- `tamber/1.10` (LLP 1121 stage 7, commit A) added component params: `describe`'s
  `ir.components` (and `contract tamber ir`'s), each component's params by full name. Stage 7's
  later wire changes add under the same version.
- `tamber/1.11` (LLP 1121 stage 8, commit D) added the app's own Contract sources to the design's
  files (`hello`, every reply's `files`, the journal, `commit`, `history` and `revert`) and the
  state part operations `set-state` and `unset-state`; real-time scrubs added a scrub frame's mark
  (`scrub` on a batch and the snapshot), the `mismatch` end of a scrub, and the build channel's
  long poll (`wait` on `/__dev/commands`); stage 8 E added held states: the `hold` method, the build
  channel's `hold` command and `held` report, the `held` and `hold-cleared` events, a build's
  `hold`/`held` in its listing, a `selection`'s `held` rows, and a lowered `:active` row's `state`
  and `at` in the map; LLP 1122 T3 added the selection's param rung. The window's later additive
  changes (stage 8) add under the same version.
- `tamber/1.12` (LLP 1121 stage 9, commit D1) added `set-row`'s address to the selection join:
  a row its author wrote carries `at`, the attribute's place, with the file's `sha256` as
  compiled, and `attr`, its name as written (a shorthand's for an expansion); real-time scrubs
  added the long poll's held revision (`held` on each answer, and `held` on the request to leave
  out an unchanged `proposals`); stage 9 E added the held state's names: a selection's `held` and
  `layout`'s held answer name the node the state is held on (`node`, `site`, `instance`: the press
  a hold named on a node inside it resolved to), each row its `token` (the runner's, as a style
  row's), and the join names an `:active` row as its author wrote it (`background-color`, with the
  press row it lowered to as `row`); LLP 1122 T4 added the selection's state rung (a style's
  `:active` part row is edited by `set-state`), and T5 the template and style rungs (a row its
  author wrote, edited by `set-row`) and the map's `layout` on a written row (D14). Stage 9's later wire changes add under the same
  version: D2's `set-row`, D3's `add-param` and `remove-param`; C1's `explain` of an icon and a
  param, and `describe`'s icon `draws`; C2's `uses` of an icon and a motion token's sites by
  instance.
- `tamber/1.13` (LLP 1121 stage 10, commit A1) checks every request's scope in one place (see
  Scopes, under Authentication): `hello` says the client's `scope` and whether its token is the
  local one (`local`), and a request above its client's scope is 403 naming both (`scope`,
  `needs`). Stage 10's later wire changes add under the same version: A2's pairing, clients and
  revoke; `pair-code`'s `remote`, the remote doors open (B).
- `tamber/1.14` (LLP 1121 stage 10b, after stage 10 closed) adds `import`'s `baseline`, a
  re-import's `dials` and `pins`, `export`'s `digest`, and an import read in a process of its own
  (413/503 past its cost). Stage 11's wire changes add under the same version, and so do LLP 1122
  T7's: the `navigate` method and command, and a `status`'s `location`; and stage 10c's relay
  (`pair-code`'s relay entry, a relayed session's own pairing bucket). Stage 11 A2's, for an app
  whose sources link packages with designs (LLP 1121 D11; nothing changes for one without):
  `hello` and `describe` list `packages`, `[{package, file, digest}]`, each package design
  read-only (its `file` is `<package>/<file>`, its `digest` its text's), an op naming one refused;
  a package token's `describe` rows say `package` and `file` (the file its value comes from: the
  package's, or the app's design when the app sets it), and `resolve` and `explain` add
  `package`, `file` and `identity` (`<package>/<file>#<token>`), with `explain`'s `package_value`
  (`{value, whens, file}`, the package's own line) when the app sets the token. `set-token` of a
  name the app's design doesn't define but a package or the base scale does writes the app's
  override line (no `when` until the app sets the token; the reply's `changed` names the app's
  file); `add-token` refuses a package's or the base scale's name, and a new name in a package's
  namespace, as `rename-token` refuses renaming onto one. A package design changed on disk is a
  `changed` event naming it in `edited`, with `packages`; a held proposal is planned again on
  accept, on the packages as they are then. LLP 1122's queue item (1), landed before stage 11 closed (theme2's
  2026-10-20 ruling): a selection's held `:hover` and `:focus-visible` rows carry their part's
  place as `source`, and the state rung or the template rung with `state`, as a held `:active`
  row does; and `uses` names a look's row at its part's line, not its node's (a correction). The
  source map behind them (development only, never on the wire) keys each state part by its row and
  state: a binding's `states`, `[{state, origin, at?}]` (no `at` for a conditional class's part,
  either style's).
- `tamber/1.15` (LLP 1121 stage 12, after stage 11 closed) adds `resolve`'s and `explain`'s per-column
  values for a length over unset base names (stage 12 D1); a shadow token's lengths as expressions over
  length tokens and base names, those over unset base names kept as written: in `resolve`'s shadows such a length is null among `lengths`, and
  `length_texts` (only then) gives every length's text, with the base names they read in `bases`
  (stage 12 B3); a param's levels (stage 12 B2-levels):
  `explain`'s and `resolve`'s `levels`, each of `resolve`'s tuples' winning `level` and `name` per
  side (`{light, dark}`: a correction within the window, which first gave one of each), and
  `describe`'s param rows' `reaches`; `describe`'s library components' `parts`, `variants` and
  `library` (stage 12 D3; a param's winning level per tuple is `resolve`'s, B2-levels); a build's `page`, `"gallery"` for the
  variant gallery's page in its registration and on the build in `builds` (LLP 1139 D5, the design tool's G3); a relay session's `spki` with its
  certificate got by ACME (stage 10c R2); and names this machine's files in no answer:
  every `file` field and file name in an answer or event is app-relative, or `<package>/<path inside it>`
  for a package's file (a package linked from outside the app included), never absolute, `hello`'s `app`
  is absolute only for the local client, and a client that is not local gets machine-written messages
  (an error's, a refusal's, `down`'s) with no directory of this machine in them (see `hello`'s `app`).
  Stage 12's other wire changes add under the same version, and so do LLP 1122's: its queue item (2) reads a selection row the runtime calls
  `dynamic` because a `:disabled` or `:checked` part folds it over its base as that base, where the
  source map says both (its `editable` the base's rung, its `at` and `attr` the base attribute's),
  with `states: [{state, source, editable}]`, each folded part's place and its own edit; the
  development-only source map lists each Contract source's `home`, from which a param's full name
  is built.
- `tamber/1.16` (after stage 12 closed) adds a shadow token's per-column values over unset base names:
  `columns` on `explain`'s value, each `when` line and each of `resolve`'s tuples (D1's form, but for the
  terminal's), and each of `resolve`'s shadows' `columns` with its `value` and `lengths`; and a shadow's
  `lengths` (*1.15*, stage 12 B3) write a zero as 0, never -0 (`calc(space.md * -0)`), a correction;
  `explain`'s `bases_by_tuple` beside `bases` (stage 13 D4): each tuple's base names, as it folds
  there (`bases` keeps its meaning, as wide as `names`); and the base rows emit the reserved source
  kind `{kind: "stretch", ref, why}` (stage 13 D5): the Mac's `space.xl` now reads `stretch` (`ref`
  `grouped-label-inset`, with its why), which read `metric` before; and a package design's icons
  (stage 13): a package icon's `describe` row carries `package` and `file`, its `resolve` answer
  `package`, `file` and `identity`, and its `explain` answer those and, when the app's design
  overrides it, `package_value`, as a package token's do (its `draws` reads the package's `assets/`).
  Also in 1.16 (stage 13 E, the design tool's second hands-on pass, theme2's 09:29 ruling): a
  selection's inherited row carries `fromAt: {file, line, component}` beside `from`, the source of the
  node it inherits from, joined from the plan site the page reports (`fromSite`, the server's alone; a
  host that gives none leaves `fromAt` out and a client falls back to `from`); and `inspect` takes
  `{select: {up: n}, build}` (n from 1 to 64, the scope `inspect` has, the build's inspecting
  client's alone): that build's page moves its selection `n` call sites up, to the outermost node above
  it of the nearest enclosing copy that owns one, past the outermost call to the app's root, and
  reports it as a click's selection. A selection that moved since the client's chain was read
  (another node, or a rebuild) is stale, 409 with `stale: true`, as staleness is everywhere, and a
  page whose selection moved while the frame was on its way reports its selection as it is now;
  `unsupported` with `what: "inspect"` is only a page that can't step at all (Apple's, for now),
  which a client takes as no `select` there, not as no inspect.

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
- **Alone.** `bun exact.mjs tamber serve <app> [--port <n>] [--lan] [--remote <host:port>] [--trust-tunnel <port>]` runs it with no app running.
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
| `/__tamber/tool/…` | GET, HEAD | the design tool's built files, to a local request (*1.1*) and on the remote doors (stage 10 B) |

- **The development gate runs first.** It answers 421 with an empty body to an unknown host, or to
  a browser POST whose `Origin` isn't the server's own (`scripts/install-page.mjs`).
- **A body** is one JSON object, at most 1 MiB less 4,672 bytes (room for the id the wire adds and
  the digests an `import` carries to the server). A larger one gets 413, or the connection is cut
  while it is being read.
- **A field of the wrong type** is an error. An optional field given as `null` reads as absent.

**Replies.** A reply is `{"ok": …}` with 200, or `{"error": {"message": …, …}}` with one of these
statuses:

| Status | When |
|---|---|
| 400 | malformed; refused by the checker; an unknown inverse; a `commit` refused |
| 401 | a paired token revoked or expired, also a request whose client ended while it sent its body (*1.13*) |
| 403 | above the client's scope (*1.13*): `{message, scope, needs}` in `error`, on a request or a change stream path alike |
| 404 | an unknown proposal or commit |
| 409 | a conflict (`conflict: true`); a revert refused; a hold from a client below `write` with 64 held and none its own (*1.13*); a commit while a write is in flight (no `conflict` field); a commit whose `scope` is stale (`stale: true`) |
| 413 | too large |
| 429 | too many pairing attempts from one peer (`retry-after`, *1.13*) |
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
- **Remote clients** pair (*1.13*, LLP 1121 stage 10 A2, below) for a token of their own; the
  remote listener and its TLS are stage 10 B (below).
- **No cookie** is ever read for authentication: a token travels only in `x-exact-tamber-token`
  (canvas page code, the app under development, can set cookies this port receives).
- **The remote listener** (stage 10 B, `scripts/tamber-remote.mjs`) is off unless asked:
  `--remote <host:port>` on a dev loop (`bun exact.mjs web`) or on `tamber serve` opens a second
  listener, bound only where it names, that speaks TLS only.
  - Its certificate is self-signed, made for the run in memory (ECDSA P-256), never written to
    disk. Its SHA-256 fingerprint is printed at start-up, for a remote client to pin.
  - Plain HTTP to it gets no protocol: the handshake fails and the connection closes, the
    pairing endpoint included.
  - It serves `/__tamber`, `/__tamber/subscribe`, `/__tamber/events`, `/__tamber/pair`, and the
    design tool's own static files under `/__tamber/tool/` (no token: they hold no secret). Every
    other path, `/__tamber/token` and the app's pages included, is not found.
  - A request there is never `access.local`: it is `{remote: true, tunnel: false, peer}`, the peer
    its socket's (no forwarding header is read). Its `Cookie` header is removed before the wire
    sees it, and a request whose `Origin` is not the listener's own is refused (403).
  - Its answers send no referrer (`referrer-policy: no-referrer`), but for the design tool's own
    files, which send their origin only (`strict-origin`).
- **A trusted tunnel** (ssh -L, a VPN's forward; stage 10 B2) arrives on loopback looking local, so
  it gets a port of its own: `--trust-tunnel <port>` opens a plain listener on `127.0.0.1:<port>`
  for the tunnel's far end, said loudly at start-up. Its requests are routed as the remote
  listener's, `{remote: true, tunnel: true, peer}`, answering to loopback names at any port:
  only a paired token works through it, never the local one. The canvas has no tunnel port in
  stage 10 (QUEUE). A tunnel with a certificate of its own (cloudflared, Tailscale Funnel) names
  its public host with `--allow-host <name>` (port 443) or `--allow-host <name>:<port>`: the tunnel
  door answers to that host and port too (a Host with no port is 443; `name:80` is not), and takes
  a page of `https://<name>` at that port as its own origin. A port left out of an `Origin` or a
  `Host` is its own scheme's default, never the other's. Any `forwarded` or `x-forwarded-*` header
  keeps a request off the main loopback port's sandboxed path, as the dev gate's local test has it.
- **A sandboxed page** (stage 10b: a Figma plugin's UI is an iframe whose origin is `null`) may call
  the wire only with `--allow-plugin-origin`, on the tunnel door, on a relay's door (its
  certificate a real one: stage 10c) and on the main loopback port (from this machine, naming a
  loopback host), never the TLS door, whose certificate no page can pin. There, `Origin: null` on
  `/__tamber`, `/__tamber/subscribe`, `/__tamber/events` and `/__tamber/pair` gets CORS
  (`Access-Control-Allow-Origin: null`; a preflight `OPTIONS` answered 204, `POST` with
  `content-type` and `x-exact-tamber-token`); every such request is a remote one, so only a
  paired token works, never the local one. No other path gives a sandboxed page CORS: the token
  route keeps the dev gate's rule, so such a page cannot read it.
- **A hosted relay** (*1.14*; stage 10c R1, LLP 1121 §5 10c): `--relay <host[:port]> --relay-account
  <label>` dials out to a relay that hands each client's TCP stream down unopened; the client's TLS
  ends in this server, so the relay reads only the ClientHello's name and never a token, a design or
  a canvas. The certificate it ends on (R2, *1.15*) is got here by ACME (Let's Encrypt, or
  `--relay-acme <directory>`): one wildcard for the account's names (`*.<label>.<relay>`), by
  DNS-01, its record put up by the relay (which serves only names under the connector's own
  account) and taken down after; its key made here, kept with the ACME account's key and the
  certificate under `~/.exact/relay/<label>/` (0600 in a 0700 directory), never sent anywhere; got
  again 30 days before it ends (checked daily; one process at a time, by a lock file, and a process
  sharing the account adopts one another renewed). Every URL the ACME server gives must be https on
  its directory's origin, with a deadline and a size bound; an authorization must be for a name
  asked for, checked before its record goes anywhere; the certificate must be for this key and
  name the wildcard itself and no name more; an order has at most one authorization a name and one
  deadline, and stops where it is when the connector ends (the command or connection putting a
  record up stopped, its records taken down still, nothing kept; closing waits for that). A
  certificate that has ended serves no client. With `--relay-name <tool> <canvas>` (names of the
  developer's own domain, one parent), the record is put up by `--relay-dns-hook <command>`, run
  here with `EXACT_ACME_ACTION` (`publish`|`clear`), `EXACT_ACME_RECORD` and `EXACT_ACME_VALUE`,
  its output never logged (and never given the relay's token): their DNS credentials never pass
  through the relay. The relay operator, whose port those names reach, could still get a
  certificate for them by TLS-ALPN-01 or HTTP-01; the CAA records printed at start close that:
  `issue` and `issuewild` on the parent (RFC 8659: `issuewild` governs only the wildcard, `issue`
  the hosts), each limited to this ACME account (`accounturi=`) by DNS-01, with the CA's CAA name
  from its directory, and the two names pointed at the relay by A/AAAA records, never a CNAME (a CA
  reads a CNAME's target's CAA, which the relay writes). They are checked at start and before each
  renewal, for the wildcard and for each of the two names (the nearest CAA set up the tree, as a CA
  reads it, every rule pinning this account and `validationmethods=dns-01`, each tag once and in
  lower case as Let's Encrypt reads them, at a CA known to honour those parameters (RFC 8657;
  `letsencrypt.org`, until every public CA must, 2027-03-15), or forbidding issuance; a CNAME on
  the way up never pinned; a kept certificate whose account URL isn't kept is not pinned either; all
  of one check's lookups within 5 s, past which it is not pinned):
  each answer is shown at once, and until they pin, the start-up line says
  plainly "not zero-trust", naming the name left open, and `pair-code`'s relay entry carries `caa: false` (*1.15*; `caa: true`
  once it is), and with `--require-caa` no order runs at all until it is, and a later check that
  no longer pins closes the relay door. The check reads the
  system resolver without DNSSEC validation, and CAA binds the CAs that honour it, which every
  public CA must (RFC 8659, the Baseline Requirements). The relay's own domain keeps its residual:
  its operator controls its CAA. `--relay-cert`/`--relay-key` (PEM files) give
  one of the developer's own instead. The account's token is `EXACT_RELAY_TOKEN` or `~/.exact/relay/<label>/token` (refused
  unless only its owner may read it), never logged, never in a URL. A relay whose own certificate
  fails WebPKI is never sent it, and no flag turns that check off. The session's names are the
  relay's (`<id>-tool.<label>.<relay>`, under `<label>.<relay>` whole, `<relay>` being the
  `--relay` host: by ACME a session under any other parent is refused), or the developer's own with `--relay-name <tool>
  <canvas>`. A relayed request is routed as the TLS listener's, at the tool's name (port 443):
  `{remote: true, tunnel: false, peer: "relay:<tool name>", reported}`, `reported` the client's
  address as the relay says it, shown in logs only. Names under a relay's domain aren't secret
  (certificate transparency publishes them), and until that domain is a public suffix another
  account's names there are same-site to yours: nothing here relies on site isolation. The
  canvas comes through the relay on its second name (R1b): a gateway of its own for the
  session's pair (`https://<canvas name>` framed by `https://<tool name>` only), its requests'
  `Host` bound to the name the client's TLS asked for; `canvas` through the relay's door mints
  from it, through the TLS door from that door's, and a grant of one pair never opens the
  other's; a revoke ends the client's canvas on both, its live streams cut. The mint limit (six
  grants a minute a client) is one across both gateways; the per-client caps on held grants (8),
  sessions (16) and open requests (32) are each gateway's. Because a relay's accounts may be
  same-site to each other (until its domain is a public suffix), no other origin may include a
  canvas's resources: each answer says `Cross-Origin-Resource-Policy: same-origin`, and a request
  whose Fetch metadata says another origin made it (`Sec-Fetch-Site` neither `same-origin` nor
  `none`) is refused unless it is a navigation; and every answer, an SVG or any other document
  the loop sends without the HTML's header included, says `frame-ancestors 'self' <tool origin>`.
  One session a run: a connection the relay
  greets with other names is not taken. Every remote request's `access.door` (`tls`, `tunnel`,
  `relay`, `local`) is set where the server accepted the stream, never read from a request. The
  relay's word for a client's name is checked against the client's own ClientHello before any
  request is read; a relay's refusal on any connection, at any time, stops the connector and cuts
  every stream it carries (a revoked account; the relay closes the streams it pipes too). At most
  64 relayed streams are carried at once; only a client whose TLS settled lets the next connection
  be dialled at once, anything else (a client not taken, one dropped unsettled, a connection that
  fails) costs a doubling wait; the idle connections are dialled again as carried ones close; each
  connection has 10 s to verify the relay and be greeted, and as long from a line's first byte;
  dials are budgeted (20 a second, a burst of 40) whatever the relay does. The relay's names are
  served only when the certificate covers them, and no word of the relay's that carries the token
  becomes a name, an address or a log line.
  The connector's own protocol (`EXACT-RELAY/1`, one random `run` id on every connection of a run)
  is outside `tamber/1`: scripts/tamber-relay.mjs; the relay itself, docs/tamber-relay-service.md.

### Pairing (*1.13*)

- **A code:** `pair-code {scope, name?}` (the local token only) answers `{code, scope, expires,
  remote?}`. `remote` lists the remote doors open (stage 10 B), for the tool to show beside the
  code: `{origin, fingerprint}` for the TLS listener, whose certificate's SHA-256 the other machine
  checks before it pairs (the lead's ruling 2), and `{origin, tunnel: true}` for a trusted tunnel's
  local end, and `{origin, fingerprint, spki, relay: true, caa?}` for a relay's session (*1.14*; `caa` with names of the developer's own, above; `spki`,
  the base64 SHA-256 of its certificate's public key, the same across renewals with the account's
  one key, a non-browser client's pin, *1.15*). It is absent when none is open. `bun exact.mjs tamber pair <url> --scope
  read|propose|write [--name <name>]` mints one from this machine and prints them. A code is 50 random bits, written `XXXXX-XXXXX` in Crockford base32 (case, hyphens, `I`,
  `L` and `O` forgiven), valid two minutes, used once. At most 8 are live: a ninth replaces the
  oldest.
- **`POST /__tamber/pair {code, name?}`** needs no token, and is the one request of the protocol a
  remote request makes without one (the design tool's static files aside). It answers `{token, client, scope, expires}` once. A wrong code is 403,
  saying nothing of which codes exist. A wrong attempt can't be told from a guess at any one code,
  so it counts against every live code, and a code with five is burned. A peer gets a burst of
  five attempts, refilled at five a minute (a relay's session, one peer for every client it hands
  down, ten a minute in a bucket of its own: *1.14*); past them, 429 with `retry-after`, whatever the code,
  before the request's body is read.
  A code that is not a string of at most 32 characters is 400. Each burn and the first limit hit
  of each burst are logged (with the peer, never a code) and told as a `pairing` event.
- **The token** is 32 random bytes, kept in memory only (a restart revokes every paired client),
  by its SHA-256. It lives 12 hours (`--pair-ttl <duration>` on `tamber serve` and the dev loops:
  `90m`, `12h`, `2d`, from a minute to 30 days). It carries the scope its code gave.
- **The client** is named by the server: `remote:`, the code's or the request's `name`, then `#n`
  (`remote:laptop#1`). On every request a paired token makes, `client` is that name, whatever the
  body says, so every event, held proposal, journal entry and conflict's `by` names it. `hello`
  gives it as `client`, with `local: false`. A local client may not name itself `remote:…` (400),
  so neither can act as the other.
- **A revoke or an expiry and requests in flight:** a request still sending its body when its
  client ends (or its deadline passes) is refused (401) once it has; a scrub release whose client
  ends while the builds take its preview writes nothing (401), and the builds get the committed
  design back; one already dispatched (a write sent to the design server) completes, as a local
  edit in flight would.
- **The local token** is the local tool's only on a request the development gate calls local: a
  LAN peer holding it is no one (404).
- **Off this machine** (stage 10 B's listener, or a tunnel the developer trusts) nothing is local:
  no token page, and the local token is no one. Without a paired token a request reaches only
  `POST /__tamber/pair`, and the design tool's own static files (no design is in them).
- **`clients`** (the local token only) answers `{clients}`: the local token (`{client: "local",
  local: true, scope: "write"}`), then each paired client's `{client, name, scope, peer, created,
  expires, lastSeen}`. No token is ever listed.
- **`revoke {client}`** (the local token only) ends a paired client at once, as its token's expiry
  does: its subscriptions end, its long polls are answered 401, its scrub ends (the builds get the
  committed design back), its pins, inspect mode, outlines and holds are handed back, and its next
  request is 401. Its held proposals stay, marked `revoked: true`, for a `write` client to accept or
  drop; they never apply themselves. A `revoked` event tells the others.

### Scopes (*1.13*)

Every method has one scope, in `scripts/tamber-pairing.mjs`'s table, which the wire checks
before any handler runs. A method the table doesn't name is refused (400). A request above its
client's scope is 403 with `{message, scope, needs}` in `error`, the change stream's paths too.

| Scope | Requests |
|---|---|
| `read` | `hello`, `describe`, `resolve`, `explain`, `uses`, `history`, `export`, `inspect` (read-only below `write`: `refresh` only), and the change stream (`/__tamber/subscribe`, `/__tamber/events`) |
| `propose` | `propose` (held or not), `import`, `revert`, and `drop` of the client's own held proposal |
| `write` | `transaction`, `undo`, `commit`, `accept`, `drop` of another client's proposal, `pin`, `preview`, `outline`, `hold`, `navigate`, `scrub`, `scrub-release`, `scrub-cancel` |
| the local token only | `pair-code`, `clients`, `revoke` (a paired `write` client may not) |

The scopes are ordered: `write` may do what `propose` may, and `propose` what `read` may. `pin`,
`preview`, `outline`, `hold` and `navigate` are `write` because they change what the developer's screens show.
`inspect` reads: below `write`, `refresh: true` answers `{selections}`, the selection each build
last reported (as its `selection` event gave it), asking no build again, since a build's answer
outlines on the developer's screens; turning inspect mode on or off takes the builds' input, and
is `write`. A client below `write` may not drop another's held proposal, so with 64 held its own
oldest goes to make room, and with none of its own its hold is refused (409). The local tool's
token is `write`.

## Requests

Each request is `POST /__tamber` with `{"method": "…", …}`.

### Files and digests

- **The design's files** are the app's design (`app.tamber`, or the file `app.json`'s `"tamber"`
  names) and every other `*.tamber` file directly in the app's directory, then the app's own
  Contract sources (*1.11*, LLP 1121 stage 8 D: each `.contract` file its `app.contract` reads
  from inside the app; a package's are read-only and not among them), whose state parts
  `set-state` writes. Paths are relative to the app's directory. A hand edit of either kind is
  announced, `commit` takes either, and the journal recovers either.
- **`files`** in a read or write reply is `{path: digest}`, where a digest is SHA-256 hex of the
  file's bytes. It covers every design file. `commit` is the exception: there, `files` is a list
  of paths.
- **A read** names a `file`, or the app's design when it names none.
- **A write** carries the digest of every file it edits, as the client last saw it. A different
  digest on disk is a conflict.

### Reads

**`hello`** answers:
- `protocol` and `format`;
- `scope` and `local` (*1.13*): the client's scope, and whether its token is the local one; a
  paired client's `client` too;
- `builds` (*1.2*): the registered development builds with an open stream, each `{id, host,
  capabilities, inspector, plan, planDigest, design, status, pin, pinRevision, hold, held,
  unsupported}` (`pinRevision` and `unsupported` *1.3*; `inspector`, the client whose inspect mode
  it is in or null, *1.6*; `hold`, the hold the server keeps for it, and `held`, its `held`
  report's `{state, reason}` on the last hold frame sent it (null until it reports on that frame),
  *1.11*; see [The build channel](#the-build-channel-llp-1122-d4-12));
- `files`;
- `app`: the app's directory, absolute, for the local client (`local: true`), which `tamber tool` tells
  its own server by (*1.1*); for any other client (paired, through a remote door or a tunnel) the
  directory's name alone. Every `file` an answer or event carries is named by one rule, for every
  client: from the app's directory where the file is in it (`node_modules/@acme/reading/index.contract`),
  else from its package (`<package.json name>/<path inside it>`, as D15 names a package's file),
  else `<outside>/<its name>`; never by its path on this machine. A client that is not local gets
  machine-written messages (an error's, a refusal's, the `down` event's) with the app's directory
  as `<app>` and the home directory as `~`, at whole paths only. What an author wrote (a token's
  value, a commit's message, a file's text) comes back as written to every client (*1.15*);
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
    written, null where unwritten (D10), and `draws` (*1.12*): `{target: source}` for each target the
    app ships, the source that target draws (`sf`, `svg`) or null for none, by D10's rule (the one
    `tamber-icon-missing` checks).
  - `components` (*1.10*, LLP 1121 D15), when the app's Contract or its design declares params, or
    it uses an exported library component (*1.15*): each
    component `{identity, name, params}`, its identity its declaring file and name
    (`post.contract#PostButton`); each param `{name, short, param, token, kind, default, values,
    range, step, control, options, label, help, group, priority}`. `name` is its full name
    (`post.contract#PostButton.radius`), `short` the `Component.param` form or null where the app has
    another component of that name (with params or none), `token` its name below the IR
    (`param.f<hash>.…`, which no author or tool writes), `default` as written and
    `options` each with a param named in full, and `values` per tuple, folded with the design and
    the app's value. `control` defaults to `token`, `priority` to 2. `set` is the app's value,
    `{value, line, whens}` of its `params` line, or null; `pinned` names the tokens the default
    reads that a literal `set` no longer follows (D14's mark). Moving the declaring file is a new
    identity, so a value keyed by the old one no longer applies.
    A param whose default is a reference adds `reaches: {token, base, app_set}` (*1.15*): the first
    token its default reaches, the base name every tuple reaches (or null), and whether the app
    overrides that token (`explain`'s `levels`, compact; none where `levels` is null).
    An exported library component (LLP 1139 D0) adds `library: true`, `parts: [{name, groups, slot}]`
    (`slot: true` on the part whose node holds `children`) and `variants: [{prop, options}]` (each
    enum or bool prop its own view reads in a class condition, a `?:` or a `match`), from the IR; one
    with no params is listed too, with `params: []` (*1.15*, stage 12 D3).
    A component that isn't Contract, declared in the design (`component MapCard` over
    `params`, stage 7 D), is listed the same way, its identity the design file and its name
    (`app.tamber#MapCard`).
  - `resolve`, `explain` and `uses` take a param's full or short name as `token` and answer by its
    full name (*1.10*); an icon param, like an icon, is read through `describe` and `resolve`,
    and since *1.12* through `explain` and `uses` too (below).

  The IR has no `when` lines. A token's source lines come from `explain`; an icon is read through
  `describe` and `resolve`, and since *1.12* `explain` and `uses`.
- `files`;
- `meta`: `{dials: {name: [[key, value], …]}, tokens: {…}}`, the `key=value` metadata words as
  written (`control`, `step`, `label`, `help`, `options`, `group`, `priority`, `pair`, …). It is
  keyed by IR names, so a role is keyed by its row spelling.
- `pairs`: `{token: [{name, token, fallback}]}`, what each token says it is drawn on (`pair=`,
  LLP 1121 D3): `token` is whether `name` is a token of the design; a role no token overrides
  has its `fallback` `{light, dark}` (*1.1*).
- `warnings`: `[{file, line, col, id, message}]`, the design's `tamber-contrast` warnings, a pair
  below 4.5:1 at its lowest tuple and scheme. They never refuse the design (*1.1*).
- `base` (*1.14*, LLP 1121 stage 11): the base scale, every base name in the kernel's table's
  order (`space.xs` to `space.xl`, `radius.sm` to `radius.full`), each a **base row**
  `{name, kind, columns, override}`:
  - `kind` is `length` (`space.*`) or `radius` (`radius.*`);
  - `columns` is all five scale columns, `ios`, `mac`, `neutral`, `material` and `terminal`, each
    `{value, source}`: `value` a CSS length (`16px`), `full` (radius.full, the pill) or the
    terminal's `<n>cell` (`0` for none); `source` where it comes from, `{kind: "metric", ref}` (a
    metrics row), `{kind: "measured", id}` or `{kind: "guideline", cite}` (an entry in
    docs/scale-measured.md), `{kind: "stretch", ref, why}` (a metrics row borrowed for a role it
    doesn't name, where theme2 ruled it, with why in a line: the Mac's `space.xl`; the table carries
    it since stage 13 D5), `{kind: "full"}` or `{kind: "terminal"}`. Every cell is the kernel's
    table's (`style::scale`), never a copy;
  - `override` is the line of the design file the request names (the app's, by default) that sets
    the name, `{file, line, col, text}`, or null. When it is set, the name is the app's token: its
    folded value is what `resolve` answers.
- `targets` (*1.14*): `[{target, column}]`, the targets the app ships (the set an icon's `draws` is
  keyed by), each with the scale column it reads (`ios`, `tvos`, `visionos`, `watchos`: `ios`;
  `macos`, `mac`: `mac`; `web`, `linux`, `windows`: `neutral`; `android`: `material`; `terminal`:
  `terminal`), from the kernel, so
  a client keeps no map of its own; `column` is null for a target the kernel names none for.

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
- **A base name** (*1.14*) the app leaves unset answers `{name, base, files}`, its base row (see
  `describe`) and no tuples: its value is each column's, not a fold. One the app sets answers its
  token's tuples as usual, with `base` beside them.

**`explain`** `{token, file?}` answers `{token, value, whens, dials, names, files}`:
- `value` and each `whens[].value` are source text, as written;
- `whens` is `[{when: [{axis, value}], value}]`;
- `dials` lists the dials it depends on, and `names` the tokens its values name, as the checker
  resolved them;
- `files` gives the digests it read (*1.1*).
- **An icon** (*1.12*) answers `{token, kind: "icon", value, whens: [], dials: [], names, icon,
  files}`: `value` its line's value as written (an `icon(…)`, or another icon's name), `names` the
  icon that value names (empty for an `icon(…)`), `icon` its `describe` entry (`draws` included:
  what it draws, the named icon's sources for an alias). An icon param answers the same by its full name, its
  `value` its default as written, `names` the icon or param that default names, and `param`
  (below).
- **A param** (*1.12*) adds `param: {component, identity, kind, file, line, col, default, set}`:
  where its line is, its default as written, and `set`, the design's line that sets the app's
  value (`{file, line, value}`), or null.
- **A param's levels** (*1.15*, stage 12 B2-levels), on `explain` and `resolve` of a param, from one
  walker. It starts at the app's own value of the param (its `params` line), else the param's default,
  and follows each reference: another param, then the app's token or icon (a role's override in
  either spelling, as the checker keys it: `secondary-label` or `-exact-secondary-label`, any case),
  then a package's token. It takes each line's value in the tuple and goes at most 8 hops. It stops
  at a line whose value is no reference (a literal such as `transparent` or `-1`, or an
  expression), at a base name or at a role, and that place wins. Each side of a tuple is walked on
  its own, as the fold picks that side's value: the dark side takes the most specific holding `when`
  of all, the light side the most specific holding `when` that names no axis `under scheme=dark`, and
  either the line's own value when none holds. An icon param answers the same.
  - Each of `resolve`'s tuples adds `level` and `name`, each `{light, dark}` as its value's sides
    (the same on both where the scheme decides nothing). A level is one of `app-param`, `app-token`,
    `package`, `base`, `role` or `default` (the param's own default, a literal or an expression: a tool's word for it is
    "the component's default"); a name is the name at
    that level (a param's full name, a token's, a base name; a role, or an override of one, by the row
    spelling the fold publishes: `AccentColor`, `-exact-secondary-label`).
  - `levels: {token, app, base, param}` when the param's default is a reference; `levels: null` for a
    literal or an expression:
    - `token` is the first token the default reaches through params' defaults (never the app's
      `params` lines; at most 8 hops), `{name, package?, file, line, value}`: a package's own line
      with stage 11 A2's `package` and `file` (`<package>/<file>`), else the app's line;
    - `app` is the app's override of that package token, `{file, line, value}`, or null;
    - `base` is the base name every tuple's walk reaches on both sides, or null when they differ or one reaches
      none (each tuple's own is on `resolve`'s tuples);
    - `param` is the app's own value of the param, `{file, line, value}`, or null (`param.set`
      stays).
- **A base name** (*1.14*) answers `{kind: "base", name, base, files}`, its base row. One the app
  sets also carries its line's fields as any token's (`token, value, whens, dials, names`), so a
  tool reads and edits its `when` lines.
- **A length over unset base names** (*1.15*, stage 12 D1): an alias of one, or an expression
  (`calc(space.md * 2)`, stage 12 B2), whose value is each host's, adds `columns` on `explain`'s
  value and on each `when` line, as the first tuple that chooses that line folds it (a line no
  tuple chooses, the default included, has none), and on each of `resolve`'s tuples:
  `{ios, mac, neutral, material, terminal: {value}}`, each the expression folded for that column
  by the kernel's own per-column evaluator, written as the base rows write a column's value
  (`"32px"`; the terminal's `"2cell"`, `"1.5cell"` or `"0"`, in column-width cells before its
  layout rounds a space to whole cells on the axis it spaces); a column it doesn't fold in is left
  out. An app-set base name folds into its own value, so one whose fold reaches no unset base name
  is one value and has none (`space.md = space.sm` still reaches `space.sm`, and has them).
  A shadow token (*1.16*) with a length over unset base names gets the same `columns` in the
  same places, minus the terminal's (the terminal draws no shadow: a stated exception to "left
  out only where it doesn't fold"), each `value` the whole shadow's text as that column's host
  reads it (the kernel's `fold_lengths`, `"0px 16px 16px #000000ff"`, a colour kept whole), and
  each of `resolve`'s shadows that has `length_texts` adds `columns: {ios, mac, neutral,
  material: {value, lengths}}`, that shadow's own text there and every one of its lengths as a
  number in px, in `lengths`' order, never null; a shadow over no unset base name (px only, or over
  a base the app sets) has no `columns` key, and no zero in them is negative.
- **A token whose values name base names** (*1.14*), an overridden base name's own line included,
  adds `bases`: each base name any of its lines reaches, `when` lines no tuple selects included (an
  unset one named bare, a set one as the token it names), and those any line of each token it
  names reaches, through any chain, every branch, not per tuple; never a word in a string. As
  strings, each once, in no promised order (parallel to `names`, which is as wide; a client looks
  each up in `describe`'s `base`). A base name read from a file that is not the design's, or
  a design that is refused, is refused as any token is.
  Beside it, `bases_by_tuple` (*1.16*, stage 13 D4): one `{modes, light, dark}` a tuple, in
  `resolve`'s tuple order with its `modes`, each side the base names the token's value in that
  tuple reaches as the checker folds it there (an unset one read bare, a set one as the token it
  names) and those of the tokens it names in the same tuple, a branch no tuple selects left out,
  never a function's name, a keyword or a word in a string (`label.gap = 4px` / `when
  density=roomy: card.pad`, with `card.pad`'s `when density=tight: space.sm`, reaches `space.sm`
  in no tuple, though `bases` lists it). A base name is a length's, whose light and dark sides are
  one (a `when` under `scheme=dark` is a colour's alone), so the two sides are alike.

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
- `sites: [{site, row, file, line, col, end_col, component, via?, state?}]`: the plan node
  (`site`), the row, and its source span from the plan's source map. A row a state part writes is
  read at its part's line, with `state`: an `:active` row lowered to the press paint (*1.11*), and
  a `:hover` or `:focus-visible` look's row (its `:hover` row's line since *1.14*, LLP 1122's
  queue item (1); its node's before, which was the wrong line). `via` is absent for a direct read, `{token}`
  through a token naming it, `{synthetic, through}` through a synthetic token's expression
  (`through` null, or the `{token}` it reads it by), and `{slot}` or `{derive}` through one;
- `plan`: the digest of the plan the sites index, compiled from the sources as they are, before
  any bake. A build joins the spans through its own source map (LLP 1122 T2b's `outline`);
- `aliases`: the tokens whose values name it, directly or through another;
- `files`: the digests it read.

A duration or easing token (*1.9*) is written into its rows at build, so its sites are found
through its source literals: the literals of a motion row the build wrote it into (`row` is that
row's name, `-` as `_`) in every source the app loads, a package's too, and the component
arguments whose literals name it (`row` null, `argument: {component, name}`), which an expansion
writes into a row. `via: {token}` says when the literal names the token through another token. A
comment is no site. Since *1.12* (stage 9 C2) the answer's `plan` is the digest, and each plan node
declared at the element whose attribute holds a literal (in any part of its value: a condition's
arm, a `+`'s side), or whose row the source map says is written at a `style`'s attribute holding
it, is its own site: `site` that node, located as the map has it (`file`, `line`, `col`, `end_col`,
`component`, `chain`), and `literal` `{file, line, col, end_col}` the literal's place, so a
component used twice gives two sites for its one literal. A literal no node is declared or written
at (a component no one uses, a component argument's) keeps the older shape, `{site: null, row,
file, line, col, end_col, argument?, via?}` at the literal's place; before 1.12 every site had that
shape and `plan` was null.

An icon (*1.12*) is read where an `image`'s source names it (a string, a template with no `${}`,
either arm of a condition or a `match`, a list's items): each plan node of such an image,
located as the map has it, `row` `symbol` (the image's source, not a style row). Through an icon
param of the image's component (by its identity) whose value (the design's line, by its full name
or by its short name where only one component has it; else its default) resolves to the icon, to a
fixed point, the site says `via: {token: <param>}`, and `aliases` lists those params; a source
that is the thing asked about has no `via`. Asked about an icon param, `uses` answers by its full
name with the images that read it (or a param resolving to it). A node that shares its element's span
with another (a button's text child) is a motion site only where it binds the row's kind.

A `file` that is not the design the app builds has no sites. The JS target's plan and a native
bake of the same sources have the same sites. How many instances of each site are mounted on a
build is not answered: that needs a report the
build channel does not carry (a LLP 1121 §9 candidate). A design token no row reads has no sites.
The app's build failing is an error (`the app does not build: …`).

**A design that doesn't check** answers `{"message": "the design is refused", "diagnostics":
[{file, line, col, id, message}]}`.

### Operations

`ops` is a list of objects, each `{"op": "…", "file"?, …}`, where `file` defaults to the app's
design (a `set-state` or `unset-state`: the Contract source that declares its `style`).

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
| `set-param` (*1.10*) | `name`, `when?: [{axis, value}]`, `value` | the app's value of a component's param (LLP 1121 D15): its line in the design's `params` block, or one `when` line of it |
| `unset-param` (*1.10*) | `name`, `when?` | removes the param's line (the component's default applies again), or one `when` line |
| `set-state` | `style`, `state` (`active`, `hover`, `focus-visible`, `disabled`, `checked`, a `:` allowed), `row`, `value` (a string, written as a Contract string, or a number), `file?` | a row of a `style`'s state part in a `.contract` file (*1.11*, stage 8 D): the block and the row made if not there; `rebuild`, no scrub |
| `set-row` | at a place: `file`, `at` `{line, col}` (1-based; `col` a UTF-8 byte column, as the map gives it), `sha256`, `row`, `state?`; or on a style: `style`, `row`, `state?`, `file?`; and `value` (a string, written as a Contract string: a token's or param's name too, `"param.fill"`; or a number) | one row of the app's own Contract, its value replaced where its author wrote it (*1.12*, stage 9 D2; LLP 1122 D6): at a place, the attribute the selection's row names (its `at`, `attr` and the file's `sha256` from the join), refused when the file moved, the attribute there is another, it is a layout row (D14), a prop, or written as an expression, or the row is a longhand of a shorthand written there (the shorthand and its text named: `set-row` edits the shorthand whole); on a style, a row it writes (a state part's is `set-state`'s); `rebuild`, no scrub |
| `add-param` | `component`, `name`, `kind`, `default` (a token value, written as a Contract string), `attrs?` (`range`, `step`, `control`, `options`, `label`, `help`, `group`, `priority`: strings or numbers), `file?` | a line of the component's `params` (*1.12*, stage 9 D3; LLP 1121 D15), the section made before its `view` where it has none; D15 refuses a param no row reads, so it goes in one transaction with the `set-row` that reads it (promote to param); refused alone, with a `hint`; `rebuild`, no scrub |
| `remove-param` | `component`, `name`, `file?` | the param's line, and the section with its last (*1.12*, stage 9 D3); refused while a row reads it, so it goes with the `set-row` that stops reading it (detach), with a `hint`; refused while the app sets a value for it in its design (unset it first, or in the same transaction: LLP 1122 D6); `rebuild`, no scrub |
| `unset-state` | `style`, `state`, `row`, `file?` | that row removed, and its block when it empties |

- **Values are source text**, as the file writes them: `"light-dark(#fff, #000)"`, `"accent"`,
  `"oklch(from accent calc(l + 0.1) c h)"`.
- **A `.contract` write** (`set-state`, `unset-state`, `set-row`, `add-param`, `remove-param`)
  finds its `style` (or, for a param, its component) in the app's own Contract sources when it
  names no `file` (refused where none or two declare it, naming them), goes
  through `contract fmt`, and is refused, writing nothing, unless the whole app then builds (the
  build's diagnostics answer it). Its inverse restores the file's bytes as they were, an
  unformatted file's included.
- **A design edit splices only the lines it touches,** laid out as `tamber fmt` would, and leaves
  every other byte alone. So a value edit is a one-line diff, even in a file that wasn't formatted.
  A `.contract` write is laid out whole by `contract fmt` (above): one line only in a file
  already formatted, and in one that isn't, its other lines laid out too.
- **A state part written under a node** is `set-row`'s (*1.12*, LLP 1122 T5): a `set-state`
  naming a `node` is refused, pointing there; `set-row` at the row's place, with `state`, edits it. A row written over more than one line is the author's to
  edit, and refused.
- **The limits are the checker's:**
  - a token's `kind` is one stage 6 built (`color`, `length`, `number`, `box-shadow`,
    `text-shadow`, `duration`, `easing`, `type`, `family`);
  - the axes may make at most 64 tuples;
  - naming a dial, token, `when` or axis value the design doesn't have is refused.
- **A param's value** (*1.10*): `name` is a param's full name (`post.contract#PostButton.radius`)
  or its short one (`PostButton.radius`); a name no param of the app answers, or a short name two
  components share (whichever has the param, or neither), is refused (`tamber-param-unknown`,
  `tamber-param-ambiguous`). The line is written by the name the design's line already has, or an
  earlier op of the transaction wrote, or the name given, so a transaction never gives one param
  two lines. An exported library component's param is written by its full name whatever name it
  was given, since the build refuses a short one (LLP 1139 D8): a new line, and a line spelled
  short that a set or a `when` line's unset keeps, which is respelled in place, its value, comment
  and other `when` lines kept; the `params` block is made at the end when there is none, indented as its lines are,
  and goes when its last line does. A `when` line needs the param's own line first. After any
  write to an app with params, every param folds with the design and is inside its range, each
  value written (a `when`'s too) at its own line (`tamber-param-range`, `tamber-range-unit`; a
  length over a base name the design leaves unset, bare or in an expression, in every column of
  the scale, the refusal naming the first column outside), so an
  unset that unmasks a default out of range is refused. The inverse is the preimage, so undoing
  the first `set-param` restores absence. A param's value is `live` where the plan folds it, as a
  token's is, `rebuild` for a duration or easing param (a motion param is written into its rows at
  build, wherever its component is used), and a live one can be scrubbed. A token's rename renames
  the `params` values naming it.
- **`set-row`'s places** (*1.12*): several in one file are spliced together into the file as the
  tool saw it, from the last up, before the transaction's other writes to it, then laid out once.
  Every refusal says why and where to edit instead (the source, the node, a param's or token's
  rung).
- **Promote and detach** (*1.12*, LLP 1122 D6): promote to param is an `add-param` with the
  `set-row` naming `param.<name>`; promote to token an `add-token` (or `set-token`) with the
  `set-row` naming the token, the design and the `.contract` file in one transaction with one
  inverse; detaching a param's last reader sends its `remove-param` with the `set-row`. A param
  added or removed alone is refused by the compile with a `hint` saying so.
- **Not yet built:** `preview` as an operation (it is a request instead: see the build channel).

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
  app's build stops before its design, or the server's background whole build found a build under
  the same retention failing, until a whole build of the files as they are now passes; the breaking
  edit itself, answered before that build reports, is `live`). An edit that breaks an element's
  literal style row that passed (a token of the wrong kind, a value the row doesn't take, a name
  removed) is refused instead, as `propose`, `transaction` and a verifying scrub frame answer any
  write check: the compiler's own refusal, its id and words, at the row in its Contract source. A
  row already failing before the edit is the build's to report, and isn't refused (LLP 1121 D3,
  D7; QUEUE names the rows not judged so). A `live` edit reaches running development builds as a batch, with
  no reload (see [The live table](#the-live-table-llp-1121-stage-4-12)); a `rebuild` one reloads.
  `propose` through the JS target's dev loop is also `rebuild` when the edit moves a token its pages
  write inline, which the loop's design read marks (a `font-weight` number, an SVG attribute's
  length or number: LLP 1121 stage 6). Such a token has no property to change in place, and the
  class is the serving loop's answer (D13): where builds disagree, `rebuild` wins. This holds
  while the loop's design reader is built; without the reader the session's class stands, and
  the release rebuilds;
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
  - **Each update is the whole gesture's edit,** folded on the files as the scrub opened on them
    (`files`, the digests its client read, given when it opens). The design server takes what the
    fold reads once, when the scrub opens, and folds the whole design per update, reading no file;
    the dev loop sets the rows that changed on the design it committed and sends every build the
    batch from the design it runs, as a value-only edit sends one, compiling nothing. A frame it
    cannot set so (a row that would gain or lose its per-tuple values, a design it has not built
    yet) is planned (`propose` with `texts`) and compiled in place of the design file, as before.
    The snapshot it serves (`/__dev/design`) is the scrub's while it lasts.
  - **Updates that come while one previews** wait; only the latest is previewed next, and the
    others are answered `{scrub, superseded: true}`. Nothing else paces them: a build shows the
    latest frame it has at its next display frame. An update back to the
    committed value previews the committed design, and the scrub stays open.
  - **Each frame a scrub sends is marked** (*1.11*): its batch and the snapshot carry `scrub`
    `{id, rev, run}`, the scrub's id, a revision that rises over every frame of every scrub the
    design server sends, ends included, and the server's run (a name per start: a restarted
    server's revisions start again). A build drops a frame or snapshot of the run it last applied
    a frame of (with no `run`, of the scrub) whose `rev` is not above that one's, so a polled
    host's late fetch of an earlier scrub's frame never shows over a later scrub or its end. A
    build that has applied a frame of a later run drops any frame of an earlier one; a frame with
    a `run` after a mark with none is a new run's. A notice that stands for a scrub frame (one over
    a stream's bound, a polled queue collapsed) keeps its mark, and the build of what a release
    wrote carries a mark above the scrub's frames. A build applies a scrub's frames with no
    transition: an authored
    `transition` on a row the frame sets neither starts nor goes on (a development preview that
    follows the tool's gesture; any other edit moves as authored). The web pages keep every
    transition off from a scrub's first frame until 150 ms after its last (one style change at
    each end, the last values styled before transitions come back; a new generation ends it at
    once): a development preview, which a release build never runs.
  - **A frame set without compiling is checked** once the gesture rests (150 ms with no update):
    the frame shown, planned and compiled in full, must be the design set. Where it is not, or the
    full check refuses it, the scrub ends (`mismatch`).
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
    A type or family token is no longer one (stage 6 C's follow-up): the JS target declares each
    row such a token writes as a property per tuple (`--x-type-body-size`, `--x-family-mono`),
    one that varies by an app axis always and every one in a development build, so an edit that
    changes only a type token's style, weight, size or line height is live and scrubbable there
    too; one that changes the rows it writes or a family rebuilds, as `propose` classes it. The
    design entry of such a token lists each row's property and its CSS per tuple (`parts`) in
    place of `prop` and `css`.
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
  (`released`, `refused`, `cancelled`, `competing write`, `disconnected`, `server lost`, `idle`,
  and `mismatch`, *1.11*).
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
  So is a write, or a scrub's resting value (`scrub-fold`'s verify; its frames before that only
  preview), that would hide an exported library component's slot fill: a node of the slot's chain
  given `opacity` 0, a zero size, `display: none` and the rest of LLP 1139 D4's list, in any tuple
  or scale column, through a token, a base name a chain's token reaches, or a param
  (`lower-anatomy-slot`, the compiler's own checker and words, at the line of the chain's token
  it names, in the app's design or a package's; *1.15*, stage 12 D2). A write no chain's tokens
  reach isn't asked. A scrub whose app components or package designs moved since it opened is a
  conflict at its verify, opened again.
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
oldest goes; below `write`, only the client's own, else 409: see Scopes), and announces it.
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
`proposal` event shows it, except a long poll that names the list's current revision (below). A
long poll's answer also carries the list's revision, `held` (*1.12*),
which moves whenever a proposal is held, dropped, accepted, expires or is recomputed. A long poll
that sends `held` with the revision it has (and this run's `epoch`) gets no `proposals` while the
list is unchanged: it keeps its own.

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
  - `what` is `token`, `dial`, `axis`, `other`, `state` or `row` (a `.contract` file's, below);
  - `before` and `after` are source text, or null;
  - they come from comparing the two versions' parsed declarations, so a version the checker
    would refuse still gives edits.
- **`edits` is null, with `refused`,** when a side doesn't parse, declares a name or `when` twice,
  or the two versions together are over 1 MiB.
- **The app's own Contract sources** (*1.11*, stage 8 D) are among the design's files: their
  state parts are the design's. A `.contract` file's edits are its styles' state rows, each
  `{what: "state", name: <style>, state, row, when: [], before, after}`, the values as Contract
  writes them (`"#000000"` quoted, `0.5`); a block whose border colours change order is one
  edit with `row` `border-*-color order`. A style's own row (*1.12*, stage 9 D2's `set-row` on a
  style) is `{what: "row", name: <style>, row, when: [], before, after}`: a paint row alone on its
  line, its value on that line; a layout row, a part's, one sharing its line, and the order of a
  style's own rows are code. A node's own attribute (`set-row` at a place) has no key a node keeps
  across versions and stays code, as does a component's `params` line (*1.12*, its revert needs
  the row that reads it). The file says `code: true` where the commit changed its other
  lines too. A `.contract` change with no state row or style row is the app's code, not the
  design's: it is left out, and a commit with nothing else is not listed (so a page may hold
  fewer than `limit`). History reads the Contract sources the server has now; a revert reads
  every one in the app outside `node_modules`, and refuses by name one the app no longer reads.

### `revert` (*1.1*)

`{commit, client}` holds, as a proposal, the inverse of a past commit's design-file changes.
- **Planned on today's files** as operations, never a byte restore. Its `review.revert` is
  `{commit, subject, moved, already, code?, only?}`. `moved` lists the files whose bytes differ
  from the commit's result. A `.contract` file's state rows are set back with `set-state` or
  `unset-state`, and a style's own row's value with `set-row` (*1.12*: one the commit added or
  removed is refused, `set-row` adding or taking away none); its other lines are left as they are, and `code` lists the files where the
  commit changed some (*1.11*).
- **Refused (409)** with `refused: [{file, edit, now, why}]` when an edit can't be inverted:
  - the file has moved on it since;
  - the commit created or removed a design file (a `.contract` file's state rows included; one
    whose code alone changed is left alone);
  - a `.contract` row put back last would paint over a border row of its block it came before,
    or the commit changed their order;
  - a `.contract` row's style is gone, it is written over more than one line, its value before
    is no string or number, or `unset-state` would take a comment (on its line, or its emptied
    block's header);
  - the `.contract` file is not one of the app's own Contract sources now;
  - it added or removed a dial;
  - it added an axis, reordered an axis's values, or changed an axis's `under`;
  - it changed a kind, a range, metadata, the design's name or its exports.

  A removed axis is restored with `add-axis`.
- **Other refusals:**
  - everything is undone already: 409 with `already`;
  - a commit that touched no design file, or only a Contract source's code: 400;
  - an unknown commit: 404.
- **Accepted after the files move,** it is planned again from the commit. A partial accept keeps
  its selection when planned again.

### `import` (*1.1*, LLP 1122 T1a)

`{kind, source, name?, prefix?, baseline?, client}`, or `{kind, sources: [{name, text}], prefix?, baseline?, client}`.
- **The source.** `source` is the file's text. `name` is its file name; a Figma export's
  `<Collection>.<Mode>.json` names its modes. `sources` takes 1 to 256 files, and only for
  `dtcg`: a stylesheet's importer reads one sheet, and several are refused (400, *1.1*).
- **`kind`** is `shadcn`, `tailwind` or `dtcg`. `prefix` is the token prefix, by default
  `shadcn`, `tw` or `dtcg`.
- **Every kind** (stage 6 H): a length, number, type, family, shadow, duration or easing
  is proposed as its kind's token, the `add-token` naming its `kind`, when it folds as that
  kind beside the import's other tokens; one that does not, or that has a value per side of
  the scheme (only a colour has a dark side), is in `unmapped` with why. Every token, a colour
  too, is checked where it lands, against the app's vocabulary: one the design has of another
  kind, one that would leave a design token reading it (or a dial) unfolded, and one naming a
  token so refused, is in `unmapped` instead of `ops`.
- **Astryx is refused here** and keeps its CLI (`bun exact.mjs tamber import astryx …`), since
  resolving its package needs a Bun install.
- **The import is held as a proposal** whose `review` is `{importer, counted, unmapped, offers,
  tokens, dials?, pins?}` (the last two a re-import's, below):
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
- **An import is read in a process of its own** (*1.14*, stage 10b), one at a time, never in the
  design's server: a source that takes more than about 1 GiB or 10 s to read, or whose review is
  over 2 MiB, answers 413 or 503, "the source is too costly to read". With four imports already
  waiting, two of this client's outstanding, or after 30 s waiting, an import answers 503 ("imports
  are busy"). A proposal (its diff and review) over 4 MiB is not held: 413. With no design file, an
  import answers 409. Within that, the DTCG importer's and re-import's work is bounded too
  (references, groups and aliases that multiply are refused).
- **A re-import's dials and pins** (*1.14*, LLP 1121 §5 stage 10b): re-importing a Tamber export, a
  changed value whose line is one colour dial alone (`accent.fill = accent`), changed to one literal
  in every context where that dial line wins, proposes `set-dial` for that dial instead of a literal
  over the line. The review lists it in `dials`, `[{dial, value, from, impact}]`: `from` the tokens
  the file changed, `impact` every token value the change moves alone against the design, `[{token,
  tuple, before, after}]` (values as the plan carries them, so `#ff0000ff` beside the dial's
  `#ff0000`), so accepting it is seen to reach past the one variable. A changed value whose line
  followed dials or tokens by an expression is a literal over it, listed in `pins`, `[{token, when,
  follows}]`, with what it no longer follows (D14's mark). Inverting an expression is not attempted.
  Each list is absent when empty, and each entry names its operations (`items`), as an offer does; a
  recomputed proposal (files moved since, or some items) is marked again on the design as it is, its
  dials' `impact` and its `pins` read afresh, each dial keeping its `from`. Tokens that would set
  one dial to different values, or a dial the design refuses, are proposed as literals instead, each
  a pin marked with the dial. Colour values agree when they are the same colour (`#F00`, `red` and
  `color(srgb 1 0 0)`). A role's row is followed by its bare name too (`background`).
- **`baseline`** (*1.14*, stage 10b): the export text a client pulled, for a client below `write`,
  whose `export` keeps none. It is taken when its header's digest is an export this server run gave
  out, a kept one, or the design's own, and its contexts (each one's name, modes and scheme) and axes
  are the design's now; else the import is refused, 409 with `stale: true` ("pull first"). It
  decides only which of the file's values are the client's edits: the operations are planned
  against the design as it is, so a baseline can at most make a client propose values it did not
  change, never revert another's edit unseen.

### `export` (*1.4*, LLP 1121 D14)

`{kind: "dtcg"}`, read-only for the design: the app's design as DTCG Format 2025.10 with the Resolver
Module 2025.10, as `bun exact.mjs tamber export dtcg <app>` writes it, from the same serializer.
- **The reply** is `{kind, text, baseline, digest}`: `text` is the file, `digest` its header's design
  digest (*1.14*, the baseline an `import` may name), `baseline` where it is kept, by
  path from the app (`.exact/tamber-dtcg/<digest>.json`, the design's digest). Each export keeps
  what it wrote there, the newest 16, in a directory whose own `.gitignore` (`*`) keeps them out of
  every commit. A link in a baseline's place is replaced, never written through. Below `write`
  scope (*1.13*), or with `keep: false`, no baseline is kept (`baseline: null`): keeping one prunes
  the oldest, which another client's import may compare against.
- **The document.** One modifier, `mode`, whose contexts are the full tuples (every app-axis tuple,
  then `light` and `dark`, named `axis=value, …, light`), each a complete set of the design's
  tokens (every kind since stage 6 G, under *1.9*), a colour checked to read back as the colour a
  host renders before it is written. The header is the root
  `$extensions["exact"]` of the one token document of a set `exact`, first in `resolutionOrder`
  (the resolver's own root takes no `$extensions`): `export` (`"tamber-dtcg/1"`), `design`,
  `digest`, `conversions`, `axes`, `contexts` (each context's `modes` and `scheme`) and `dials`.
  The root `description` names the conversions too.
- **Each token** is `$type: color` with its value as a 2025.10 colour object (`hex` the 8-bit sRGB
  fallback), and in `$extensions["exact"]`: `source`, the line the context takes when it is not a
  plain colour; `role` or `platform`, for a value at that role's or platform colour's fallback;
  `overrides`, for a role override, exported as a token named for the role's row (`AccentColor`);
  `lossy`, the CSS of a colour past Oklab's range (an HDR one), clipped.
- **Every other kind** (stage 6 G) takes its DTCG type, one value in light and dark:
  - a length `dimension`, px or `rem`; an `em`, a text style or the hairline as px at the default
    root size (16), `lossy` its source;
  - a number `number`;
  - a family `fontFamily`, its members unquoted;
  - a type `typography`, every property written: what the token leaves unsaid (the text style's
    weight and leading, the family) as the web's default, named in `unsaid`; a style in
    `fontStyle`; a line height that is no number in `leading`; a text style's size `lossy`;
  - a shadow `shadow`, a text shadow marked `kind: "text-shadow"`, `currentcolor` as black marked
    `currentcolor`;
  - a duration `duration` in ms;
  - an easing `cubicBezier`, a keyword as its curve, a `steps()`, `linear()` or spring as
    `linear`'s, `lossy` its source.

  An icon is no token of the export. Re-imported, a changed value proposes the line that wins
  where every tuple it wins in moved to it, else a `when` naming the tuple's axes. An unchanged
  part keeps its source: a type's text style where its size did not move.
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
| `proposal` | the held proposal: `proposal`, `client`, `diff`, `changed`, `class`, `expires`, `ops`, `replaces?`, `review?`, `revoked?` (*1.13*: its client was revoked) |
| `proposal-dropped` | `proposal`, `client` (null when it `expired`, went `stale` or was one `too many held`), `reason`: `accepted`, `dropped`, `replaced` (with `replacedBy`), `stale`, `expired` or `too many held` |
| `down` | `message`: the design server stopped or never started |
| `paired` | `client`, `scope` (*1.13*: a client paired) |
| `revoked` | `client`, `reason`: `revoked` or `expired` (*1.13*) |
| `pairing` | `what`: `burned` (a code had five wrong attempts), `limited` (a peer had too many) or `replaced` (a ninth code) (*1.13*) |
| `builds` | `builds`: the registered builds, as `hello` lists them, after one registers, goes, or its pins change |
| `build-status` | `build`: one build, after its `status` report (one that moves only the design, up to 150 ms after: see the build channel's reports) |
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
- **`POST /__tamber/events`** `{after, epoch?, wait?, client?, held?}` answers `{events, seq, epoch, missed,
  files, held, proposals?}` (`proposals` left out when `held` names the current revision, *1.12*):
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
  - `scrub` (*1.11*), on a scrub's batch and on the snapshot it leaves: `{id, rev, run}`, the frame's
    mark (see [Scrub sessions](#scrub-sessions-12-llp-1121-d13-llp-1122-d6)).
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
  `{nonce, kind: "register", id, host, capabilities, plan, planDigest?, design}` (`host`: `web`,
  `web-js`, `ios` (tvOS too), `macos`, `linux`, or `android` for the Android reader, *1.9*). `planDigest`
  (*1.9*) is the SHA-256 of the plan the build runs, the digest its source map names; the server
  joins and outlines that build from that plan's map.
  - `page` (*1.15*): `"gallery"` for the variant gallery's page (the JS dev loop's `/__dev/gallery/`,
    development only; the wasm loop answers it 404 with why: its stream `/__dev/gallery/stream` carries
    its own reloads and errors, never the app's, nor the app's its), absent for the app's own page. Listed on the build in `builds` and
    `hello` as it was said. It is the page's own claim, used only to group and label builds: it
    grants nothing and changes nothing a build may receive (a gallery page offers no `navigate`).
  - `id` is the build's own, stable across its reconnects (a tab's, kept across its reloads but
    not shared with a duplicated tab; a process's), so its pin is replayed when it registers
    again. Every registration is told the pin the server holds for it, or none.
  - A web page in a frame its parent names `exact-copy-<n>` (`n` of one or two digits; the
    design tool's copies, *1.14*, LLP 1122 T7) keeps its id under that name, since a tab's
    same-origin frames share its storage. The page only reads the name, which never enters the app's URL, is never sent, and
    ties nothing; a frame of any other name keeps the tab's one id. A framed page posts `{type:
    "exact-build", id}` to its parent each time it registers, so the page framing it finds which
    frame is which build by the message's source (the id grants nothing without the nonce).
  - The design tool frames each copy at a loopback name of its own,
    `http://exact-copy-<n>.localhost:<port>/` (theme2's F1 ruling: a browser carries at most six
    HTTP/1.1 connections a host, and each copy holds a stream). The dev loop admits those names
    (`n` 1 to 99), never as local. Each copy is then its own origin, with its own storage.
  - In a copy (a page in an `exact-copy-<n>` frame) that is pinned, the pin holds over the app's
    own `setScheme`, `setMode` and `setRootFontSize` for the facts it pins (theme2's F2 ruling;
    elsewhere the app's own wins, LLP 1122 D3). What the app asked instead is kept, takes effect
    when the pin is handed back, and is said in the copy's `status` as `app`.
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
    - **A long poll** (*1.11*): `&wait=<ms>` holds a poll with nothing to answer until a frame
      is queued for its stream or the wait passes (at most 4 s), so a frame reaches the build at
      once. One is held per stream: a newer poll answers the older at once. The JS loop's resident
      producer holds each poll 4 s; Linux holds 250 ms, so a report it queues waits at most that
      for the poll before it to end.
    - **Bounded.** Only the latest `pin` waits in a stream's queue. Past 64 frames or 128 KiB
      queued, the live-table frames waiting collapse into one notice of the latest design, and
      the build fetches the snapshot.
    - A nonce no stream holds is 404 with the body `null`: the stream is gone (the loop
      restarted, or it went unpolled 5 seconds), and the build says hello again.
- **Reports up** (`POST /__dev/report`, each with the nonce), told to every client as events:
  - `status`: the effective `scheme`, `contrast`, `mode` (`{axis: value}`) and `rootFontSize`,
    the facts that paint, the `plan` and `design` digests (and `planDigest`, *1.9*, as it moves),
    and the pin `revision` it applied. The web pages also say their `location` (*1.14*, LLP 1122
    T7): the path, query and fragment they show, on their own origin (at most 2 KiB; no scheme or
    host), reported when it moves. A pinned copy also says `app` (*1.14*): `{scheme?, mode?,
    rootFontSize?}`, what the app asked instead of its pin (strings; `mode` `{axis: value}`; at
    most 1 KiB), or nothing.
    Sent on connect and on any change, the app's own `setScheme`, `setMode` and
    `setRootFontSize` included (at most once a turn). A scrub's frames move the design without a
    status each: the web pages report it once, about 150 ms after the drag rests. A native host
    reports every scrub frame's; the server tells clients a `build-status` whose only change is
    the design 150 ms after the first such status waiting, with the latest design then (a later
    one replaces it without restarting the wait). Any other change, and any other report from
    that build (a selection, a held state, a refusal), is told at once, after the design waiting.
    `hello` and the `builds` list are not held: they have the design last reported.
  - `unsupported`: a command or batch it can't apply, by name (`what`), with the pin frame's
    `revision` when it is one (*1.3*, optional).
  - `selected` (*1.3*, LLP 1122 D5; the wasm web page and the iOS and Mac hosts send it in inspect mode):
    `{selected, inspect?, click?, plan, design, revision}`, the number of the `inspect` command it
    was made under (*1.6*; one since replaced is refused), a click's token (*1.7*, 1 to 64
    characters: a selection a press made, re-sent when it is reported again), the digests and pin
    revision the build runs, and
    `selected` `{node, type, testId, site, instance, planDigest, tuple, rows, held?, trimmed}`: the
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
  - `inspect` (*1.3*) `{on, seq}` or `{refresh: true}`, or `{select: {up, levels, plan, from, instance}}` (*1.16*:
    `levels` are the plan sites of each enclosing copy from `up` call sites out to the app's own,
    each that owns a node, nearest first, as the server's map gives them for the selection at site
    `from` (its instance path `instance`) under `plan`; a page whose selection or plan is another reports its selection as it is now, else it
    selects, in the first level with a node above its selection, the outermost by its own tree,
    and reports it as a click's selection): inspect mode on or off (`seq` numbers
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
    a finger down (the pointer over a view, on the Mac) highlights what it would select. The JS target's
    development page does as the wasm page does (T2b): it has no runner, so it builds each row from
    the plan's inspect table (`/dev-inspect.json`, which the JS dev loop makes when first asked) and the page, and a
    row bound by an expression is the element's computed CSS, said `computed: true` (*1.9*). tvOS and
    the Linux host answer `inspect` unsupported. A build that registers again
    is told inspect mode as the server holds it. A build that can says
    `inspect` among its `capabilities` (the wasm and JS web pages, and the iOS and Mac hosts).
    Development builds only.
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
  - `hold` (*1.11*, LLP 1122 D8, LLP 1121 stage 8 E) `{revision, hold, plan?}`: `hold` is `{site,
    instance, state}`, a state (`active`, `hover` or `focus-visible`) held on the one view of the
    build's own plan at that site and instance path, or null to let it go. One a build: each
    replaces the one before. The host shows it as the real state, on its own tracking, handlers
    never told: `active` is the press of the node the press resolves to (the innermost node with a
    `press`, as a real press's), its paint and scale eased as a real press's (at once on the
    agent's clock); `hover` and `focus-visible` that node's look, at once. It reports `held`
    `{state, revision}` when it holds it, and `{state: null, reason, revision}` when it lets one go:
    `input` (real input on the node: a press, a key, the pointer coming over it or leaving it for a
    hover, the focus coming or going for a focus), `disabled` (the node is or became disabled: one
    refuses a hold), `gone` (no single view at that site and instance path, no press there for
    `active`, or the view went; a hold it held before is let go too), `replaced`. Every report
    names the `revision` of the hold frame it answers. A host that can't hold that state answers
    `unsupported` with `what: "hold"` and that `revision` (iOS for `hover`). A build that can says
    `hold` among its `capabilities`. `plan` is the plan digest the site addresses, when the server
    knows it: a build running another plan (one installed since) takes the frame as nothing, unsaid,
    and the server's replay after the status that names the new plan corresponds it (Linux says
    `planDigest` in its registration and `status` for it). A host's new generation takes the hold
    again itself under the same plan, unsaid; a frame that comes while it swaps waits for it, and a
    clear always applies. A revision a host ended itself is not taken again
    when its frame is resent; a lower revision is a new server's.
  - `navigate` (*1.14*, LLP 1122 T7) `{seq, location}`: route to `location`, a path with its
    query and fragment on the build's own origin, in place, as a link to it would: the app's
    navigation root's `navigate` handler, else the router's own `go`, with no reload, so the
    app's state is kept. A fragment that differs is then set as the page's own. A location that
    is no route of the app, or a page with no router, answers `unsupported` with `what:
    "navigate"` and that `seq` as its `revision`. It moves the page's `location`, which its next
    `status` says. A build that can says `navigate` among its `capabilities` (the wasm and JS web
    pages; the native hosts don't yet). It is navigation, not input: no tap or key is driven.
- **Navigation at the server** (*1.14*). `{method: "navigate", client, build, location}` needs
  write scope and a present client, and names a registered build (else 400) that can navigate
  (else 409 with `unsupported: "navigate"`; the design tool then reloads that copy's frame at
  the location instead). `location` must start with one `/` (not `//` or `/\`) and be
  spelled as a URL and the router write it (RFC 3986's path, query and fragment characters but
  `'`, each `%` a whole escape), at most 2 KiB as given and as a URL spells it once resolved (no
  dot segment may make it `//…`); any other is 400. A status's `location` is any printable
  ASCII path within the bound. Each is journaled with its client
  and answered `{navigated: true, build, seq}`. A navigation holds nothing on the build: there is
  nothing to replay when it registers again or to clear when its client goes. Only the latest
  waits in a polled stream's queue. A build's `unsupported` for it is its sender's event alone
  (`only`), and only for the last `seq` sent that build.
- **Holds at the server** (*1.11*). `{method: "hold", client, build, site, instance, state}`
  (`state` null to let it go) needs write scope and a present client, and names a registered
  build that holds states (else 409, said so). The hold is its client's: let go on that build when
  the client goes. A build that registers again is sent it again under the same plan (or `hold:
  null` when the server holds none for it); one that runs another plan (a `status` whose
  `planDigest` moved, or its registration's) is sent it at the one site of the new plan's map at
  the old site's source location and call-site chain: a component called at two places is two
  copies there, one copy a chain (its sites, such as a `button "…"` and its label, told apart by
  their order). With the same calls in both plans, the copy with that chain; with as many copies,
  the copy of the same rank in source order where each pair has the same calls' files and
  components, the copies share calls alike, and each file's calls moved as one edit there moves
  them (those above it by none, those below by one count of lines), else the unmoved copies kept and the rest so among
  themselves (one call moved: the one left over); with copies added or removed, the copy whose
  calls did not move. The correspondence is by places alone (the map
  carries no other identity of a call or an element), so it is a guess wherever places are
  reused: calls reordered with their spacing kept, a call or element moved onto another's old
  place (a line or a column), or a copy added or removed while one keeps its line, can carry
  the hold to the other copy (where the copies' instance paths differ, the host finds no view
  for it and lets it go, `gone`); and a slot filled at two outlets is one copy, its outlets told
  apart by their order. A call moved past another with the lines between them changed clears
  the hold. When there is not exactly one, the build is sent `hold: null` and every client
  told (`hold-cleared {build, client, reason}`). Each frame, a replay included, has a new
  revision, and the build's `held` in a listing is null until it reports on that frame. Only a
  report naming the last hold frame's revision counts (an
  older one is dropped, 204): a `held` report is then every client's event (`held {build, state,
  reason}`), and one that lets the hold go ends it at the server too; a hold `unsupported` ends it
  and is its owner's `unsupported` event alone (theme2's 07:48 (c): `only` names the client, and no
  other subscription or poll sees it). A hold or its end announces `builds`. Holds
  are not kept across a server restart, as pins are not.
- **A selection joined to source** (*1.3*, LLP 1122 D5). The server takes a `selected` report
  only while the build is in someone's inspect mode (*1.6*: one sent as it ended, or naming an
  `inspect` command since replaced, is refused, 409, and not published; a page's exit, `selected: null`, is published only for the build's
  current inspect command), and only when it is the build's latest: its `plan`, `design` and `revision` those of its latest
  status. One from before (a batch or a pin since) is refused (409) and the build is asked again
  (`inspect {refresh: true, status: true}`: its status first, then the selection), once per
  status it has. A current one is joined to the source map of the plan it names
  (the dev loop's `app.plan.map.json`, verified by that digest) and published as the `selection`
  event: `source` `{file, line, col, end_col, component, chain, library?}` (the component call sites above
  it; `library` *1.15*, stage 12 D4, LLP 1139 D5: `{identity, part}` on a node of an exported library
  component's own view, its declaring identity (`@scope/pkg/file.contract#Name`) and the part it is
  named as, or null, joined from the map since `part=` never reaches a host, beside
  `component`, the bare name the map gives (the declared one, which two packages can share; an alias
  the app imports it by is in neither); absent outside one, a slot's fill being the caller's), and each row with its map `origin`
  (with the node's `library` too, on a row whose map `origin` isn't null, where the node has one;
  a held state's rows the held node's) (`own`, `class:<Name>`, `tag`) and `editable`: `{rung:
  "token", token, modes}` only for a row the node itself sets (`authored`, `own`) whose whole
  value is one token; `{rung: "param", param, short, token?, modes, template?}` (*1.11*, LLP 1122 T3; `template` *1.12*, T5: the template rung's fields where the map places the row, which a detach writes at) for
  such a row whose whole value is one component param: `param` its full name, `short` its short
  one (null where two components share it), `token` the token it aliases directly (its value as
  the app has it, the line's or the default, when that names one declared token, a role override
  by its role's spelling too; none for a param with `when` lines), `modes` the copy's mode tuple,
  as the token rung's (the client reads the branch that won by `explain`). A row's `token.names`
  and `text`, in the `selection` event and the published `selected` report alike, name a param by
  its full name, never by the IR token the plan names it by, also inside a synthetic token's
  expression or as the custom property a JS page writes for it (`var(--x-param-…)`); one the server has not read yet (its `describe` of the app, read again when the
  app's files change, for a new plan, and after a failed read) is left out, and the row is
  read-only, said so; while a new plan's read is out the names stand and no `token` is offered. A
  row whose one name is a param (an inherited or dynamic one) links it by its full name. A client
  that knows no `param` rung offers no edit for it. A row a style's state part writes (its map part, a `states` entry's
  `{state, origin, at}`, the origin `class:<Name>`) answers `{rung: "state", style, state, row, file, text?, token?}`
  (*1.12*, LLP 1122 T4), the default: the style by the name its file declares it with, the state,
  the row as its author wrote it under the state (`background-color`, not the press paint's
  `press_background_color`), the style's file relative to the app (what `set-state` takes), the
  row's value as written there where it is one plain value (as the template rung's `text`, below;
  else none, and an editor starts empty), and the token its whole value is, offered beside it, never the
  default (D6). A style outside the app's own Contract sources as the server counts them (a
  package's, a vendored or linked one included) answers a reason. A held state's rows carry the
  same where the build names the held node (its `held.site`), whose parts' files count toward
  `readOnly` too; with none named they are shown, not offered. A row its author wrote (the map's
  `at` and `attr`, D1's address) answers, unless it reads a param (the param rung, above), the
  **template** rung (origin `own`) `{rung: "template", file, at: {line, col}, sha256, row, state?,
  instances, text, component?, token?, modes?}` (*1.12*, LLP 1122 T5; `component` the one whose
  view it is in, by the name its file declares it with; a held state's row too, where it is
  written under the node in that state and the build names the held node: its `instances` that
  node's) or the **style** rung (origin a single
  `class:<Name>`; a conditional class, `class:A/B`, is refused) `{rung: "style", style, row, file,
  text, token?, modes?}`, the default (D6), for an `authored` row of that one origin only:
  `set-row`'s address (at the place: the file relative to the app, the attribute's line and
  column and the file's digest as compiled; on a style: the style by the name its file declares
  it with, and its file), `row` the attribute as written (a shorthand's, for a longhand of it:
  `set-row` edits the shorthand whole), `state` for a state row written under the node (its `row`
  the attribute its author wrote under the state, which the map does not name), `instances` the
  copies of this source node in the map, one a call-site chain (a template edit is every one's; a
  `button "…"`'s label is no second copy), `text` the value as written, and the token its whole
  value is, offered beside it, never the default. Only a plain value is offered, as `set-row`'s
  `plain` takes one: a string (its escapes decoded) or a bare `#…` colour, a number (an exponent
  and a minus included), inside any parentheses, then the line's end, a comment, another
  attribute or a positional argument (a string, a template, one in parentheses or a name; read at the
  map's UTF-8 byte column, the attribute's name checked there): an
  expression or a name in scope answers a reason, as does a file outside the app's own Contract
  sources, and a layout row (its `layout`, D14: `set-row`'s reason, up front; the selection's row
  carries `layout` too); any of these offers the token rung where its whole value is one token. A row the map gives no place answers a reason (the source's to edit), or, a whole
  token, the token rung. An `:active` part's rows (lowered to the press paint) carry their part's
  place in the map, and so, held (*1.14*, LLP 1122's queue item (1)), do a `:hover` or
  `:focus-visible` part's: the map keys each part by its row and state, never by its row alone, so
  a look's row is never offered at its base row's place. A row a `:disabled` or `:checked` part
  folds over its base (one value, `disabled ? D : B`, the runtime's `dynamic`) is read through the
  map (*1.15*, LLP 1122's queue item (2)) where it says both the base and the parts: its `editable`
  is its base's rung, whichever branch the runtime shows now (param, one `param.<name>` as written,
  by its full name among the params read for the component it is written in, a package's included,
  its template address the base attribute's, never a part's; template, style or token), and it adds `states: [{state, source, editable}]`, one per folded part
  (`disabled`, `checked`; two of one state, a class's and the node's, each),
  `source` the part's place in the form a held row's has (its node's for a part with none: a
  conditional class's), `editable` that part's own edit: `set-state` on a style's part or
  `set-row` with `state` on the node's own, never a param it names (the base's and every other
  use's). A fold the map can't explain (no base an author wrote: a link's colour, `none`, a
  metric) stays `dynamic`, read-only. Else `{rung: null, reason, link?}` (a row reached by inheritance, a dynamic
  expression, a boundary, or an expression over tokens links its token, never this node's edit).
  A row an `:active` part lowered to
  the press paint (*1.11*) carries `state` and `at`, its part's place (none for a conditional
  class's part, either style's; since *1.14*) (its edit, the state rung
  above for a style's part, the template rung with `state` for one written under the node). A row its author wrote on the node or in its class's `style` (*1.12*)
  carries `at` `{file, line, col, end_col, sha256}`, the attribute's place and that file's digest
  as compiled (`set-row`'s address, LLP 1122 D6), and `attr`, the attribute's name as written:
  `padding` for `padding_top`; and `layout: true` where that attribute writes a layout row (LLP
  1122 D14: `set-row` refuses it, by the same test), so a client offers it no `set-row` edit (a
  param or token it reads whole is still offered). A tag's row, one the compiler derived, a conditional class's, and one the compiler composed or renamed (an animation's
  longhands, a symbol's tint, a grouped field's `flex` made `flex-grow`) carry neither. A selection made while a state is held carries `held` `{state, node?, site?, instance?, rows}`
  (*1.11*; `node`, `site`, `instance` and a row's `token` *1.12*): the node the state is held on
  (the press it resolved to, named on a node inside it) and the rows that state shows (`active`:
  the press paint's; a look's otherwise), each with its value, the token it read, `state` and
  `source` (its part's place: an `:active` row's, and since *1.14* a `:hover` or `:focus-visible`
  look's row's; the node's for a row the map gives no part in that state, or a part no place: a
  conditional class's, either style's), joined at the held
  node's site, and `editable` as a row's above: a look's row is edited as an `:active` row is (the
  state rung on a style's part, the template rung with its `state` on one written under the node),
  a border side by the name written at its part: `border-color` (its four sides whole) or the
  side's own longhand (`border-top-color`). An `:active` row is named as its author wrote it under the part (`background-color`), the
  press row it lowered to beside it (`row`, *1.12*). A host that cannot name a row's token says
  `provenance: "incomplete"`. Each file the join reads is
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
- **`inspect`** `{client, build?, on | refresh: true}` (*1.3*, a present client; `on`, and `refresh` sent to the builds, need write scope; below it `refresh` reads the last selections, *1.13*), or `{client, build, select: {up: n}}` (*1.16*, n from 1 to 64; write scope and the build's inspecting client alone, 409 otherwise, or with no selection to move from, or with `stale: true` when the selection moved since its chain was read: its selection moves `n` call sites up, past the outermost to the root, answered `{builds}`)
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

- **`GET /__tamber/tool/…`** serves the built tool (`target/design-tool-web`) to local requests,
  and on stage 10 B's remote doors (no design is in it; it pairs before it reads one). It serves no
  dot file, and follows no symlink out of the build.
- **`bun exact.mjs tamber tool [<app>] [--via <dev url>]`** serves the tool at the fixed origin
  `http://127.0.0.1:7740`, which the tool's grant names. So one tool runs per machine, on one
  project. With `--via`, port 7740 forwards `/__tamber` to the dev loop that holds the design.
  See `apps/design-tool/README.md`.

## Not yet in the protocol

- **`uses`' mounted-instance count** needs a report outside the build channel's (a LLP 1121 §9
  candidate).
- **LLP 1121 stage 4's gate also names `resolve-on`.** LLP 1122 defers it to T2b, where a build's
  `selected` report carries the value it resolved.
- **Later stages:** stage 10's canvas gateway (C) and the design tool's remote mode (D, LLP 1122
  T6); a hosted relay (stage 10c).
