# Recipe: a website

`bun exact.mjs pattern site` writes a tested one: a list built ahead, a page per item
rendered for each request, and an API's origin as a setting
([the site starter](../patterns/site/app.contract)).

## Which policy fits which data

A route's `render` says when its page is made:

| Policy | Made | Fits |
|---|---|---|
| `render=build` | by `web-build` | what the build has: docs, a landing page, a list in app.ts |
| `render=request` | by `serve`, per request | a live API's data |
| `render=cached` | by `serve`, kept 60 s | an API's answer every reader may share for a while |
| `render=client`, the default | in the browser | one reader's own, behind a sign-in |

The starter's planet page is `render=request`: its figures are the API's, which change after
the build, and a build makes a parameterised route's pages only from a `pages=` list, which
an API's data cannot give. `serve` renders that page with its data, so it arrives whole; a
static host serves its shell, and the browser draws it once the data comes.

## Serving

`bun exact.mjs web-build --out dist` writes the site to `dist/`, and `bun exact.mjs serve
dist` serves it on 127.0.0.1:8765 (`--port <n>`; `--lan` for other devices). A page
`serve` renders asks the API as it is made: start the stand-in first (`bun server.ts`;
`test` and `verify` stop theirs), or set `API_BASE_URL` to an API that answers.
`npm start` runs `bun exact.mjs serve`, which serves the last `web-build` without `--out`. Any
static host serves `dist/` by this rule: the file at the path; else, for a browser's
navigation to a path a route in `routes.json` matches, `shell.html` with 200, whose client
draws the page (`serve` renders a `request` or `cached` route's page in its place); else
404, with `404.html` where the build wrote one (a `notfound` route declared
`render=build`), else with `shell.html`, whose client draws the `notfound` route. A host
with one fallback page serves `shell.html` for every page it lacks.

## A setting

An API's origin is read at build:

```text
app.json  "env": ["API_BASE_URL"]
app.ts    export const API = import.meta.env.API_BASE_URL;
          export const grants = `net.fetch ${API}`;
.env      API_BASE_URL=http://127.0.0.1:8787
```

The value is the environment's, else `.env.local`'s (out of git), else `.env`'s, else a
default written after the read (`?? 'https://…'`); with none the build fails. It is inlined
into the module on every host, so it is public: never a secret. A build records each value,
and one changed since makes `test`, `agent`, `web` and `serve` build again where the app is
beside the build; `serve` refuses a build copied away from it, naming both values.

## A deep link

`/planet/7#details` opens the page at its `id="details"` once its data has arrived. Write
it `path("planet", id) + "#details"`, or `link href="#details"` on the page shown;
`fragment(top(nav))` reads it, and a test opens there with `at "/planet/7#details"`. A query
follows the path: `` path("home") + `?q=${v}` ``, read with `searchParam(top(nav), "q")`.
