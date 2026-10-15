# The design tool

LLP 1122's "living Figma" over an app's Tamber design (LLP 1121), unnamed
until §9 Q1 names it. `bun exact.mjs tamber tool <app>` builds it if stale
and opens it at `http://127.0.0.1:7740/__tamber/tool/`, served by the app's
design server; with the app's dev loop running, which holds the design,
`--via <dev url>` reaches that loop's server instead. Development only: it
is never bundled into an app's release build.

- `app.ts` is the `tamber/1` client: the token from `GET /__tamber/token`
  once, every request a `POST /__tamber`, and changes followed by the long
  poll, `POST /__tamber/events`. Its grants name the tool's one origin.
- Every write is `design-tool:<person>`, the person asked once and kept.
- `app.test.contract` runs against `stub/tamber-stub.ts`, canned replies;
  `scripts/design-tool.test.mjs` drives the built tool against a real
  server on a copy of `apps/theme-fixture`.
