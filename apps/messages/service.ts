// Local OpenRouter transport. The credential never enters a plan, JS bundle or browser.
// Run: bun apps/messages/service.ts. Loopback-only; native simulators use the same address.
import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { MODELS } from './model';

const PORT = Number(process.env.MESSAGES_PORT || 4318);
const API = 'https://openrouter.ai/api/v1';
const keyFile = process.env.OPENROUTER_KEY_FILE || join(homedir(), 'Dropbox/APIKeys/openrouter.txt');
const raw = process.env.OPENROUTER_API_KEY || await readFile(keyFile, 'utf8').catch(() => '');
const key = raw.match(/sk-or-v1-[A-Za-z0-9_-]+/)?.[0] || raw.trim();
type Job = { id: string; content: string; done: boolean; error: string; model: string; at: number; abort: AbortController };
const jobs = new Map<string, Job>();
const active = new Map<string, string>();
let catalog = MODELS;
let catalogAt = 0;
const snapshot = (job: Job) => ({ id: job.id, content: job.content, done: job.done, error: job.error, model: job.model });
const safeError = (value: unknown) => String(value instanceof Error ? value.message : value).replace(/sk-or-[\w-]+/g, '[redacted]').slice(0, 300);
const headers = () => ({ Authorization: `Bearer ${key}`, 'Content-Type': 'application/json', 'X-OpenRouter-Title': 'Exact Messages' });

