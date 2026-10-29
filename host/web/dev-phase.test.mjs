// The resident's phase lines (dev-phase.mjs; dtool2's pass 3 #8): a build
// says what it builds and how it went; a noisy build that succeeds is not
// cut off for its output; one that fails says the compiler's own error,
// plain, not the colours or the trace after it.
import { expect, test } from 'bun:test';
import { FAILED, gist, phased } from './dev-phase.mjs';

const said = () => { const lines = []; return { lines, log: (l) => lines.push(l) }; };

test('a build that writes megabytes of diagnostics and succeeds is built', () => {
  const { lines, log } = said();
  const result = phased('a noisy tool', process.execPath, ['-e', "process.stderr.write('w'.repeat(3 << 20)); process.exit(0)"], {}, log, () => {});
  expect(result.status).toBe(0);
  expect(lines[0]).toBe('phase: building a noisy tool');
  expect(lines[1]).toMatch(/^phase: a noisy tool: built in \d+\.\d s$/);
  expect(lines.length).toBe(2);
});

test('a build that fails says its error lines, plain', () => {
  const { lines, log } = said();
  const program = "console.error('\\x1b[1m\\x1b[31merror[E0425]\\x1b[0m: cannot find value `x`'); console.error('error: could not compile `exact-js-bake`'); console.error('    at main (/x.rs:1)'); process.exit(101)";
  const result = phased('the module producer', process.execPath, ['-e', program], {}, log, () => {});
  expect(result.status).toBe(101);
  expect(lines.at(-1)).toBe(`${FAILED}error[E0425]: cannot find value \`x\` · error: could not compile \`exact-js-bake\``);
  expect(gist('first\nsecond\nthird\nfourth')).toBe('second · third · fourth');
});
