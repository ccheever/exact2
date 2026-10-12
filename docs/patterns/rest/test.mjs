// `bun exact.mjs api-test web` (app.json commands): the tests on the app's own server, run where app.ts
// calls it on an empty database and stopped after, as `verify` runs it (exact2's scripts/verify.mjs).
if (!process.env.EXACT2) throw new Error('run it as bun exact.mjs api-test web');
const { withAppServer } = await import(`${process.env.EXACT2}/scripts/verify.mjs`);
process.exit(await withAppServer(import.meta.dir, () =>
  Bun.spawn(['bun', 'exact.mjs', 'test', ...process.argv.slice(2)], { cwd: import.meta.dir, stdio: ['inherit', 'inherit', 'inherit'] }).exited));
