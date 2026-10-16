// Tamber's DTCG export (LLP 1121 D14, stage 5 commit 6) against DTCG 2025.10's
// own JSON Schemas, pinned in tamber/tests/dtcg/: the theme fixture's design
// and the Bluesky clone's (tamber/tests/corpus/bluesky.tamber). The validator
// below is the part of JSON Schema draft-07 those schemas use; it is checked
// on a document each schema refuses too. The export's round trip is
// tamber/tests/it/export.rs.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, copyFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '..');
const schema = name => JSON.parse(readFileSync(resolve(ROOT, 'tamber/tests/dtcg', name), 'utf8'));

/** Every error of `value` against draft-07 `root`, by JSON pointer. */
export function validate(root, value) {
  const ids = new Map();
  (function index(node) {
    if (!node || typeof node !== 'object') return;
    if (typeof node.$id === 'string') ids.set(node.$id.replace(/#$/, ''), node);
    for (const v of Object.values(node)) index(v);
  })(root);
  const pointer = (node, frag) => frag.split('/').slice(1).map(s => decodeURIComponent(s).replaceAll('~1', '/').replaceAll('~0', '~')).reduce((n, k) => n?.[k], node);
  const resolveRef = (ref, base) => {
    const [uri, frag = ''] = ref.split('#');
    const id = uri ? new URL(uri, base || undefined).href : base;
    const doc = ids.get(id);
    if (!doc) throw new Error(`unresolved $ref ${ref} from ${base}`);
    return { node: frag ? pointer(doc, frag) : doc, base: id };
  };
  const type = v => (v === null ? 'null' : Array.isArray(v) ? 'array' : Number.isInteger(v) ? 'integer' : typeof v);
  const is = (v, t) => t === type(v) || (t === 'number' && typeof v === 'number');
  const errors = [];
  function check(s, v, at, base, out) {
    if (s === true || s === undefined) return;
    if (s === false) return out.push(`${at}: false schema`);
    if (s.$id) base = s.$id.replace(/#$/, '');
    if (s.$ref) { const r = resolveRef(s.$ref, base); return check(r.node, v, at, r.base, out); }
    if (s.type && ![s.type].flat().some(t => is(v, t))) return out.push(`${at}: not ${s.type}`);
    if (s.enum && !s.enum.some(e => JSON.stringify(e) === JSON.stringify(v))) out.push(`${at}: not one of ${JSON.stringify(s.enum)}`);
    if ('const' in s && JSON.stringify(s.const) !== JSON.stringify(v)) out.push(`${at}: not ${JSON.stringify(s.const)}`);
    if (typeof v === 'number') {
      if (s.minimum !== undefined && v < s.minimum) out.push(`${at}: below ${s.minimum}`);
      if (s.maximum !== undefined && v > s.maximum) out.push(`${at}: above ${s.maximum}`);
      if (s.exclusiveMaximum !== undefined && v >= s.exclusiveMaximum) out.push(`${at}: not below ${s.exclusiveMaximum}`);
    }
    if (typeof v === 'string' && s.pattern && !new RegExp(s.pattern, 'u').test(v)) out.push(`${at}: not /${s.pattern}/`);
    if (Array.isArray(v)) {
      if (s.minItems !== undefined && v.length < s.minItems) out.push(`${at}: fewer than ${s.minItems} items`);
      if (s.maxItems !== undefined && v.length > s.maxItems) out.push(`${at}: more than ${s.maxItems} items`);
      if (Array.isArray(s.items)) v.forEach((x, i) => check(i < s.items.length ? s.items[i] : s.additionalItems, x, `${at}/${i}`, base, out));
      else if (s.items) v.forEach((x, i) => check(s.items, x, `${at}/${i}`, base, out));
    }
    if (type(v) === 'object') {
      if (s.minProperties !== undefined && Object.keys(v).length < s.minProperties) out.push(`${at}: fewer than ${s.minProperties} properties`);
      for (const r of s.required ?? []) if (!(r in v)) out.push(`${at}: no ${r}`);
      for (const [k, x] of Object.entries(v)) {
        let matched = false;
        if (s.properties && k in s.properties) { matched = true; check(s.properties[k], x, `${at}/${k}`, base, out); }
        for (const [p, ps] of Object.entries(s.patternProperties ?? {})) if (new RegExp(p, 'u').test(k)) { matched = true; check(ps, x, `${at}/${k}`, base, out); }
        if (!matched && s.additionalProperties !== undefined) check(s.additionalProperties, x, `${at}/${k}`, base, out);
      }
    }
    const passes = sub => { const o = []; check(sub, v, at, base, o); return o.length === 0; };
    for (const sub of s.allOf ?? []) check(sub, v, at, base, out);
    if (s.anyOf && !s.anyOf.some(passes)) out.push(`${at}: none of anyOf`);
    if (s.oneOf) { const n = s.oneOf.filter(passes).length; if (n !== 1) out.push(`${at}: ${n} of oneOf`); }
    if (s.not && passes(s.not)) out.push(`${at}: matches not`);
    if (s.if !== undefined) { if (passes(s.if)) { if (s.then !== undefined) check(s.then, v, at, base, out); } else if (s.else !== undefined) check(s.else, v, at, base, out); }
  }
  check(root, value, '', root.$id, errors);
  return errors;
}

function exported(design) {
  const dir = mkdtempSync(resolve(tmpdir(), 'tamber-dtcg-'));
  try {
    copyFileSync(design, resolve(dir, 'app.tamber'));
    const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'export', 'dtcg', resolve(dir, 'app.tamber')], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20 });
    if (r.status !== 0) throw new Error(r.stderr);
    // Each export keeps its baseline beside the design, ignored by its own file.
    expect(readFileSync(resolve(dir, '.exact/.gitignore'), 'utf8')).toBe('*\n');
    expect(readFileSync(resolve(dir, '.exact/tamber-dtcg.json'), 'utf8')).toBe(r.stdout);
    return JSON.parse(r.stdout);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('the validator refuses what the schemas refuse', () => {
  const resolver = schema('resolver.json');
  expect(validate(resolver, { version: '2025.10', resolutionOrder: [{ $ref: '#/sets/a' }] })).toEqual([]);
  expect(validate(resolver, { version: '2025.10', resolutionOrder: [], $extensions: {} }).length).toBeGreaterThan(0);
  const token = c => ({ version: '2025.10', sets: { a: { sources: [{ t: { $type: 'color', $value: c } }] } }, resolutionOrder: [{ $ref: '#/sets/a' }] });
  expect(validate(resolver, token({ colorSpace: 'srgb', components: [0, 0.5, 1], hex: '#0080ff' }))).toEqual([]);
  expect(validate(resolver, token({ colorSpace: 'srgb', components: [0, 0.5, 1.2] })).length).toBeGreaterThan(0);
  expect(validate(resolver, token({ colorSpace: 'srgb', components: [0, 0.5, 1], hex: '#0080ff80' })).length).toBeGreaterThan(0);
  expect(validate(schema('format.json'), { 'a.b': { $type: 'color', $value: '#fff' } }).length).toBeGreaterThan(0);
});

test('the export of the theme fixture and of the clone validates against DTCG 2025.10', () => {
  const resolver = schema('resolver.json'), format = schema('format.json');
  for (const design of ['apps/theme-fixture/app.tamber', 'tamber/tests/corpus/bluesky.tamber']) {
    const doc = exported(resolve(ROOT, design));
    expect(validate(resolver, doc), design).toEqual([]);
    // Each context's token document is a DTCG Format document on its own.
    for (const [name, sources] of Object.entries(doc.modifiers.mode.contexts)) {
      for (const src of sources) expect(validate(format, src), `${design} ${name}`).toEqual([]);
    }
    for (const src of doc.sets.exact.sources) expect(validate(format, src), design).toEqual([]);
  }
}, 600_000);
