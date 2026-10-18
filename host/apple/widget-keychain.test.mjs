// LLP 1126.000 WP5: opt-in simulator integration, using the archive kit's
// SpringBoard driver. The caller creates/owns EXACT_SIM and supplies the kit
// directory as EXACT_WIDGET_KIT. All products stay in this worktree's target/.
import { test, expect } from 'bun:test';
import { cpSync, mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../..');
test.skipIf(!process.env.EXACT_WIDGET_KIT)('getTimeline reads the secret saved by the app in its shared Keychain group', async () => {
  const sim = process.env.EXACT_SIM;
  if (!sim) throw new Error('Create a test simulator and set EXACT_SIM; this test never selects a shared device');
  const dir = resolve(root, 'target/widget-keychain-test'), app = resolve(dir, 'app');
  // AX joins wrapped widget lines with ", "; keep the random secret on one line.
  const id = 'com.exact.widget-keychain-test', nonce = crypto.randomUUID().slice(0, 8);
  const env = { ...process.env, DEVELOPER_DIR: '/Applications/Xcode.app/Contents/Developer' };
  delete env.EXACT_AGENT; delete env.EXACT_STORE;
  mkdirSync(dir, { recursive: true });
  async function run(argv, name, extra = {}) {
    const output = Bun.file(resolve(dir, `${name}.log`));
    const p = Bun.spawn(argv, { cwd: root, env: { ...env, ...extra }, stdout: output, stderr: output });
    const code = await p.exited;
    if (code) throw new Error(`${name} exited ${code}: ${await output.text()}`);
  }
  if (!existsSync(app)) await run([process.execPath, 'scripts/exact.mjs', 'new', app, '--name', 'Widget Keychain', '--bundle-id', id], 'new');
  const manifest = JSON.parse(readFileSync(resolve(app, 'app.json'), 'utf8'));
  manifest.widgets = [{ kind: 'secret', title: 'Shared secret', description: 'Keychain integration test', families: ['systemSmall'], component: 'Card', props: 'Props', refresh: 'refresh', preview: { text: 'preview only' } }];
  writeFileSync(resolve(app, 'app.json'), JSON.stringify(manifest));
  writeFileSync(resolve(app, 'app.contract'), `shape Props
  text: string
component App
  mutation saved as shape Props
  action save
    send saved = keep()
  view
    column
      button press=save
        text "Save test secret"
      match saved
        case some(value)
          text value.text
        case none
          text "Not saved"
component Card
  props
    text: string
  view
    text text
`);
  writeFileSync(resolve(app, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';
export const appId = ${JSON.stringify(id)};
export const grants = 'secret.keep widget-test';
const sources: Sources = {
  keep: (_args, store) => { store.set('widget-test', ${JSON.stringify(nonce)}); return {text:'Saved'}; },
  refresh: ([date], store) => [{date, props:{text:store.get('widget-test') ?? 'missing secret'}}],
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
`);
  await run([process.execPath, resolve(app, 'exact.mjs'), 'ios', '--run'], 'build');
  const kit = resolve(dir, 'archive');
  mkdirSync(resolve(kit, 'out'), { recursive: true });
  cpSync(resolve(process.env.EXACT_WIDGET_KIT), resolve(kit, 'kit'), { recursive: true });
  const control = resolve(kit, 'kit/control.mjs'), state = resolve(kit, 'build/control.json');
  writeFileSync(resolve(dir, 'controller.log'), '');
  const server = Bun.spawn([process.execPath, control, sim, 'serve'], { cwd: root, env: { ...env, PROBE_APP_ID: id }, stdout: Bun.file(resolve(dir, 'controller.log')), stderr: 'inherit' });
  try {
    let exited = false;
    server.exited.then(() => { exited = true; });
    const until = Date.now() + 90_000;
    while (!readFileSync(resolve(dir, 'controller.log'), 'utf8').includes('READY ') && !exited && Date.now() < until) await Bun.sleep(100);
    expect(readFileSync(resolve(dir, 'controller.log'), 'utf8'), 'the archive driver must reach its simulator').toContain('READY ');
    const flow = async (steps, name) => run([process.execPath, control, sim, 'flow', JSON.stringify(steps), name], name);
    await flow([{op:'app'}, {op:'appbutton',label:'Save test secret'}, {op:'home'}, {op:'hold',xy:[210,650],seconds:1.2}, {op:'button',label:'Edit'}, {op:'button',label:'Add Widget'}, {op:'search',text:'Widget Keychain'}], 'search');
    await flow([{op:'text',label:'Widget Keychain'}, {op:'button',label:'Add Widget'}, {op:'button',label:'Done'}], 'add');
    await flow({expect:nonce}, 'secret');
    expect(JSON.parse(readFileSync(resolve(kit, 'out/secret.json'), 'utf8')).tree).toContain(nonce);
    // Only getTimeline reads the Keychain; gallery/placeholder use "preview only".
    await run(['xcrun','simctl','spawn',sim,'log','show','--last','2m','--predicate','subsystem == "dev.exact.widgets"','--style','compact'], 'extension');
    expect(readFileSync(resolve(dir, 'extension.log'), 'utf8')).toContain('getTimeline');
  } finally {
    if (existsSync(state)) {
      const owned = JSON.parse(readFileSync(state, 'utf8'));
      if (owned.serverPid === server.pid) { try { process.kill(owned.xcodebuildPid, 'SIGTERM'); } catch {} }
    }
    server.kill();
  }
}, 900_000);
