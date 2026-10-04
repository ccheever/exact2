// The paint-order fixture on a real host (LLP 1083.000 D6). Every case of
// kernel/tests/it/fixtures/browser_paint_order.tsv (Chrome-measured, bare
// and with the kernel's isolation) becomes a plan of plain `view`s, each an
// opaque colour of its own; the host is driven to a screenshot, and the
// colour at each probe must be the case's Exact answer (its sixth field).
// The root sits in a yellow frame with 8 points of padding, found by its
// colour, so no host's window chrome matters and its padding gives the scale.
//
//   bun host/web/tests/paint-order/drive.mjs <web|linux|macos|ios> [name-substring]
//
// Exits 1 on any mismatch, after reporting every one.
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { decodePng } from '../../../../scripts/png.mjs';

const root = resolve(new URL('.', import.meta.url).pathname, '../../../..');
const [host = 'web', only] = process.argv.slice(2);
const fixture = resolve(root, 'kernel/tests/it/fixtures/browser_paint_order.tsv');
const work = mkdtempSync(resolve(tmpdir(), 'exact-paint-order-drive-'));

// Twelve colours far apart, so a host's colour management cannot confuse two.
const PALETTE = ['#ffffff', '#e6194b', '#3cb44b', '#4363d8', '#f58231', '#911eb4', '#42d4f4', '#f032e6', '#bfef45', '#469990', '#9a6324', '#000075'];
const rgb = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
const colour = id => PALETTE[id % PALETTE.length];
const FRAME = '#ffe119', PAD = 8;

// CSS declarations as Contract attributes: the names are CSS's (LLP 1081).
function attrs(css) {
  return css.split(';').filter(Boolean).map(decl => {
    const [name, value] = decl.split(':');
    const n = value.endsWith('px') ? value.slice(0, -2) : value;
    return /^-?[\d.]+$/.test(n) ? `${name}=${n}` : `${name}=${JSON.stringify(value)}`;
  }).join(' ');
}

function contract(name, rootCss, nodes) {
  const children = new Map();
  for (const [id, parent, css] of nodes) children.set(parent, [...(children.get(parent) ?? []), [id, css]]);
  const lines = [`// ${name}`, 'component Case', '  view', `    view testId="frame" background-color="${FRAME}" padding=${PAD}`];
  const emit = (id, css, depth) => {
    lines.push(`${'  '.repeat(depth)}view testId="n${id}" background-color="${colour(id)}" ${attrs(css)}`);
    for (const [c, ccss] of children.get(id) ?? []) emit(c, ccss, depth + 1);
  };
  emit(1, rootCss, 3);
  return lines.join('\n') + '\n';
}

const cases = readFileSync(fixture, 'utf8').split('\n').filter(Boolean).map(l => l.split('\t'))
  .filter(f => !only || f[0].includes(only));
const failures = [];
let i = 0;
for (const [name, rootCss, nodesField, probesField, , exactField] of cases) {
  const nodes = nodesField.split(' & ').map(n => { const [id, parent, ...css] = n.split('>'); return [Number(id), Number(parent), css.join('>')]; });
  const probes = probesField.split(' ').map(p => p.split(',').map(Number));
  const exact = exactField.split(' ').map(Number);
  const src = resolve(work, `case${i}.contract`), plan = resolve(work, `case${i}.plan`), shot = resolve(work, `case${i}.png`);
  i++;
  writeFileSync(src, contract(name, rootCss, nodes));
  const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', src, '-o', plan], { cwd: root, encoding: 'utf8' });
  if (built.status !== 0) { failures.push(`${name}: the plan did not compile: ${built.stderr.trim().split('\n').pop()}`); continue; }
  const shotOp = host === 'macos' ? `screenshot ${shot} window` : `screenshot ${shot}`;
  const drove = spawnSync('bun', ['scripts/agent.mjs', host, '--plan', plan, 'clock settle', shotOp], { cwd: root, encoding: 'utf8', env: process.env });
  // A host that cannot run at all fails every case the same way: say it once.
  if (drove.status !== 0) { console.log(`${name}: the ${host} drive failed: ${(drove.stderr || drove.stdout).trim().split('\n').slice(-2).join(' ')}`); process.exit(1); }
  const png = decodePng(readFileSync(shot));
  const at = (x, y) => { const o = (y * png.width + x) * 4; return [png.data[o], png.data[o + 1], png.data[o + 2]]; };
  const near = (a, b) => a.every((v, k) => Math.abs(v - b[k]) <= 40);
  // The frame's top-left corner, then its padding's thickness: the scale.
  const yellow = rgb(FRAME);
  let frame = null;
  for (let y = 0; y < png.height && !frame; y++) for (let x = 0; x < png.width; x++) if (near(at(x, y), yellow)) { frame = [x, y]; break; }
  if (!frame) { failures.push(`${name}: no frame colour in the ${host} screenshot`); continue; }
  let thick = 0;
  // Along the diagonal, the frame ends where the root begins: PAD × scale.
  while (frame[1] + thick < png.height && near(at(frame[0] + thick, frame[1] + thick), yellow)) thick++;
  const scale = Math.round(thick / PAD * 2) / 2 || 1;
  const origin = [frame[0] + PAD * scale, frame[1] + PAD * scale];
  if (process.env.KEEP) console.log(name, 'frame', frame, 'scale', scale);
  probes.forEach(([x, y], k) => {
    const px = at(Math.round(origin[0] + x * scale), Math.round(origin[1] + y * scale));
    const id = PALETTE.findIndex(c => near(px, rgb(c)));
    const got = nodes.map(n => n[0]).concat(1).find(n => colour(n) === PALETTE[id]);
    if (got !== exact[k]) failures.push(`${name} at ${x},${y}: ${host} paints ${got ?? `rgb(${px})`}, Exact is ${exact[k]}`);
  });
}
if (!process.env.KEEP) rmSync(work, { recursive: true, force: true }); else console.log(work);
if (failures.length) { console.log(failures.join('\n')); console.log(`${failures.length} mismatches on ${host}`); process.exit(1); }
console.log(`${cases.length} cases agree on ${host}`);
