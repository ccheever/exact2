import type { Answer, Overlay, Storage } from './app.contract.d.ts';
import { MODELS, blank, fixtures, presentation, time, type Thread, type Message } from './model';
export const appId = 'com.exact.messages';
export const grants = 'net.fetch http://127.0.0.1:4318\nfs.read app:/data/messages.json\nfs.write app:/data/messages.json';
const SERVICE = 'http://127.0.0.1:4318';
let threads = fixtures(), models = MODELS.slice(), revision = 0, serial = 0;
let loaded = false;
let connected = false, notice = '', lastHealth = -100000;
const find = (id: string) => threads.find(t => t.id === id);
const change = (id = '', message = '') => ({ revision: ++revision, id, message });
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
const path = 'app:/data/messages.json';
async function persist(storage: Storage) {
  const bytes = new TextEncoder().encode(JSON.stringify({ version: 1, threads, serial }));
  // Initiate the atomic write in this answer's own storage turn. Sharing an
  // in-flight promise between answers loses the native executor's owner.
  try { await storage.fs.atomicWriteFile(path, bytes); notice = ''; } catch (e) { notice = `Could not save this conversation: ${errorText(e)}`; }
}
async function load(storage: Storage) {
  if (loaded) return;
  try {
    const data = JSON.parse(new TextDecoder().decode(await storage.fs.readFile(path)));
    if (!loaded && data.version === 1 && Array.isArray(data.threads) && data.threads.every((t: Thread) => typeof t.id === 'string' && Array.isArray(t.messages))) {
      threads = data.threads; serial = Number(data.serial) || 0;
      for (const t of threads) for (const m of t.messages) m.fresh = false;
    }
  } catch { /* First launch has the baked reference conversation. */ }
  loaded = true;
}
async function api(route: string, method = 'GET', body?: unknown) {
  const response = await fetch(SERVICE + route, { method, headers: { 'Content-Type': 'application/json' }, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  const data = await response.json() as any;
  if (!response.ok) throw new Error(data.error || `Messages service returned ${response.status}.`);
  return data;
}
function add(t: Thread, from: string, body: string, at: number, reply = ''): Message {
  const m: Message = { id: `m-${++serial}`, from, body, at, reply, status: '', reaction: '', fresh: true, edited: false };
  t.messages.push(m); return m;
}
function snapshot(query: string, id: string, at: number, offset = 0) {
  const t = find(id) || threads[0] || fixtures()[0], q = query.toLowerCase();
  return {
    revision, connected, notice,
    threads: threads.filter(t => !q || `${t.name} ${t.messages.map(m => m.body).join(' ')}`.toLowerCase().includes(q)).map(t => ({
      id: t.id, name: t.name, initials: t.initials, color: t.color, model: t.model, unread: t.unread, muted: t.muted,
      preview: t.pending ? 'Typing…' : t.draft ? `Draft: ${t.draft}` : t.messages[t.messages.length - 1]?.body || 'Start a conversation',
      time: t.messages.length ? time(t.messages[t.messages.length - 1]!.at, offset) : '', draft: t.draft,
    })),
    id: t.id, name: t.name, initials: t.initials, color: t.color, model: t.model, draft: t.draft,
    messages: presentation(t, at, offset), typing: t.pending && !t.messages.find(m => m.id === t.response)?.body,
    busy: t.pending, error: t.error, count: t.messages.length,
    latest: t.messages[t.messages.length - 1]?.id || '',
  };
}
async function start(t: Thread, user: Message, storage: Storage) {
  if (!t.model) return;
  t.pending = true; t.error = ''; t.response = '';
  try {
    const data = await api('/turns', 'POST', { id: `${t.id}-${user.id}-${user.at}`, model: t.model,
      messages: t.messages.filter(m => m.body).slice(-60).map(m => ({ role: m.from === 'me' ? 'user' : 'assistant', content: m.body })) });
    t.job = data.id; connected = true;
  } catch (e) { t.pending = false; t.error = errorText(e).includes('fetch') ? 'Start the Messages AI service on your Mac, then tap Try Again.' : errorText(e); }
  revision++; await persist(storage);
}
async function pulse(at: number, storage: Storage) {
  await load(storage);
  const jobs = threads.filter(t => t.pending && t.job);
  await Promise.all(jobs.map(async t => {
    try {
      const job = await api(`/turns/${t.job}`);
      if (!t.pending) return;
      if (job.content) {
        let response = t.messages.find(m => m.id === t.response);
        if (!response) { response = add(t, t.name, '', at); t.response = response.id; }
        response.body = job.content;
      }
      if (job.done) { t.pending = false; t.job = ''; t.error = job.error; await persist(storage); }
    } catch { t.pending = false; t.job = ''; t.error = 'Connection lost. Your message is saved. Tap Try Again.'; await persist(storage); }
  }));
  if (at - lastHealth > 10000) {
    lastHealth = at;
    try { connected = !!(await api('/health')).connected; } catch { connected = false; }
  }
  return change();
}
async function command(op: string, id: string, value: string, at: number, storage: Storage, reply = '') {
  await load(storage);
  const t = find(id);
  if (op === 'new') {
    const model = models.find(m => m.id === value) || MODELS.find(m => m.id === value);
    if (!model) return change('', 'Choose an available model.');
    const created = blank(model, `thread-${++serial}`); threads.splice(1, 0, created); await persist(storage); return change(created.id);
  }
  if (!t) return change();
  let result = '';
  if (op === 'send' && value.trim() && !t.pending) {
    const m = add(t, 'me', value.trim(), at, reply); result = m.id; t.draft = ''; t.error = '';
    await persist(storage); await start(t, m, storage);
  } else if (op === 'retry' && !t.pending) {
    const last = [...t.messages].reverse().find(m => m.from === 'me');
    if (last) { if (t.response) t.messages = t.messages.filter(m => m.id !== t.response); t.response = ''; last.at = at; await start(t, last, storage); }
  } else if (op === 'stop') {
    t.pending = false;
    if (t.job) try { await api(`/turns/${t.job}`, 'DELETE'); } catch {}
    t.job = ''; t.error = ''; await persist(storage);
  } else if (op === 'draft') { t.draft = value; await persist(storage); }
  else if (op === 'read') { t.unread = false; await persist(storage); }
  else if (op === 'unread') { t.unread = !t.unread; await persist(storage); }
  else if (op === 'mute') { t.muted = !t.muted; await persist(storage); }
  else if (op === 'delete-thread' && !t.pending) { threads = threads.filter(x => x !== t); await persist(storage); }
  else if (op === 'delete' && !t.pending) { t.messages = t.messages.filter(m => m.id !== value); await persist(storage); }
  else if (op === 'reaction') { const [mid, glyph] = value.split('|'); const m = t.messages.find(m => m.id === mid); if (m) m.reaction = m.reaction === glyph ? '' : glyph; await persist(storage); }
  else if (op === 'model' && !t.pending) { const m = models.find(m => m.id === value); if (m) { t.model = m.id; t.name = m.name; t.color = m.color; t.initials = m.initials; } await persist(storage); }
  else if (id === 'demo') {
    if (op === 'receive') add(t, 'Ada Lovelace', 'Scroll up first — you should stay where you are.', at);
    if (op === 'fifty') for (let i = 0; i < 50; i++) add(t, i % 3 === 1 ? 'me' : ['Ada Lovelace', 'Grace Hopper', 'Alan Turing'][i % 3], `Message ${t.messages.length + 1}, ${i % 3 === 1 ? 'sent.' : 'from the conversation.'}`, at);
    if (op === 'reset') { t.messages = fixtures()[0].messages; t.draft = ''; }
    if (op === 'typing') t.pending = !t.pending;
    if (op === 'long') add(t, 'Ada Lovelace', Array(100).fill('A long message opens in the reader, where the full text is selectable. ').join(''), at);
    await persist(storage);
  }
  return change(result);
}
async function catalog(query: string, refresh: number) {
  if (refresh > 0 && models.length === MODELS.length) try { models = (await api('/models')).models; } catch {}
  const q = query.toLowerCase();
  return models.filter(m => `${m.name} ${m.maker} ${m.id}`.toLowerCase().includes(q)).slice(0, 80);
}
type Snapshot = ReturnType<typeof snapshot>;
// A sent message shows in its conversation from the send until an answer has it, as the message the service
// adds: from "me", with the sent text, at the send's time. One the service ignores (a blank text, or one sent
// while a reply is coming) shows until its reply, which carries no id. The inbox previews it as the answer
// will, "Typing…" when a model answers.
export const overlay: Overlay = (source, args, answer, writes) => {
  if (source !== 'snapshot') return undefined;
  const shown = answer as Snapshot, [, , , clock, offset] = args as [string, string, number, number, number];
  const sent = writes.flatMap(w => {
    if (w.source !== 'sendMessage' || w.args[0] !== shown.id || w.reply?.id === '') return [];
    const body = w.args[1].trim(), at = w.args[3];
    if (!body || shown.messages.some(m => m.from === 'me' && m.at === at && m.body === body)) return [];
    if (w.answered) w.keep();
    return [{ id: `sending-${w.id}`, from: 'me', body, at, reply: w.args[2], status: '', reaction: '', fresh: true, edited: false }];
  });
  if (!sent.length) return undefined;
  const messages = presentation({ model: shown.model, messages: [...shown.messages, ...sent] } as Thread, clock, offset);
  const last = sent[sent.length - 1]!;
  return { ...shown, messages, count: messages.length, latest: last.id, draft: '',
    threads: shown.threads.map(t => t.id === shown.id ? { ...t, preview: shown.model ? 'Typing…' : last.body, time: time(last.at, offset), draft: '' } : t) } as never;
};
export const answer: Answer = ((source: string, args: any[], _store: unknown, storage: Storage) => {
  if (source === 'snapshot') return snapshot(args[0], args[1], args[3], args[4]);
  if (source === 'pulse') return pulse(args[0], storage);
  if (source === 'command') return command(args[0], args[1], args[2], args[3], storage);
  if (source === 'sendMessage') return command('send', args[0], args[1], args[3], storage, args[2]);
  if (source === 'catalog') return catalog(args[0], args[1]);
  throw new Error(`Unknown Messages source: ${source}`);
}) as Answer;
