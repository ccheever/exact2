// LLP 1133.004 stage 0's acceptance, through this module outside the tree:
// an app `exact new` made outside the repo depends on a copy of it by
// `file:`, as on any installed package.
//
// - The bake finds the module wherever the app imports it: from a nested
//   Contract file only, and from data code only.
// - On the web, bound to its implementation: a ring is a button the
//   implementation binds through the host's `activation`; a script's
//   `dispatchEvent` click raises nothing, a trusted click raises `ringclick`
//   once; an emitted `chime` carries the device's origin, so the request its
//   handler makes while `prompt` is refused; a data-code call of a send that
//   answers nothing resolves `undefined`.
// - Under the agent, substituted: the post waits on the permission ticket,
//   is listed, and `tap @ref` raises the event; a relaunch with
//   `--notification '<entry>'` carries the entry's data to the handler.
//
// usage: bun contract/cli/tests/bell/seams.mjs [--keep]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, root = resolve(here, '../../../..');
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

const scratch = mkdtempSync(join(tmpdir(), 'exact-bell-'));
const pkg = join(scratch, 'bell');
cpSync(here, pkg, { recursive: true, filter: (src) => src !== join(here, 'app') && !src.startsWith(join(here, 'app') + '/') && !src.endsWith('seams.mjs') });
const app = join(scratch, 'bellapp');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
cpSync(join(here, 'app'), app, { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@fixture/bell': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);

// Discovery: the app imports the module from app.contract, from the nested
// Panel, and from data code; each alone is enough.
const { usedPackages } = await import(resolve(root, 'js/bake/src/typescript.mjs'));
const found = (dir) => usedPackages(dir).names.has('@fixture/bell');
check('bake', 'finds the module the app imports', found(app), [...usedPackages(app).names]);
const probe = (name, edit) => {
  const dir = join(scratch, name);
  cpSync(app, dir, { recursive: true, filter: (src) => !src.includes('node_modules') });
  edit(dir);
  return found(dir);
};
check('bake', 'finds it imported only from a nested Contract file', probe('nested', (dir) => {
  writeFileSync(join(dir, 'app.contract'), 'use Panel from "./panel.contract"\n\ncomponent App\n  view\n    Panel()\n');
  writeFileSync(join(dir, 'app.ts'), 'export const answer = () => null;\n');
}), 'not found');
check('bake', 'finds it imported only from data code', probe('data', (dir) => {
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "x"\n');
  writeFileSync(join(dir, 'panel.contract'), '');
}), 'not found');

check('bake', 'follows an import of the emitted name, ./src/quiet.js', probe('emitted', (dir) => {
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "x"\n');
  writeFileSync(join(dir, 'panel.contract'), '');
  writeFileSync(join(dir, 'app.ts'), "import { quiet } from './src/quiet.js';\nexport const answer = () => quiet;\n");
}), 'not found');
check('bake', 'counts no type-only import and no import in a comment', !probe('typed', (dir) => {
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "x"\n');
  writeFileSync(join(dir, 'panel.contract'), '');
  writeFileSync(join(dir, 'app.ts'), "import type { Ring } from '@fixture/bell';\n// import { ring } from '@fixture/bell';\n/* import { silence } from '@fixture/bell'; */\nexport const answer = () => null;\n");
}), 'found');

// The web, bound.
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
  return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const browser = await chromium.launch();
const text = (page, testId) => page.getByTestId(testId).innerText().catch(() => '');
const until = async (page, testId, want, ms = 5000) => {
  const end = Date.now() + ms;
  let got = '';
  while (Date.now() < end) { got = await text(page, testId); if (want(got)) return got; await page.waitForTimeout(50); }
  return got;
};
try {
  const page = await browser.newPage();
  await page.goto(origin);
  await page.getByTestId('post').click();
  const bell = page.getByTestId('bell-t1');
  await bell.waitFor({ timeout: 5000 });
  await bell.evaluate((b) => b.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  await page.waitForTimeout(200);
  check('web', 'a script\'s dispatchEvent click raises nothing', (await text(page, 'heard')) === '' && await bell.isVisible(), await text(page, 'heard'));
  await bell.click();
  const heard = await until(page, 'heard', (t) => t !== '');
  check('web', 'a trusted click raises ringclick with the entry\'s data', heard === 't1 thread-1', heard);
  const answered = await until(page, 'answered', (t) => t !== 'none');
  check('web', 'the handler\'s request, a person\'s origin, answers', answered === 'granted', answered);

  const fresh = await browser.newPage();
  await fresh.goto(origin);
  await fresh.getByTestId('chime').click();
  const chimed = await until(fresh, 'heard', (t) => t !== '');
  await fresh.waitForTimeout(300);
  check('web', 'an emitted chime is heard', chimed === 'chime chime-1', chimed);
  check('web', 'its handler\'s request, the device\'s origin, is refused while prompt', (await text(fresh, 'answered')) === 'none' && (await text(fresh, 'status')) === 'prompt', `${await text(fresh, 'answered')} ${await text(fresh, 'status')}`);
  await fresh.getByTestId('hush-data').click();
  const hushed = await until(fresh, 'hushed', (t) => t !== 'none');
  check('web', 'a data-code send that answers nothing resolves undefined', hushed === 'true', hushed);
} finally {
  await browser.close();
  server.stop();
}

// Under the agent: the declared substitute.
const agent = (...ops) => JSON.parse(`[${run('bun', ['exact.mjs', 'agent', 'web', '--json', ...ops], app).trim().split('\n').join(',')}]`);
const [, held] = agent('tap post', 'state');
const ticket = (held.pending ?? []).find((p) => p.device?.capability === 'permission')?.ticket;
check('agent', 'a post while prompt waits on the permission ticket', ticket != null && (held.rings ?? []).length === 0, JSON.stringify(held.pending));
const after = agent('tap post', `tap @${ticket} allow`, 'state');
const entry = after[2].rings?.[0];
check('agent', 'allowed, the post is listed with its data', entry?.tag === 't1' && entry?.data === 'thread-1' && entry?.delivered === true, JSON.stringify(after[2].rings));
const click = agent('tap post', `tap @${ticket} allow`, `tap @${entry?.ref}`, 'clock settle', 'state');
check('agent', 'tap @ref raises ringclick with the entry\'s data', click[4].slots?.heard === 't1 thread-1', JSON.stringify(click[2]));
const launched = agent('--permission', 'bell=granted', '--notification', JSON.stringify(entry ?? {}), 'clock settle', 'state');
check('agent', 'a relaunch\'s --notification carries the entry\'s data to the handler', launched.at(-1).slots?.heard === 't1 thread-1', JSON.stringify(launched.at(-1).slots));

if (!process.argv.includes('--keep')) rmSync(scratch, { recursive: true, force: true });
else console.log(`kept ${scratch}`);
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
