// LLP 1126.000 WP5: the same module answers now, fetches, and reads a saved file.
const appId = 'test.widget-js';
const grants = 'net.fetch https://widget.test\nfs.read app:/data\nfs.write app:/data\nsecret.keep session\n';
const row = (date: number, title: string) => [{date, props: {title}}];
function answer(_source: string, [date]: number[], store: any, storage: any) {
  if (date === 1) return row(date, store.get('session') ?? 'missing');
  if (date === 2) return fetch('https://widget.test/value').then(r => r.text()).then(text => row(date, text));
  if (date === 3) return storage.fs.readFile('app:/data/value').then((b: Uint8Array) => row(date, String.fromCharCode(...new Uint8Array(b))));
  if (date === 4) return storage.fs.atomicWriteFile('app:/data/value', new Uint8Array([88])).then(() => row(date, 'wrote'));
  if (date === 5) { store.set('session', 'overwrite'); return row(date, 'wrote secret'); }
  throw new Error('private refusal');
}
(globalThis as any).exact = {abi: 1, appId, grants, answer};
