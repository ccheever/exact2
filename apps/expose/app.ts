// Expose, the phone-OS demo: everything below the seam. The Contract owns the
// screens; this module owns the words behind them — the assistant thread that
// answers through OpenRouter, the two authored human threads, the Reader's
// articles, a live Palo Alto forecast from Open-Meteo, and the clock's
// strings. The OpenRouter key comes from Settings → Intelligence (kept under
// `secret.keep`) or, failing that, from `local.ts`, which build.rs writes from
// EXPOSE_OPENROUTER_KEY and git never sees.
import type { Answer, Sources, Result, Store } from './app.contract.d.ts';
import { bakedKey } from './local';

export const appId = 'com.exact.expose';
export const grants = [
  'net.fetch https://openrouter.ai',
  'net.fetch https://api.open-meteo.com',
  'secret.keep assistant.key',
  'secret.keep assistant.model',
].join('\n');

type Clock = Result<'clock'>;
type Inbox = Result<'inbox'>;
type Thread = Inbox['threads'][number];
type Conversation = Result<'conversation'>;
type Message = Conversation['messages'][number];
type Turn = Result<'ask'>;
type Library = Result<'library'>;
type Article = Library['articles'][number];
type Weather = Result<'weather'>;
type Intelligence = Result<'intelligence'>;

// ----------------------------------------------------------------- the clock
const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
// The wall clock is the runner's epoch shifted by the device's fixed UTC
// offset (LLP 1027.000.000), read through UTC getters: no ambient time here.
function clock(minuteKey: number, utcOffset: number): Clock {
  const d = new Date(minuteKey * 60000 + utcOffset * 60000);
  const h = d.getUTCHours(), m = d.getUTCMinutes();
  const hour12 = h % 12 || 12;
  const time = `${hour12}:${m < 10 ? '0' : ''}${m}`;
  const greeting = h < 5 ? 'Still up?' : h < 12 ? 'Good morning' : h < 17 ? 'Good afternoon' : h < 21 ? 'Good evening' : 'Good night';
  return {
    time,
    ampm: h < 12 ? 'AM' : 'PM',
    weekday: WEEKDAYS[d.getUTCDay()],
    date: `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`,
    day: String(d.getUTCDate()),
    month: MONTHS[d.getUTCMonth()].slice(0, 3).toUpperCase(),
    greeting,
  };
}
function stamp(epochMs: number): string {
  const d = new Date(epochMs);
  const h = d.getUTCHours(), m = d.getUTCMinutes();
  return `${h % 12 || 12}:${m < 10 ? '0' : ''}${m} ${h < 12 ? 'AM' : 'PM'}`;
}

// --------------------------------------------------------------- the threads
interface Person { id: string; name: string; initials: string; kind: string; time: string; unread: boolean }
const people: Person[] = [
  { id: 'expose', name: 'Expose', initials: 'E', kind: 'assistant', time: 'now', unread: false },
  { id: 'nico', name: 'Nico', initials: 'N', kind: 'person', time: '9:12 AM', unread: true },
  { id: 'crew', name: 'Type Crew', initials: 'TC', kind: 'person', time: '8:47 AM', unread: true },
  { id: 'builds', name: 'Expose Builds', initials: 'Ex', kind: 'person', time: 'Yesterday', unread: false },
  { id: 'mom', name: 'Mom', initials: 'M', kind: 'person', time: 'Saturday', unread: false },
];
let counter = 0;
const msg = (thread: string, role: string, sender: string, body: string, time: string): Message => ({ id: `${thread}-${++counter}`, role, sender, body, time, streaming: false });
const threads = new Map<string, Message[]>([
  ['expose', [
    msg('expose', 'assistant', 'Expose', 'Hi, I’m Expose — the assistant that lives in this phone. Ask me anything, or ask about what you’re reading in Reader. I’m set in Exposure Sans; the titles around me are Exciting Lilt.', '9:41 AM'),
  ]],
  ['nico', [
    msg('nico', 'them', 'Nico', 'Got the v18 Exciting build onto the Pink phone', '8:31 AM'),
    msg('nico', 'them', 'Nico', 'The clock in Exciting Black is *loud*. In a good way. It reads from across the room.', '8:32 AM'),
    msg('nico', 'me', 'Me', 'That was the whole point of the capital room pass', '8:40 AM'),
    msg('nico', 'me', 'Me', 'How does Lilt feel next to it for the titles?', '8:41 AM'),
    msg('nico', 'them', 'Nico', 'Better than I expected. Exciting for the one loud thing, Lilt for the names, Sans for everything you actually read. Three voices, no arguing.', '9:10 AM'),
    msg('nico', 'them', 'Nico', 'Can you push the build to Silver tonight? I want to see the lock screen on the 4a Pro.', '9:12 AM'),
  ]],
  ['crew', [
    msg('crew', 'them', 'James', 'New Exposure Sans 10 is in the branch. The ẞ and ₹ are drawn now, not borrowed.', '8:20 AM'),
    msg('crew', 'them', 'Nico', 'The italic a is still the best glyph in the family, fight me', '8:31 AM'),
    msg('crew', 'them', 'James', 'I still think the g needs a smaller ear', '8:47 AM'),
    msg('crew', 'me', 'Me', 'Ship the ear as is. We tune it after we see it on the lock screen.', '8:52 AM'),
  ]],
  ['builds', [
    msg('builds', 'them', 'Expose Builds', 'N-Silver rung 11 audio probe: attended run passed. 3 of 3 sine sweeps audible on the speaker.', '11:20 PM'),
    msg('builds', 'them', 'Expose Builds', 'Next: Maple playback root staging for Lavender.', '11:21 PM'),
  ]],
  ['mom', [
    msg('mom', 'them', 'Mom', 'Did you eat?', '6:02 PM'),
    msg('mom', 'me', 'Me', 'Yes 😄 pasta', '6:15 PM'),
    msg('mom', 'them', 'Mom', 'Good. Call me when you land 🩵', '6:16 PM'),
  ]],
]);
let revision = 0;
const streaming = new Map<string, boolean>();
const errors = new Map<string, string>();
const read = new Set<string>();

