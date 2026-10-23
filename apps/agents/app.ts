import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.exact.agents';
export const SERVICE = 'http://127.0.0.1:4319'; // service.ts, which holds the OpenRouter key
const FILE = 'app:/data/conversations.json';
export const grants = [`net.fetch ${SERVICE}`, `fs.read ${FILE}`, `fs.write ${FILE}`].join('\n');

type Option = Result<'catalog'>[number];
type Msg = { id: string; mine: boolean; body: string; reasoning: string; thinkingMs: number; at: number; error: string };
type Conversation = {
  id: string; title: string; titled: boolean; model: string; modelName: string; maker: string; effort: string;
  created: number; updated: number; messages: Msg[]; job: string; streaming: boolean; reply: string;
};

// The ones offered first; the search reaches OpenRouter's whole catalog.
export const FEATURED: Option[] = [
  ['anthropic/claude-opus-5.5', 'Claude Opus 5.5', 'Anthropic'],
  ['anthropic/claude-sonnet-5.5', 'Claude Sonnet 5.5', 'Anthropic'],
  ['anthropic/claude-haiku-5.5', 'Claude Haiku 5.5', 'Anthropic'],
  ['anthropic/claude-fable-5.1', 'Claude Fable 5.1', 'Anthropic'],
  ['openai/gpt-6.1-sol', 'GPT-6.1 Sol', 'OpenAI'],
  ['openai/gpt-6.1-sol-pro', 'GPT-6.1 Sol Pro', 'OpenAI'],
  ['google/gemini-4-argon', 'Gemini 4 Argon', 'Google'],
  ['google/gemini-3.8-flash', 'Gemini 3.8 Flash', 'Google'],
  ['x-ai/grok-4.7', 'Grok 4.7', 'xAI'],
  ['deepseek/deepseek-v4.1-flash', 'DeepSeek V4.1 Flash', 'DeepSeek'],
  ['moonshotai/kimi-k3', 'Kimi K3', 'Moonshot AI'],
  ['z-ai/glm-5.3-prime', 'GLM 5.3 Prime', 'Z.ai'],
  ['qwen/qwen3.8-max-prime', 'Qwen3.8 Max Prime', 'Qwen'],
  ['mistralai/mistral-large-4-0', 'Mistral Large 4', 'Mistral'],
].map(([id, name, maker]) => ({ id: id!, name: name!, maker: maker!, reasoning: true }));
export const EFFORTS: Record<string, string> = { none: 'No Thinking', low: 'Low', medium: 'Medium', high: 'High', xhigh: 'Max' };

let conversations: Conversation[] = [];
let loaded = false, serial = 0, revision = 0, connected = true, mock = false, lastHealth = -1e12, notice = '';
let catalogCache: Option[] = [];

const change = (id = '') => ({ revision: ++revision, id });
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

