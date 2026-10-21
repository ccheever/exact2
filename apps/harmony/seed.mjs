// Harmony's demo data: six people, a handful of channels, three days of chat,
// reactions, pins, a few direct messages and custom emoji. Writes seed.json,
// which the tests load with `fresh "../seed.json"` and `bun exact.mjs seed`
// posts to the running dev server. Times are relative to when this runs.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
let counter = 0;
const id = () => {
  let n = ++counter, s = '';
  while (n > 0) { s = CROCKFORD[n % 32] + s; n = Math.floor(n / 32); }
  return ('01HARMNY' + s.padStart(18, '0'));
};

const rows = [];
const put = (table, row) => { rows.push({ table, row }); return row; };

const now = Date.now();
const H = 3600_000, M = 60_000, D = 24 * H;
// A clock time on a past day; today's messages are spread over the last
// six hours instead, so they stay in order before anything sent now.
const at = (daysAgo, hour, minute = 0) => {
  if (daysAgo === 0) return now - (24 - hour) * 15 * M - (60 - minute) * 4000 - 30_000;
  const d = new Date(now - daysAgo * D);
  d.setHours(hour, minute, Math.floor(Math.random() * 50), 0);
  return d.getTime();
};

const people = {
  ada: { name: 'Ada Quill', handle: 'ada', status: 'Shipping Harmony 🚀', hue: 1 },
  grace: { name: 'Grace Marlowe', handle: 'grace', status: 'In the zone', hue: 2 },
  lin: { name: 'Lin Byrne', handle: 'lin', status: 'Debugging the universe', hue: 3 },
  maggie: { name: 'Maggie Osei', handle: 'maggie', status: 'Coffee first ☕️', hue: 4 },
  alan: { name: 'Alan Reyes', handle: 'alan', status: 'On a walk', hue: 5 },
  ken: { name: 'Ken Takahashi', handle: 'ken', status: '🎧 lo-fi', hue: 6 },
};
const P = Object.fromEntries(Object.keys(people).map((k) => [k, `dev:${k}`]));
for (const [k, p] of Object.entries(people)) put('profiles', { id: id(), principal: P[k], ...p });

const channels = [
  ['Information', 'announcements', 'Team-wide news. Big things only.', 0],
  ['Text Channels', 'general', 'The whole team, all day', 10],
  ['Text Channels', 'random', 'Off-topic, memes and dogs', 11],
  ['Text Channels', 'design', 'Mockups, type and colour', 12],
  ['Text Channels', 'engineering', 'Builds, bugs and pull requests', 13],
  ['Text Channels', 'music', 'What are you listening to?', 14],
  ['Projects', 'harmony-launch', 'Getting Harmony out the door', 20],
  ['Projects', 'bake-off', 'Exact against the rest, round by round', 21],
];
const room = {};
for (const [category, name, topic, position] of channels) {
  const r = put('rooms', { id: id(), kind: 'text', name, topic, category, position, createdBy: P.ada, createdAt: at(30, 9) });
  room[name] = r.id;
  for (const k of Object.keys(people)) put('members', { id: id(), roomId: r.id, principal: P[k], joinedAt: at(30, 9) });
}
const dm = (a, b) => {
  const r = put('rooms', { id: id(), kind: 'dm', name: '', topic: '', category: '', position: 0, createdBy: P[a], createdAt: at(5, 9) });
  put('members', { id: id(), roomId: r.id, principal: P[a], joinedAt: at(5, 9) });
  put('members', { id: id(), roomId: r.id, principal: P[b], joinedAt: at(5, 9) });
  return r.id;
};
room['dm:ada:grace'] = dm('ada', 'grace');
room['dm:ada:lin'] = dm('ada', 'lin');
room['dm:ada:ken'] = dm('ada', 'ken');
room['dm:grace:maggie'] = dm('grace', 'maggie');

