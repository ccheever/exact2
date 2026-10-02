// Duo Lab's data module (LLP 1077 D8): the words and rows behind the four
// screens. Everything is generated deterministically here — no network, no
// storage, no clock — so the bake answers every resource once and the app is
// the same on every host and at every pose.
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.exact.duolab';
export const grants = '';

// A small linear congruential generator: the same document on every bake.
function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

const WORDS = (
  'fold hinge panel segment posture viewport inset band margin layout continuous folded ' +
  'open book half closed cover inner display scale raster text cache reflow column pane ' +
  'list detail sheet keyboard route push back tab scroll offset frame commit settle clock ' +
  'web kernel runner host agent plan bake contract style length width height left right ' +
  'top bottom row gap padding border radius shadow image caption feed note draft'
).split(' ');

const PLACES = ['North Shore', 'Ochre Dunes', 'Alpine Water', 'Winter Ridge'];
const ASSETS = ['assets/north-shore.png', 'assets/ochre-dunes.png', 'assets/alpine-water.png', 'assets/winter-ridge.png'];
const PEOPLE = ['ada', 'bram', 'cleo', 'dev', 'esme', 'finn', 'greta', 'hollis', 'ines', 'jude'];

function sentence(next: () => number, words: number): string {
  const out: string[] = [];
  for (let i = 0; i < words; i++) out.push(WORDS[Math.floor(next() * WORDS.length)]);
  const s = out.join(' ');
  return s[0].toUpperCase() + s.slice(1) + '.';
}
function paragraph(next: () => number, sentences: number): string {
  const out: string[] = [];
  for (let i = 0; i < sentences; i++) out.push(sentence(next, 8 + Math.floor(next() * 10)));
  return out.join(' ');
}

// ------------------------------------------------------------- Fold's items
const ITEM_TITLES = [
  'The division region', 'Two postures, no more', 'Segments count from the left', 'The band belongs to nobody',
  'A closed Duo is continuous', 'Width is the only cover fact', 'Occlusions are insets', 'The hinge relayouts',
  'One ABI entry', 'The web resolves env() itself', 'Chromium is the oracle', 'prefer segments 2x1',
  'Index past the grid', 'No fallback argument', 'The bake answers continuous', 'A fold mid-reading',
  'Keyboard across a fold', 'Sheet across a fold', 'Push across a fold', 'Scroll across a fold',
  'Both sessions follow', 'Rotation is not here', 'Split View is not here', 'The margins are UIKit\'s',
];
function items() {
  const next = lcg(1076);
  return ITEM_TITLES.map((title, i) => ({
    id: i + 1,
    title,
    summary: sentence(next, 6 + Math.floor(next() * 6)),
    body: [paragraph(next, 4), paragraph(next, 5), paragraph(next, 3)].join('\n\n'),
  }));
}

// ---------------------------------------------------------- Images' rows
function imageRows() {
  const next = lcg(466);
  const rows = [];
  for (let i = 0; i < 300; i++) {
    const k = i % ASSETS.length;
    rows.push({ id: i + 1, asset: ASSETS[k], caption: `${i + 1}. ${PLACES[k]} — ${sentence(next, 5 + Math.floor(next() * 5))}` });
  }
  return rows;
}

// --------------------------------------------------------- Reflow's document
function document() {
  const next = lcg(951);
  const parts: string[] = ['# Reflow', '', 'A long document for the text caches: headings, paragraphs, lists and code, laid out at 951 points and again at 466.', ''];
  for (let h = 1; h <= 14; h++) {
    parts.push(`## ${h}. ${sentence(next, 3).slice(0, -1)}`, '');
    for (let p = 0; p < 4; p++) parts.push(paragraph(next, 5 + Math.floor(next() * 4)), '');
    parts.push(`### ${sentence(next, 2).slice(0, -1)}`, '');
    for (let b = 0; b < 5; b++) parts.push(`- ${sentence(next, 6 + Math.floor(next() * 8))}`);
    parts.push('', '```', `fn segment(x: u8, y: u8) -> Rect { grid[y * cols + x] } // ${h}`, `let posture = if any_division_active { "folded" } else { "continuous" };`, '```', '');
    for (let p = 0; p < 2; p++) parts.push(paragraph(next, 4 + Math.floor(next() * 4)), '');
    parts.push(`> ${sentence(next, 12)}`, '');
  }
  return { body: parts.join('\n') };
}

// ------------------------------------------------------------- Reflow's feed
function feed() {
  const next = lcg(669);
  const rows = [];
  for (let i = 0; i < 200; i++) rows.push({ id: i + 1, who: PEOPLE[i % PEOPLE.length], body: paragraph(next, 1 + Math.floor(next() * 3)) });
  return rows;
}

const sources: Sources = {
  items: () => items(),
  imageRows: () => imageRows(),
  document: () => document(),
  feed: () => feed(),
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
