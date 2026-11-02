# Agent framework benchmark

[Nolan Lawson's agent framework benchmark](https://github.com/nolanlawson/agent-framework-benchmark),
with Exact added. A Claude Code agent builds the same Pokédex site from a spec, a design and a
test suite, told only which framework to use. The harness records the agent's tokens, cost, time
and turns, runs the tests, measures each page with Lighthouse and counts the app's source lines.
Every agent and app runs in its own container.

| File | |
|---|---|
| `harness.patch` | Commits applied to the upstream harness at `b95ada0` (see below) |
| `setup.sh [dir]` | Clones upstream at that commit into `dir` (default `~/bench-afb/harness`) and applies the patch |
| `run.sh` | One round: five Exact agents at this checkout's `origin/main`, then tests, measurement and `report.md` |
| `bin/podman` | Podman's CLI over Colima's Docker, which the harness calls as `podman` |
| `view-app.sh` | Serves one agent's built app to look at in a browser |
| `tools/` | Reads agent transcripts (below) |
| `results/` | The competitors' round and Exact's latest round as the harness reported them, and every Exact round's medians (`rounds.md`) |

## What the patch changes

- **Exact images.** `Containerfile.exact` adds an exact2 checkout at `/opt/exact2` (a
  `git archive` of `EXACT2_REF` from `EXACT2_DIR`), Bun, the Rust toolchains and a warmed
  build cache. Exact's agent, test and measurement containers start from it. The agent's
  prompt names the framework as "Exact, the framework checked out at /opt/exact2".
  Only committed work is benchmarked.
- **ARM64.** agent-browser drives Playwright's Chromium, since Chrome for Testing has no
  ARM64 Linux build.
- **Subscription credentials.** With `CLAUDE_CREDENTIALS_FILE` set, each agent gets only
  the Claude Code login's current access token, read again before it starts. The refresh
  token never leaves the host: it rotates on use, so a copy in a container would break
  the host's login.
- **Lines of code** count `.contract` files, excluding `.test.contract`. Upstream counted
  only JS, TS, HTML, CSS and component files, so it missed an Exact app's pages.
  `app.tamber` stays uncounted, like every app's copy of `design/tokens.css`, because it
  is that file converted.

## Setting up

Use a Mac with [Colima](https://github.com/abiosoft/colima), the Docker CLI and Node. The
harness expects rootless Podman on Linux, and the `bin/podman` shim stands in for it.

```sh
colima start --profile bench --cpu 10 --memory 24 --disk 200 --vm-type vz --mount-type virtiofs
bench/agent-framework/setup.sh        # → ~/bench-afb/harness
```

## Running a round

```sh
N=5 OUT=exact-agents bench/agent-framework/run.sh               # → ~/bench-afb/runs/exact-agents/<timestamp>/
FRAMEWORKS=astro,next,sveltekit OUT=competitors bench/agent-framework/run.sh
node ~/bench-afb/harness/bench/report.mjs <run dir>             # writes the report again
```

- **Cost.** A round of five Exact agents takes about 75 minutes, of which about 50 are
  the agents. It uses about $17 of model time.
- **Credentials.** Without `ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN`, `run.sh` uses
  the host's Claude Code login. It waits until the access token has more than 110 minutes
  left. Claude Code running on the host refreshes the token.
- **Disk.** Each round builds about 10 GB of images, and `run.sh` prunes the untagged
  ones first. A full VM disk fails the image build with `ENOSPC`.
- **Variance.** Medians move about ±20% from round to round, and one agent in five often
  runs long. Judge a change by what it removes from the transcripts, not by one round's
  median.

## Reading a round

Each agent's directory, `<run dir>/exact-N/`, holds:
- `agent/transcript.jsonl`, the Claude Code session;
- `repo/app`, the app;
- `measure/results.json`, Lighthouse for all six pages (`report.md` shows only the home page).

For one agent's turns, cost and time, use the transcript's first `"type":"result"` line.
Later result lines are follow-up prompts.

| Tool | Prints |
|---|---|
| `tools/calls.mjs <transcripts…>` | Each agent's tool calls by kind (docs read, builds, screenshots, tests) |
| `tools/context.mjs <transcripts…>` | Characters written and read back, by kind of call |
| `tools/growth.mjs <transcript>` | Context per turn and the calls before its largest jumps (`TOP=n`) |
| `tools/diagnostics.mjs <transcripts…>` | Each distinct Contract diagnostic, with the agents that hit it |
| `tools/results.mjs <transcript>` | The largest tool results, with their calls (`TOP=n`) |

`view-app.sh NAME IMAGE APPDIR APPPORT APIPORT` serves one app and its API, for example
`view-app.sh e1 pokedex-base-exact <run dir>/exact-1/repo/app 3101 9101`. Set `HOSTNAME_SEEN`
to the address a browser on another machine uses: the app's server fetches the API at that
address, so images break if it is `localhost`.

## Results

These are medians of five agents. The competitors ran once, on 2026-10-30
([results/competitors.md](results/competitors.md)); its Exact row is from before the work
below. Exact's latest round ran at 3d60499e26
([results/exact-round21.md](results/exact-round21.md)).

| | Tokens (M) | Cost | Time (min) | Turns | Home LCP (ms) | JS (KiB) | Perf | Lines |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Astro | 3.62 | $2.42 | 6.6 | 63 | 805 | 0 | 100 | 1,363 |
| Next.js | 5.31 | $3.22 | 10.0 | 75 | 2,014 | 142 | 99 | 1,591 |
| SvelteKit | 6.73 | $3.65 | 9.3 | 85 | 1,150 | 36 | 100 | 1,580 |
| Exact, 2026-10-30 | 17.04 | $6.67 | 15.2 | 134 | 2,161 | 97 | 95 | — |
| Exact, 3d60499e26 | 6.59 | $3.48 | 8.1 | 86 | 802 | 0 | 100 | 914 |

All five agents passed the tests in every round shown. The report gives Exact 329 lines,
counted before the `.contract` fix. The fixed counter gives 888 to 984 for the five apps,
a median of 914.

Exact now leads or ties on every page-load measure: all six pages score perf 100 and ship
0 KiB of script at load, and LCP is within noise of Astro's. It writes the fewest lines.
It beats SvelteKit on tokens, cost and time and ties it on turns (86 to 85), and beats
Next.js on time. It is behind Next.js on tokens and cost, and behind Astro on all four
agent measures.

Rounds 18 to 21 stayed within 6.45–6.79M tokens and $3.41–3.56. In that time, changes to
exact2's guides and diagnostics moved the medians less than round-to-round noise. Agents
spend more turns learning Exact than Astro, which they already know.
