import { mkdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
export const cache = join(process.env.HOME, 'Library/Caches/exact2-cluster-lod/out/web');
export const root = resolve(import.meta.dir, '..');
export async function run(args, opts = {}) {
  const p = Bun.spawn(args, { cwd: root, stdout: 'inherit', stderr: 'inherit', ...opts });
  console.log(JSON.stringify({ command: args, pid: p.pid }));
  const code = await p.exited;
  if (code) throw new Error(`${args[0]} exited ${code}`);
}
if (import.meta.main) {
  await mkdir(join(cache, 'pkg'), { recursive: true });
  const failures = [];
  for (const command of [
    ['cargo', 'build', '-p', 'clod-web', '--release', '--target', 'wasm32-unknown-unknown'],
    ['cargo', 'build', '-p', 'clod-view', '--bin', 'clod-view', '--example', 'web_prepare'],
  ]) { try { await run(command); } catch (e) { failures.push(String(e)); } }
  if (!failures.length) {
    try {
      await run([join(process.env.HOME, '.cargo/bin/wasm-bindgen'), 'target/wasm32-unknown-unknown/release/clod_web.wasm', '--target', 'web', '--out-dir', join(cache, 'pkg'), '--out-name', 'clod_web']);
      const file = join(cache, 'pkg/clod_web_bg.wasm');
      const raw = await Bun.file(file).arrayBuffer();
      await Bun.write(join(cache, 'clod_web_raw.wasm'), raw);
      await run(['/opt/homebrew/bin/wasm-opt', '-O3', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', file, '-o', join(cache, 'optimized.wasm')]);
      await Bun.write(file, Bun.file(join(cache, 'optimized.wasm')));
      const optimized = await Bun.file(file).arrayBuffer();
      const numbers = { raw: raw.byteLength, optimized: optimized.byteLength, gzip: Bun.gzipSync(new Uint8Array(optimized)).byteLength };
      await Bun.write(join(cache, 'sizes.json'), JSON.stringify(numbers));
      console.log(JSON.stringify({ wasm_bytes: numbers }));
    } catch (e) { failures.push(String(e)); }
  }
  console.log(JSON.stringify({ failures }));
  process.exitCode = failures.length ? 1 : 0;
}
