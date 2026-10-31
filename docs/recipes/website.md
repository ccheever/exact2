# Recipe: a website

`bun exact.mjs pattern site` writes a tested one: a list built ahead, a page per item
rendered for each request, and an API's origin as a setting
([the site starter](../patterns/site/app.contract)).

## Serving

`bun exact.mjs web-build --out dist` writes the site to `dist/`, and `bun exact.mjs serve
dist` serves it on 127.0.0.1:8765 (`--port <n>`; `--lan` for the other devices on this
network). `npm start` runs `bun exact.mjs serve`, which serves the last `web-build`
without `--out`. Any static host serves `dist/` by the rule `serve` keeps: the file at the
path; else, for a browser's navigation to a path a route in `routes.json` matches,
`shell.html` with 200; else `404.html` with 404. A host with one fallback page serves
`shell.html` for every page it lacks, and the client draws the `notfound` route.

## Which policy fits which data

A route's `render` says when its page is made:

| Policy | Made | Fits |
|---|---|---|
| `render=build` | by `web-build` | what the build has: docs, a landing page, a list in app.ts |
| `render=request` | by `serve`, per request | a live API's data |
| `render=cached` | by `serve`, kept 60 s | an API's answer every reader may share for a while |
| unsaid | in the browser | one reader's own, behind a sign-in |

The starter's planet page is `render=request`: its figures are the API's, which change after
the build, and a build makes a parameterised route's pages only from a `pages=` list, which
an API's data cannot give. `serve` renders that page with its data, so it arrives whole; a
static host serves its shell, and the browser draws it once the data comes.

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
