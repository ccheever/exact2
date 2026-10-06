// The web's output for the voice table (LLP 1096 D6, D7, §5), in Chrome,
// Firefox and WebKit: sound-glue.js over an OfflineAudioContext at 48 kHz,
// rendered and read back by frame, its runner time mapped through an output
// timestamp the test supplies; the held voice of a `{0, 0}` timestamp; the
// Audio Session API; and the drop before a user activation. Firefox and
// WebKit come from Playwright (`bunx playwright@1.63.0 install firefox
// webkit`), Chrome from the machine's; an engine that is missing is skipped.
import { test, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { chromium as installed } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const glue = readFileSync(resolve(WEB, 'sound-glue.js'), 'utf8');
const page = session => `<!doctype html><meta charset="utf-8"><div id="exact-root"${session ? ` data-audio-session="${session}"` : ''}></div><button id="press">press</button>
<script type="module">
  import './sound-glue.js';
  // A 48 kHz 16-bit mono WAV of \`frames\` frames, every sample \`value\`.
  window.wav = (frames, value) => {
    const b = new DataView(new ArrayBuffer(44 + 2 * frames)), s = (at, t) => [...t].forEach((c, i) => b.setUint8(at + i, c.charCodeAt(0)));
    s(0, 'RIFF'); b.setUint32(4, 36 + 2 * frames, true); s(8, 'WAVEfmt '); b.setUint32(16, 16, true); b.setUint16(20, 1, true); b.setUint16(22, 1, true);
    b.setUint32(24, 48000, true); b.setUint32(28, 96000, true); b.setUint16(32, 2, true); b.setUint16(34, 16, true); s(36, 'data'); b.setUint32(40, 2 * frames, true);
    for (let i = 0; i < frames; i++) b.setInt16(44 + 2 * i, Math.round(value * 32767), true);
    return b.buffer;
  };
  // An output over an OfflineAudioContext of \`seconds\`, its timestamp \`stamp()\`, runner time 0 at 1000 ms.
  window.offline = (seconds, files, stamp = () => ({ contextTime: 0, performanceTime: 1000 }), now = () => 1000) => {
    const context = new OfflineAudioContext({ numberOfChannels: 1, length: 48000 * seconds, sampleRate: 48000 }), lines = [];
    const out = globalThis.exact.installSound({ files: Object.keys(files), log: l => lines.push(l), origin: () => 1000, context, timestamp: stamp, now, activated: () => true, fetchFile: src => Promise.resolve(files[src]) });
    return { out, context, lines };
  };
  // Each node's start and stop, as called.
  window.calls = [];
  const { start, stop } = AudioBufferSourceNode.prototype;
  AudioBufferSourceNode.prototype.start = function (when = 0, ...rest) { calls.push(['start', when]); return start.call(this, when, ...rest); };
  AudioBufferSourceNode.prototype.stop = function (when = 0) { calls.push(['stop', when]); return stop.call(this, when); };
  window.decoded = () => new Promise(r => setTimeout(r, 300));
  // Before any activation (Playwright's own evaluate runs as a gesture): an
  // output on the page's real AudioContext, and a voice it is handed at once.
  if (location.search.includes('playback')) {
    window.lines = [];
    window.out = globalThis.exact.installSound({ files: ['assets/a.wav'], log: l => lines.push(l), origin: () => 0, fetchFile: () => Promise.resolve(wav(4800, 0.5)) });
    window.early = { state: out.context.state, stamp: out.context.getOutputTimestamp?.() };
    await decoded();
    out.ops([{ op: 'play', id: 1, sound: 0, at: performance.now() + 50, gain: 1 }]);
  }
  window.ready = true;
</script>`;

const engines = ['chromium', 'firefox', 'webkit'];

async function browser(name) {
  const playwright = await import('playwright-core');
  if (name === 'chromium') {
    const { executable, unavailable } = installed();
    if (unavailable) return null;
    return playwright.chromium.launch({ executablePath: executable, headless: true, args: ['--autoplay-policy=user-gesture-required'] });
  }
  if (!existsSync(playwright[name].executablePath())) return null;
  return playwright[name].launch({ headless: true });
}

async function serve(session) {
  const server = createServer((req, res) => {
    if (req.url.startsWith('/sound-glue.js')) { res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(glue); return; }
    res.writeHead(200, { 'content-type': 'text/html' }); res.end(page(req.url.includes('playback') ? 'playback' : session));
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() };
}

for (const engine of engines) {
  test(`${engine}: voices render at the frames runner time maps to`, async () => {
    const b = await browser(engine);
    if (!b) return console.log(`skip: ${engine} is not installed`);
    const server = await serve();
    try {
      const p = await b.newPage();
      await p.goto(server.url);
      await p.waitForFunction(() => window.ready);
      const got = await p.evaluate(async () => {
        const first = (data, from = 0) => { for (let i = from; i < data.length; i++) if (Math.abs(data[i]) > 1e-4) return i; return -1; };
        // A voice at 250 ms, gain 0.5, over a constant 0.5: its first frame is 12000, at 0.25.
        const a = offline(1, { 'assets/a.wav': wav(48000, 0.5) });
        a.out.ops([{ op: 'play', id: 1, sound: 0, at: 250, gain: 0.5 }]);
        await decoded();
        const one = (await a.context.startRendering()).getChannelData(0);
        // A choke: voice 1 at 250 cut by voice 2 at 300, which sounds -0.25.
        const c = offline(1, { 'assets/a.wav': wav(48000, 0.5), 'assets/b.wav': wav(48000, -0.25) });
        c.out.ops([{ op: 'play', id: 1, sound: 0, at: 250, gain: 1 }, { op: 'play', id: 2, sound: 1, at: 300, gain: 1 }, { op: 'end', id: 1, at: 300 }]);
        await decoded();
        const two = (await c.context.startRendering()).getChannelData(0);
        // A voice mapped 40 ms into the past starts at once, attack included, and nothing throws.
        const l = offline(1, { 'assets/a.wav': wav(4800, 0.5) });
        l.out.ops([{ op: 'play', id: 1, sound: 0, at: -40, gain: 1 }]);
        await decoded();
        const three = (await l.context.startRendering()).getChannelData(0);
        return { first: first(one), value: one[first(one)], cutBefore: two[14399], cutAt: two[14400], cutFirst: first(two), late: first(three), lateValue: three[0], lateCount: l.out.state().late, lines: [...a.lines, ...c.lines, ...l.lines] };
      });
      expect(got.first).toBe(12000);
      expect(got.value).toBeCloseTo(0.25, 3);
      expect(got.cutFirst).toBe(12000);
      expect(got.cutBefore).toBeCloseTo(0.5, 3);
      expect(got.cutAt).toBeCloseTo(-0.25, 3);
      expect(got.late).toBe(0);
      expect(got.lateValue).toBeCloseTo(0.5, 3);
      expect(got.lateCount).toBe(1);
      expect(got.lines).toEqual([]);
    } finally { await b.close(); server.close(); }
  }, 60_000);

  test(`${engine}: a held voice starts once, by the formula or by its own time`, async () => {
    const b = await browser(engine);
    if (!b) return console.log(`skip: ${engine} is not installed`);
    const server = await serve();
    try {
      const p = await b.newPage();
      await p.goto(server.url);
      await p.waitForFunction(() => window.ready);
      const got = await p.evaluate(async () => {
        // `{0, 0}` until the test says the output is live.
        let liveAt = null;
        const stamp = () => liveAt == null ? { contextTime: 0, performanceTime: 0 } : { contextTime: 0, performanceTime: 1000 };
        const h = offline(1, { 'assets/a.wav': wav(48000, 0.5) }, stamp);
        await decoded();
        calls.length = 0;
        // Due (runner time 0 at now 1000): `start(0)`. Future: held, no node.
        h.out.ops([{ op: 'play', id: 1, sound: 0, at: 0, gain: 1 }, { op: 'play', id: 2, sound: 0, at: 200, gain: 1 }]);
        const held = calls.slice();
        // An End during the hold sets its stopAt and calls no stop.
        h.out.ops([{ op: 'end', id: 2, at: 250 }]);
        const afterEnd = calls.slice();
        liveAt = 1;
        await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
        const live = calls.slice();
        // The timer's path: a voice 30 ms ahead while the timestamp stays `{0, 0}`.
        calls.length = 0;
        const origin = performance.now();
        const t = offline(1, { 'assets/a.wav': wav(48000, 0.5) }, () => ({ contextTime: 0, performanceTime: 0 }), () => performance.now() - origin + 1000);
        await decoded();
        const at = performance.now() - origin + 30;
        t.out.ops([{ op: 'play', id: 1, sound: 0, at, gain: 1 }, { op: 'play', id: 2, sound: 0, at: at + 5, gain: 1 }, { op: 'end', id: 2, at: at + 5 }]);
        const before = calls.slice();
        await new Promise(r => setTimeout(r, 120));
        return { held, afterEnd, live, before, timer: calls.slice() };
      });
      expect(got.held).toEqual([['start', 0]]);
      expect(got.afterEnd).toEqual([['start', 0]]);
      expect(got.live).toEqual([['start', 0], ['start', 0.2], ['stop', 0.25]]);
      // A voice cut at its start never starts; the other starts once, at once.
      expect(got.before).toEqual([]);
      expect(got.timer).toEqual([['start', 0]]);
    } finally { await b.close(); server.close(); }
  }, 60_000);

  test(`${engine}: the session, the press, and the drop before activation`, async () => {
    const b = await browser(engine);
    if (!b) return console.log(`skip: ${engine} is not installed`);
    const server = await serve();
    try {
      const p = await b.newPage();
      await p.goto(server.url + '?playback');
      // Not `waitForFunction`: Playwright evaluates as a user gesture, which would activate the page.
      await p.waitForTimeout(1000);
      const got = await p.evaluate(() => ({ session: navigator.audioSession?.type ?? null, has: 'audioSession' in navigator, lines, ...early }));
      expect(got.lines).toEqual(['sound blocked: the page has had no user activation (assets/a.wav)']);
      if (got.has) expect(got.session).toBe('playback');
      else expect(got.session).toBe(null);
      if (engine === 'webkit') expect(got.has).toBe(true);
      // The press: the capture-phase listener resumes the context, and the press's voice is scheduled.
      calls: {
        await p.evaluate(() => { calls.length = 0; document.getElementById('press').addEventListener('click', () => out.ops([{ op: 'play', id: 2, sound: 0, at: performance.now() + 100, gain: 1 }])); });
        await p.click('#press');
        const pressed = await p.evaluate(() => ({ calls: calls.slice(), state: out.state(), stamp: out.context.getOutputTimestamp?.() }));
        expect(pressed.state.blocked).toBe(1);
        // Started at once (the `{0, 0}` hold's due voice is start(0)), or held for its time, or scheduled ahead by the formula.
        expect(pressed.calls.length === 0 || pressed.calls[0][0] === 'start').toBe(true);
        await p.waitForTimeout(250);
        const later = await p.evaluate(() => calls.slice());
        expect(later.filter(c => c[0] === 'start').length).toBe(1);
        // Firefox before `running`: a live performanceTime, and the formula schedules ahead.
        if (engine === 'firefox' && got.state === 'suspended' && got.stamp?.performanceTime > 0) expect(later[0][1]).toBeGreaterThan(0);
      }
    } finally { await b.close(); server.close(); }
  }, 60_000);
}
