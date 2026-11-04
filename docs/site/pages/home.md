# Exact Documentation

Write an app once. It runs natively on the web, macOS, iOS, and Linux.

## Set up Exact

Requires Bun 1.4.2, Git and rustup, plus Playwright Chromium with its headless shell or a Chromium browser selected with `CHROME`. Follow the [installation guide](https://github.com/ccheever/exact2#1-install-the-tools) and [browser setup](https://github.com/ccheever/exact2/blob/main/docs/testing.md#browsers) first.

Start in the directory where you want to keep Exact; these commands clone it, install its pinned toolchains and open a first web app.

```sh
git clone https://github.com/ccheever/exact2.git
cd exact2
bun scripts/exact.mjs setup         # once: the pinned toolchains
bun scripts/exact.mjs new ../hello  # a new app
cd ../hello
bun exact.mjs web                   # the dev loop, at http://127.0.0.1:8765/
```