const msgs = {};
let last = null;
// m(room, who, daysAgo, hour, minute, body, {reply, edited, key})
const m = (r, who, daysAgo, hour, minute, body, o = {}) => {
  const row = put('messages', { id: id(), roomId: room[r], authorId: P[who], body,
    replyToId: o.reply ? msgs[o.reply].id : null, editedAt: o.edited ? at(daysAgo, hour, minute + 2) : null, at: at(daysAgo, hour, minute) });
  if (o.key) msgs[o.key] = row;
  last = row;
  return row;
};
const react = (msg, who, emoji) => put('reactions', { id: id(), roomId: msg.roomId, messageId: msg.id, principal: P[who], emoji, at: msg.at + 5 * M });
const pin = (msg, who) => put('pins', { id: id(), roomId: msg.roomId, messageId: msg.id, pinnedBy: P[who], at: msg.at + 10 * M });

// #announcements
m('announcements', 'ada', 6, 9, 0, 'Welcome to Harmony! This is the team server for everything we are building. Channels on the left, people on the right, your stuff under You.', { key: 'welcome' });
pin(msgs.welcome, 'ada');
react(msgs.welcome, 'grace', '🎉'); react(msgs.welcome, 'lin', '🎉'); react(msgs.welcome, 'ken', '👋'); react(msgs.welcome, 'maggie', '🎉');
m('announcements', 'maggie', 2, 10, 30, 'Demo day is Friday at 2pm. We are showing Harmony on an iPhone, a Mac and in Safari, all from the same source file. Bring snacks.', { key: 'demo' });
react(msgs.demo, 'alan', '🔥'); react(msgs.demo, 'ada', '🔥'); react(msgs.demo, 'lin', ':ship:');
m('announcements', 'ada', 0, 8, 5, 'The web build is live at the usual place. Hard refresh if you still see yesterday’s header.');

// #general
m('general', 'grace', 3, 9, 12, 'Morning all. Coffee machine on the 4th floor is fixed.');
m('general', 'ken', 3, 9, 14, 'Hero.');
m('general', 'maggie', 3, 9, 20, '☕️☕️☕️');
m('general', 'lin', 3, 11, 2, 'Does anyone else find it odd that the thing we are building to replace our chat app is… a chat app', { key: 'odd' });
react(msgs.odd, 'ada', '😂'); react(msgs.odd, 'grace', '😂'); react(msgs.odd, 'ken', '😂'); react(msgs.odd, 'alan', '🤔');
m('general', 'ada', 3, 11, 4, 'It is the only honest benchmark. If we would not use it ourselves, nobody else should.', { reply: 'odd' });
m('general', 'alan', 2, 14, 30, 'Lunch at the ramen place? Leaving in 10.');
m('general', 'grace', 2, 14, 31, 'In.');
m('general', 'lin', 2, 14, 31, 'In, if someone orders for me. On a call until 2:45.');
m('general', 'ken', 2, 14, 35, 'Got you. Spicy miso as usual?');
m('general', 'lin', 2, 14, 36, 'You know me too well.');
m('general', 'maggie', 1, 10, 0, '@ada the Mac build from this morning looks great. The sidebar is doing the thing.', { key: 'sidebar' });
react(msgs.sidebar, 'ada', '❤️');
m('general', 'ada', 1, 10, 3, 'Zero lines of sidebar code. It is just the tab bar, and the Mac draws it as a sidebar because that is what a Mac does.', { reply: 'sidebar', key: 'zero' });
react(msgs.zero, 'grace', '🤯'); react(msgs.zero, 'alan', '🤯'); react(msgs.zero, 'ken', ':exact:');
m('general', 'grace', 0, 9, 2, 'Good morning. Three days to demo.');
m('general', 'ken', 0, 9, 10, 'Reactions landed last night, including custom emoji. Try :ship: on something.', { key: 'reactions' });
react(msgs.reactions, 'grace', ':ship:'); react(msgs.reactions, 'maggie', ':ship:'); react(msgs.reactions, 'lin', '🚀');
m('general', 'maggie', 0, 9, 12, 'Already did 😄');
m('general', 'alan', 0, 9, 40, 'Typing indicators are in too. Start typing in a channel and watch the other person’s screen.');

