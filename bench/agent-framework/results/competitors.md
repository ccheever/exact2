# Pokédex framework benchmark

Run `combined`, started 2026-10-30T17:03:23.371Z.
5 iteration(s) per framework; numbers are medians across iterations.
Lighthouse values are for the home page (`/`): the median of 5 runs, taken per metric, so LCP
and TBT can come from different runs. Each iteration's measure/results.json has all 6 URLs.
Sorted by total tokens.

## Building the app (agent)

Total tokens = cache reads + uncached input (input and cache writes) + output.

| Framework | Tests | Total tokens (M) | Cache read (M) | Uncached input (k) | Output (k) | Cost ($) | Time (min) | Turns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| astro | 5/5 | 3.62 | 3.46 | 120 | 40 | 2.42 | 6.6 | 63 |
| next | 5/5 | 5.31 | 5.12 | 137 | 54 | 3.22 | 10.0 | 75 |
| sveltekit | 5/5 | 6.73 | 6.51 | 162 | 52 | 3.65 | 9.3 | 85 |
| exact | 5/5 | 17.04 | 16.73 | 241 | 77 | 6.67 | 15.2 | 134 |

## Page loads (Lighthouse, mobile settings)

| Framework | Mobile perf | LCP (ms) | FCP (ms) | TBT (ms) | CLS | JS (KiB) | A11y |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| astro | 100 | 805 | 802 | 0 | 0.000 | 0 | 100 |
| next | 99 | 2014 | 798 | 51 | 0.000 | 142 | 100 |
| sveltekit | 100 | 1150 | 1057 | 0 | 0.000 | 36 | 100 |
| exact | 95 | 2161 | 1278 | 22 | 0.130 | 97 | 95 |

## Code

Non-blank lines in the app's source files (comments included). Not counted: JSON, lockfiles,
tests, generated files, and verbatim copies of the provided design files.

| Framework | Lines of code |
| --- | ---: |
| astro | 1363 |
| next | 1591 |
| sveltekit | 1580 |
| exact | 464 |

## Versions

Direct dependencies of each app, as installed for measurement, and the agent tooling.

