// `bun exact.mjs api-test web` (app.json commands): every test run gets its own server and an
// empty database, so a test never meets rows the dev server or another run left.
const up = () => fetch('http://127.0.0.1:8787/health').then((r) => r.ok, () => false);
if (await up()) throw new Error('port 8787 is in use: stop the dev server (bun server.ts) first, so the tests get an empty database');
const server = Bun.spawn(['bun', 'server.ts'], { cwd: import.meta.dir, env: { ...process.env, DB: ':memory:' }, stdio: ['ignore', 'ignore', 'inherit'] });
for (let i = 0; !(await up()); i++) { if (i > 50) throw new Error('server.ts did not start'); await Bun.sleep(100); }
const tests = Bun.spawn(['bun', 'exact.mjs', 'test', ...process.argv.slice(2)], { cwd: import.meta.dir, stdio: ['inherit', 'inherit', 'inherit'] });
const code = await tests.exited;
server.kill();
process.exit(code);