// #random
m('random', 'ken', 2, 16, 10, 'Dog tax: Biscuit discovered the leaf pile today. Biscuit was not prepared for the leaf pile.', { key: 'biscuit' });
react(msgs.biscuit, 'ada', '😂'); react(msgs.biscuit, 'grace', '❤️'); react(msgs.biscuit, 'maggie', '❤️'); react(msgs.biscuit, 'lin', '🐶'); react(msgs.biscuit, 'alan', '🐶');
m('random', 'maggie', 2, 16, 12, 'We need Biscuit at demo day.');
m('random', 'ken', 2, 16, 13, 'Biscuit does not do mornings.');
m('random', 'alan', 1, 12, 45, 'Hot take: tabs are better than spaces in a chat app because nobody can see them.');
m('random', 'lin', 1, 12, 46, 'Reported.');
m('random', 'grace', 0, 11, 20, 'Who keeps leaving the office plant in the dark. It has feelings.');
m('random', 'ada', 0, 11, 25, 'The plant has been moved. The plant is thriving. The plant has a Harmony account now.', { edited: true });

// #design
m('design', 'maggie', 3, 10, 15, 'New direction for message rows: no bubbles. Avatar, name, time, then the text. Grouped when the same person keeps talking.', { key: 'rows' });
react(msgs.rows, 'ada', '👍'); react(msgs.rows, 'grace', '👍'); react(msgs.rows, 'ken', '👍'); react(msgs.rows, 'lin', '👍');
pin(msgs.rows, 'maggie');
m('design', 'grace', 3, 10, 30, 'Agree. Bubbles are for two people. Rooms are for many.');
m('design', 'maggie', 3, 10, 32, 'Exactly. And the composer should look like the platform’s field, not a painted pill.');
m('design', 'alan', 2, 15, 0, 'Reaction chips: small, rounded, the accent as a tint when it is yours. Tap to toggle.', { key: 'chips' });
react(msgs.chips, 'maggie', '❤️'); react(msgs.chips, 'ken', '👍');
m('design', 'ada', 2, 15, 8, 'That is what shipped. The tint is the system accent, so it follows the user’s choice on the Mac.', { reply: 'chips' });
m('design', 'maggie', 1, 9, 30, 'Dark mode check on iPhone tonight. Separators, secondary labels, the whole lot. Leave colours unsaid and it just works.');
m('design', 'grace', 0, 10, 15, 'Custom emoji are one-colour shapes for now. Honestly they look great small.', { key: 'oneColour' });
react(msgs.oneColour, 'ada', ':sparkle:'); react(msgs.oneColour, 'ken', ':bolt:');

// #engineering
m('engineering', 'lin', 3, 13, 5, 'Backend is Snapback4. Every table syncs to a partition on the device, so the thread list reads locally and works offline.', { key: 'backend' });
pin(msgs.backend, 'lin');
react(msgs.backend, 'ada', '🔥'); react(msgs.backend, 'alan', '🔥');
m('engineering', 'ken', 3, 13, 20, 'How are DMs modelled?');
m('engineering', 'lin', 3, 13, 22, 'Same as channels: a room with members. The read rule is membership, so the server never hands you a DM you are not in, and sync infers the audience from that rule.', { reply: 'backend' });
m('engineering', 'ken', 3, 13, 23, 'Nice. One message table then.');
m('engineering', 'grace', 2, 11, 40, 'PR up: unread counts. The inbox query walks your memberships and counts messages after your last-read mark.', { key: 'unread' });
react(msgs.unread, 'lin', '👍'); react(msgs.unread, 'ada', '🚀');
m('engineering', 'alan', 2, 11, 55, 'Reviewed. One nit about the cap on members per person, otherwise ship it.');
m('engineering', 'grace', 2, 12, 10, 'Merged.', { edited: true });
m('engineering', 'ada', 1, 16, 0, 'Cold start to first frame on the iPhone 15: 91 ms. No JavaScript runs before the first pixel.', { key: 'cold' });
react(msgs.cold, 'lin', '🤯'); react(msgs.cold, 'grace', '🤯'); react(msgs.cold, 'ken', '🔥'); react(msgs.cold, 'maggie', '🔥'); react(msgs.cold, 'alan', ':bolt:');
m('engineering', 'lin', 1, 16, 4, 'That is faster than our old app shows its splash screen.', { reply: 'cold' });
m('engineering', 'ken', 0, 8, 50, 'Search is word-prefix over every room you are in. Try searching “ramen”.', { key: 'search' });
react(msgs.search, 'alan', '👍');
m('engineering', 'alan', 0, 8, 58, 'Flaky test in the offline suite, looking.');
m('engineering', 'alan', 0, 9, 30, 'Found it. The test advanced the UI clock and expected the server to notice. Fixed by waiting on data instead.');