function preview(thread: string): string {
  const rows = threads.get(thread) ?? [];
  const last = rows[rows.length - 1];
  if (!last) return '';
  const body = last.body.replace(/\s+/g, ' ').trim();
  return last.role === 'me' ? `You: ${body}` : body;
}
function inbox(): Inbox {
  const rows: Thread[] = people.map((p) => {
    const rows = threads.get(p.id) ?? [];
    const last = rows[rows.length - 1];
    return { id: p.id, name: p.name, initials: p.initials, kind: p.kind, preview: preview(p.id) || 'Say hello', time: p.id === 'expose' && last ? last.time : p.time, unread: p.unread && !read.has(p.id) };
  });
  return { threads: rows, unread: rows.filter((r) => r.unread).length };
}
function conversation(id: string): Conversation {
  const p = people.find((x) => x.id === id);
  if (!p) return { id: '', name: '', initials: '', kind: '', messages: [], streaming: false, error: '' };
  read.add(id);
  return { id, name: p.name, initials: p.initials, kind: p.kind, messages: threads.get(id) ?? [], streaming: streaming.get(id) ?? false, error: errors.get(id) ?? '' };
}

// ------------------------------------------------------------- the assistant
const MODELS: { id: string; name: string; maker: string }[] = [
  { id: 'anthropic/claude-sonnet-5.5', name: 'Claude Sonnet 5.5', maker: 'Anthropic' },
  { id: 'anthropic/claude-opus-5.5', name: 'Claude Opus 5.5', maker: 'Anthropic' },
  { id: 'anthropic/claude-fable-5.1', name: 'Claude Fable 5.1', maker: 'Anthropic' },
  { id: 'openai/gpt-5.6-sol', name: 'GPT-5.6 Sol', maker: 'OpenAI' },
  { id: 'google/gemini-3.8-flash', name: 'Gemini 3.8 Flash', maker: 'Google' },
  { id: 'x-ai/grok-4.7', name: 'Grok 4.7', maker: 'xAI' },
];
const DEFAULT_MODEL = MODELS[0].id;
const SYSTEM = (model: string) => [
  'You are Expose, the assistant built into the Expose phone — a new mobile operating system that runs on Nothing and Pixel hardware, built by a small team led by Charlie Cheever (the creator of Expo).',
  'You are talking inside the phone’s Messages app, so answer like a thoughtful friend texting back: warm, direct, a little playful, and short. One to four sentences unless the person clearly wants more. No headings, no bullet lists, no markdown; plain text only.',
  'Things you know about this phone: the system UI is grayscale-first, with colour reserved for content. Its three typefaces are Exciting (a heavy display face inspired by Chicago, used for the clock and app marks), Exciting Lilt (the titling face, Semibold and Bold), and Exposure Sans (the text face you are set in). The apps in this demo are Messages, Reader, Settings and the lock and home screens.',
  'If someone asks what you can do, tell them honestly: you can chat, answer questions, and talk about the articles in Reader when they are shared with you. You cannot set alarms, send messages to other people, or control the phone.',
  `The model thinking for you right now is ${model}, reached through OpenRouter; the person can change it in Settings → Intelligence.`,
].join('\n');
function keyFor(store: Store): { key: string; source: string } {
  const kept = store.get('assistant.key');
  if (kept && kept.trim()) return { key: kept.trim(), source: 'settings' };
  // A string, whatever local.ts holds: an unset key's `""` would narrow to never.
  const baked: string = bakedKey;
  if (baked.trim()) return { key: baked.trim(), source: 'baked' };
  return { key: '', source: 'none' };
}
function modelFor(store: Store): string {
  const kept = store.get('assistant.model');
  return kept && MODELS.some((m) => m.id === kept) ? kept : DEFAULT_MODEL;
}
function turn(done: boolean, error = ''): Turn {
  return { revision, done, error };
}
function finish(thread: string, error = ''): Turn {
  streaming.set(thread, false);
  if (error) errors.set(thread, error);
  const rows = threads.get(thread) ?? [];
  const last = rows[rows.length - 1];
  if (last && last.role === 'assistant') {
    last.streaming = false;
    if (!last.body.trim()) last.body = error ? 'Something went wrong on the way back. Try once more?' : '…';
  }
  revision++;
  return turn(true, error);
}
// One turn: the person's words go in, the assistant's answer comes back;
// `conversation(thread, revision)` re-asks as the revision moves.
function ask(thread: string, text: string, article: string, epochMs: number, store: Store): Turn | Promise<Turn> {
  const body = text.trim();
  const rows = threads.get(thread);
  if (!rows || thread !== 'expose') return turn(true, 'This thread is authored; only Expose answers.');
  if (streaming.get(thread)) return turn(false, 'Expose is still answering.');
  const piece = article ? articles.find((a) => a.id === article) : undefined;
  if (!body && !piece) return turn(true);
  errors.delete(thread);
  const shown = piece ? (body || `What do you make of “${piece.title}”?`) : body;
  rows.push(msg(thread, 'me', 'Me', shown, stamp(epochMs)));
  const reply = msg(thread, 'assistant', 'Expose', '', stamp(epochMs));
  reply.streaming = true;
  rows.push(reply);
  streaming.set(thread, true);
  revision++;
  const { key } = keyFor(store);
  if (!key) {
    reply.body = 'I don’t have a key to think with yet. Add an OpenRouter key in Settings → Intelligence and I’ll be right here.';
    return finish(thread);
  }
  // The model sees the whole thread, and the article when one was shared.
  const history = rows.slice(0, -1).slice(-24).map((m) => ({ role: m.role === 'assistant' ? 'assistant' : 'user', content: m.body }));
  if (piece) {
    const last = history[history.length - 1];
    last.content = `${shown}\n\n(I’m sharing this article from Reader.)\n\n${piece.section} — ${piece.title}\n${piece.deck}\nBy ${piece.author}\n\n${piece.body.join('\n\n')}`;
  }
  // A host with the native executor (Apple, Linux, the wasm web host) keeps
  // only the newest undelivered stream message (LLP 1016.000 D4), which for a
  // token stream means dropped words. There the answer is one JSON body. The
  // page runtime runs this module with the browser's own fetch, so the
  // response resolves and its events are read from the body as they arrive;
  // the Contract re-asks the conversation once a second while the reply is
  // pending, so the bubble grows there.
  const native = typeof (globalThis as { __exact_message?: unknown }).__exact_message === 'function';
  const model = modelFor(store);
  const request = {
    model,
    stream: !native,
    max_tokens: 700,
    messages: [{ role: 'system', content: SYSTEM(MODELS.find((m) => m.id === model)?.name ?? model) }, ...history],
  };
  // One event's `data:` payload.
  const consume = (data: string): Turn | null => {
    if (!data) return null;
    if (data === '[DONE]') return finish(thread);
    try {
      const parsed = JSON.parse(data) as { choices?: { delta?: { content?: string } }[]; error?: { message?: string } };
      if (parsed.error?.message) return finish(thread, parsed.error.message);
      const delta = parsed.choices?.[0]?.delta?.content;
      if (typeof delta === 'string' && delta) {
        reply.body += delta;
        revision++;
      }
    } catch {
      // A keep-alive comment or a partial line: nothing to add yet.
    }
    return null;
  };
  return fetch('https://openrouter.ai/api/v1/chat/completions', {
    method: 'POST',
    headers: { 'Authorization': `Bearer ${key}`, 'Content-Type': 'application/json', 'HTTP-Referer': 'https://expose.dev', 'X-Title': 'Expose' },
    body: JSON.stringify(request),
  }).then(async (response) => {
    if (!streaming.get(thread)) return turn(true);
    if (!response.ok) return finish(thread, `Expose couldn’t reach the model (${response.status}).`);
    if (native) {
      const parsed = (await response.json()) as { choices?: { message?: { content?: string } }[]; error?: { message?: string } };
      const content = parsed.choices?.[0]?.message?.content;
      if (typeof content === 'string') reply.body = content.trim();
      return finish(thread, parsed.error?.message ?? '');
    }
    const reader = response.body?.getReader();
    let buffer = '';
    const lines = (chunk: string, flush: boolean): Turn | null => {
      buffer += chunk;
      const parts = buffer.split('\n');
      buffer = flush ? '' : parts.pop() ?? '';
      for (const line of parts) {
        if (!line.startsWith('data:')) continue;
        const ended = consume(line.slice(5).trim());
        if (ended) return ended;
      }
      return null;
    };
    if (!reader) return lines(await response.text(), true) ?? finish(thread);
    const decoder = new TextDecoder();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      const ended = lines(decoder.decode(value, { stream: true }), false);
      if (ended) return ended;
    }
    return lines(decoder.decode(), true) ?? finish(thread);
  }, (error: unknown) => finish(thread, `Expose couldn’t reach the model. ${error instanceof Error ? error.message : String(error)}`));
}
function clearThread(thread: string): Turn {
  if (thread === 'expose' && !streaming.get('expose')) {
    const rows = threads.get('expose')!;
    rows.splice(1);
    errors.delete('expose');
    revision++;
  }
  return turn(true);
}
function intelligence(store: Store): Intelligence {
  const { key, source } = keyFor(store);
  const model = modelFor(store);
  return {
    hasKey: !!key,
    keySource: source,
    keyHint: key ? `${key.slice(0, 9)}…${key.slice(-4)}` : '',
    model,
    modelName: MODELS.find((m) => m.id === model)?.name ?? model,
    models: MODELS.map((m) => ({ id: m.id, name: m.name, maker: m.maker, selected: m.id === model })),
  };
}
function saveKey(value: string, store: Store): Turn {
  const key = value.trim();
  if (key) store.set('assistant.key', key); else store.forget('assistant.key');
  revision++;
  return turn(true);
}
function saveModel(value: string, store: Store): Turn {
  if (MODELS.some((m) => m.id === value)) store.set('assistant.model', value);
  revision++;
  return turn(true);
}

