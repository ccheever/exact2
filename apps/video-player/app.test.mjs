import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs video-player`.
for (const scheme of ['light', 'dark']) {
  test(`the player and its controls stay dark under ${scheme} preference`, async () => {
    const session = await open({ host: 'web', app: 'video-player', size: [420, 900],
      env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    try {
      await session.prefer({ 'prefers-color-scheme': scheme });
      await session.clock('data');
      expect((await session.state()).slots.source).toBe('assets/motion.mp4');
      expect((await session.state()).slots.duration).toBeGreaterThan(0);
      const paint = await session.carrier.evaluate(`(() => {
        const rgba = value => value.match(/[\\d.]+/g).map(Number);
        return Object.fromEntries(['player-app', 'source', 'note', 'done', 'video', 'progress-fill'].map(id => {
          const element = document.querySelector('[data-testid="' + id + '"]');
          const style = getComputedStyle(element);
          const ink = getComputedStyle(id === 'done' ? element.firstElementChild : element).color;
          return [id, { scheme: style.colorScheme, fill: rgba(style.backgroundColor), ink: rgba(ink) }];
        }));
      })()`);
      expect(Object.fromEntries(['player-app', 'source', 'note'].map(id => [id, paint[id].scheme])))
        .toEqual({ 'player-app': 'dark', source: 'dark', note: 'dark' });
      for (const id of ['player-app', 'source', 'note']) {
        expect(Math.max(...paint[id].fill.slice(0, 3))).toBeLessThanOrEqual(64);
        expect(ratio(paint[id].ink, paint[id].fill)).toBeGreaterThanOrEqual(4.5);
      }
      expect(paint.video.fill).toEqual([0, 0, 0]);
      expect(paint.done.fill).toEqual(paint['progress-fill'].fill);
      // The pale accent needs dark ink; Apple's AccentColorText would be white.
      expect(paint.done.ink).toEqual([0, 0, 0]);
      expect(ratio(paint.done.ink, paint.done.fill)).toBeGreaterThanOrEqual(4.5);
      const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
      const surface = '#' + paint['player-app'].fill.slice(0, 3)
        .map(n => n.toString(16).padStart(2, '0')).join('');
      expect(manifest.theme_color).toBe(surface);
      expect(scheme === 'dark' ? manifest.background_color_dark ?? manifest.background_color
        : manifest.background_color).toBe(surface);
      await session.type('note', 'Keep this moment.');
      await session.tap('done');
      expect((await session.state()).slots.note).toBe('Keep this moment.');
      const checked = await checkOp(session, [scheme]);
      expect(checked.check.map(report => report.scheme)).toEqual([scheme]);
      expect(checked.check[0].findings.filter(f => f.kind === 'contrast')).toEqual([]);
      await session.carrier.evaluate(`document.querySelector('video').dispatchEvent(new Event('error'))`);
      await session.tree('video-error');
      expect((await session.state()).slots.failure).toBe('src-not-supported');
      const failed = await checkOp(session, [scheme]);
      expect(failed.check[0].findings.filter(f => f.kind === 'contrast')).toEqual([]);
    } finally {
      await session.close();
    }
  }, 30000);
}
