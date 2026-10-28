// A Markdown node's atom faces on the JS target (LLP 1045.000 D2): named as
// the runner's bridge names them, each colour read as a bound colour row's.
import { expect, test } from 'bun:test';
import { atomFacesRecord } from './shape.js';

test('a face\'s colours go through the colour map; what is no colour is empty and journalled', () => {
  const journal = [];
  globalThis.exact = { journal, now: () => 7 };
  // The build's map spells an Exact role as the page's property, and refuses a platform colour no literal names.
  const cm = c => c.includes('-exact-platform-color(') ? 'unset' : c.replace('-exact-separator', 'var(--exact-separator, #3c3c4349)');
  const faces = [
    ['@a', 'A', '', 'Alice', 'light-dark(#d52c36, #ff6762)', '-exact-separator'],
    ['@b', '', '', '', '', '-exact-platform-color(web Highlight, #fff)'],
    ['@c', '', '', '', 'inherit', 'var(--brand)'],
  ];
  try {
    expect(JSON.parse(atomFacesRecord(faces, cm))).toEqual([
      { source: '@a', label: 'A', icon: '', title: 'Alice', color: 'light-dark(#d52c36, #ff6762)', backgroundColor: 'var(--exact-separator, #3c3c4349)' },
      { source: '@b', label: '', icon: '', title: '', color: '', backgroundColor: '' },
      { source: '@c', label: '', icon: '', title: '', color: '', backgroundColor: '' },
    ]);
    expect(journal).toEqual([
      't=7 invalid atomFaces colour "-exact-platform-color(web Highlight, #fff)"; its kind\'s paint stands',
      't=7 invalid atomFaces colour "inherit"; its kind\'s paint stands',
      't=7 invalid atomFaces colour "var(--brand)"; its kind\'s paint stands',
    ]);
    expect(atomFacesRecord(null, cm)).toBe(null);
  } finally {
    delete globalThis.exact;
  }
});
