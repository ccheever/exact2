// The local OpenRouter transport. The key stays in this Bun process: it never enters the
// plan, a bundle or a browser. Run: bun service.ts (loopback only; the iOS simulator shares it).
// AGENTS_MOCK=1 answers with canned streams, for driving the app without a key.
import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

const PORT = Number(process.env.AGENTS_PORT || 4319);
const API = 'https://openrouter.ai/api/v1';
const MOCK = process.env.AGENTS_MOCK === '1';
const TITLE_MODEL = 'google/gemini-3.8-flash';
const keyFile = process.env.OPENROUTER_KEY_FILE || join(homedir(), 'Dropbox/APIKeys/openrouter.txt');
const raw = process.env.OPENROUTER_API_KEY || await readFile(keyFile, 'utf8').catch(() => '');
const key = raw.match(/sk-or-v1-[A-Za-z0-9_-]+/)?.[0] || raw.trim();

type Turn = { role: 'user' | 'assistant'; content: string };
type Job = {
  id: string; model: string; content: string; reasoning: string; done: boolean; error: string;
  started: number; thinkingMs: number; at: number; abort: AbortController;
};
const jobs = new Map<string, Job>();
const snapshot = (j: Job) => ({ id: j.id, model: j.model, content: j.content, reasoning: j.reasoning, done: j.done, error: j.error, thinkingMs: j.thinkingMs });
const safeError = (e: unknown) => String(e instanceof Error ? e.message : e).replace(/sk-or-[\w-]+/g, '[redacted]').slice(0, 300);
const headers = () => ({ Authorization: `Bearer ${key}`, 'Content-Type': 'application/json', 'X-OpenRouter-Title': 'Agents' });
const SYSTEM = 'You are a helpful assistant in a native chat app. Answer directly and well. Use Markdown when it helps: short paragraphs, lists, tables and fenced code blocks.';

function lines(consume: (data: string) => void) {
  let buffer = '';
  return (chunk: string, finish = false) => {
    buffer += chunk;
    const parts = buffer.split('\n');
    buffer = finish ? '' : parts.pop() || '';
    for (const line of parts) if (line.startsWith('data:')) consume(line.slice(5).trim());
  };
}

async function request(job: Job, messages: Turn[], effort: string, withReasoning: boolean) {
  const body: Record<string, unknown> = { model: job.model, stream: true, messages: [{ role: 'system', content: SYSTEM }, ...messages] };
  if (withReasoning) body.reasoning = effort === 'none' ? { effort: 'none', exclude: true } : { effort };
  return fetch(`${API}/chat/completions`, { method: 'POST', headers: headers(), signal: job.abort.signal, body: JSON.stringify(body) });
}

async function generate(job: Job, messages: Turn[], effort: string) {
  const timeout = setTimeout(() => job.abort.abort(), 600000);
  let thinkingFrom = 0;
  const endThinking = () => { if (thinkingFrom && !job.thinkingMs) job.thinkingMs = Date.now() - thinkingFrom; };
  try {
    let response = await request(job, messages, effort, true);
    // A model that refuses the reasoning parameter answers without it.
    if (response.status === 400) response = await request(job, messages, effort, false);
    if (!response.ok) {
      const payload = await response.json().catch(() => ({})) as { error?: { message?: string } };
      throw new Error(payload.error?.message || `OpenRouter returned ${response.status}.`);
    }
    if (!response.body) throw new Error('OpenRouter returned an empty response.');
    const consume = lines((data) => {
      if (!data || data === '[DONE]') return;
      const event = JSON.parse(data);
      if (event.error) throw new Error(event.error.message || 'The model stopped mid-answer.');
      const delta = event.choices?.[0]?.delta ?? {};
      if (typeof delta.reasoning === 'string' && delta.reasoning) {
        if (!thinkingFrom) thinkingFrom = Date.now();
        job.reasoning += delta.reasoning;
      }
      if (typeof delta.content === 'string' && delta.content) { endThinking(); job.content += delta.content; }
    });
    const decoder = new TextDecoder();
    for await (const bytes of response.body) consume(decoder.decode(bytes, { stream: true }));
    consume(decoder.decode(), true);
    endThinking();
    if (!job.content.trim()) throw new Error('The model returned no text. Try again, or pick another model.');
  } catch (e) {
    job.error = job.abort.signal.aborted ? '' : safeError(e);
  } finally {
    clearTimeout(timeout); endThinking(); job.done = true;
  }
}

const MOCK_REPLY = [
  'Here’s a quick sketch of how I’d approach it:\n\n',
  '1. **Start small.** Get one path working end to end.\n',
  '2. **Measure** before optimising anything.\n',
  '3. **Polish** the parts people touch most.\n\n',
  '```swift\nlet window = NSWindow()\nwindow.titlebarAppearsTransparent = true\n```\n\n',
  '| Step | Cost | Payoff |\n| :--- | :---: | ---: |\n| **Start small** | Low | Fast feedback |\n| Measure | Medium | Fewer guesses |\n| Polish | High | Delight |\n\n',
  'Want me to go deeper on any of these?',
];
async function mock(job: Job, effort: string) {
  const thoughts = effort === 'none' ? [] : ['Considering what they’re asking. ', 'A short plan with an example reads best. ', 'Keep it brief.'];
  const from = Date.now();
  for (const t of thoughts) { if (job.abort.signal.aborted) break; await Bun.sleep(350); job.reasoning += t; }
  if (thoughts.length) job.thinkingMs = Date.now() - from;
  for (const part of MOCK_REPLY) for (const word of part.split(/(?<= )/)) {
    if (job.abort.signal.aborted) { job.done = true; return; }
    await Bun.sleep(28); job.content += word;
  }
  job.done = true;
}