// ----------------------------------------------------------------- the reader
const articles: Article[] = [
  {
    id: 'loud-clock', section: 'Design', minutes: 4, author: 'Nico', posted: 'Today',
    title: 'The clock is the loudest thing on your phone',
    deck: 'You look at it two hundred times a day. It should be the one place the system is allowed to shout.',
    pull: 'One loud thing, then quiet.',
    body: [
      'Every phone shows you the time before it shows you anything else. It is the first thing on the lock screen and the last thing you see before the screen goes dark, and most systems set it in the same face they use for a settings row: a polite, neutral sans, a little larger. It reads fine. It says nothing.',
      'On Expose the clock is set in Exciting, a heavy display face with a flat-topped A and a spiral for an at-sign. It is not a text face and it never pretends to be. It is drawn for the moments the system speaks loudest — the time, the mark on an app tile, a number you need from across the room — and for almost nothing else.',
      'That restraint is the whole trick. If the clock is allowed to be loud, the rest of the screen can be quiet. The date underneath it is Exposure Sans at seventeen pixels; the notifications are Lilt for the names and Sans for the words. Three voices, one of them shouting, and the shouting one is the one you were going to look at anyway.',
      'We tried it the other way for a week. A calmer clock, a busier home screen. Nobody could say what time it was without looking twice.',
    ],
  },
  {
    id: 'grayscale', section: 'Principles', minutes: 5, author: 'Charlie Cheever', posted: 'Yesterday',
    title: 'A grayscale operating system',
    deck: 'Colour is for what you put on the phone, not for the phone.',
    pull: 'The system is the frame. Your life is the picture.',
    body: [
      'Open the home screen of most phones and count the colours. Twenty tiles, twenty saturated squares, each designed by a different company to win the same square inch of your attention. The result is a wall of small shouts. Nothing on it is more important than anything else, so nothing on it is important.',
      'Expose draws its own chrome in gray. The status bar, the settings rows, the message list, the tiles for the apps it ships with — all of it sits somewhere between near-black and near-white. When something on the screen is colour, it is colour because it came from you: a photo, a message from a friend with a heart in it, a page in Reader with a red pull quote the author chose.',
      'There is exactly one colour the system keeps for itself, a warm orange, and it uses it to mean one thing: this needs you. An unread dot. The send button when there is something to send. The cursor while the assistant is still writing. It shows up, it does its job, and it leaves.',
      'The hard part was not the palette. The hard part was saying no to the tile that wanted to be blue because blue tests well. It probably does. It also makes the phone about the tile.',
      'We think of the system as a frame. A good frame is the same colour all the way around.',
    ],
  },
  {
    id: 'lilt', section: 'Type', minutes: 6, author: 'Type Crew', posted: 'Yesterday',
    title: 'Lilt, or why the titles have a flare',
    deck: 'Exciting Lilt is what happens when a display face learns to be pleasant at seventeen pixels.',
    pull: 'A title should be recognisable, not merely legible.',
    body: [
      'Exciting is a wonderful face for one word at a time. Set a sentence in it and the sentence becomes a wall. So the titles on Expose needed something related but lighter on its feet: the same skeleton, the same flat apex on the A, but with terminals that lift a little at the end of a stroke — a lilt, which is what we ended up calling it.',
      'Lilt comes in two weights. Bold carries titles and actions from seventeen to thirty-six pixels, where it matches the colour of Exposure Sans Bold in the same line. Semibold carries the large numerals in widgets and anything set at forty-eight pixels and above, where Bold starts to clot.',
      'The interesting problems were all at the small end. At seventeen pixels a terminal flare is two pixels wide, and two pixels is exactly the size where a hint that helps a stem hurts a curve. Version four of the face is where the t stopped being mush. Version ten is where the Bold got heavy enough to hold its own next to a Sans paragraph without looking like it was apologising.',
      'The rule we ended up with is simple to say and hard to keep: Lilt never sets a paragraph. If you find yourself wanting to, the paragraph is too important to be a title and should be a paragraph.',
      'The g still has a big ear. James is right about it. We are shipping it anyway, and we will tune it once we have seen it on a lock screen for a month.',
    ],
  },
  {
    id: 'nine-days', section: 'Field notes', minutes: 7, author: 'Expose Builds', posted: 'Monday',
    title: 'The Nothing phone that ran Expose for nine days',
    deck: 'A bring-up diary from the bench: what worked, what rebooted, and what the speaker said when it finally spoke.',
    pull: 'Rung eleven: the speaker played a sine sweep and four people cheered.',
    body: [
      'Silver is a Nothing Phone (4a) Pro with a cracked corner and the best-documented USB port in the building. It has been the primary bring-up device since the reset in July, and last week it ran a build of Expose continuously for nine days without a bench operator touching it.',
      'The ladder we climb is written down as rungs. Rung one is a kernel that boots and stays up. Rung two is a framebuffer we own. Rung three is the Glyph matrix, which on this phone is a single LED node and on the next one will not be. Rung five is the host tooling that lets an agent flash a phone without a person holding the cable. Rung eleven, the one we just passed, is audio: a sine sweep out of the loudspeaker, audible to a human, three times in a row.',
      'Most of what went wrong was not on the phone. It was in the room. A USB hub that negotiated the wrong role depending on which host it was plugged into. A build machine whose clock drifted enough to confuse a signed receipt. A Windows bench script that stopped working when someone renamed a folder to be helpful.',
      'What worked was boring on purpose. Every phone in the fleet has a colour name and a serial pinned to a file; nobody says “the Pixel” in a message any more, because there are three of them. Every flash is a script with a log. Every rung has a test a person can watch.',
      'Nine days is not a product. It is, though, the first time the bench has been quiet long enough for the type people to ask for a build with the new fonts on it, and that is how this issue of Reader came to exist.',
    ],
  },
  {
    id: 'assistant', section: 'Intelligence', minutes: 3, author: 'Charlie Cheever', posted: 'Sunday',
    title: 'An assistant that lives in Messages',
    deck: 'Not a separate app, not a floating orb. A thread, pinned to the top, that answers.',
    pull: 'It knows what you are reading because you told it.',
    body: [
      'The assistant on Expose is a conversation in Messages. That sounds like a small decision and it is a large one: it means the assistant has a place, a history, a name and a face like everyone else you talk to, and it means asking it something costs exactly what texting a friend costs.',
      'It also means it can be interrupted. If you leave the thread while it is writing, the answer finishes in the background and arrives as a notification, the way a person’s reply would.',
      'What it knows is deliberately narrow. It knows about the phone. It knows about an article when you share one with it from Reader, and not before. It does not read your other threads and it will tell you so if you ask.',
      'The model behind it is whichever one you pick in Settings — today the default is Claude Sonnet, routed through OpenRouter with a key you paste in once. The phone remembers the key; the model does not remember you between threads.',
    ],
  },
];
function library(): Library {
  return { articles, lead: articles[0].id };
}

