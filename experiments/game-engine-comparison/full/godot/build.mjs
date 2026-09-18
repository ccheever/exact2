import { mkdir } from "node:fs/promises";

const godot = process.env.GODOT ??
  "/tmp/exact-engine-comparison-20260917/native/Godot_v4.7.2-stable_linux.x86_64";
await mkdir("dist", { recursive: true });
const proc = Bun.spawn([godot, "--headless", "--path", ".", "--export-release", "Web", "dist/index.html"], {
  stdout: "inherit",
  stderr: "inherit",
});
const code = await proc.exited;
if (code !== 0) process.exit(code);
