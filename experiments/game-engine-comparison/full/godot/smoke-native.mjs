const godot = process.env.GODOT ??
  "/tmp/exact-engine-comparison-20260917/native/Godot_v4.7.2-stable_linux.x86_64";
const proc = Bun.spawn([godot, "--headless", "--path", ".", "--", "--self-test"], {
  stdout: "pipe",
  stderr: "inherit",
});
const output = await new Response(proc.stdout).text();
const code = await proc.exited;
process.stdout.write(output);
if (code !== 0 || !output.includes("NATIVE_SMOKE_PASS")) process.exit(1);