- **astro-1**: `@astrojs/node@11.1.9`, `astro@7.4.4`
  - Tooling: MCP https://mcp.docs.astro.build/mcp (1 tools, sha256 aaaf70d4c3d1)
  - MCP servers in the session: astro-docs:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **astro-2**: `@astrojs/node@11.1.9`, `astro@7.4.4`
  - Tooling: MCP https://mcp.docs.astro.build/mcp (1 tools, sha256 aaaf70d4c3d1)
  - MCP servers in the session: astro-docs:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **astro-3**: `@astrojs/node@11.1.9`, `@astrojs/preact@6.1.1`, `astro@7.4.4`, `preact@11.1.0`
  - Tooling: MCP https://mcp.docs.astro.build/mcp (1 tools, sha256 aaaf70d4c3d1)
  - MCP servers in the session: astro-docs:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **astro-4**: `@astrojs/node@11.1.9`, `astro@7.4.4`
  - Tooling: MCP https://mcp.docs.astro.build/mcp (1 tools, sha256 aaaf70d4c3d1)
  - MCP servers in the session: astro-docs:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **astro-5**: `@astrojs/node@11.1.9`, `astro@7.4.4`
  - Tooling: MCP https://mcp.docs.astro.build/mcp (1 tools, sha256 aaaf70d4c3d1)
  - MCP servers in the session: astro-docs:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-1**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"f726d13124610a5e3a1ed420fafa6ed99262a50f"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-2**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"f726d13124610a5e3a1ed420fafa6ed99262a50f"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-3**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"f726d13124610a5e3a1ed420fafa6ed99262a50f"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-4**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"f726d13124610a5e3a1ed420fafa6ed99262a50f"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **exact-5**: not measured
  - Tooling: {"kind":"checkout","name":"exact2","path":"/opt/exact2","commit":"f726d13124610a5e3a1ed420fafa6ed99262a50f"}
  - MCP servers in the session: none
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **next-1**: `next@16.4.3`, `react@19.3.1`, `react-dom@19.3.1`, `@types/node@26.6.6`, `@types/react@19.3.0`, `@types/react-dom@19.3.0`, `typescript@7.0.2`
  - Tooling: MCP next-devtools-mcp@0.4.0; skill vercel/next.js@7571895 (--skill next-dev-loop)
  - MCP servers in the session: next-devtools:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **next-2**: `next@16.4.3`, `react@19.3.1`, `react-dom@19.3.1`, `@types/node@24.19.2`, `@types/react@19.3.0`, `@types/react-dom@19.3.0`, `typescript@5.9.3`
  - Tooling: MCP next-devtools-mcp@0.4.0; skill vercel/next.js@7571895 (--skill next-dev-loop)
  - MCP servers in the session: next-devtools:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **next-3**: `next@16.4.3`, `react@19.3.1`, `react-dom@19.3.1`, `@types/node@24.19.2`, `@types/react@19.3.0`, `@types/react-dom@19.3.0`, `typescript@5.9.3`
  - Tooling: MCP next-devtools-mcp@0.4.0; skill vercel/next.js@7571895 (--skill next-dev-loop)
  - MCP servers in the session: next-devtools:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **next-4**: `next@16.4.3`, `react@19.3.1`, `react-dom@19.3.1`, `server-only@0.0.1`, `@types/node@26.6.6`, `@types/react@19.3.0`, `@types/react-dom@19.3.0`, `typescript@7.0.2`
  - Tooling: MCP next-devtools-mcp@0.4.0; skill vercel/next.js@7571895 (--skill next-dev-loop)
  - MCP servers in the session: next-devtools:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **next-5**: `next@16.4.3`, `react@19.3.0`, `react-dom@19.3.0`, `@types/node@20.19.43`, `@types/react@19.3.0`, `@types/react-dom@19.3.0`, `typescript@5.9.3`
  - Tooling: MCP next-devtools-mcp@0.4.0; skill vercel/next.js@7571895 (--skill next-dev-loop)
  - MCP servers in the session: next-devtools:connected
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **sveltekit-1**: `@sveltejs/adapter-node@6.0.1`, `@sveltejs/kit@3.1.1`, `@sveltejs/vite-plugin-svelte@7.3.1`, `@types/node@24.19.2`, `svelte@5.57.3`, `svelte-check@4.7.7`, `typescript@6.0.3`, `vite@8.3.5`
  - Tooling: plugin svelte@svelte@75a688c (@sveltejs/mcp@0.1.28)
  - MCP servers in the session: plugin:svelte:svelte:pending
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **sveltekit-2**: `@sveltejs/adapter-node@6.0.1`, `@sveltejs/kit@3.1.1`, `@sveltejs/vite-plugin-svelte@7.3.1`, `@types/node@24.19.2`, `svelte@5.57.3`, `svelte-check@4.7.7`, `typescript@6.0.3`, `vite@8.3.5`
  - Tooling: plugin svelte@svelte@75a688c (@sveltejs/mcp@0.1.28)
  - MCP servers in the session: plugin:svelte:svelte:pending
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **sveltekit-3**: `@sveltejs/adapter-node@6.0.1`, `@sveltejs/kit@3.1.1`, `@sveltejs/vite-plugin-svelte@7.3.1`, `@types/node@24.19.2`, `svelte@5.57.3`, `svelte-check@4.7.7`, `typescript@6.0.3`, `vite@8.3.5`
  - Tooling: plugin svelte@svelte@75a688c (@sveltejs/mcp@0.1.28)
  - MCP servers in the session: plugin:svelte:svelte:pending
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **sveltekit-4**: `@sveltejs/adapter-node@6.0.1`, `@sveltejs/kit@3.1.1`, `@sveltejs/vite-plugin-svelte@7.3.1`, `@types/node@24.19.2`, `svelte@5.57.3`, `svelte-check@4.7.7`, `typescript@6.0.3`, `vite@8.3.5`
  - Tooling: plugin svelte@svelte@75a688c (@sveltejs/mcp@0.1.28)
  - MCP servers in the session: plugin:svelte:svelte:pending
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2
- **sveltekit-5**: `@sveltejs/adapter-node@6.0.1`, `@sveltejs/kit@3.1.1`, `@sveltejs/vite-plugin-svelte@7.3.1`, `@types/node@24.19.2`, `svelte@5.57.3`, `svelte-check@4.7.7`, `typescript@6.0.3`, `vite@8.3.5`
  - Tooling: plugin svelte@svelte@75a688c (@sveltejs/mcp@0.1.28)
  - MCP servers in the session: plugin:svelte:svelte:pending
  - Node v24.22.1, npm 11.19.1, Claude Code 2.1.310 (Claude Code), agent-browser agent-browser 0.39.2

## Iterations

| Iteration | Claude Code | Model | Node | Final tests | Problems |
| --- | --- | --- | --- | --- | --- |
| astro-1 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| astro-2 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| astro-3 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| astro-4 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| astro-5 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-1 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-2 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-3 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-4 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| exact-5 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| next-1 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| next-2 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| next-3 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| next-4 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| next-5 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| sveltekit-1 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| sveltekit-2 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| sveltekit-3 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| sveltekit-4 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
| sveltekit-5 | 2.1.310 (Claude Code) | claude-opus-5-5 (high) | v24.22.1 | pass | — |
