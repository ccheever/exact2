surfaces: web, macos, ios, linux
agent: Claude Code, Claude Opus 5.5
task: build a small app that finds devices on the local network and controls them

## Rough

- Lights' Linux crate could not be the render host Weatherlight's `linux` crate
  is: that one renders pages and has no app loop. Shelf's `linux` crate is the
  one to copy for an app a person uses; the two share a directory name and
  nothing tells them apart but the bin's name.
- `@exact/local-network` needs the root `package.json` dependency and a
  `bun install` that rewrites `bun.lock` before any app can `use` it; the
  error until then names `node_modules` and not the missing line.
- An interface that exports `./linux` and `./windows` fails every app's
  compile with "No such file or directory" until both implementations exist,
  even when the app never builds for either. Export only what exists.
- On the web the origin of a fetch is known only during an answer's first
  turn: after an `await` the runtime no longer knows which commit asked it, so
  the rule for a tool's fetch falls back to the least privileged answer in
  flight.

- On the web's JS target under the agent, a module name the substitute
  refuses at once (a browse, where the web has none) refused the whole boot
  ("boot refused while building the view: … NotSupportedError"), where a
  bound implementation's refusal only fails the resource. Fixed in
  `host/web-js/modules.js`: a reader's immediate refusal now fails the
  resource, as a send's already failed its mutation.
- Lights' Linux executable composed no package: an in-repo app's build names
  its app as `..`, and the search for installed packages walked that
  relative path's ancestors, never reaching the repository's `node_modules`.
  Fixed in `contract/cli/src/native/packages.rs`. Until then the Linux host,
  run on this Mac, also read every module's per-target words as macOS's,
  since it names its target only when it serves a package.
- The `@exact/local-network` integration tests' host loop forgot the module
  requests it had parked between calls, so a ticket answered later released
  nothing; they keep them now.

## Lean in

- `prefer extension local-network.browseServices=[…]` gives a drive a whole
  network with no device and no multicast, and a fixture on `127.0.0.1` stands
  in for each light because a development build admits loopback under
  `net.fetch local-network`.
- A typed address and a found light go through the same `fetch` and the same
  grant line, so the fallback costs one text field.

## Needed

- local network discovery: provided (built here, `@exact/local-network`)
- local network access: provided (the `local-network` grant source)
- settings deep link: provided (`openURL("app-settings:")`)

## Checkpoints

- editing app.contract: smooth
- web dev loop: smooth once the boot refusal above was fixed
- driving the app: smooth on the web's JS target and Linux headless (the
  address field, a light switched); see LLP 1133.036 §14's as-built notes
  for the native drives