// #music
m('music', 'ken', 2, 17, 0, 'Lo-fi beats to ship a chat app to. Five hours, no vocals, no regrets.');
m('music', 'grace', 2, 17, 5, 'The Caltrain delay playlist. Mostly ambient.');
m('music', 'alan', 1, 18, 30, 'Anyone else do the thing where you pick an album for a bug and then the album is cursed forever', { key: 'cursed' });
react(msgs.cursed, 'lin', '😂'); react(msgs.cursed, 'ken', '😂');
m('music', 'maggie', 1, 18, 31, 'I cannot listen to Kind of Blue because of a layout bug in 2023.');
m('music', 'lin', 0, 12, 0, 'New tune for the demo intro. Thirty seconds, builds, drops right as the sidebar appears.');

// #harmony-launch
m('harmony-launch', 'maggie', 3, 9, 0, 'Launch checklist lives here. Shout when something is done.', { key: 'checklist' });
pin(msgs.checklist, 'maggie');
m('harmony-launch', 'ada', 3, 9, 5, '☑ Channels\n☑ Direct messages\n☑ Reactions and custom emoji\n☑ Pins\n☐ Voice (later)\n☐ Video (later)', { key: 'list', edited: true });
react(msgs.list, 'grace', '👍'); react(msgs.list, 'lin', '👍');
m('harmony-launch', 'grace', 2, 10, 0, 'Pins are done. Long-press a message, or right-click on the Mac.');
m('harmony-launch', 'alan', 1, 15, 20, 'Switching accounts in the demo: You tab, tap a person. No login screen, no server round trip for the switch itself.');
m('harmony-launch', 'ken', 0, 10, 40, 'Three devices, one source file, one backend. Let’s go.', { key: 'go' });
react(msgs.go, 'ada', ':ship:'); react(msgs.go, 'grace', ':ship:'); react(msgs.go, 'maggie', ':ship:'); react(msgs.go, 'lin', ':ship:'); react(msgs.go, 'alan', ':ship:');

// #bake-off
m('bake-off', 'ada', 2, 13, 0, 'Round 22 numbers are in. Same app, three stacks. Exact is the smallest download by a wide margin and first to interactive on every device.', { key: 'r22' });
react(msgs.r22, 'lin', '🔥'); react(msgs.r22, 'ken', ':exact:'); react(msgs.r22, 'alan', ':exact:');
m('bake-off', 'lin', 2, 13, 12, 'And the one that is actually native on each platform, which the other two are not.');
m('bake-off', 'grace', 1, 11, 0, 'Round 23 is a chat app. Guess which one.');
m('bake-off', 'maggie', 1, 11, 1, '👀');

