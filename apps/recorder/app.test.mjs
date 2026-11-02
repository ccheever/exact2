import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

test('Recorder can stop and save when the microphone becomes unavailable', async () => {
  const session = await open({ host: 'web', app: 'recorder', storage: `mic-reset-${Date.now()}`,
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    await session.prefer({ 'permission.microphone': 'granted' });
    await session.tap('toggle');
    await session.clock('+3000');
    await session.prefer({ extension: 'media-capture.microphoneStatus={"available":false}' });
    await session.clock('settle');
    expect((await session.state()).resources.mic.available).toBe(false);
    await session.tap('toggle');
    await session.clock('settle');
    const state = await session.state();
    expect(state.slots.recorderId).toBe('');
    expect(state.resources.library.takes.length).toBe(1);
  } finally {
    await session.close();
  }
});

test('Recorder offers a missing local language and waits for installation before transcribing', async () => {
  const session = await open({ host: 'web', app: 'recorder', size: [420, 720], storage: `language-${Date.now()}`,
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  const ids = async () => new Set((await session.tree()).nodes.map(node => node.props?.testId));
  const availability = async value => {
    await session.prefer({ extension: `speech-recognition.speechRecognitionAvailability=${JSON.stringify({ availability: value })}` });
    await session.clock('settle');
  };
  try {
    await session.prefer({ 'permission.microphone': 'granted', 'permission.speech-recognition': 'granted' });
    await availability('downloadable');
    expect((await session.state()).resources.speech.local).toBe(true);
    expect((await ids()).has('speech-install')).toBe(true);
    await session.tap('toggle');
    await session.clock('settle');
    expect((await session.state()).slots.recognitionId).toBe('');
    await session.clock('+3000');
    await session.tap('toggle');
    await session.clock('settle');
    expect((await ids()).has('transcribe-take-1')).toBe(false);
    await availability('downloading');
    expect((await ids()).has('speech-downloading')).toBe(true);
    expect((await ids()).has('speech-install')).toBe(false);
    await availability('available');
    expect((await ids()).has('speech-local')).toBe(false);
    expect((await ids()).has('transcribe-take-1')).toBe(true);
  } finally {
    await session.close();
  }
}, 30000);

// Build first with `bun host/web/build.mjs recorder`.
test('Recorder follows the scheme with readable recording and saved states', async () => {
  const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
  const session = await open({ host: 'web', app: 'recorder', size: [420, 720], storage: `scheme-${Date.now()}`,
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    await session.prefer({ 'permission.microphone': 'granted' });
    const readPaint = () => {
      const node = id => document.querySelector(`[data-testid="${id}"]`);
      const rgba = value => value.match(/[\d.]+/g).map(Number);
      const over = (colour, behind) => colour.slice(0, 3).map((channel, i) =>
        channel * (colour[3] ?? 1) + behind[i] * (1 - (colour[3] ?? 1)));
      const background = element => element
        ? over(rgba(getComputedStyle(element).backgroundColor), background(element.parentElement))
        : [255, 255, 255];
      const paint = (element, label = element) => {
        const fill = background(element);
        return { fill, ink: over(rgba(getComputedStyle(label).color), fill) };
      };
      return {
        schemes: getComputedStyle(node('recorder')).colorScheme.split(' '),
        preference: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
        page: paint(node('recorder'), node('recorder').firstElementChild),
        status: paint(node('status') ?? node('elapsed')),
        toggle: paint(node('toggle'), node('toggle').firstElementChild),
        label: node('toggle').textContent,
        saved: node('saved-message') ? paint(node('saved-message')) : null,
        rows: [...node('takes').children].map(row => paint(row, row.firstElementChild)),
      };
    };
    const failures = [];
    const require = (condition, detail) => { if (!condition) failures.push(detail); };
    for (const state of ['idle', 'recording', 'saved']) {
      if (state !== 'idle') await session.tap('toggle');
      await session.clock('settle');
      const data = (await session.state()).resources;
      expect(data.mic.available).toBe(true);
      expect(data.rec.state === 'recording').toBe(state === 'recording');
      expect(data.library.takes.length).toBe(state === 'saved' ? 1 : 0);
      const paints = [];
      for (const scheme of ['light', 'dark']) {
        await session.prefer({ 'prefers-color-scheme': scheme });
        await session.clock('settle');
        const paint = await session.carrier.evaluate(`(${readPaint.toString()})()`);
        paints.push(paint);
        require(paint.preference === scheme && paint.schemes.includes(scheme),
          { state, scheme, preference: paint.preference, supported: paint.schemes });
        expect(paint.label).toBe(state === 'recording' ? 'Stop' : 'Record');
        expect(paint.saved !== null).toBe(state === 'saved');
        expect(paint.rows.length).toBe(state === 'saved' ? 1 : 0);
        for (const [name, surface] of Object.entries({ page: paint.page, status: paint.status,
          ...Object.fromEntries(paint.rows.map((row, i) => [`take-${i}`, row])) })) {
          require(scheme === 'dark' ? Math.max(...surface.fill) < 65 : Math.min(...surface.fill) > 200,
            { state, scheme, name, expected: `${scheme} surface`, surface });
          // Secondary labels follow the platform's 3:1 contrast allowance.
          require(ratio(surface.ink, surface.fill) >= (name === 'status' ? 3 : 4.5),
            { state, scheme, name, expected: 'readable ink', surface });
        }
        for (const [name, surface] of Object.entries({ toggle: paint.toggle,
          ...(paint.saved ? { saved: paint.saved } : {}) })) {
          require(ratio(surface.ink, surface.fill) >= 4.5,
            { state, scheme, name, expected: 'contrast >= 4.5', surface });
        }
        const launch = scheme === 'light' ? manifest.background_color : manifest.background_color_dark;
        const surface = '#' + paint.page.fill.map(n => n.toString(16).padStart(2, '0')).join('');
        require(launch === surface, { state, scheme, expected: 'launch matches page', launch, surface });
        const checked = await checkOp(session, [scheme]);
        expect(checked.check.map(report => report.scheme)).toEqual([scheme]);
        failures.push(...checked.check[0].findings.filter(f => f.kind === 'contrast')
          .map(f => ({ state, scheme, ...f })));
      }
      for (const property of ['page', 'status', 'toggle', ...(state === 'saved' ? ['saved', 'rows'] : [])]) {
        require(JSON.stringify(paints[0][property]) !== JSON.stringify(paints[1][property]),
          { state, expected: `${property} changes with scheme` });
      }
      for (const property of ['page', 'status', ...(state === 'saved' ? ['saved'] : [])]) {
        require(JSON.stringify(paints[0][property].ink) !== JSON.stringify(paints[1][property].ink),
          { state, expected: `${property} ink changes with scheme` });
      }
      if (state === 'recording') await session.clock('+3000');
    }
    expect(failures).toEqual([]);
  } finally {
    await session.close();
  }
}, 30000);
