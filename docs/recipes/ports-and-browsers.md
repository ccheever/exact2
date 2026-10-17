# Recipe: ports, Bun and Chrome

**Ports.** `--port <n>` on `web`, `test web` and `agent web`, or `EXACT_PORT=<n>`,
serves on that port; a held port is refused. Otherwise each picks a free port within `PORT_LO`–`PORT_HI` when both are set, and
prints its URL. A native client starts a second loop on `n+1`. A storage store
keeps its data per port, so drive one store on one port.

- **Bun and Chrome.** Launchers prefer `$BUN`, then `~/.bun-1.4.2/bin/bun`,
  independently of a login shell’s PATH. Generated commands and child scripts
  inherit that binary. `./exact.mjs` also works when PATH has no Bun. Set `CHROME=/absolute/path/to/chrome` to select Chromium.
- **Named Chrome stores.** `agent web --storage s1` starts empty each run.
  Add `--keep-storage` to reopen its saved data and password session, or
  `--reset-storage` to explicitly reset it. A live drive owns its profile;
  another drive cannot reset it. Authored tests always isolate their browser
  stores, including across repeated suite runs. Backend rows persist: use
  `fresh` with `dev`, or create uniquely keyed fixtures with `serve`.
  Firefox/WebKit use a new profile each drive and do not preserve named stores.
- **Native stores.** `agent ios --storage s1` (macOS, Linux, Android alike) is kept
  between drives; `--reset-storage` empties it. One drive holds a store at a time.