async function load(storage: Storage) {
  if (loaded) return;
  try {
    const data = JSON.parse(new TextDecoder().decode(await storage.fs.readFile(FILE)));
    if (data.version === 1 && Array.isArray(data.conversations)) {
      conversations = data.conversations;
      serial = Number(data.serial) || 0;
      // An answer that was streaming when the app quit is kept as far as it got.
      for (const c of conversations) if (c.streaming) { c.streaming = false; c.job = ''; }
    }
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e;
  }
  loaded = true;
}
async function persist(storage: Storage) {
  try {
    await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify({ version: 1, serial, conversations })));
    notice = '';
  } catch (e) { notice = `Couldn’t save: ${message(e)}`; }
}
async function api(route: string, method = 'GET', body?: unknown) {
  const res = await fetch(SERVICE + route, {
    method, exactTimeout: 20000, headers: { 'content-type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  } as RequestInit);
  const data = (await res.json().catch(() => ({}))) as any;
  if (!res.ok) throw new Error(data.error || 'The Agents service had a problem. Try again.');
  return data;
}
const offline = (e: unknown) => /fetch|connect|network|refused|timed out/i.test(message(e))
  ? 'Can’t reach the Agents service. Run `bun service.ts` on this Mac, then try again.' : message(e);

// ---- presentation ----

const DAY = 86400000;
function clock(at: number, offset: number) {
  const d = new Date(at + offset * 60000), h = d.getUTCHours();
  return `${h % 12 || 12}:${String(d.getUTCMinutes()).padStart(2, '0')} ${h < 12 ? 'AM' : 'PM'}`;
}
function when(at: number, now: number, offset: number) {
  const day = (t: number) => Math.floor((t + offset * 60000) / DAY);
  const ago = day(now) - day(at);
  if (ago <= 0) return clock(at, offset);
  if (ago === 1) return 'Yesterday';
  const d = new Date(at + offset * 60000);
  if (ago < 7) return ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'][d.getUTCDay()]!;
  return `${d.getUTCMonth() + 1}/${d.getUTCDate()}/${String(d.getUTCFullYear()).slice(2)}`;
}
const subtitle = (c: Conversation) => `${c.modelName} · ${c.effort === 'none' ? 'No thinking' : `${EFFORTS[c.effort] ?? c.effort} thinking`}`;
const plain = (s: string) => s.replace(/```[\s\S]*?(```|$)/g, ' ').replace(/[#*_`>|~-]+/g, ' ').replace(/\s+/g, ' ').trim();
function thought(ms: number) {
  const s = Math.max(1, Math.round(ms / 1000));
  return s < 60 ? `Thought for ${s} second${s === 1 ? '' : 's'}` : `Thought for ${Math.round(s / 60)} min`;
}

function snapshot(id: string, query: string, now: number, offset: number): Result<'snapshot'> {
  const q = query.trim().toLowerCase();
  const rows = [...conversations].sort((a, b) => b.updated - a.updated)
    .filter((c) => !q || `${c.title} ${c.modelName} ${c.messages.map((m) => m.body).join(' ')}`.toLowerCase().includes(q))
    .map((c) => {
      const last = c.messages[c.messages.length - 1];
      return {
        id: c.id, title: c.title, subtitle: subtitle(c), time: when(c.updated, now, offset), streaming: c.streaming,
        preview: c.streaming && !last?.body ? 'Thinking…' : last ? plain(last.body).slice(0, 160) || 'Thinking…' : 'New conversation',
      };
    });
  const c = conversations.find((x) => x.id === id);
  const messages = (c?.messages ?? []).map((m, i, all) => {
    const live = !!c?.streaming && m.id === c.reply;
    return {
      id: m.id, mine: m.mine, body: m.body, parts: m.mine ? [] : parts(m.body), error: m.error, thinking: m.reasoning,
      thought: m.reasoning ? (live && !m.body ? 'Thinking…' : thought(m.thinkingMs)) : '',
      waiting: live && !m.body, live, last: i === all.length - 1,
    };
  });
  return {
    revision, connected, mock, notice, rows, found: !!c, id: c?.id ?? '', title: c?.title ?? '',
    subtitle: c ? subtitle(c) : '', model: c?.model ?? '', effort: c?.effort ?? 'medium',
    streaming: !!c?.streaming, messages, empty: rows.length === 0,
  };
}

// A pipe table (GFM): a header row, then its delimiter row, then rows while lines hold a pipe.
// `markup="markdown"` draws one as its source in a fixed pitch, so a reply is cut into runs of
// Markdown and tables the view draws as grids. Fenced code is never searched for tables.
const DELIMITER = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;
const cellsOf = (line: string) => {
  const inner = line.trim().replace(/^\|/, '').replace(/(?<!\\)\|$/, '');
  return inner.split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, '|'));
};
export function parts(body: string): Result<'snapshot'>['messages'][number]['parts'] {
  const lines = body.split('\n'), out: ReturnType<typeof parts> = [];
  let text: string[] = [], fence = '';
  const flush = () => { if (text.join('\n').trim()) out.push({ key: `p${out.length}`, table: false, text: text.join('\n'), rows: [] }); text = []; };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!, open = /^\s*(```|~~~)/.exec(line)?.[1];
    if (open && (!fence || fence === open)) fence = fence ? '' : open;
    const next = lines[i + 1];
    if (fence || !line.includes('|') || next === undefined || !next.includes('-') || !DELIMITER.test(next)) { text.push(line); continue; }
    const head = cellsOf(line), aligns = cellsOf(next).map((d) => (d.endsWith(':') ? (d.startsWith(':') ? 'center' : 'right') : 'left'));
    const body: string[][] = [];
    for (i += 2; i < lines.length && lines[i]!.includes('|') && lines[i]!.trim(); i++) body.push(cellsOf(lines[i]!));
    i--;
    flush();
    const row = (cells: string[], r: number, headRow: boolean) => ({
      key: `r${r}`, head: headRow,
      cells: head.map((_, c) => {
        const cell = cells[c] ?? '';
        return { key: `c${c}`, first: c === 0, align: aligns[c] ?? 'left', text: headRow && cell && !cell.startsWith('**') ? `**${cell}**` : cell };
      }),
    });
    out.push({ key: `p${out.length}`, table: true, text: '', rows: [row(head, 0, true), ...body.map((cells, r) => row(cells, r + 1, false))] });
  }
  flush();
  return out;
}

// ---- turns ----

async function start(c: Conversation, at: number, storage: Storage) {
  const history = c.messages.filter((m) => m.body && !m.error).map((m) => ({ role: m.mine ? 'user' : 'assistant', content: m.body }));
  const reply: Msg = { id: `m${++serial}`, mine: false, body: '', reasoning: '', thinkingMs: 0, at, error: '' };
  c.messages.push(reply); c.reply = reply.id; c.streaming = true; c.updated = at;
  try {
    const job = await api('/turns', 'POST', { model: c.model, effort: c.effort, messages: history });
    c.job = job.id; connected = true;
  } catch (e) {
    c.streaming = false; c.job = ''; reply.error = offline(e);
  }
  await persist(storage);
}

async function name(c: Conversation, storage: Storage) {
  const first = c.messages.find((m) => m.mine);
  if (c.titled || !first) return;
  c.titled = true;
  try {
    const { title } = await api('/title', 'POST', { text: first.body });
    if (title) { c.title = title; revision++; await persist(storage); }
  } catch { /* the first words stay the title */ }
}

async function pulse(at: number, storage: Storage) {
  await load(storage);
  await Promise.all(conversations.filter((c) => c.streaming && c.job).map(async (c) => {
    const reply = c.messages.find((m) => m.id === c.reply);
    try {
      const job = await api(`/turns/${c.job}`);
      if (!c.streaming || !reply) return;
      reply.body = job.content; reply.reasoning = job.reasoning; reply.thinkingMs = job.thinkingMs;
      if (job.done) {
        c.streaming = false; c.job = ''; c.updated = at; reply.error = job.error;
        if (!reply.body && !reply.error) c.messages = c.messages.filter((m) => m !== reply); // stopped before a word
        await persist(storage);
        void name(c, storage);
      }
    } catch (e) {
      c.streaming = false; c.job = '';
      if (reply) reply.error = offline(e);
      await persist(storage);
    }
  }));
  if (at - lastHealth > 15000) {
    lastHealth = at;
    try { const h = await api('/health'); connected = !!h.connected; mock = !!h.mock; } catch { connected = false; }
  }
  return change();
}

async function command(op: string, id: string, value: string, at: number, storage: Storage) {
  await load(storage);
  if (op === 'new') {
    const [model = '', effort = 'medium'] = value.split('|');
    const option = [...FEATURED, ...catalogCache].find((m) => m.id === model);
    if (!option) return change();
    const c: Conversation = {
      id: `c${++serial}`, title: 'New Conversation', titled: false, model: option.id, modelName: option.name, maker: option.maker,
      effort: EFFORTS[effort] ? effort : 'medium', created: at, updated: at, messages: [], job: '', streaming: false, reply: '',
    };
    conversations.push(c);
    await persist(storage);
    return change(c.id);
  }
  const c = conversations.find((x) => x.id === id);
  if (!c) return change();
  if (op === 'send' && value.trim() && !c.streaming) {
    c.messages.push({ id: `m${++serial}`, mine: true, body: value.trim(), reasoning: '', thinkingMs: 0, at, error: '' });
    if (!c.titled && c.title === 'New Conversation') c.title = value.trim().split(/\s+/).slice(0, 6).join(' ').slice(0, 48);
    await start(c, at, storage);
  } else if (op === 'retry' && !c.streaming) {
    const last = c.messages[c.messages.length - 1];
    if (last && !last.mine) c.messages.pop();
    if (c.messages.length && c.messages[c.messages.length - 1]!.mine) await start(c, at, storage);
  } else if (op === 'stop' && c.streaming) {
    if (c.job) try { await api(`/turns/${c.job}`, 'DELETE'); } catch { /* it ends with the service */ }
    // The next pulse records how far it got.
    if (!c.job) c.streaming = false;
  } else if (op === 'effort' && EFFORTS[value]) {
    c.effort = value; await persist(storage);
  } else if (op === 'model') {
    const option = [...FEATURED, ...catalogCache].find((m) => m.id === value);
    if (option && !c.streaming) { c.model = option.id; c.modelName = option.name; c.maker = option.maker; await persist(storage); }
  } else if (op === 'delete') {
    if (c.job) try { await api(`/turns/${c.job}`, 'DELETE'); } catch { /* gone with it */ }
    conversations = conversations.filter((x) => x !== c);
    await persist(storage);
  }
  return change(c.id);
}

async function catalog(query: string, open: boolean): Promise<Option[]> {
  if (open && !catalogCache.length) {
    try {
      const { models } = await api('/models');
      catalogCache = (models as (Option & { created: number })[]).sort((a, b) => b.created - a.created)
        .map(({ id, name, maker, reasoning }) => ({ id, name, maker, reasoning }));
    } catch (e) { if ((e as { code?: string }).code === 'bake') throw e; }
  }
  const q = query.trim().toLowerCase();
  if (!q) return FEATURED;
  const seen = new Set<string>();
  return [...FEATURED, ...catalogCache]
    .filter((m) => !seen.has(m.id) && seen.add(m.id) && `${m.name} ${m.maker} ${m.id}`.toLowerCase().includes(q))
    .slice(0, 60);
}

const sources: Sources = {
  snapshot: async ([id, query, , now, offset], _store, storage) => { await load(storage); return snapshot(id, query, now, offset); },
  catalog: ([query, open]) => catalog(query, open),
  command: ([op, id, value, at], _store, storage) => command(op, id, value, at, storage),
  pulse: ([at], _store, storage) => pulse(at, storage),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
