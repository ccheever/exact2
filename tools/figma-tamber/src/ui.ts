// The Figma plugin's UI (LLP 1121 §5 stage 10b, rt1's plan E4): it pairs with
// a Tamber design server (a code from `bun exact.mjs tamber pair`, the server
// run with --allow-plugin-origin), keeps the paired token in the plugin's
// client storage (this user, this machine; never the file), and holds every
// request: a pull (export) and a push (import, held as a proposal for a person
// to accept; the plugin never writes the design).

const $ = (id: string) => document.getElementById(id) as HTMLInputElement;
const post = (msg: object) => parent.postMessage({ pluginMessage: msg }, '*');
let server = '', token: string | null = null;

const show = (text: string) => { $('status').textContent = text; };
const call = async (path: string, body: object, withToken = true) => {
  const res = await fetch(`${server}${path}`, { method: 'POST', headers: { 'content-type': 'application/json', ...(withToken && token ? { 'x-exact-tamber-token': token } : {}) }, body: JSON.stringify(body) });
  const reply = await res.json().catch(() => ({}));
  if (res.status === 401) { token = null; post({ type: 'save', server, token }); }
  if (!res.ok) throw new Error(reply.error?.message ?? reply.message ?? `${res.status}`);
  return reply.ok ?? reply;
};

/** Messages from the main thread. */
const waiting = new Map<string, (msg: any) => void>();
const next = (type: string) => new Promise<any>(ok => waiting.set(type, ok));
onmessage = (e: MessageEvent) => {
  const msg = e.data.pluginMessage;
  if (msg.type === 'error') { show(msg.message); return; }
  waiting.get(msg.type)?.(msg);
  waiting.delete(msg.type);
};

async function pair() {
  server = $('server').value.replace(/\/+$/, '');
  const got = await call('/__tamber/pair', { code: $('code').value.trim(), name: 'figma' }, false);
  token = got.token;
  post({ type: 'save', server, token });
  await next('saved');
  show(`paired as ${got.client}, ${got.scope}, until ${got.expires}`);
}

async function pullDesign() {
  const { text } = await call('/__tamber', { method: 'export', kind: 'dtcg', client: 'figma' });
  post({ type: 'check', text });
  const { conflicts } = await next('checked');
  if (conflicts.length && !confirm(`${conflicts.length} value(s) changed in Figma and in the design since the last pull:\n${conflicts.map((c: any) => `${c.variable} (${c.mode})`).join('\n')}\n\nOK takes the design's; Cancel keeps Figma's (push them first).`)) { show('kept Figma\'s: push them, then pull'); return; }
  post({ type: 'apply', text });
  const done = await next('applied');
  show(`pulled into ${done.collections.join(', ')}${done.skipped.length ? `; no variable for ${done.skipped.length} token(s): ${done.skipped.map((s: any) => s.token).join(', ')}` : ''}`);
}

async function pushDesign() {
  post({ type: 'collect' });
  const { source, baseline } = await next('collected');
  const got = await call('/__tamber', { method: 'import', kind: 'dtcg', client: 'figma', source, baseline });
  if (!got.proposal) { show(got.message ?? 'nothing to propose'); return; }
  const r = got.review ?? {};
  const dials = (r.dials ?? []).map((d: any) => `dial ${d.dial} → ${d.value} (moves ${new Set((d.impact ?? []).map((i: any) => i.token)).size} token(s))`);
  const pins = (r.pins ?? []).map((p: any) => `${p.token} no longer follows ${p.follows.join(', ')}`);
  show([`proposed ${got.proposal}: accept it in the design tool`, ...dials, ...pins].join('\n'));
}

const guard = (f: () => Promise<void>) => () => f().catch(e => show(String(e?.message ?? e)));
$('pair').onclick = guard(pair);
$('pull').onclick = guard(pullDesign);
$('push').onclick = guard(pushDesign);
post({ type: 'load' });
next('loaded').then(msg => {
  server = msg.server ?? '';
  token = msg.token ?? null;
  $('server').value = server;
  show(token ? `paired with ${server}${msg.digest ? `; last pull ${msg.digest.slice(0, 8)}` : ''}` : 'not paired: enter the server and a code from `bun exact.mjs tamber pair`');
});
