// Writes each spec's data/*.json from a fixed seed, so every build of a spec starts from the same rows.
//   bun bench/bakeoff/specs/gen.mjs
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const DIR = resolve(new URL('.', import.meta.url).pathname);
let s = 49979;
const rnd = () => ((s = (s * 1103515245 + 12345) % 2147483648) / 2147483648);
const int = (a, b) => a + Math.floor(rnd() * (b - a + 1));
const pick = (xs) => xs[Math.floor(rnd() * xs.length)];
const some = (xs, a, b) => { const n = int(a, b), out = new Set(); while (out.size < Math.min(n, xs.length)) out.add(pick(xs)); return [...out]; };
const day = (d) => new Date(Date.UTC(2026, 9, 26) - d * 86400000).toISOString().slice(0, 10);
const write = (spec, name, rows) => {
  mkdirSync(resolve(DIR, spec, 'data'), { recursive: true });
  writeFileSync(resolve(DIR, spec, 'data', name), JSON.stringify(rows, null, 1) + '\n');
};

const FIRST = 'Ada Alan Amara Ana Ben Bianca Carlos Chen Chloe Dana David Diego Elena Eli Emma Farah Felix Grace Hana Hugo Ines Isaac Jada James Jonah Kai Keiko Lars Layla Leo Lina Lucas Maya Mei Milo Nadia Noah Nora Omar Oscar Priya Quinn Rafael Rosa Ruth Sam Sara Theo Uma Victor Wen Yara Yusuf Zoe'.split(' ');
const LAST = 'Abbott Adeyemi Alvarez Baker Banerjee Becker Brooks Castro Chang Cohen Diaz Dubois Edwards Evans Fischer Foster Garcia Gupta Hall Hansen Ito Jensen Johnson Kaur Kim Kowalski Lee Lopez Martin Meyer Moreno Nakamura Nguyen Novak Okafor Olsen Park Patel Quinn Rossi Santos Schmidt Silva Singh Tanaka Turner Vargas Wagner Walsh Wang Weber Young Zhang'.split(' ');
const COMPANY = 'Acme Bluebird Cedar Labs Driftwood Ember Fieldstone Granite Harbor Ironwood Juniper Kestrel Lumen Meridian Northwind Orchard Pinecone Quarry Redwood Saltmarsh Tidewater Umbra Vantage Willow'.split(' ');

write('contacts', 'contacts.json', Array.from({ length: 320 }, (_, i) => {
  const first = pick(FIRST), last = pick(LAST);
  return { id: i + 1, first, last, company: rnd() < 0.7 ? `${pick(COMPANY)} ${pick(['Co', 'Inc', 'Studio', 'Group', 'Partners'])}` : null,
    phone: `(${int(200, 989)}) ${int(200, 989)}-${String(int(0, 9999)).padStart(4, '0')}`,
    email: `${first}.${last}@example.com`.toLowerCase(), birthday: rnd() < 0.4 ? `${int(1960, 2004)}-${String(int(1, 12)).padStart(2, '0')}-${String(int(1, 28)).padStart(2, '0')}` : null,
    favorite: rnd() < 0.08 };
}));

const CATS = ['Breakfast', 'Soups', 'Salads', 'Pasta', 'Mains', 'Vegetarian', 'Baking', 'Desserts', 'Drinks'];
const DISH = 'Lemon Garlic Smoky Spiced Herbed Crispy Roasted Creamy Golden Charred Honey Ginger Miso Sesame Tomato Basil Coconut Maple Chili Saffron'.split(' ');
const BASE = 'Chicken Lentils Salmon Cauliflower Risotto Noodles Tacos Pancakes Oats Soup Stew Salad Flatbread Gnocchi Curry Tofu Chickpeas Muffins Tart Cake Lemonade Smoothie Frittata Dumplings'.split(' ');
const ING = 'garlic onion olive oil butter lemon salt pepper flour sugar eggs milk rice ginger soy sauce tomato basil parsley cumin paprika honey yogurt carrots spinach chickpeas coconut milk cheese stock potatoes mushrooms'.split(' ');
write('recipes', 'recipes.json', Array.from({ length: 160 }, (_, i) => ({
  id: i + 1, title: `${pick(DISH)} ${pick(DISH)} ${pick(BASE)}`, category: pick(CATS), minutes: int(2, 18) * 5, servings: int(1, 8),
  rating: Math.round((3 + rnd() * 2) * 10) / 10,
  ingredients: some(ING, 5, 11).map((x) => `${int(1, 4)} ${pick(['cup', 'tbsp', 'tsp', 'cloves', 'g', ''])} ${x}`.replace(/\s+/g, ' ')),
  steps: Array.from({ length: int(3, 7) }, (_, k) => `${pick(['Heat', 'Whisk', 'Chop', 'Simmer', 'Fold in', 'Season', 'Bake', 'Toss', 'Rest'])} ${pick(ING)} ${pick(['for 5 minutes', 'until golden', 'gently', 'to taste', 'until smooth', 'over medium heat'])}.`),
})));

