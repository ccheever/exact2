import type { Answer, Result, Sources, Storage, Store } from './app.contract.d.ts';
import {
  abortLanguageModelResponse, createLanguageModel, destroyLanguageModel, languageModelAvailability, languageModelResponse, prompt, promptStreaming,
} from '@exact/language-model';

export const appId = 'com.exact.agents';
export const SERVICE = 'http://127.0.0.1:4319'; // service.ts, which holds the OpenRouter key
const FILE = 'app:/data/conversations.json';
export const grants = [`net.fetch ${SERVICE}`, `fs.read ${FILE}`, `fs.write ${FILE}`].join('\n');

type Option = Result<'catalog'>[number];
// `searched`: what the on-device model looked up in earlier conversations for this reply.
type Msg = { id: string; mine: boolean; body: string; reasoning: string; thinkingMs: number; at: number; error: string; searched?: string };
// `session`: the on-device model's session, made from the history when the conversation is first sent to
// after a launch (ids last only as long as the app's session); `overflowed`: its earliest messages no longer fit.
type Conversation = {
  id: string; title: string; titled: boolean; model: string; modelName: string; maker: string; effort: string;
  created: number; updated: number; messages: Msg[]; job: string; streaming: boolean; reply: string;
  session?: string; overflowed?: boolean;
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
// The device's own model (LLP 1133.016 §10): no key, no service, nothing leaves the device.
export const DEVICE: Option = { id: 'device', name: 'On this device', maker: 'Apple Intelligence, Gemini Nano or the browser', reasoning: false };
export const EFFORTS: Record<string, string> ={ none: 'No Thinking', low: 'Low', medium: 'Medium', high: 'High', xhigh: 'Max' };

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
      for (const c of conversations) { if (c.streaming) { c.streaming = false; c.job = ''; } c.session = ''; }
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
const subtitle = (c: Conversation) => c.model === DEVICE.id ? DEVICE.name
  : `${c.modelName} · ${c.effort === 'none' ? 'No thinking' : `${EFFORTS[c.effort] ?? c.effort} thinking`}`;
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
  // The line that says the on-device model forgot the start shows above the newest reply, once it has.
  const newest = c?.overflowed ? [...c.messages].reverse().find((m) => !m.mine)?.id : undefined;
  const messages = (c?.messages ?? []).map((m, i, all) => {
    const live = !!c?.streaming && m.id === c.reply;
    return {
      id: m.id, mine: m.mine, body: m.body, parts: m.mine ? [] : parts(m.body), error: m.error, thinking: m.reasoning,
      thought: m.reasoning ? (live && !m.body ? 'Thinking…' : thought(m.thinkingMs)) : '',
      waiting: live && !m.body, live, last: i === all.length - 1,
      searched: m.searched ? `Searched your conversations for “${m.searched}”` : '', forgot: m.id === newest,
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
  if (c.model === DEVICE.id) return startOnDevice(c, history as Turn[], reply, storage);
  try {
    const job = await api('/turns', 'POST', { model: c.model, effort: c.effort, messages: history });
    c.job = job.id; connected = true;
  } catch (e) {
    c.streaming = false; c.job = ''; reply.error = offline(e);
  }
  await persist(storage);
}

// ---- the on-device model (LLP 1133.016 §10) ----

type Turn = { role: 'user' | 'assistant'; content: string };
const code = (e: unknown) => (e as { code?: string }).code ?? '';
const unavailable = (e: unknown) => code(e) === 'NotSupportedError' ? 'The on-device model isn’t available here.'
  : code(e) === 'QuotaExceededError' ? 'That message is too long for the on-device model.'
  : code(e) === 'NotAllowedError' ? 'The on-device model needs to be downloaded first. Choose it in New Conversation.'
  : `The on-device model had a problem (${code(e) || message(e)}). Try again.`;
// The tool the model may call, where the platform runs tools: the person's earlier conversations (D9).
const FIND = {
  name: 'findConversations',
  description: 'Finds the person’s earlier conversations about a topic, with their titles and an excerpt of each.',
  inputSchema: { type: 'object', properties: { query: { type: 'string', description: 'The topic, in a few words' } }, required: ['query'] },
};

/** The conversation's session: the one this launch made, or one made now from the history before `newest`. */
async function sessionFor(c: Conversation, earlier: Turn[]) {
  if (c.session) return c.session;
  const { tools } = await languageModelAvailability({});
  const made = await createLanguageModel({ initialPrompts: earlier, tools: tools ? [FIND] : [] });
  c.session = made.id;
  if (made.overflowed) c.overflowed = true;
  return made.id;
}
async function startOnDevice(c: Conversation, history: Turn[], reply: Msg, storage: Storage) {
  const input = history.pop();
  try {
    const session = await sessionFor(c, history);
    const response = await promptStreaming({ session, input: input?.content ?? '' });
    c.job = response.id;
  } catch (e) {
    c.streaming = false; c.job = ''; reply.error = unavailable(e);
  }
  await persist(storage);
}
/** A reply on the device: its text so far, what it searched, and its end (D7: read once a pulse). */
async function pulseOnDevice(c: Conversation, reply: Msg, at: number, storage: Storage) {
  let r;
  try { r = await languageModelResponse({ response: c.job }); } catch (e) {
    // A response the module no longer holds (a reload while it streamed): the reply keeps what it had.
    c.streaming = false; c.job = '';
    if (!reply.body) reply.error = unavailable(e);
    await persist(storage);
    return;
  }
  if (!c.streaming) return;
  reply.body = r.text;
  const search = r.toolCalls.find((t) => t.name === FIND.name);
  if (search) try { reply.searched = String(JSON.parse(search.arguments).query ?? ''); } catch { /* cut at 4 KB: no query to show */ }
  if (r.overflowed) c.overflowed = true;
  if (r.state === 'generating' || r.state === 'calling-tool') return;
  c.streaming = false; c.job = ''; c.updated = at;
  if (r.state === 'failed') reply.error = r.reason === 'refusal' ? 'The on-device model declined to answer that.' : unavailable({ code: r.error });
  if (!reply.body && !reply.error) c.messages = c.messages.filter((m) => m !== reply);
  await persist(storage);
  // Awaited, so the pulse that lands the reply lands the title too: no later read is due once it ends.
  await name(c, storage);
}
/** A title by structured output (D8), from a session of its own. */
async function titleOnDevice(text: string) {
  const { id } = await createLanguageModel({});
  try {
    const answer = await prompt({
      session: id, input: `Write a title of at most six words for a conversation that begins: ${text.slice(0, 600)}`,
      responseConstraint: { type: 'object', properties: { title: { type: 'string' } }, required: ['title'] },
    });
    return String(JSON.parse(answer.text).title ?? '').trim();
  } finally {
    await destroyLanguageModel({ session: id }).catch(() => {});
  }
}

/** The on-device model's tools (LLP 1133.016 D9): it reads conversations and changes none. */
export const tools = {
  findConversations: async ({ query }: { query: string }, _store: Store, storage: Storage) => {
    await load(storage);
    const words = String(query).toLowerCase().split(/\s+/).filter(Boolean);
    return conversations
      .map((c) => ({ c, text: `${c.title} ${c.messages.map((m) => m.body).join(' ')}`.toLowerCase() }))
      .filter(({ text }) => words.length && words.every((w) => text.includes(w)))
      .sort((a, b) => b.c.updated - a.c.updated)
      .slice(0, 5)
      .map(({ c }) => {
        const hit = c.messages.find((m) => words.some((w) => m.body.toLowerCase().includes(w)));
        return { title: c.title, model: c.modelName, updated: new Date(c.updated).toISOString().slice(0, 10), excerpt: plain(hit?.body ?? '').slice(0, 280) };
      });
  },
};

async function name(c: Conversation, storage: Storage) {
  const first = c.messages.find((m) => m.mine);
  if (c.titled || !first) return;
  c.titled = true;
  if (c.model === DEVICE.id) {
    try {
      const title = await titleOnDevice(first.body);
      if (title) { c.title = title.slice(0, 64); revision++; await persist(storage); }
    } catch { /* the first words stay the title */ }
    return;
  }
  try {
    const { title } = await api('/title', 'POST', { text: first.body });
    if (title) { c.title = title; revision++; await persist(storage); }
  } catch { /* the first words stay the title */ }
}

async function pulse(at: number, storage: Storage) {
  await load(storage);
  await Promise.all(conversations.filter((c) => c.streaming && c.job).map(async (c) => {
    const reply = c.messages.find((m) => m.id === c.reply);
    if (c.model === DEVICE.id) { if (reply) await pulseOnDevice(c, reply, at, storage); return; }
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
    // `device|<effort>|<session>`: the on-device model, with the session its download made, if one did.
    const [model = '', effort = 'medium', session = ''] = value.split('|');
    const option = [DEVICE, ...FEATURED, ...catalogCache].find((m) => m.id === model);
    if (!option) return change();
    const c: Conversation = {
      id: `c${++serial}`, title: 'New Conversation', titled: false, model: option.id, modelName: option.name, maker: option.maker,
      effort: EFFORTS[effort] ? effort : 'medium', created: at, updated: at, messages: [], job: '', streaming: false, reply: '',
      ...(option === DEVICE ? { effort: 'none', session } : {}),
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
    c.session = ''; // the on-device session holds the failed turn: the retry makes one from the history
    const last = c.messages[c.messages.length - 1];
    if (last && !last.mine) c.messages.pop();
    if (c.messages.length && c.messages[c.messages.length - 1]!.mine) await start(c, at, storage);
  } else if (op === 'stop' && c.streaming) {
    if (c.job && c.model === DEVICE.id) try { await abortLanguageModelResponse({ response: c.job }); } catch { /* it has ended */ }
    else if (c.job) try { await api(`/turns/${c.job}`, 'DELETE'); } catch { /* it ends with the service */ }
    // The next pulse records how far it got.
    if (!c.job) c.streaming = false;
  } else if (op === 'effort' && EFFORTS[value]) {
    c.effort = value; await persist(storage);
  } else if (op === 'model') {
    const option = [DEVICE, ...FEATURED, ...catalogCache].find((m) => m.id === value);
    if (option && !c.streaming) { c.session = ''; c.model = option.id; c.modelName = option.name; c.maker = option.maker; await persist(storage); }
  } else if (op === 'delete') {
    if (c.session) try { await destroyLanguageModel({ session: c.session }); } catch { /* gone with it */ }
    if (c.job && c.model !== DEVICE.id) try { await api(`/turns/${c.job}`, 'DELETE'); } catch { /* gone with it */ }
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
