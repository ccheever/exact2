surfaces: ios
agent: Codex gpt-6.1-sol
task: contain route and thread-feed horizontal overflow

### Rough

The thread feed had no horizontal overflow policy, and each route could grow past
the settled workspace width when a child exposed an unbreakable value. That made
long links or a fast fling expose a sideways scroll surface.

### Lean in

The route wrapper now owns a shrinkable, bounded inline size and clips incidental
horizontal overflow. The thread feed applies the same rule to its virtualized
scrollport. Markdown code and table blocks keep their nested horizontal scroll
surfaces.

### Needed

A clean Duo drive should confirm that long thread content cannot move the route
sideways and that nested code blocks still scroll horizontally.

### Checkpoints

The focused scroll contract test passes. The broader layout and history run is
blocked by a pre-existing missing generated `settings-b-lucide` module.
