// `bun exact.mjs verify`'s `contract verify` (LLP 1151 D8): started beside the device run, its report kept in a file,
// and the part of it the console shows.
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// `contract verify` (LLP 1151 D8) reads only the app's Contract and its tests, so it runs beside the device run rather
// than after it. Its report goes whole to .exact/verify/properties.txt. The console has a line on it, any line saying
// what failed, the receipt an author keeps (D7), and then the device report's last line: printed whole after that
// report, it pushed the verdict out of a builder's `verify | tail`, and the builder ran verify again to find it.
export function contractVerify(app, json) {
  let output = '', errors = '';
  // Its own process group, so that stopping it stops the tester `cargo run` starts under it too.
  const child = spawn('cargo', ['run', '-q', '-p', 'contract', '--', 'verify', resolve(app.dir, 'app.contract')], { cwd: ROOT, detached: true, stdio: ['ignore', 'pipe', json ? 'inherit' : 'pipe'] });
  const pass = signal => { try { process.kill(-child.pid, signal); } catch { /* It has exited. */ } }, signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  for (const signal of signals) process.on(signal, pass);
  child.stdout.on('data', data => { output += data; });
  child.stderr?.on('data', data => { errors += data; });
  const done = new Promise(resolveResult => {
    const finish = exit => { for (const signal of signals) process.off(signal, pass); resolveResult({ exit, output, errors }); };
    child.on('error', e => { errors += e.message; finish(1); });
    child.on('exit', exit => finish(exit ?? 1));
  });
  return { done, stop: () => pass('SIGTERM') };
}

// Lines of `contract verify`'s report that are routine: one per delivered or outside script, what the oracle cannot
// deliver, and the counts the receipt repeats.
const ROUTINE = /^(WHOLE|PART|NOTE|OUTSIDE) |^(undeliverable parser step kinds|delivered forms|runner|properties|commit kinds the trace leaves out|world): /;

/** The console's part of a `contract verify` report kept whole in `file`. */
export function semanticsSummary({ exit, output, errors }, file) {
  const lines = output.split('\n');
  const receiptAt = lines.findIndex(l => l.startsWith('verify receipt'));
  const observedAt = lines.findIndex(l => l.startsWith('Observed over'));
  // The receipt without the suggestions each dropped or widened, a line each, which stay in the file.
  const receipt = receiptAt >= 0 ? lines.slice(receiptAt).filter(l => l.trim() && !/^ {4}(dropped|widened) /.test(l)) : [];
  const suggested = observedAt >= 0 ? lines.slice(observedAt, receiptAt >= 0 ? receiptAt : undefined).filter(l => /^(always|after|never) /.test(l)).length : 0;
  const head = `contract verify: ${exit ? `failed (exit ${exit})` : 'done'}${suggested ? `; ${plural(suggested, 'suggested property', 'suggested properties')} (observed, not proved: keep the ones the app means)` : ''}; every line: ${file}`;
  if (!exit) return [head, ...receipt];
  const said = [...lines.slice(0, observedAt >= 0 ? observedAt : receiptAt >= 0 ? receiptAt : undefined), ...errors.split('\n')].filter(l => l.trim() && !ROUTINE.test(l));
  const shown = said.slice(0, 40);
  if (said.length > shown.length) shown.push(`  … and ${said.length - shown.length} more lines (${file})`);
  return [head, ...shown, ...receipt];
}
