# Recipe: a website

`bun exact.mjs pattern site` writes a tested one: a list built ahead, a page per item
rendered for each request, and an API's origin as a setting
([the site starter](../patterns/site/app.contract)).

## Which policy fits which data

A route's `render` says when its page is made:

| Policy | Made | Fits |
|---|---|---|
| `render=build` | by `web-build` | what the build has: docs, a landing page, a list in app.ts |
| `render=request` | by `serve`, per request | a live API's data, at each visit |
| `render=cached` | by `serve`, kept 60 s | an API's answer every reader may share for a while |
| `render=client`, the default | in the browser, after its script | one reader's own, behind a sign-in |

The starter's planet page is `render=request`: its figures are a live API's, and a build
makes a parameterised route's pages only from a `pages=` list. `serve` sends it whole, with
its data.

## Serving

`bun exact.mjs web-build --out dist` writes the site to `dist/`, and `bun exact.mjs serve
dist` serves it on 127.0.0.1:8765 (`--port`, else `PORT`), building it again when you edit.
A page it renders asks the API as it is made: start the stand-in first (`bun server.ts`),
or set `API_BASE_URL` to an API that answers.
`npm run build` and `npm start` do the same, as hosts run them. A static host serves
`dist/` by the rule `serve --help` states.

## The markup

Each tag is the HTML element it names, so the landmarks and roles are the browser's: `main`,
`nav aria-label`, a `header` and `footer` beside `main` (banner, contentinfo), `section
aria-labelledby` (a region), `h1`–`h6`, `table` with `caption`, `th scope` and `aria-sort`,
`dl`, `select`, `input type="search"`, `link href` (an `a`), `image "src" alt=""` (an
`img`). A list of links or cards is a `column role="list"` of `role="listitem"`s; a field's
label, a `text id=` the field names with `aria-labelledby`. `curl` a page `serve` renders
to read it; `agent web --url /planet/3 "screenshot .exact/p.png compare design/p.png"` says
in text where it differs from a design's capture of the page.

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
into the module on every host, so it is public: never a secret. A value changed since the
build makes `test`, `agent`, `web` and `serve` build again.

## A deep link

`/planet/7#details` opens the page at its `id="details"` once its data has arrived. Write
it `path("planet", id) + "#details"`, or `link href="#details"` on the page shown;
`fragment(top(nav))` reads it, and a test opens there with `at "/planet/7#details"`. A query
follows the path: `` path("home") + `?q=${v}` ``, read with `searchParam(top(nav), "q")`.
