# Real-GitHub lane

The clone's pull request features are verified against real GitHub (user decision 2026-10-07,
which replaced the fake `gh` of `20261005-fake-github-fixture`). A lane T3 server runs on
isolated homes, and the `gh` it spawns is the real GitHub CLI on a lane-only config dir that
the user signed in to. Task record: `.exact/implementation/20261005-t3code-macos-parity/tasks/20261007-real-github-lane.md`.

Nothing here reads, prints or commits a token. The lane never uses `~/.config/gh`, the
keyring, `gh auth token` from a normal shell, `~/.t3`, port 3773 or the `t3code` URL scheme,
and touches no repository but the approved sandbox and the second account's fork of it.

## Directories and variables

| Variable | Default | Holds |
| --- | --- | --- |
| `T3_GITHUB_LANE_SHARED` | `<checkout>/target/t3-ui-parity/github-lane` | `gh/` (primary account), `gh-second/` (second account), `sandbox.json` (the sandbox, the logins, the seeded numbers). Point every worktree at the base checkout's copy so the login is reused. |
| `T3_GITHUB_LANE` | `<checkout>/target/github-lane` | this checkout's `bin/<account>/gh` wrappers, `home/<account>` (`.zprofile`, `.gitconfig`), `servers/<account>` (T3 home, Codex, Claude, XDG, tmp, the sandbox clone, `server.log`, `server.pid`), `work/` (the seed's object store), `logs/gh-calls.tsv`, `logs/probe-*.json` |
| `T3_LANE_SERVER` | the staged release (`bun examples/t3-code/stage-runtime.mjs`), unpacked into `runtime/` | another server: a `t3` executable or a `bin.mjs` (run by `node`) |
| `T3_GITHUB_LANE_GH` | `gh` on `PATH` | the real GitHub CLI the wrappers exec |

A lane server's environment is built from nothing (`serverEnv`): `HOME`, `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_HOME`, `TMPDIR` under its server dir,
`T3CODE_TELEMETRY_ENABLED=false`, `GH_CONFIG_DIR` set to the account's lane config dir,
`GIT_CONFIG_NOSYSTEM=1`, ports 16520 (primary) and 16521 (second), 16500-16599 only. The
server rebuilds `PATH` from `zsh -ilc`, so the isolated home's `.zprofile` puts the lane's
`bin/<account>` first; `bun lane.mjs which <account>` shows what that shell resolves. The
wrapper logs every call (argv only) to `logs/gh-calls.tsv` and refuses to run unless the config
dir holds its own token, so a missing login cannot fall through to the keyring. Git in the
isolated home uses no system or keychain helper, only `gh auth git-credential` of the lane gh.

## Sign-in (the user, once)

Run in a terminal, with the shared dir of the base checkout. `--insecure-storage` keeps the
token in that dir's `hosts.yml` instead of the macOS keychain, where it would overwrite the
machine's own `gh` login.

```sh
GH_CONFIG_DIR=<shared>/gh        gh auth login --hostname github.com --git-protocol https --web --clipboard --insecure-storage --scopes repo,read:org,workflow
GH_CONFIG_DIR=<shared>/gh-second gh auth login --hostname github.com --git-protocol https --web --clipboard --insecure-storage --scopes repo,read:org,workflow
```

The first is the primary account (it owns the sandbox), the second its collaborator. Sign the
browser in to the matching account before entering the code at github.com/login/device.

## Use

```sh
export T3_GITHUB_LANE_SHARED=<base checkout>/target/t3-ui-parity/github-lane
bun lane.mjs setup && bun lane.mjs whoami
bun seed.mjs --create --visibility private   # --create only once the user approved the sandbox; re-runs need no flag
bun probe.mjs                                # starts both servers, probes, stops them
bun lane.mjs start primary && bun lane.mjs project primary && bun lane.mjs pair primary   # for a live drive
bun lane.mjs stop primary
```

The seed is idempotent: it finds its pull requests by branch (`seed/<key>`, `seed/<key>-gN`)
and its comments by an invisible `<!-- lane:… -->` marker, puts a drifted one back where one
call does it (draft again, closed again, reopened), and otherwise opens the next generation.
The probe writes only to its own pull requests (`probe/<run>/…`, label `probe`), so a drive
always finds the seeded states. The sandbox, its fork and both logins stay in place for the
later pull request tasks.