const MERCH = { Groceries: ['Fresh Market', 'Green Grocer', 'Corner Store'], Dining: ['Noodle Bar', 'Taqueria Sol', 'Blue Door Cafe', 'Pizza Pi'], Transport: ['Metro Card', 'City Bike', 'Rideshare'], Bills: ['Power & Light', 'Water Utility', 'Internet Co'], Shopping: ['Bookshop', 'Hardware Hut', 'Outfitters'], Fun: ['Cinema 8', 'Climbing Gym', 'Concert Hall'], Health: ['Pharmacy', 'Dentist'] };
write('expenses', 'transactions.json', Array.from({ length: 240 }, (_, i) => {
  const category = pick(Object.keys(MERCH));
  return { id: i + 1, date: day(int(0, 89)), merchant: pick(MERCH[category]), category, amount: Math.round((category === 'Bills' ? 40 + rnd() * 120 : 3 + rnd() * 90) * 100) / 100, note: rnd() < 0.2 ? pick(['split with Sam', 'work lunch', 'gift', 'refund pending']) : '' };
}).sort((a, b) => b.date.localeCompare(a.date) || a.id - b.id));

const MUSCLE = ['Chest', 'Back', 'Legs', 'Shoulders', 'Arms', 'Core', 'Glutes'];
const MOVE = 'Press Row Squat Deadlift Curl Extension Raise Fly Lunge Pulldown Pushup Plank Bridge Crunch Dip Shrug Step-up Thruster'.split(' ');
const KIT = ['Barbell', 'Dumbbell', 'Cable', 'Machine', 'Bodyweight', 'Kettlebell', 'Band'];
const exercises = Array.from({ length: 120 }, (_, i) => ({ id: i + 1, name: `${pick(KIT)} ${pick(['Incline', 'Seated', 'Standing', 'Single-arm', 'Wide', 'Close-grip', 'Bulgarian', 'Romanian', ''])} ${pick(MOVE)}`.replace(/\s+/g, ' '), muscle: pick(MUSCLE), equipment: pick(KIT) }));
write('workout-log', 'exercises.json', exercises);
write('workout-log', 'history.json', Array.from({ length: 60 }, (_, i) => ({ id: i + 1, date: day(i * 2 + int(0, 1)), name: pick(['Push', 'Pull', 'Legs', 'Full body', 'Upper', 'Lower']),
  exercises: some(exercises, 3, 6).map((e) => ({ exercise: e.id, sets: Array.from({ length: int(2, 5) }, () => ({ reps: int(5, 15), weight: int(4, 40) * 2.5 })) })) })));

const SHOWS = ['The Long Field', 'Small Engines', 'Night Kitchen', 'Orbit Report', 'Paper Trails', 'Quiet Hours', 'Market Hall', 'The Repair Shop', 'Deep Water', 'Common Ground', 'Signal & Noise', 'Weekend Atlas'];
write('podcasts', 'episodes.json', Array.from({ length: 420 }, (_, i) => ({ id: i + 1, show: pick(SHOWS), number: 0, title: `${pick(['Why', 'How', 'When', 'The case for', 'Inside', 'Notes on', 'Against', 'A history of'])} ${pick(['bridges', 'sourdough', 'tides', 'small towns', 'old radios', 'night trains', 'cartography', 'beekeeping', 'jazz standards', 'glaciers', 'typefaces', 'lighthouses'])}`,
  date: day(int(0, 400)), minutes: int(12, 95), summary: `${pick(['A conversation', 'A field recording', 'An interview', 'A deep dive'])} about ${pick(['craft', 'memory', 'weather', 'cities', 'tools', 'food', 'maps'])} and ${pick(['patience', 'luck', 'failure', 'repair', 'distance', 'home'])}.`, played: rnd() < 0.3 }))
  .sort((a, b) => b.date.localeCompare(a.date) || a.id - b.id));

const WORDS = 'call dentist about the cleaning; ideas for the garden: tomatoes, basil, marigolds; book list for winter; trip packing list; gift ideas for Mom; meeting notes with the design team; recipe to try on Sunday; quotes I liked; things to fix in the apartment; running plan for the 10k; podcast recommendations; questions for the landlord; camera settings for night shots; wifi password for the cabin; birthday party plan'.split('; ');
write('notes', 'notes.json', Array.from({ length: 90 }, (_, i) => {
  const t = pick(WORDS);
  return { id: i + 1, title: t[0].toUpperCase() + t.slice(1), body: Array.from({ length: int(1, 6) }, () => `${pick(['Remember to', 'Maybe', 'Ask about', 'Try', 'Check'])} ${pick(WORDS)}.`).join('\n'), updated: `${day(int(0, 200))}T${String(int(7, 22)).padStart(2, '0')}:${String(int(0, 59)).padStart(2, '0')}:00Z`, pinned: rnd() < 0.06 };
}));

