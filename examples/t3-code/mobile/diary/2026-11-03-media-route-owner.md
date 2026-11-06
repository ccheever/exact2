surfaces: ios
agent: Codex gpt-6-astra
task: separate media retry identity from native presentation route ownership

### Rough

Media retry identity includes the read version so a new signed source replaces an old one. The native iOS presenter instead indexes live routes by the Exact route key. Document actions passed the retry identity directly into the share bridge, so a valid share could be rejected after a retry or route transition. HTML-render sharing also omitted the route key entirely.

### Lean in

The app now sends the route ID separately while retaining the versioned media key for JavaScript ownership and cache invalidation. The fallback for direct HTML action callers extracts the visit from its owned context.

### Needed

- media share: provided; document and HTML-render shares carry the native route key
- route lifetime: native presenter still rejects a departed or covered route

### Checkpoints

- focused media/document tests: passed (52 tests, 303 assertions)
- strict TypeScript: passed with the app's ES2024 target
- Contract build: passed
- generated source check: passed

