# Exact Documentation

Write an app once. It runs natively on the web, macOS, iOS, and Linux.

## Set up Exact

Requires Bun 1.4.2, Git, rustup and Chromium. [Install the tools](install.md) first.

Start in the directory where you want to keep Exact; these commands clone it, install its pinned toolchains and open a first web app.

```sh
git clone https://github.com/ccheever/exact2.git
cd exact2
bun scripts/exact.mjs setup         # once: the pinned toolchains
bun scripts/exact.mjs new ../hello  # a new app
cd ../hello
bun exact.mjs web                   # the dev loop, at http://127.0.0.1:8765/
```
