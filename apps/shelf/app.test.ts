// `bun test apps/shelf`: the recipe stays tell-free (LLP 1115), and the data
// module keeps its store's rules. The flows are app.test.contract's, driven on
// the web and iOS.
import { expect, test } from 'bun:test';
import { tells, tellsIn } from '../../scripts/no-tells.mjs';

test('Shelf writes no colour, font size or weight of its own', () => {
  expect(tells(import.meta.dir)).toEqual([]);
});

test('no-tells finds literal colours and sizes, and lets roles and text styles by', () => {
  const found = tellsIn([
    'component A',
    '  view',
    '    column background-color="#f2f2f7" border="1px solid #d1d1d6"',
    '      text "a" color=(on ? "rgb(0, 122, 255)" : "red") font-size=34 font-weight=700',
    '      text "b" color="-exact-secondary-label" font="-exact-footnote" border-bottom="0.5px solid -exact-separator"',
    '      text "c" role="heading" aria-level=1 font-size="-exact-caption1" background-color="Canvas"',
    '      // color="#ffffff" in a comment is not a tell',
    '      text "#ffffff is text, not a colour"',
    '      row background-color="#B5562B" // no-tells: the brand',
  ].join('\n'));
  expect(found.map(t => `${t.line} ${t.property}`)).toEqual(['3 background-color', '3 border', '4 color', '4 font-size', '4 font-weight']);
});

// The store's rules, through the data module's own answers.
async function shelf() {
  let bytes: Uint8Array | undefined;
  const storage = { fs: {
    readFile: async () => { if (!bytes) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' }); return bytes.buffer; },
    mkdir: async () => {},
    atomicWriteFile: async (_path: string, next: Uint8Array) => { bytes = next.slice(); },
  } };
  const source = await import(`./app.ts?case=${Math.random()}`);
  const call = (name: string, ...args: unknown[]) => source.answer(name, args, {}, storage, {});
  const book = async (id: string) => (await call('library')).books.find((b: { id: string }) => b.id === id);
  return { call, book };
}

test('progress clamps, a page read starts a book, the last page finishes it', async () => {
  const { call, book } = await shelf();
  await call('setPage', 'seed-2', -50);
  expect((await book('seed-2')).currentPage).toBe(0);
  await call('setPage', 'seed-3', 10);
  expect((await book('seed-3')).status).toBe('reading');
  await call('setPage', 'seed-3', 10000);
  expect(await book('seed-3')).toMatchObject({ currentPage: 387, status: 'done' });
  await call('setStatus', 'seed-2', 'done');
  expect((await book('seed-2')).currentPage).toBe(272);
});

test('the goal stays between 1 and 365, and the average counts rated finished books', async () => {
  const { call } = await shelf();
  await call('setGoal', 0);
  expect((await call('library')).goal).toBe(1);
  await call('setGoal', 400);
  expect((await call('library')).goal).toBe(365);
  expect((await call('library')).avgText).toBe('5.0 ★');
});