async function title(text: string) {
  if (MOCK || !key) return text.split(/\s+/).slice(0, 5).join(' ');
  const response = await fetch(`${API}/chat/completions`, {
    method: 'POST', headers: headers(), signal: AbortSignal.timeout(20000),
    body: JSON.stringify({ model: TITLE_MODEL, max_tokens: 24, reasoning: { effort: 'none', exclude: true }, messages: [
      { role: 'system', content: 'Name this conversation in 2 to 5 words, Title Case, no quotes or final punctuation. Reply with the title only.' },
      { role: 'user', content: text.slice(0, 2000) },
    ] }),
  });
  const data = await response.json() as { choices?: { message?: { content?: string } }[] };
  return (data.choices?.[0]?.message?.content || '').trim().replace(/^["'“]|["'”.]$/g, '').slice(0, 60);
}

let catalog: unknown[] = [], catalogAt = 0;
async function models() {
  if (Date.now() - catalogAt < 600000 && catalog.length) return catalog;
  const response = await fetch(`${API}/models`, { signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw new Error('The model list is unavailable right now.');
  const { data } = await response.json() as { data: any[] };
  catalog = data.filter((m) => m.architecture?.output_modalities?.includes('text') && !m.id.includes(':'))
    .map((m) => ({ id: m.id, name: String(m.name).replace(/^[^:]+:\s*/, ''), maker: String(m.name).split(':')[0].replace('SpaceXAI', 'xAI'),
      reasoning: (m.supported_parameters || []).includes('reasoning'), created: m.created || 0 }));
  catalogAt = Date.now();
  return catalog;
}

const valid = (b: any) => typeof b?.model === 'string' && /^[\w./:@-]{1,150}$/.test(b.model)
  && ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'].includes(b.effort)
  && Array.isArray(b.messages) && b.messages.length > 0 && b.messages.length <= 400
  && b.messages.every((m: any) => ['user', 'assistant'].includes(m.role) && typeof m.content === 'string');

export const server = Bun.serve({
  hostname: '127.0.0.1', port: PORT, maxRequestBodySize: 4 * 1024 * 1024,
  async fetch(req) {
    const url = new URL(req.url), origin = req.headers.get('origin');
    // No wildcard CORS: only the local dev pages may spend this key.
    if (origin && !/^http:\/\/(127\.0\.0\.1|localhost):\d+$/.test(origin)) return new Response('Forbidden', { status: 403 });
    const cors = origin ? { 'Access-Control-Allow-Origin': origin, Vary: 'Origin' } : {};
    const json = (body: unknown, status = 200) => Response.json(body, { status, headers: { ...cors, 'Cache-Control': 'no-store' } });
    if (req.method === 'OPTIONS') return new Response(null, { status: 204, headers: { ...cors, 'Access-Control-Allow-Methods': 'GET, POST, DELETE', 'Access-Control-Allow-Headers': 'Content-Type' } });
    try {
      if (url.pathname === '/health') return json({ connected: MOCK || !!key, mock: MOCK });
      if (url.pathname === '/models') return json({ models: await models() });
      if (url.pathname === '/title' && req.method === 'POST') return json({ title: await title(String((await req.json() as any).text || '')) });
      if (url.pathname === '/turns' && req.method === 'POST') {
        if (!MOCK && !key) return json({ error: 'No OpenRouter key. Put it in ~/Dropbox/APIKeys/openrouter.txt and restart the service.' }, 503);
        const body = await req.json() as any;
        if (!valid(body)) return json({ error: 'That request was malformed.' }, 400);
        for (const [id, j] of jobs) if (j.done && Date.now() - j.at > 3600000) jobs.delete(id);
        if ([...jobs.values()].filter((j) => !j.done).length >= 8) return json({ error: 'Eight answers are already streaming. Wait for one to finish.' }, 429);
        const job: Job = { id: crypto.randomUUID(), model: body.model, content: '', reasoning: '', done: false, error: '', started: Date.now(), thinkingMs: 0, at: Date.now(), abort: new AbortController() };
        jobs.set(job.id, job);
        void (MOCK ? mock(job, body.effort) : generate(job, body.messages, body.effort));
        return json(snapshot(job));
      }
      const match = /^\/turns\/([\w-]+)$/.exec(url.pathname), job = match && jobs.get(match[1]!);
      if (job && req.method === 'GET') return json(snapshot(job));
      if (job && req.method === 'DELETE') { job.abort.abort(); job.done = true; return json(snapshot(job)); }
      return json({ error: 'That answer is no longer available. Try again.' }, 404);
    } catch (e) { return json({ error: safeError(e) }, 500); }
  },
});
console.log(`Agents service on http://127.0.0.1:${server.port} · ${MOCK ? 'mock replies' : key ? 'OpenRouter connected' : 'no OpenRouter key'}`);