const SYL = ['AR', 'BL', 'CR', 'DX', 'EV', 'FN', 'GL', 'HM', 'JK', 'LM', 'NV', 'OR', 'PX', 'QT', 'RZ', 'SN', 'TK', 'VL', 'WX', 'ZE'];
const seen = new Set();
write('watchlist', 'tickers.json', Array.from({ length: 260 }, (_, i) => {
  let sym; do sym = pick(SYL) + pick(['', 'A', 'C', 'T', 'X']) + pick(['', 'O', 'I']); while (seen.has(sym)); seen.add(sym);
  const price = Math.round((5 + rnd() * 600) * 100) / 100;
  return { symbol: sym, name: `${pick(COMPANY)} ${pick(['Holdings', 'Systems', 'Energy', 'Foods', 'Biotech', 'Motors', 'Bank', 'Retail'])}`, price, open: Math.round(price * (0.96 + rnd() * 0.08) * 100) / 100, volume: int(10, 9000) * 1000 };
}));

const LINES = ['Are we still on for tonight?', 'Running ten minutes late, sorry!', 'Did you see the game?', 'haha yes', 'Can you grab milk on the way?', 'Sure', 'What time works for you tomorrow?', 'Sent you the photos from the hike', 'These are great', 'Let me check and get back to you', 'ok', 'I found a really good recipe for the dinner party, it uses that miso we bought last week and takes about forty minutes', 'Thanks!', 'On my way', 'Do you remember the name of that cafe near the station?', 'Blue Door', 'Perfect', '👍', 'Call me when you are free?', 'Just landed'];
let clock = Date.UTC(2026, 6, 1, 8);
write('chat', 'messages.json', Array.from({ length: 520 }, (_, i) => {
  clock += int(1, 240) * 60000 * (rnd() < 0.1 ? 20 : 1);
  return { id: i + 1, from: rnd() < 0.5 ? 'me' : 'sam', text: pick(LINES), at: new Date(Math.min(clock, Date.UTC(2026, 9, 25, 20))).toISOString() };
}));

const MOOD = ['Coast', 'Forest', 'Desert', 'City', 'Garden', 'Studio', 'Market', 'Harbor', 'Alpine', 'Night'];
write('moodboard', 'tiles.json', Array.from({ length: 600 }, (_, i) => ({ id: i + 1, title: `${pick(MOOD)} ${int(1, 99)}`, board: pick(MOOD), hue: int(0, 359), saturation: int(35, 85), lightness: int(35, 75), symbol: pick(['🌊', '🌲', '🌵', '🏙️', '🌷', '🎨', '🧺', '⚓️', '🏔️', '🌙', '☕️', '📷']), added: day(int(0, 300)), liked: rnd() < 0.1 }))
  .sort((a, b) => b.added.localeCompare(a.added) || a.id - b.id));

const SUBJ = ['Your order has shipped', 'Lunch next week?', 'Invoice #', 'Weekly digest', 'Re: project timeline', 'Photos from Saturday', 'Reminder: appointment', 'Welcome to the team', 'Re: apartment viewing', 'Your receipt', 'Quick question', 'Agenda for Thursday'];
write('inbox', 'mail.json', Array.from({ length: 320 }, (_, i) => {
  const subject = pick(SUBJ);
  return { id: i + 1, from: `${pick(FIRST)} ${pick(LAST)}`, subject: subject.endsWith('#') ? subject + int(1000, 9999) : subject, preview: `${pick(['Hi', 'Hey', 'Hello', 'Good morning'])}, ${pick(['just checking in about', 'following up on', 'wanted to share', 'a quick note on'])} ${pick(['the plan', 'next steps', 'the draft', 'the dates', 'the budget', 'Saturday'])}. ${pick(['Let me know what you think.', 'Talk soon.', 'Thanks!', 'No rush.'])}`,
    at: `${day(int(0, 120))}T${String(int(6, 23)).padStart(2, '0')}:${String(int(0, 59)).padStart(2, '0')}:00Z`, unread: rnd() < 0.35, flagged: rnd() < 0.07 };
}).sort((a, b) => b.at.localeCompare(a.at) || a.id - b.id));

