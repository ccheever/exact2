# Pokédex framework benchmark

Run `20261102-091742`, started 2026-11-02T09:17:42.640Z.
5 iteration(s) per framework; numbers are medians across iterations.
Lighthouse values are for the home page (`/`): the median of 5 runs, taken per metric, so LCP
and TBT can come from different runs. Each iteration's measure/results.json has all 6 URLs.
Sorted by total tokens.

## Building the app (agent)

Total tokens = cache reads + uncached input (input and cache writes) + output.

| Framework | Tests | Total tokens (M) | Cache read (M) | Uncached input (k) | Output (k) | Cost ($) | Time (min) | Turns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| exact | 5/5 | 6.59 | 6.38 | 162 | 45 | 3.48 | 8.1 | 86 |

## Page loads (Lighthouse, mobile settings)

| Framework | Mobile perf | LCP (ms) | FCP (ms) | TBT (ms) | CLS | JS (KiB) | A11y |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| exact | 100 | 802 | 802 | 0 | 0.000 | 0 | 100 |

## Code

Non-blank lines in the app's source files (comments included). Not counted: JSON, lockfiles,
tests, generated files, and verbatim copies of the provided design files.

| Framework | Lines of code |
| --- | ---: |
| exact | 329 |

## Versions

Direct dependencies of each app, as installed for measurement, and the agent tooling.

- **exact-1**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"3d60499e26cc3e2d7b232e58b9c84fd7de431b64"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-2**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"3d60499e26cc3e2d7b232e58b9c84fd7de431b64"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-3**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"3d60499e26cc3e2d7b232e58b9c84fd7de431b64"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-4**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"3d60499e26cc3e2d7b232e58b9c84fd7de431b64"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-5**: `bun@1.4.2`
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"3d60499e26cc3e2d7b232e58b9c84fd7de431b64"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2

## Iterations

| Iteration | Claude Code | Model | Node | Final tests | Problems |
| --- | --- | --- | --- | --- | --- |
| exact-1 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-2 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-3 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-4 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-5 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
