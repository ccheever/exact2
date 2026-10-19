// The Figma plugin's UI (LLP 1121 §5 stage 10b, rt1's plan E4): it pairs with
// a Tamber design server (a code from `bun exact.mjs tamber pair`, the server
// run with --allow-plugin-origin), keeps the paired token in the plugin's
// client storage (this user, this machine; never the file), and holds every
// request: a pull (export) and a push (import, held as a proposal for a person
// to accept; the plugin never writes the design).

const $ = (id: string) => document.getElementById(id) as HTMLInputElement;
// The server and its token, together: a token goes only to the server it was
// paired with, and both change only when a pairing succeeds.
let paired: { server: string; token: string | null } = { server: '', token: null };

const show = (text: string) => { $('status').textContent = text; };
const call = async (at: { server: string; token: string | null }, path: string, body: object) => {
  // No redirect followed: the token goes to the server it was paired with, and a pairing is the server's typed.
  const res = await fetch(`${at.server}${path}`, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json', ...(at.token ? { 'x-exact-tamber-token': at.token } : {}) }, body: JSON.stringify(body) });
  const reply = await res.json().catch(() => ({}));
  if (res.status === 401 && at.token && at === paired) { paired = { ...paired, token: null }; await ask({ type: 'save', ...paired }, 'saved'); }
  if (!res.ok) throw new Error(reply.error?.message ?? reply.message ?? `${res.status}`);
  return reply.ok ?? reply;
};

/** A request to the main thread and its reply, matched by `id`; an error
 * there rejects it. */
let ids = 0;
const waiting = new Map<number, { type: string; ok: (msg: any) => void; no: (e: Error) => void }>();
const ask = (msg: object, reply: string) => new Promise<any>((ok, no) => {
  const id = ++ids;
  waiting.set(id, { type: reply, ok, no });
  parent.postMessage({ pluginMessage: { ...msg, id } }, '*');
});
onmessage = (e: MessageEvent) => {
  const msg = e.data.pluginMessage, w = waiting.get(msg?.id);
  if (!w) return;
  waiting.delete(msg.id);
  if (msg.type === 'error') w.no(new Error(msg.message));
  else if (msg.type === w.type) w.ok(msg);
  else w.no(new Error(`unexpected reply ${msg.type}`));
};

async function pair() {
  // Against the server typed, with no token: the paired one stays as it was
  // unless this succeeds.
  const server = $('server').value.trim().replace(/\/+$/, '');
  const got = await call({ server, token: null }, '/__tamber/pair', { code: $('code').value.trim(), name: 'figma' });
  paired = { server, token: got.token };
  await ask({ type: 'save', ...paired }, 'saved');
  show(`paired as ${got.client}, ${got.scope}, until ${got.expires}`);
}

async function pullDesign() {
  const { text } = await call(paired, '/__tamber', { method: 'export', kind: 'dtcg', client: 'figma' });
  const { conflicts, contexts, unread, drops, aliases } = await ask({ type: 'check', text }, 'checked');
  const lines = conflicts.map((c: any) => `${c.collection}: ${c.variable} (${c.mode})${c.gone ? ', which the design no longer has' : c.figmaOnly ? ', changed in Figma only' : ''}`);
  const moved = [...contexts.added.map((c: string) => `+ ${c}`), ...contexts.removed.map((c: string) => `- ${c}`)];
  const gone = conflicts.some((c: any) => c.gone), kept = conflicts.some((c: any) => !c.gone);
  const after = [kept ? 'push the values the design still has, then pull' : '', gone ? 'a value the design no longer has can\'t be pushed: it stays until the next pull removes it' : ''].filter(Boolean).join('; ');
  if (conflicts.length && !confirm(`${conflicts.length} value(s) changed in Figma since the last pull, which this pull writes over:\n${lines.join('\n')}\n\nOK takes the design's; Cancel keeps Figma's (${after}).`)) { show(`kept Figma's: ${after}`); return; }
  // Values the plugin could not read (an alias to nowhere, a mode or collection gone): a pull writes over them unseen.
  if (unread.length && !confirm(`${unread.length} value(s) in Figma could not be read, so a pull can't tell whether they changed:\n${unread.map((u: any) => `${u.variable}${u.mode ? ` (${u.mode})` : ''}: ${u.why}`).join('\n')}\n\nOK writes the design's over them.`)) { show('not pulled: values that could not be read'); return; }
  if (moved.length && !confirm(`The design's contexts changed since the last pull:\n${moved.join('\n')}\n\nOK rewrites the Tamber collections: a mode or collection the design no longer has goes, and what is bound to it in this file loses it.`)) { show('not pulled: the contexts changed'); return; }
  // Aliases of the person's the pull writes the design's value over.
  if (aliases.length && !confirm(`${aliases.length} alias(es) in Figma will be replaced by the design's values:\n${aliases.map((a: any) => `${a.collection}: ${a.variable} (${a.mode})`).join('\n')}\n\nOK pulls.`)) { show('not pulled: aliases would be replaced'); return; }
  // Variables a pull removes or makes again: what is bound to them in this file loses them.
  if (drops.length && !confirm(`${drops.length} variable(s) go or are made again with this pull, and what is bound to them loses them:\n${drops.map((d: any) => `${d.variable}: ${d.why}`).join('\n')}\n\nOK pulls.`)) { show('not pulled: variables would go'); return; }
  const done = await ask({ type: 'apply', text }, 'applied');
  show(`pulled into ${done.collections.join(', ')}${done.skipped.length ? `; no variable for ${done.skipped.length} token(s): ${done.skipped.map((s: any) => s.token).join(', ')}` : ''}`);
}

async function pushDesign() {
  const { source, baseline, unread } = await ask({ type: 'collect' }, 'collected');
  const got = await call(paired, '/__tamber', { method: 'import', kind: 'dtcg', client: 'figma', source, baseline });
  const r = got.review ?? {};
  // What the design could not take, and why (a length changed in Light only, say: both schemes take one value).
  const unmapped = [
    ...(r.unmapped ?? []).map((u: any) => `not proposed: ${u.name}${u.block ? ` (${u.block})` : ''}: ${u.why}`),
    // Values the plugin could not read (an alias to nowhere, a cycle): sent as they were pulled.
    ...unread.map((u: any) => `not read: ${u.variable} (${u.mode}): ${u.why}`),
  ];
  if (!got.proposal) { show([got.message ?? 'nothing to propose', ...unmapped].join('\n')); return; }
  const dials = (r.dials ?? []).map((d: any) => `dial ${d.dial} → ${d.value} (moves ${new Set((d.impact ?? []).map((i: any) => i.token)).size} token(s))`);
  const pins = (r.pins ?? []).map((p: any) => `${p.token} no longer follows ${(p.follows ?? []).join(', ')}`);
  show([`proposed ${got.proposal}: accept it in the design tool`, ...dials, ...pins, ...unmapped].join('\n'));
}

// One at a time: a pull or push in flight holds the others off, and none
// starts before the stored pairing is loaded.
let busy = true;
const guard = (f: () => Promise<void>) => async () => {
  if (busy) return show('busy: one request at a time');
  busy = true;
  try { await f(); } catch (e: any) { show(String(e?.message ?? e)); } finally { busy = false; }
};
$('pair').onclick = guard(pair);
$('pull').onclick = guard(pullDesign);
$('push').onclick = guard(pushDesign);
ask({ type: 'load' }, 'loaded').then(msg => {
  paired = { server: msg.server ?? '', token: msg.token ?? null };
  $('server').value = paired.server;
  show(paired.token ? `paired with ${paired.server}${msg.digest ? `; last pull ${msg.digest.slice(0, 8)}` : ''}` : 'not paired: enter the server and a code from `bun exact.mjs tamber pair`');
}, e => show(String(e?.message ?? e))).finally(() => { busy = false; });