const LISTS = ['Home', 'Work', 'Errands', 'Garden', 'Reading', 'Trip'];
const VERB = ['Buy', 'Call', 'Email', 'Fix', 'Clean', 'Plan', 'Read', 'Water', 'Book', 'Return', 'Sort', 'Write'];
const THING = ['the gutters', 'the plumber', 'Priya', 'the tax forms', 'the garage', 'the tomatoes', 'chapter 4', 'train tickets', 'the library books', 'the photos', 'thank-you notes', 'the bike'];
write('tasks', 'tasks.json', Array.from({ length: 180 }, (_, i) => ({ id: i + 1, list: pick(LISTS), title: `${pick(VERB)} ${pick(THING)}`, due: rnd() < 0.5 ? day(int(-30, 10)) : null, done: rnd() < 0.25, priority: pick(['none', 'none', 'low', 'high']), notes: rnd() < 0.2 ? 'See the email from last week.' : '' })));

// The scroll-stress specs. Their pictures are generated (images.swift), not kept in the repository.
const pictures = (spec, count, seed, side) => {
  const r = spawnSync('swift', [resolve(DIR, 'images.swift'), resolve(DIR, spec, 'data/images'), String(count), String(seed), String(side)], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`images.swift: ${r.stderr}`);
  return JSON.parse(readFileSync(resolve(DIR, spec, 'data/images/images.json'), 'utf8'));
};
const SENT = ['Finally finished the bookshelf.', 'Morning light on the harbor today.', 'Does anyone have a good recipe for focaccia?', 'Three weeks into learning the cello and my neighbors are being very patient.', 'The new bike lane on Fifth is great.', 'Saw the first frost on the garden this morning, earlier than last year by almost two weeks, which feels like a sign of something.', 'Trying to read more poetry this winter.', 'Lunch spot recommendations near the station?', 'Our team shipped the thing!', 'Repotted every plant in the apartment and now I have soil everywhere.', 'Highly recommend the exhibit at the maritime museum.', 'Ran my first 10k.'];
const feedImages = pictures('feed', 120, 11, 900);
write('feed', 'posts.json', Array.from({ length: 400 }, (_, i) => {
  const kind = rnd();
  return { id: i + 1, author: `${pick(FIRST)} ${pick(LAST)}`, handle: `@${pick(FIRST).toLowerCase()}${int(1, 99)}`, minutesAgo: i * int(3, 40), text: Array.from({ length: int(1, 6) }, () => pick(SENT)).join(' '),
    image: kind < 0.45 ? pick(feedImages).file : null,
    carousel: kind > 0.82 ? Array.from({ length: int(4, 10) }, () => ({ title: `${pick(DISH)} ${pick(BASE)}`, image: pick(feedImages).file })) : null,
    likes: int(0, 900), liked: rnd() < 0.1, replies: int(0, 60) };
}));
const photoImages = pictures('photos', 240, 23, 1200);
const ALBUMS = ['Summer', 'Hikes', 'Family', 'City', 'Food', 'Pets', 'Screenshots', 'Travel'];
write('photos', 'photos.json', Array.from({ length: 1500 }, (_, i) => ({ id: i + 1, file: photoImages[i % photoImages.length].file, width: photoImages[i % photoImages.length].width, height: photoImages[i % photoImages.length].height,
  taken: `${day(Math.floor(i / 6))}T${String(int(7, 21)).padStart(2, '0')}:${String(int(0, 59)).padStart(2, '0')}:00Z`, album: pick(ALBUMS), favorite: rnd() < 0.05 })));
const STATIONS = ['Harbor', 'Market St', 'Civic Center', 'Union', 'Elm Park', 'University', 'Riverside', 'Old Town', 'Airport', 'Stadium', 'North Hill', 'Lakeview', 'Mill Creek', 'Ferry Building', 'Chinatown', 'Mission', 'Glen Park', 'Balboa', 'Daly', 'Colma'];
const TRANSIT = [{ id: 'R', name: 'Red', color: '#D62828' }, { id: 'B', name: 'Blue', color: '#1D4ED8' }, { id: 'G', name: 'Green', color: '#15803D' }, { id: 'Y', name: 'Yellow', color: '#CA8A04' }, { id: 'O', name: 'Orange', color: '#EA580C' }];
const deps = [];
for (const station of STATIONS) for (let t = 5 * 60; t < 25 * 60; t += int(4, 14)) {
  const line = pick(TRANSIT);
  deps.push({ station, line: line.id, destination: pick(STATIONS.filter((x) => x !== station)), minute: t % (24 * 60), platform: int(1, 4), status: rnd() < 0.08 ? `Delayed ${int(2, 15)} min` : rnd() < 0.02 ? 'Cancelled' : 'On time' });
}
write('timetable', 'departures.json', { lines: TRANSIT, stations: STATIONS, departures: deps });
console.log('wrote data for the 15 core specs');
