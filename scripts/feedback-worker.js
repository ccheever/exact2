// The diary endpoint scripts/feedback.mjs sends to (docs/diary.md): a Cloudflare
// Worker over one KV namespace. A diary is stored once, under a random receipt
// that is also the only way to delete it; Slack is told counts, never content.
//
//   POST   /diary        {version, exact2, text} -> {id}
//   DELETE /diary/<id>   delete that diary
//   GET    /diaries      every stored diary (Authorization: Bearer $ADMIN_TOKEN)
//   GET    /needs        each capability's provided / by hand / missing count across them (same token)
//
// Deploy: bunx wrangler deploy scripts/feedback-worker.js --name exact-diaries
// with the DIARIES KV binding and the SLACK_WEBHOOK and ADMIN_TOKEN secrets.
const LIMIT = 256 * 1024;
const json = (body, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

/** A Needed line's capability and how it went: `- camera: by hand (...)`. */
const NEED = /^\s*[-*]\s+([a-z][a-z0-9 /&-]{0,39}?)\s*:\s*(provided|by hand|missing)\b/i;

/** What Slack hears: how many of each, from the diary's own headings, and which
 * generic capabilities were needed (the Needed section names nothing else). */
export function summary(text) {
  let section = '', diaries = 0, rough = 0, lean = 0, smooth = 0, roughSteps = 0, commands = 0, failed = 0;
  const needs = [];
  for (const line of text.split('\n')) {
    const heading = /^#+\s*(.*)$/.exec(line);
    if (heading) {
      section = heading[1].trim().toLowerCase();
      if (/\.md$/.test(section)) diaries++;
      continue;
    }
    const item = /^\s*[-*]\s+\S/.test(line);
    if (item && section === 'rough') rough++;
    if (item && section.startsWith('lean in')) lean++;
    if (item && section === 'checkpoints') { if (/\bsmooth\b/i.test(line)) smooth++; else if (/\brough\b/i.test(line)) roughSteps++; }
    if (section === 'needed') { const need = NEED.exec(line); if (need) needs.push([need[1].trim().toLowerCase(), need[2].toLowerCase()]); }
    if (section === 'commands' && /exit=/.test(line)) { commands++; if (!/exit=0\b/.test(line)) failed++; }
  }
  return { diaries, rough, lean, smooth, roughSteps, commands, failed, needs };
}

/** Across diaries: per capability, how often it was provided, built by hand or
 * missing, most-needed first. */
export function needs(texts) {
  const counts = new Map();
  for (const text of texts) for (const [name, how] of summary(text).needs) {
    const row = counts.get(name) ?? { name, provided: 0, 'by hand': 0, missing: 0 };
    row[how]++;
    counts.set(name, row);
  }
  const total = row => row.provided + row['by hand'] + row.missing;
  return [...counts.values()].sort((a, b) => total(b) - total(a) || (b['by hand'] + b.missing) - (a['by hand'] + a.missing) || a.name.localeCompare(b.name));
}

export default {
  async fetch(request, env, context) {
    const url = new URL(request.url);
    const receipt = /^\/diary\/([0-9a-f-]{36})$/.exec(url.pathname)?.[1];
    if (request.method === 'POST' && url.pathname === '/diary') {
      const raw = await request.text();
      if (raw.length > LIMIT) return json({ error: 'too large' }, 413);
      let body;
      try { body = JSON.parse(raw); } catch { return json({ error: 'not JSON' }, 400); }
      if (typeof body.text !== 'string' || !body.text.trim()) return json({ error: 'no text' }, 400);
      const id = crypto.randomUUID(), received = new Date().toISOString();
      await env.DIARIES.put(`diary:${id}`, JSON.stringify({ id, received, exact2: body.exact2 ?? null, version: body.version ?? null, text: body.text }));
      if (env.SLACK_WEBHOOK) {
        const s = summary(body.text);
        const text = `New exact diary \`${id}\` (exact2 ${body.exact2 ?? '?'}): ${s.diaries} ${s.diaries === 1 ? 'diary' : 'diaries'}, `
          + `${s.rough} rough, ${s.lean} lean-in; checkpoints ${s.smooth} smooth / ${s.roughSteps} rough; `
          + `${s.commands} commands, ${s.failed} failed.`
          + (s.needs.length ? ` Needed: ${s.needs.map(([name, how]) => `${name} (${how})`).join(', ')}.` : '');
        context.waitUntil(fetch(env.SLACK_WEBHOOK, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ text }) }));
      }
      return json({ id });
    }
    if (request.method === 'DELETE' && receipt) {
      if (await env.DIARIES.get(`diary:${receipt}`) === null) return json({ error: 'no such diary' }, 404);
      await env.DIARIES.delete(`diary:${receipt}`);
      return json({ deleted: receipt });
    }
    if (request.method === 'GET' && (url.pathname === '/diaries' || url.pathname === '/needs')) {
      if (!env.ADMIN_TOKEN || request.headers.get('authorization') !== `Bearer ${env.ADMIN_TOKEN}`) return json({ error: 'unauthorized' }, 401);
      const diaries = [];
      let cursor;
      do {
        const page = await env.DIARIES.list({ prefix: 'diary:', cursor });
        for (const key of page.keys) { const value = await env.DIARIES.get(key.name); if (value) diaries.push(JSON.parse(value)); }
        cursor = page.list_complete ? undefined : page.cursor;
      } while (cursor);
      if (url.pathname === '/needs') return json(needs(diaries.map(d => d.text)));
      return json(diaries.sort((a, b) => a.received.localeCompare(b.received)));
    }
    return json({ error: 'not found' }, 404);
  },
};