// ---------------------------------------------------------------- the weather
type Json = Record<string, unknown>;
const EMPTY_WEATHER: Weather = { ready: false, temp: '—°', condition: 'Fetching the sky', high: '—', low: '—', place: 'Palo Alto', hours: [] };
let lastWeather: Weather = EMPTY_WEATHER;
function condition(code: number, day: boolean): string {
  if (code === 0) return day ? 'Clear' : 'Clear night';
  if (code === 1) return 'Mostly clear';
  if (code === 2) return 'Partly cloudy';
  if (code === 3) return 'Overcast';
  if (code === 45 || code === 48) return 'Fog';
  if (code >= 51 && code <= 57) return 'Drizzle';
  if (code >= 61 && code <= 67) return 'Rain';
  if (code >= 71 && code <= 77) return 'Snow';
  if (code >= 80 && code <= 82) return 'Showers';
  if (code >= 95) return 'Thunderstorms';
  return 'Changeable';
}
function glyph(code: number, day: boolean): string {
  if (code <= 1) return day ? '○' : '☾';
  if (code === 2) return '◐';
  if (code <= 48) return '●';
  if (code >= 95) return 'ϟ';
  return '☂';
}
async function weather(revision: number, utcOffset: number): Promise<Weather> {
  if (revision === 0) return EMPTY_WEATHER;
  try {
    const url = 'https://api.open-meteo.com/v1/forecast?latitude=37.4419&longitude=-122.143&timezone=auto&forecast_days=2&temperature_unit=fahrenheit&current=temperature_2m,weather_code,is_day&hourly=temperature_2m,weather_code,is_day&daily=temperature_2m_max,temperature_2m_min';
    const response = await fetch(url);
    if (!response.ok) throw new Error(`weather ${response.status}`);
    const data = (await response.json()) as Json;
    const current = data.current as Json, hourly = data.hourly as Json, daily = data.daily as Json;
    const times = hourly.time as string[], temps = hourly.temperature_2m as number[], codes = hourly.weather_code as number[], days = hourly.is_day as number[];
    const now = String(current.time);
    const start = Math.max(0, times.findIndex((t) => t.slice(0, 13) === now.slice(0, 13)));
    const hours = times.slice(start + 1, start + 7).map((t, i) => {
      const h = Number(t.slice(11, 13));
      return { id: t, label: `${h % 12 || 12}${h < 12 ? 'a' : 'p'}`, temp: `${Math.round(temps[start + 1 + i])}°`, glyph: glyph(codes[start + 1 + i], days[start + 1 + i] === 1) };
    });
    lastWeather = {
      ready: true,
      temp: `${Math.round(Number(current.temperature_2m))}°`,
      condition: condition(Number(current.weather_code), Number(current.is_day) === 1),
      high: `${Math.round((daily.temperature_2m_max as number[])[0])}°`,
      low: `${Math.round((daily.temperature_2m_min as number[])[0])}°`,
      place: 'Palo Alto',
      hours,
    };
    void utcOffset;
    return lastWeather;
  } catch {
    return lastWeather.ready ? lastWeather : { ...EMPTY_WEATHER, condition: 'No forecast right now' };
  }
}

const sources: Sources = {
  clock: ([minuteKey, utcOffset]) => clock(minuteKey, utcOffset),
  inbox: () => inbox(),
  conversation: ([id]) => conversation(id),
  ask: ([thread, text, article, epochMs], store) => ask(thread, text, article, epochMs, store),
  clearThread: ([thread]) => clearThread(thread),
  library: () => library(),
  weather: ([revision, utcOffset]) => weather(revision, utcOffset),
  intelligence: (_args, store) => intelligence(store),
  saveKey: ([value], store) => saveKey(value, store),
  saveModel: ([value], store) => saveModel(value, store),
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