// direct messages
m('dm:ada:grace', 'grace', 1, 17, 30, 'Hey, do you have five minutes before standup tomorrow? Want to walk you through the unread counts.');
m('dm:ada:grace', 'ada', 1, 17, 42, 'Yes! 9:15?');
m('dm:ada:grace', 'grace', 1, 17, 43, 'Perfect.');
m('dm:ada:grace', 'grace', 0, 9, 20, 'That went well. Thanks for the review.', { key: 'thanks' });
react(msgs.thanks, 'ada', '❤️');
m('dm:ada:lin', 'lin', 0, 10, 5, 'Can you look at the offline test when you get a sec? I think the clock handling is wrong.');
m('dm:ada:lin', 'ada', 0, 10, 20, 'Alan found it, it was waiting on the UI clock instead of data. Should be green now.');
m('dm:ada:lin', 'lin', 0, 10, 21, 'Legend.');
m('dm:ada:ken', 'ken', 2, 19, 0, 'Biscuit says hi');
m('dm:ada:ken', 'ada', 2, 19, 8, 'Tell Biscuit the leaf pile was a setup');
m('dm:grace:maggie', 'maggie', 0, 11, 0, 'Lunch?');
m('dm:grace:maggie', 'grace', 0, 11, 2, 'Give me 20.');

// custom emoji: one-colour paths in a 24-unit square
const emoji = [
  ['ship', 'M2 21l21-9L2 3v7l15 2-15 2z', 'hue.sky'],
  ['exact', 'M12 2l8.66 5v10L12 22l-8.66-5V7z', 'hue.violet'],
  ['bolt', 'M13 2L3 14h8l-1 8 10-12h-8z', 'hue.amber'],
  ['sparkle', 'M12 2l2.4 7.6L22 12l-7.6 2.4L12 22l-2.4-7.6L2 12l7.6-2.4z', 'hue.teal'],
  ['heartbeat', 'M12 21s-7-4.5-9.5-9A5.5 5.5 0 0 1 12 6a5.5 5.5 0 0 1 9.5 6c-2.5 4.5-9.5 9-9.5 9z', 'hue.rose'],
];
for (const [name, path, fill] of emoji) put('emoji', { id: id(), name, path, fill, createdBy: P.ken, at: at(4, 12) });

// Everyone has read everything but the newest few in the busy channels.
for (const k of Object.keys(people)) {
  for (const [name, rid] of Object.entries(room)) {
    const mine = rows.filter((r) => r.table === 'members' && r.row.roomId === rid && r.row.principal === P[k]).length > 0;
    if (!mine) continue;
    const lastAt = Math.max(...rows.filter((r) => r.table === 'messages' && r.row.roomId === rid).map((r) => r.row.at));
    const unreadSome = ['general', 'engineering', 'announcements', 'dm:ada:lin', 'dm:ada:grace'].includes(name);
    put('lastReads', { id: id(), roomId: rid, principal: P[k], at: unreadSome ? now - 20 * H : lastAt + M });
  }
}

// Arguments, all known or refused: `--json <file>` (where the rows go: seed.json
// beside this file by default), `--post [origin]` (empty that dev server and seed
// it; an origin may come anywhere, so `bun exact.mjs seed --force <origin>`,
// which arrives as `--post --force <origin>`, means that origin), and `--force`
// or `--reset`. More than one origin, or anything else, is refused.
let json = null, post = false, force = false;
const origins = [];
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a === '--json' && process.argv[i + 1] && !process.argv[i + 1].startsWith('--')) json = process.argv[++i];
  else if (a === '--post') post = true;
  else if (a === '--force' || a === '--reset') force = true;
  else if (/^https?:\/\//.test(a)) origins.push(a);
  else { console.error(`seed: no argument ${a} (it takes --post [origin], --force or --reset, and --json <file>)`); process.exit(2); }
}
if (origins.length > 1) { console.error(`seed: one origin, not ${origins.join(' and ')}`); process.exit(2); }
if (origins.length && !post) { console.error(`seed: an origin (${origins[0]}) is for --post`); process.exit(2); }

