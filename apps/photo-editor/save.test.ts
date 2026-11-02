import { expect, mock, test } from 'bun:test';

const added: string[] = [];
mock.module('@exact/photos', () => ({ addToPhotoLibrary: async ({ path }: { path: string }) => { added.push(path); return { asset: path }; }, photoAssets: async () => ({ assets: [] }) }));
const { answer } = await import('./app.ts');

test('overlapping saves give each edit its own file, and a failed transform adds nothing', async () => {
  const files = new Map<string, string>();
  let finish!: () => void;
  const gate = new Promise<void>(resolve => { finish = resolve; });
  const storage = { fs: {
    mkdir: async () => {},
    transformImage: async (photo: string, path: string) => {
      if (photo === 'broken') throw new Error('encode failed');
      files.set(path, photo);
      if (files.size === 2) finish();
      await gate;
      return { path, metadata: '' };
    },
  } };
  const call = (photo: string) => (answer as any)('savePhoto', [photo, '{}'], {}, storage);
  const saved = await Promise.all([call('first'), call('second')]);
  expect(saved.every(r => r.saved)).toBe(true);
  expect(new Set(added).size).toBe(2);
  expect(added.map(p => files.get(p))).toEqual(['first', 'second']);
  expect((await call('broken')).saved).toBe(false);
  expect(added).toHaveLength(2);
});
