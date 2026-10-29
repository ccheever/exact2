// An importer's proposal as the wire holds it (LLP 1121 D14, stage 10b):
// its operations, each role override offered on its own, and a re-import's
// dial changes and marked pins, each naming the operations it is. And
// `bun exact.mjs tamber import`, which plans one on this machine and holds it.

import { realpathSync } from 'node:fs';
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
  // The accent as the packages' brand fills (*1.18*), after the role overrides: a list of its own, so a tool that
  // reads only `offers` never shows one, and its accept, naming the items it keeps, leaves them off.
  const brands = (imported.brands ?? []).map(b => {
    const items = b.ops.map((op, k) => ops.length + k);
    ops.push(...b.ops);
    return { sets: b.sets, token: b.token, items, ...(b.note ? { note: b.note } : {}), contrast: b.contrast };
  });
  // (a choice the source didn't say, and what decided it: a Pencil number's kind, *1.17*)
  return { ops, review: { importer: imported.importer, counted: imported.counted, unmapped: imported.unmapped, offers, ...(brands.length ? { brands } : {}), tokens: imported.ops.length, ...(imported.notes?.length ? { notes: imported.notes } : {}), ...marked(ops, imported.dials, imported.pins) } };
}

/** A re-import's dial changes and marked pins (stage 10b), when it has any,
 * each naming the operations it is (`items`), as an offer does. */
export function marked(ops, dials = [], pins = []) {
  // Each operation's place by what it sets, found once (round 3 of the checker sweep: a scan a pin).
  const places = new Map();
  ops.forEach((op, i) => {
    const key = op.op === 'set-dial' ? JSON.stringify(['dial', op.name]) : op.op === 'set-token' ? JSON.stringify(['token', op.name, op.when ?? []]) : null;
    if (key !== null) (places.get(key) ?? places.set(key, []).get(key)).push(i);
  });
  const at = key => places.get(key) ?? [];
  dials = dials.map(d => ({ ...d, items: at(JSON.stringify(['dial', d.dial])) }));
  pins = pins.map(p => ({ ...p, items: at(JSON.stringify(['token', p.token, p.when ?? []])) }));
  return { ...(dials.length ? { dials } : {}), ...(pins.length ? { pins } : {}) };
}

/** `bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>]
 * [--prefix <p>]`: another system's theme (LLP 1121 D14) as held proposals
 * on the design server: its transaction as one, each role override it
 * offers as its own, for a person to accept or decline. Prints the count
 * and the unmapped report. The server plans and holds it (`import`, as the
 * design tool and the Figma plugin ask), so no client sends an import's
 * operations (tamber/1.17's `tamber-ops-size`); Astryx's theme, which a Bun
 * install on this machine resolves and the wire never forwards, is planned
 * here and held. */
export async function importTheme(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const words = argv.filter((a, i) => !a.startsWith('-') && !['--app', '--prefix'].includes(argv[i - 1]));
  const [kind, source, base] = words;
  if (!kind || !source || !base) { console.error('usage: bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>] [--prefix <prefix>]'); return 2; }
  const app = resolve(at('--app') ?? '.');
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber import: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const client = `import:${kind}`;
  const post = body => fetch(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: JSON.stringify(body) }).then(r => r.json());
  const fail = error => { console.error(`tamber import: ${error?.message}${error?.diagnostics ? `\n  ${error.diagnostics.map(d => d.message ?? d).join('\n  ')}` : ''}`); return 1; };
  /** The count, the unmapped report, the notes and the offers, as the review says them. */
  const say = review => {
    const { source: n, mapped, reported } = review.counted;
    console.log(`${kind}: ${n} source values, ${mapped} mapped, ${reported} reported`);
    for (const u of review.unmapped) console.log(`  unmapped ${u.block} ${u.name}: ${u.value || '(empty)'} (${u.why}${u.token ? `; would be ${u.token}` : ''})`);
    for (const note of review.notes ?? []) console.log(`  note ${note}`);
    for (const o of review.offers) console.log(`  offer ${o.role} → ${o.token}${o.items.length ? `: items ${o.items.join(', ')}` : `: ${o.note}`}`);
    for (const b of review.brands ?? []) console.log(`  brand ${b.sets} → ${b.token}: items ${b.items.join(', ')} (its label ${b.contrast.light ?? 'no one ratio'}:1 light, ${b.contrast.dark ?? 'no one ratio'}:1 dark)${b.note ? `: ${b.note}` : ''}`);
  };
  const told = (proposal, ops, review) => console.log(`proposal ${proposal}: ${ops.length} operations (${review.tokens} tokens and axes, ${[...review.offers, ...(review.brands ?? [])].flatMap(o => o.items).length} offered), held for review`);
  let sources;
  try { sources = sourcesOf(resolve(source)); } catch (e) { console.error(`tamber import: ${e.message}`); return 1; }
  if (kind !== 'astryx') {
    // The server plans on its own app: one `--app` names elsewhere is refused (Grok's round 1).
    const hello = await post({ method: 'hello' });
    const served = hello.ok?.app && realpathSync(resolve(hello.ok.app));
    if (served !== realpathSync(app)) { console.error(`tamber import: ${origin} serves ${served ?? 'an app it does not name'}, not ${app}`); return 1; }
    const held = await post({ method: 'import', kind, client, sources, ...(at('--prefix') !== undefined ? { prefix: at('--prefix') } : {}) });
    if (!held.ok) return fail(held.error);
    say(held.ok.review);
    if (!held.ok.proposal) { console.log(held.ok.message); return 0; }
    told(held.ok.proposal, held.ok.ops, held.ok.review);
    return 0;
  }
  // Astryx: planned here, watched as the wire's imports are (scripts/tamber-isolate.mjs), its theme as resolved on this machine.
  const planned = await planAlone({ kind, sources, prefix: at('--prefix'), resolved: true }, designPath(app) ?? resolve(app, 'app.tamber'), {}, app);
  if (!planned.ok) return fail(planned.error);
  const imported = planned.ok;
  // Held on the design it was planned on (its digest), as the wire's imports are; with no design file, refused.
  if (!imported.digest) { console.error('tamber import: the design file is not there: create it, then import'); return 1; }
  const { ops, review } = importReview(imported);
  say(review);
  // The source as the design has it already: nothing to propose.
  // (A re-import whose only news is a brand offer, *1.18*, is held for it, as the wire holds one.)
  if (!imported.ops.length && !review.brands?.length) { console.log('nothing to propose: the design has every mapped value already'); return 0; }
  const held = await post({ method: 'propose', hold: true, client, ops, review, files: { [relative(app, designPath(app))]: imported.digest } });
  if (!held.ok) return fail(held.error);
  told(held.ok.proposal, ops, review);
  return 0;
}
