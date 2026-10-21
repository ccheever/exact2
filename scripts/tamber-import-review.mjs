// An importer's proposal as the wire holds it (LLP 1121 D14, stage 10b):
// its operations, each role override offered on its own, and a re-import's
// dial changes and marked pins, each naming the operations it is. And
// `bun exact.mjs tamber import`, which plans one on this machine and holds it.

import { relative, resolve } from 'node:path';
import { designPath } from './tamber-design-path.mjs';
import { planAlone, sourcesOf } from './tamber-isolate.mjs';

/** An importer's proposal (`contract tamber import`'s JSON) as one held
 * proposal's operations: the tokens, then each role override as its own
 * items, for the review to leave off until ticked (accepting some items
 * recomputes); and the review the tool reads. */
export function importReview(imported) {
  const ops = [...imported.ops];
  const offers = imported.offers.map(o => {
    const items = o.ops.map((op, k) => ops.length + k);
    ops.push(...o.ops);
    return { role: o.role, token: o.token, items, note: o.note };
  });
  // (a choice the source didn't say, and what decided it: a Pencil number's kind, *1.16*)
  return { ops, review: { importer: imported.importer, counted: imported.counted, unmapped: imported.unmapped, offers, tokens: imported.ops.length, ...(imported.notes?.length ? { notes: imported.notes } : {}), ...marked(ops, imported.dials, imported.pins) } };
}

/** A re-import's dial changes and marked pins (stage 10b), when it has any,
 * each naming the operations it is (`items`), as an offer does. */
export function marked(ops, dials = [], pins = []) {
  const same = (a, b) => JSON.stringify(a ?? []) === JSON.stringify(b ?? []), at = test => ops.flatMap((op, i) => (test(op) ? [i] : []));
  dials = dials.map(d => ({ ...d, items: at(op => op.op === 'set-dial' && op.name === d.dial) }));
  pins = pins.map(p => ({ ...p, items: at(op => op.op === 'set-token' && op.name === p.token && same(op.when, p.when)) }));
  return { ...(dials.length ? { dials } : {}), ...(pins.length ? { pins } : {}) };
}

/** `bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>]
 * [--prefix <p>]`: another system's theme (LLP 1121 D14) as held proposals
 * on the design server: its transaction as one, each role override it
 * offers as its own, for a person to accept or decline. Prints the count
 * and the unmapped report. */
export async function importTheme(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const words = argv.filter((a, i) => !a.startsWith('-') && !['--app', '--prefix'].includes(argv[i - 1]));
  const [kind, source, base] = words;
  if (!kind || !source || !base) { console.error('usage: bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>] [--prefix <prefix>]'); return 2; }
  // Planned here, watched as the wire's imports are (scripts/tamber-isolate.mjs); Astryx's theme as resolved on this machine.
  const app = resolve(at('--app') ?? '.');
  const planned = await planAlone({ kind, sources: sourcesOf(resolve(source)), prefix: at('--prefix'), ...(kind === 'astryx' ? { resolved: true } : {}) }, designPath(app) ?? resolve(app, 'app.tamber'), {}, app);
  if (!planned.ok) { console.error(`tamber import: ${planned.error?.message}${planned.error?.diagnostics ? `\n  ${planned.error.diagnostics.map(d => d.message ?? d).join('\n  ')}` : ''}`); return 1; }
  const imported = planned.ok;
  // Held on the design it was planned on (its digest), as the wire's imports are; with no design file, refused.
  if (!imported.digest) { console.error('tamber import: the design file is not there: create it, then import'); return 1; }
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber import: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const client = `import:${kind}`;
  const { source: n, mapped, reported } = imported.counted;
  console.log(`${kind}: ${n} source values, ${mapped} mapped, ${reported} reported`);
  for (const u of imported.unmapped) console.log(`  unmapped ${u.block} ${u.name}: ${u.value || '(empty)'} (${u.why}${u.token ? `; would be ${u.token}` : ''})`);
  for (const note of imported.notes ?? []) console.log(`  note ${note}`);
  const { ops, review } = importReview(imported);
  const { offers } = review;
  const offered = offers.flatMap(o => o.items);
  for (const o of offers) console.log(`  offer ${o.role} → ${o.token}${o.items.length ? `: items ${o.items.join(', ')}` : `: ${o.note}`}`);
  // The source as the design has it already: nothing to propose; a role
  // override declined before is not offered again on its own.
  if (!imported.ops.length) { console.log('nothing to propose: the design has every mapped value already'); return 0; }
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: JSON.stringify({ method: 'propose', hold: true, client, ops, review, ...(imported.digest ? { files: { [relative(app, designPath(app))]: imported.digest } } : {}) }) });
  const held = await res.json();
  if (!held.ok) { console.error(`tamber import: ${held.error?.message}`); return 1; }
  console.log(`proposal ${held.ok.proposal}: ${ops.length} operations (${imported.ops.length} tokens and axes, ${offered.length} offered), held for review`);
  return 0;
}
