# Grow a Garden

A small 3D farming garden. Walk with WASD or the arrows, plant and harvest with E, water with Q, refill at the blue barrel with R, and feed with F.

```bash
bun install
bunx playwright install chromium
bun run proof
```

`bun run proof` is the whole check: the rules self-check, then a headless Chromium play-through (movement, growth, watering, save and restore in a second browser, an offline hour, frame time at about 100 and about 1,000 plants, and screenshots in `artifacts/`).

`bun run start` serves the game at a local port and prints the URL.
