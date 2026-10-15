# The design tool

LLP 1122's "living Figma" over an app's Tamber design (LLP 1121), unnamed
until §9 Q1 names it. `bun exact.mjs tamber tool <app>` builds it if stale
and opens it at `http://127.0.0.1:7740/__tamber/tool/`, served by the app's
design server; with the app's dev loop running, which holds the design,
`--via <dev url>` reaches that loop's server instead. Development only: it
is never bundled into an app's release build.

The tool always lives at `127.0.0.1:7740` (its grant names that one origin),
so one runs per machine, on one project. To switch projects, stop it with
ctrl-c and run `bun exact.mjs tamber tool <other app>`, adding `--via <its
URL>` if that app's dev loop is running. A second `tamber tool` for the same
app reuses the running one; for another app it is refused, naming the app
whose design the port serves.

- `app.ts` is the `tamber/1` client: the token from `GET /__tamber/token`
  once, every request a `POST /__tamber`, and changes followed by the long
  poll, `POST /__tamber/events`. Its grants name the tool's one origin.
- Every write is `design-tool:<person>`, the person asked once and kept.
- The dials panel and the token grid are drawn from `describe` and
  `resolve`; a cell edits the branch it shows, and every edit is proposed
  (its diff and "live" or "rebuilds") before Apply writes it. Undo and redo
  are this run of the server's inverses: a new run starts them again.
- Changes lists every write since the tool opened, the tool's own and
  others' (agents, hand edits), with the token values before and after.
  Commit commits what the session wrote, with a message naming each change
  and who made it; the server refuses a file that was not only the
  session's, and never pushes. History reads the design files' git log as
  token edits; Revert holds a past commit's inverse for review.
- `app.test.contract` runs against `stub/tamber-stub.ts`, canned replies;
  `scripts/design-tool.test.mjs` drives the built tool against a real
  server on a copy of `apps/theme-fixture`.
