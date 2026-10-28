// Lights' data module (LLP 1133.036 §10): each light listed by DNS-SD is
// resolved (`resolveService`) and read over WLED's JSON API, and a typed
// address is read the same way without the resolve. A light seen this
// launch whose instance leaves the network stays listed as "Not on this
// network", known by the MAC address WLED states.
import type { Answer, Args, Sources, Result } from './app.contract.d.ts';
import { resolveService } from '@exact/local-network';

export const appId = 'com.exact.lights';
export const grants = 'device.local-network lanPurpose\nnet.dns-sd _wled._tcp\nnet.fetch local-network';

type Light = Result<'lights'>['items'][number];
const TYPE = '_wled._tcp';
/** Lights seen this launch, by MAC. */
const seen = new Map<string, Light>();

/** Where a light is: a typed address as written, else its resolved host and port. */
async function where(id: string): Promise<string> {
  if (id.startsWith('addr:')) {
    const typed = id.slice(5);
    return /^https?:\/\//.test(typed) ? typed.replace(/\/+$/, '') : `http://${typed}`;
  }
  const s = await resolveService({ name: id.slice(4), type: TYPE });
  const host = s.host.includes(':') && !s.host.startsWith('[') ? `[${s.host}]` : s.host;
  return `http://${host}:${s.port}`;
}

/** A light's name as a test id: lower case, a hyphen for each run of other characters. */
const slug = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, '-');

async function read(id: string, name: string): Promise<Light> {
  try {
    const r = await fetch(`${await where(id)}/json/si`);
    if (!r.ok) throw new Error(`WLED answered ${r.status}`);
    const j = await r.json() as { info?: { name?: string; mac?: string }; state?: { on?: boolean; bri?: number } };
    const named = j.info?.name || name;
    return { id, name: named, on: !!j.state?.on, bri: j.state?.bri ?? 128, mac: j.info?.mac ?? '', slug: slug(named), status: '' };
  } catch {
    return { id, name, on: false, bri: 0, mac: '', slug: slug(name), status: 'Not responding. Try again.' };
  }
}

async function lights(found: Args<'lights'>[0], added: string): Promise<Result<'lights'>> {
  const wanted = found.services.map(s => ({ id: `svc:${s.name}`, name: s.name }));
  if (added) wanted.push({ id: `addr:${added}`, name: added });
  const items = await Promise.all(wanted.map(w => read(w.id, w.name)));
  for (const l of items) if (l.mac) seen.set(l.mac, l);
  const here = new Set(items.map(l => l.mac));
  const gone = [...seen.values()].filter(l => !here.has(l.mac) && !items.some(i => i.name === l.name)).map(l => ({ ...l, id: `gone:${l.mac}`, status: 'Not on this network' }));
  return { items: [...items, ...gone] };
}

async function setLight(id: string, on: boolean, bri: number): Promise<Result<'setLight'>> {
  try {
    const r = await fetch(`${await where(id)}/json/state`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ on, bri: Math.round(bri) }),
    });
    return { ok: r.ok };
  } catch {
    return { ok: false };
  }
}

const sources: Sources = {
  lights: ([found, added]) => lights(found, added),
  setLight: ([id, on, bri]) => setLight(id, on, bri),
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
