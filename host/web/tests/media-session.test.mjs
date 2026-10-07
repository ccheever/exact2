// The web's media session (LLP 1098 D3–D6, §5), in Chrome, Firefox and
// WebKit: media-glue.js over real `audio` and `video` elements whose props and
// handlers the test hands it as the hosts do (`exactMedia`), with
// `navigator.mediaSession`'s calls recorded. The owner rule, what is
// published and never thrown, a retired owner, the artwork, and the remote
// play's latch over `playbackVisibilityThreshold`. Firefox and WebKit come from
// Playwright (`bunx playwright@1.63.0 install firefox webkit`), Chrome from
// the machine's; an engine that is missing is skipped.
import { test, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { chromium as installed } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const glue = readFileSync(resolve(WEB, 'media-glue.js'), 'utf8');
const page = `<!doctype html><meta charset="utf-8"><div id="exact-root"></div>
<script type="module">
  // A second of silence at 8 kHz, a WAV every engine decodes.
  const b = new DataView(new ArrayBuffer(44 + 16000)), s = (at, t) => [...t].forEach((c, i) => b.setUint8(at + i, c.charCodeAt(0)));
  s(0, 'RIFF'); b.setUint32(4, 36 + 16000, true); s(8, 'WAVEfmt '); b.setUint32(16, 16, true); b.setUint16(20, 1, true); b.setUint16(22, 1, true);
  b.setUint32(24, 8000, true); b.setUint32(28, 16000, true); b.setUint16(32, 2, true); b.setUint16(34, 16, true); s(36, 'data'); b.setUint32(40, 16000, true);
  window.src = URL.createObjectURL(new Blob([b], { type: 'audio/wav' }));
  // What the glue asked of navigator.mediaSession, in order; a throw is recorded, never raised.
  window.calls = [];
  const ms = navigator.mediaSession, set = ms.setPositionState.bind(ms), handler = ms.setActionHandler.bind(ms);
  window.handlers = {};
  ms.setPositionState = (...a) => { calls.push(['position', ...a]); try { set(...a); } catch (e) { calls.push(['threw', e.name]); } };
  ms.setActionHandler = (name, f) => { handlers[name] = f; handler(name, f); };
  // What the glue declared, beside what the browser reads back.
  window.declared = [];
  { let o = ms, d; while (o && !(d = Object.getOwnPropertyDescriptor(o, 'playbackState'))) o = Object.getPrototypeOf(o);
    Object.defineProperty(ms, 'playbackState', { get() { return d.get.call(this); }, set(v) { declared.push(v); d.set.call(this, v); } }); }
  let next = 1;
  window.exact = { views: new Map(), assetURL: v => v.startsWith('assets/') ? 'blob:card/' + v : v };
  // A media element the way a host makes one: its props, its handlers, its id, its reports.
  window.make = (tag, props, handlers = [], where = document.getElementById('exact-root')) => {
    const el = document.createElement(tag), id = next++;
    el.dataset.view = String(id); if (props.testId) el.dataset.testid = props.testId;
    el.exactMedia = { props: { src, muted: 'true', ...props }, handlers };
    el.sent = []; el.src = src; el.muted = true;
    where.append(el); exact.views.set(id, el);
    exact.installMedia(el, text => el.sent.push(text));
    return el;
  };
  window.change = (el, props) => { Object.assign(el.exactMedia.props, props); exact.installMedia(el, null); };
  window.frames = (n = 2) => new Promise(r => { const f = k => k ? requestAnimationFrame(() => f(k - 1)) : r(); f(n); });
  window.until = async (ok, ms = 3000) => { const end = performance.now() + ms; while (!ok() && performance.now() < end) await new Promise(r => setTimeout(r, 10)); return ok(); };
  window.meta = title => ({ mediaTitle: title, mediaArtist: 'Show', mediaAlbum: '', mediaArtwork: '' });
  window.read = () => ({ title: ms.metadata?.title ?? null, artwork: ms.metadata ? [...ms.metadata.artwork].map(a => a.src) : null, state: ms.playbackState });
  await import('./media-glue.js');
  window.ready = true;
</script>`;

const engines = ['chromium', 'firefox', 'webkit'];

async function browser(name) {
  const playwright = await import('playwright-core');
  if (name === 'chromium') {
    const { executable, unavailable } = installed();
    if (unavailable) return null;
    return playwright.chromium.launch({ executablePath: executable, headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  }
  if (!existsSync(playwright[name].executablePath())) return null;
  return playwright[name].launch({ headless: true, ...(name === 'firefox' ? { firefoxUserPrefs: { 'media.autoplay.default': 0 } } : {}) });
}

async function serve() {
  const server = createServer((req, res) => {
    if (req.url.startsWith('/media-glue.js')) { res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(glue); return; }
    res.writeHead(200, { 'content-type': 'text/html' }); res.end(page);
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() };
}

async function opened(engine, body) {
  const b = await browser(engine);
  if (!b) return console.log(`skip: ${engine} is not installed`);
  const server = await serve();
  try {
    const p = await b.newPage();
    await p.goto(server.url);
    await p.waitForFunction(() => window.ready);
    await body(p);
  } finally { await b.close(); server.close(); }
}

for (const engine of engines) {
  test(`${engine}: the owner by mount, then by play, kept paused, handed on and cleared`, () => opened(engine, async p => {
    const got = await p.evaluate(async () => {
      const out = {};
      // Two claimants and a non-claimant, mounted in one turn: the later in the document owns.
      const a = make('audio', { testId: 'a', ...meta('A'), seekforwardOffset: '30' }, ['seekforward', 'seekto', 'nexttrack']);
      const b = make('audio', { testId: 'b', ...meta('B') }, ['stop']);
      make('audio', { testId: 'plain' });
      await frames();
      out.mount = { state: exact.mediaSession.state(el => Number(el.dataset.view)), read: read(), handlers: Object.keys(handlers).filter(k => handlers[k]).sort() };
      // `a` plays: it owns, and keeps the session after it pauses.
      await a.play(); await until(() => read().title === 'A'); out.played = read();
      a.pause(); await until(() => read().state === 'paused'); out.paused = { ...read(), owner: exact.mediaSession.state(el => el.dataset.testid).owner };
      // The driver's path: the registered handler, the element's offset when none is given.
      out.forward = exact.mediaSession.act(Number(a.dataset.view), 'seekforward', null);
      out.forward5 = exact.mediaSession.act(Number(a.dataset.view), 'seekforward', 5);
      out.seekto = exact.mediaSession.act(Number(a.dataset.view), 'seekto', 120);
      out.next = exact.mediaSession.act(Number(a.dataset.view), 'nexttrack', null);
      out.notOwner = exact.mediaSession.act(Number(b.dataset.view), 'stop', null);
      out.unhandled = exact.mediaSession.act(Number(a.dataset.view), 'stop', null);
      out.seektoBare = handlers.seekto({ action: 'seekto' });
      out.sent = a.sent.filter(t => !/^(play|pause|playing|loadedmetadata|durationchange|timeupdate|canplay|ratechange|volumechange|seeking|seeked|waiting)\n/.test(t));
      // Unmounting the owner hands the session on; the last unmount clears it.
      exact.removeMedia(a); a.remove();
      await frames(); out.handed = read();
      exact.removeMedia(b); b.remove();
      await frames(); out.cleared = { ...read(), declared: declared.at(-1), handlers: Object.keys(handlers).filter(k => handlers[k]) };
      // A retired owner's late report publishes nothing.
      const before = calls.length; a.dispatchEvent(new Event('play')); out.late = calls.length - before;
      return out;
    });
    expect(got.mount.state.testId).toBe('b');
    expect(got.mount.state.claimants).toEqual([1, 2]);
    expect(got.mount.state.actions).toEqual(['pause', 'play', 'stop']);
    expect(got.mount.read).toEqual({ title: 'B', artwork: [], state: 'paused' });
    expect(got.mount.handlers).toEqual(['pause', 'play', 'stop']);
    expect(got.played).toEqual({ title: 'A', artwork: [], state: 'playing' });
    expect(got.paused).toEqual({ title: 'A', artwork: [], state: 'paused', owner: 'a' });
    expect(got.forward).toEqual({ mediaSession: 'seekforward', seekOffset: 30, seekTime: 0 });
    expect(got.forward5).toEqual({ mediaSession: 'seekforward', seekOffset: 5, seekTime: 0 });
    expect(got.seekto).toEqual({ mediaSession: 'seekto', seekOffset: 0, seekTime: 120 });
    expect(got.next).toEqual({ mediaSession: 'nexttrack', seekOffset: 0, seekTime: 0 });
    expect(got.notOwner).toEqual({ error: 'mediasession: "b" does not own the media session; "a" does' });
    expect(got.unhandled).toEqual({ error: 'mediasession: "a" has no stop' });
    expect(got.seektoBare).toBe(null);
    expect(got.sent).toEqual(['seekforward\n30 0 0', 'seekforward\n5 0 0', 'seekto\n0 120 0', 'nexttrack\n0 0 0']);
    expect(got.handed.title).toBe('B');
    // WebKit 26.6 here read back `paused` after the glue declared `none` (the
    // glue's last write, by the spy); not reproduced on a bare page.
    const { state, ...cleared } = got.cleared;
    expect(cleared).toEqual({ title: null, artwork: null, declared: 'none', handlers: [] });
    if (engine !== 'webkit') expect(state).toBe('none');
    expect(got.late).toBe(0);
  }), 60_000);

  test(`${engine}: what is published never throws, and the artwork is resolved where it is published`, () => opened(engine, async p => {
    const got = await p.evaluate(async () => {
      const out = {};
      const a = make('audio', { testId: 'a', ...meta('A'), mediaArtwork: 'assets/art.png' });
      await until(() => a.readyState > 0); await frames();
      out.card = read().artwork;
      change(a, { mediaArtwork: 'app:/tmp/art.png' }); await frames();
      out.app = { artwork: read().artwork, error: exact.mediaSession.state(el => el.dataset.view).artworkError ?? null };
      change(a, { mediaArtwork: 'https://example.com/a.png' }); await frames();
      out.remote = read().artwork;
      // A position past the end, a rate of 0 and a live source: never handed to the browser.
      await a.play(); await until(() => !a.paused);
      a.currentTime = 0.5; await until(() => read().state === 'playing'); await frames();
      Object.defineProperty(a, 'currentTime', { configurable: true, get: () => 9 });
      a.dispatchEvent(new Event('seeked'));
      Object.defineProperty(a, 'playbackRate', { configurable: true, get: () => 0 });
      a.dispatchEvent(new Event('ratechange'));
      Object.defineProperty(a, 'duration', { configurable: true, get: () => Infinity });
      a.dispatchEvent(new Event('durationchange'));
      a.pause(); await until(() => read().state === 'paused');
      Object.defineProperty(a, 'paused', { configurable: true, get: () => true });
      a.dispatchEvent(new Event('ended')); await frames();
      out.calls = calls; out.ended = read().state;
      return out;
    });
    expect(got.card).toEqual(['blob:card/assets/art.png']);
    expect(got.app.artwork).toEqual([]);
    expect(got.app.error).toContain('app:/');
    expect(got.remote).toEqual(['https://example.com/a.png']);
    expect(got.calls.filter(c => c[0] === 'threw')).toEqual([]);
    const positions = got.calls.filter(c => c[0] === 'position' && c[1]).map(c => c[1]);
    expect(positions.length).toBeGreaterThan(0);
    for (const pos of positions) {
      expect(Number.isFinite(pos.duration)).toBe(true);
      expect(pos.position).toBeLessThanOrEqual(pos.duration);
      expect(pos.playbackRate).toBeGreaterThan(0);
    }
    // The live source cleared it.
    expect(got.calls.at(-1)).toEqual(['position']);
    expect(got.ended).toBe('paused');
  }), 60_000);

  test(`${engine}: a remote play over the visibility threshold stays playing until the app pauses`, () => opened(engine, async p => {
    const got = await p.evaluate(async () => {
      const out = {};
      // Below its threshold: off screen.
      const far = document.createElement('div'); far.style.marginTop = '5000px'; document.getElementById('exact-root').append(far);
      const v = make('video', { testId: 'v', ...meta('V'), paused: 'true', playbackVisibilityThreshold: '0.5' }, ['play', 'pause'], far);
      await until(() => v.readyState > 0); await frames(4);
      out.before = v.paused;
      handlers.play();
      await until(() => !v.paused); out.played = !v.paused;
      // A commit that leaves `paused` (a `play` handler that writes something else) and an intersection callback.
      change(v, { mediaTitle: 'V2' }); await frames(4);
      change(v, { playbackVisibilityThreshold: '0.6' }); await frames(4); // a new observer's first callback
      out.through = !v.paused;
      // The app's mirror, then its pause.
      change(v, { paused: 'false' }); await frames(); out.mirrored = !v.paused;
      change(v, { paused: 'true' }); await until(() => v.paused); out.ended = v.paused;
      return out;
    });
    expect(got).toEqual({ before: true, played: true, through: true, mirrored: true, ended: true });
  }), 60_000);
}
