// The design server's watcher (scripts/tamber-server.mjs `watchAll`): a directory replaced and filled again before
// its new watch is set is still read, since what landed in it then has no event of its own. Run on the real server
// (`contract tamber serve`), the fill made inside the watch call itself, so the gap is there every run.
import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, watch, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

test('a directory replaced, then filled before its watch is set, is read once the watch is', async () => {
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-watch-'));
  mkdirSync(join(repo, 'src/ui'), { recursive: true });
  const POST = 'component PostButton\n  view\n    text "Post"\n';
  writeFileSync(join(repo, 'src/ui/post.contract'), POST);
  writeFileSync(join(repo, 'app.contract'), 'use PostButton from "./src/ui/post.contract"\ncomponent App\n  view\n    PostButton()\n');
  writeFileSync(join(repo, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  // Armed, the next watch of src/ fills it first: src/ui/post.contract lands after the read that found src/ empty
  // and before src/'s watch, as a checkout's can. The swap's own events on the app's directory that come after it
  // (the platform's, late) are dropped, as though they had come before the read, so nothing reads it by chance.
  let fill = null, filled = false;
  const watchDir = (dir, listener) => {
    if (fill && dir === join(repo, 'src')) {
      mkdirSync(join(repo, 'src/ui'));
      writeFileSync(join(repo, 'src/ui/post.contract'), fill);
      fill = null; filled = true;
    }
    return watch(dir, (event, name) => { if (!(filled && dir === repo)) listener(event, name); });
  };
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port, at = `http://127.0.0.1:${port}`;
  const gate = developmentGate([{ origin: at }], port);
  const wire = tamberServer({ appDir: repo, watchDir, log: () => {}, settle: 50 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  const digest = f => createHash('sha256').update(readFileSync(join(repo, f))).digest('hex');
  try {
    await wire.ready;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const res = await fetch(`${at}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': token } });
    const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
    let buf = '';
    (async () => { for (;;) { const { value, done } = await reader.read().catch(() => ({ done: true })); if (done) return; buf += decoder.decode(value, { stream: true }); const parts = buf.split('\n\n'); buf = parts.pop(); for (const p of parts) if (p.startsWith('data: ')) events.push(JSON.parse(p.slice(6))); } })();
    const until = async (pass, what) => { const end = Date.now() + 20_000; while (!pass(events)) { if (Date.now() > end) throw new Error(`no event: ${what}, in ${JSON.stringify(events)}`); await Bun.sleep(20); } };
    await until(e => e[0]?.event === 'subscribed' && e[0].files['src/ui/post.contract'] === digest('src/ui/post.contract'), 'subscribed with the source');
    for (let round = 1; round <= 3; round++) {
      // Quiet first: past the settle, then a read answered (the server answers in order), so no read taken before the
      // swap lands after it.
      await Bun.sleep(300);
      expect((await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify({ method: 'hello' }) })).status).toBe(200);
      fill = POST.replace('Post"', `Post ${round}"`); filled = false;
      renameSync(join(repo, 'src'), join(repo, `src.old${round}`));
      mkdirSync(join(repo, 'src'));
      // src/'s new watch is set (the fill made in it), then its source is read: the fill had no event of its own.
      await until(() => filled, `src/ watched again, round ${round}`);
      await until(e => e.at(-1)?.files?.['src/ui/post.contract'] === digest('src/ui/post.contract'), `the source filled before the watch, round ${round}`);
    }
    reader.cancel();
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 120_000);