export function eventStream(consume: (value: string) => void) {
  let buffer = '';
  return (chunk: string, finish = false) => {
    buffer += chunk;
    const lines = buffer.split('\n');
    buffer = finish ? '' : lines.pop() || '';
    for (const line of lines) if (line.startsWith('data:')) consume(line.slice(5).trim());
  };
}
async function generate(job: Job, messages: { role: string; content: string }[]) {
  const timeout = setTimeout(() => job.abort.abort(), 120000);
  try {
    const response = await fetch(`${API}/chat/completions`, {
      method: 'POST', headers: headers(), signal: job.abort.signal,
      body: JSON.stringify({ model: job.model, stream: true, max_tokens: 2048, messages: [
        { role: 'system', content: 'You are chatting in a Messages app. Be helpful, natural, and concise. Use plain text unless the user asks for formatting. You are an AI model, not a human contact. Do not claim to perform actions outside this conversation.' }, ...messages,
      ] }),
    });
    if (!response.ok) {
      const payload = await response.json().catch(() => ({})) as { error?: { message?: string } };
      throw new Error(payload.error?.message || `OpenRouter returned ${response.status}.`);
    }
    if (!response.body) throw new Error('OpenRouter returned an empty response.');
    const consume = eventStream(data => {
      if (!data || data === '[DONE]') return;
      const event = JSON.parse(data);
      if (event.error) throw new Error(event.error.message || 'The model interrupted its response.');
      const delta = event.choices?.[0]?.delta?.content;
      if (typeof delta === 'string') job.content += delta;
      if (event.model) job.model = event.model;
    });
    const decoder = new TextDecoder();
    for await (const bytes of response.body) consume(decoder.decode(bytes, { stream: true }));
    consume(decoder.decode(), true);
    if (!job.content.trim()) throw new Error('The model returned no text. Try again or choose another model.');
  } catch (error) {
    job.error = job.abort.signal.aborted ? 'Response stopped.' : safeError(error);
  } finally {
    clearTimeout(timeout); job.done = true;
  }
}
async function models() {
  if (Date.now() - catalogAt < 300000) return catalog;
  try {
    const response = await fetch(`${API}/models`, { signal: AbortSignal.timeout(15000) });
    if (!response.ok) throw new Error('Model catalog unavailable');
    const { data } = await response.json() as { data: any[] };
    catalog = data.filter(m => m.architecture?.output_modalities?.includes('text') && !m.id.includes(':batch'))
      .map(m => {
        const maker = String(m.name).split(':')[0];
        const favorite = MODELS.find(f => f.id === m.id);
        return favorite || { id: m.id, name: String(m.name).replace(/^[^:]+: /, ''), maker, initials: maker.slice(0, 1), color: '#8997ae' };
      }).sort((a, b) => {
        const x = MODELS.findIndex(m => m.id === a.id), y = MODELS.findIndex(m => m.id === b.id);
        return (x < 0 ? 100 : x) - (y < 0 ? 100 : y) || a.name.localeCompare(b.name);
      });
    catalogAt = Date.now();
  } catch { /* Last known catalog remains usable offline. */ }
  return catalog;
}
export const server = Bun.serve({
  hostname: '127.0.0.1', port: PORT, maxRequestBodySize: 256 * 1024,
  async fetch(request) {
    const url = new URL(request.url), origin = request.headers.get('origin');
    // No wildcard CORS and no arbitrary upstream URL: a web page cannot spend this key.
    const allowed = !origin || /^http:\/\/(127\.0\.0\.1|localhost):(8765|8766|8767|8768)$/.test(origin);
    const host = request.headers.get('host') || '';
    if (!allowed || ![`127.0.0.1:${PORT}`, `localhost:${PORT}`].includes(host)) return new Response('Forbidden', { status: 403 });
    const cors = origin ? { 'Access-Control-Allow-Origin': origin, Vary: 'Origin' } : {};
    const json = (body: unknown, status = 200) => Response.json(body, { status, headers: { ...cors, 'Cache-Control': 'no-store' } });
    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: { ...cors, 'Access-Control-Allow-Methods': 'GET, POST, DELETE', 'Access-Control-Allow-Headers': 'Content-Type' } });
    try {
      if (url.pathname === '/health') return json({ connected: !!key });
      if (url.pathname === '/models' && request.method === 'GET') return json({ models: await models() });
      if (url.pathname === '/turns' && request.method === 'POST') {
        if (!key) return json({ error: 'No OpenRouter key found. Set OPENROUTER_KEY_FILE and restart the Messages service.' }, 503);
        if (!request.headers.get('content-type')?.startsWith('application/json')) return json({ error: 'JSON required' }, 415);
        const { id, model, messages } = await request.json() as any;
        if (typeof id !== 'string' || !/^[a-zA-Z0-9_-]{1,100}$/.test(id) || typeof model !== 'string' || !/^[\w./:@-]{1,150}$/.test(model)
          || !Array.isArray(messages) || !messages.length || messages.length > 80 || messages.some(m => !['user', 'assistant'].includes(m.role) || typeof m.content !== 'string')) return json({ error: 'Invalid chat request.' }, 400);
        if (active.has(id)) return json(snapshot(jobs.get(active.get(id)!)!));
        for (const [jobId, job] of jobs) if (job.done && Date.now() - job.at > 3600000) { jobs.delete(jobId); for (const [turn, value] of active) if (value === jobId) active.delete(turn); }
        if ([...jobs.values()].filter(j => !j.done).length >= 6) return json({ error: 'Six models are already responding. Wait for one to finish.' }, 429);
        const job: Job = { id: crypto.randomUUID(), content: '', done: false, error: '', model, at: Date.now(), abort: new AbortController() };
        jobs.set(job.id, job); active.set(id, job.id); void generate(job, messages);
        return json(snapshot(job));
      }
      const match = /^\/turns\/([\w-]+)$/.exec(url.pathname), job = match && jobs.get(match[1]);
      if (job && request.method === 'GET') return json(snapshot(job));
      if (job && request.method === 'DELETE') { job.abort.abort(); job.done = true; job.error = 'Response stopped.'; return json(snapshot(job)); }
      return json({ error: 'This response is no longer available. Try sending again.' }, 404);
    } catch (error) { return json({ error: safeError(error) }, 500); }
  },
});
if (import.meta.main) console.log(`Messages AI service: http://127.0.0.1:${server.port} · OpenRouter ${key ? 'connected' : 'key missing'} (key stays on this Mac)`);