const out = json ?? fileURLToPath(new URL('./seed.json', import.meta.url));
writeFileSync(out, JSON.stringify(rows, null, 1) + '\n');
console.log(`${rows.length} rows → ${out}`);

/** The dev server's `/dev/status`, proven whole, or a refusal that nothing was erased. */
async function devStatus(origin) {
  let r;
  try { r = await fetch(`${origin}/dev/status`, { signal: AbortSignal.timeout(5000) }); }
  catch (e) { throw new Error(`nothing answered at ${origin} (${e.cause?.code ?? e.message})`); }
  if (!r.ok) throw new Error(`${origin}/dev/status answered ${r.status}`);
  const s = await r.json().catch(() => null);
  const named = (v) => typeof v === 'string' && v.length > 0;
  if (!named(s?.storeId) || !named(s.instance) || !Number.isInteger(s.pid) || s.pid <= 0 || !named(s.personas?.[0])) {
    throw new Error(`${origin} is no snapback4 dev server (no storeId, instance, pid or persona in /dev/status)`);
  }
  return s;
}

/** The store's commit head (`seq`), which no reset turns back: 0 only for a store never written to. */
async function head(origin, status) {
  const r = await fetch(`${origin}/changes?since=0`, { headers: { 'x-snapback-persona': status.personas[0] }, signal: AbortSignal.timeout(5000) });
  const c = r.ok ? await r.json().catch(() => null) : null;
  if (!Number.isInteger(c?.seq) || c.seq < 0 || c.store_id !== status.storeId) throw new Error(`${origin}/changes gave no head for store ${status.storeId}`);
  return c.seq;
}

// `--post`: seed the dev server with these rows. Unasked only where nothing was
// ever written (the store's head is 0; a reset is itself a commit, so a store
// seeded before counts as written to), and then by `/seed`, an upsert that erases
// nothing, so a write landing after the head was read stays. (Not one to a row
// this seed writes, which it updates: only if another seed of this server landed
// between this one's head read and its seed and someone edited what it wrote; no
// request can make the head check and the seed one step.) What a store holds can't be read whole
// from outside (a query shows only what a persona may see), so any other store,
// in memory or not, may hold what people wrote and is refused unless `--force`
// (or `--reset`) says to empty it first (`/dev/reset`). Both name the dev
// instance they asked, so a server started in its place refuses them (409).
if (post) {
  const origin = new URL(origins[0] ?? 'http://127.0.0.1:4400').origin;
  const fail = (m) => { console.error(`seed: ${m}; nothing was erased`); process.exit(1); };
  let before, n, now;
  try { before = await devStatus(origin); n = await head(origin, before); } catch (e) { fail(e.message); }
  if (n !== 0 && !force) {
    console.error(`seed: the server at ${origin} has been written to (${n} commits${before.temporary === true ? ', in memory' : ', kept on disk'}), so it may hold what people wrote. Seeding it would ERASE EVERYONE'S MESSAGES, channels and profiles there, for every person using it. Nothing was erased. If this is a server of your own whose data you mean to lose, run it again with --force.`);
    process.exit(1);
  }
  let res;
  if (!force) res = await fetch(`${origin}/seed`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-snapback-dev-instance': before.instance }, body: JSON.stringify(rows) });
  else {
    // The same server still (its instance, store and process), as it was asked.
    try { now = await devStatus(origin); } catch (e) { fail(e.message); }
    if (now.instance !== before.instance || now.storeId !== before.storeId || now.pid !== before.pid) fail(`the server at ${origin} changed while it was asked (store ${before.storeId} → ${now.storeId})`);
    res = await fetch(`${origin}/dev/reset`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-snapback-dev-instance': before.instance }, body: JSON.stringify(rows) });
  }
  const body = await res.text();
  let answer = null;
  try { answer = JSON.parse(body); } catch {}
  if (!res.ok || answer?.denied) { console.error(`${res.url}: ${res.status} ${body}`); process.exit(1); }
  console.log(`${origin}: ${body}`);
}
