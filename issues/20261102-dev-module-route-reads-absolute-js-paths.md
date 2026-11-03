# The dev server's module route reads absolute `.js` paths

**Status:** Closed
**Systems:** Web dev loop (`host/web/dev.mjs`, `scripts/origin.mjs`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** For an app that has `modules/web`, the development server answers `GET /modules//some/absolute/file.js` with that file's bytes, when the process can read it. With `--lan`, any peer that can open the dev server can do this. On loopback, any local client can.

**Cause.** `webRequestURL` rejects `..` and does not reject an empty path segment, and the URL parser keeps the double slash. The serve function then handles a path that starts with `/modules/`, matches `/^\/modules\/[\w./-]+\.js$/`, and does not contain `..`. `file.slice('/modules/'.length)` of `/modules//Users/name/secret.js` is `/Users/name/secret.js`. `path.resolve` of an absolute second argument drops the module directory. `readFileSync` returns the file. This branch does not use the found-file result of `readWebRequest`.

**Reproduction.** Start `bun host/web/dev.mjs` for an app whose manifest has a web module. `curl -sS --path-as-is http://127.0.0.1:<port>/modules//etc/hosts.js` is 404 because that file does not end in `.js` and is not named that way; `curl -sS --path-as-is http://127.0.0.1:<port>/modules/<an existing .js path with a leading slash>` returns the file. A probe of `webRequestURL('/modules//Users/ccheever/secret.js')` keeps that pathname, the module regular expression accepts it, and `resolve` yields `/Users/ccheever/secret.js`.

**Fix.** Resolve the slice only when it is relative, and require it to stay under the module directory after `realpath`. Reject an empty segment in this route even if `webRequestURL` allows one elsewhere.

**Acceptance.** `/modules//…` and any slice that resolves outside the module directory are 404. `/modules/index.js` still serves the app's module.

## Resolution

Constrain live module paths to relative segments inside the real module directory. Reject absolute paths, traversal and symlinks that escape it, and return 404 without falling through to static files.

## Verification

`bun test host/web/deep-link.test.mjs -t "live module route"` passes valid-file and confinement cases.
