import { cp, mkdir, rm } from "node:fs/promises";

await rm("dist", { recursive: true, force: true });
await mkdir("dist/assets", { recursive: true });
await Promise.all([
  cp("index.html", "dist/index.html"),
  cp("style.css", "dist/style.css"),
  cp("level.json", "dist/level.json"),
  cp("assets/Fox.glb", "dist/assets/Fox.glb"),
  cp("assets/Fox-LICENSE.md", "dist/assets/Fox-LICENSE.md"),
  cp("node_modules/@babylonjs/havok/lib/esm/HavokPhysics.wasm", "dist/HavokPhysics.wasm"),
]);
