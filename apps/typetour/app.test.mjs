import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs typetour`.
for (const appearance of ['system', 'light', 'dark']) {
  test(`Type Tour contrast with ${appearance} appearance`, async () => {
    const session = await open({ host: 'web', app: 'typetour', size: [420, 900],
      env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    try {
      const failures = [];
      const check = async (screen, part = 'top') => {
        await session.clock('settle');
        await session.tree(screen);
        const state = (await session.state()).slots;
        expect(state.screen).toBe(screen === 'messages-list' || screen === 'thread-screen'
          ? 'messages' : screen.replace('-screen', ''));
        if (screen === 'thread-screen') expect(state.thread).toBe('nico');
        if (screen === 'messages-list') expect(state.thread).toBe('');
        const result = await checkOp(session, [appearance === 'system' ? 'both' : appearance]);
        expect(result.check.map(report => report.scheme))
          .toEqual(appearance === 'system' ? ['light', 'dark'] : [appearance]);
        for (const report of result.check) {
          const contrast = report.findings.filter(f => f.kind === 'contrast');
          failures.push(...contrast.map(f => ({ screen, part, scheme: report.scheme, ...f })));
        }
      };
      // Check each viewport of the long Settings and specimen pages.
      const scrollPage = async screen => {
        let more = true;
        while (more) {
          const position = await session.carrier.evaluate(`(() => {
            const root = document.querySelector('[data-testid="${screen}"]');
            const scroller = [...root.querySelectorAll('*')].find(el =>
              ['auto', 'scroll'].includes(getComputedStyle(el).overflowY));
            if (!scroller) throw new Error('Missing scroller in ${screen}');
            const before = scroller.scrollTop;
            scroller.scrollTop += scroller.clientHeight * 0.8;
            return { before, after: scroller.scrollTop,
              more: scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1 };
          })()`);
          expect(position.after).toBeGreaterThan(position.before);
          await check(screen, `scroll ${position.after}`);
          more = position.more;
        }
      };
      await session.tap('lock-unlock');
      await session.tap('app-typetour');
      expect((await session.type('appearance', appearance)).error).toBeUndefined();
      expect((await session.state()).slots.appearance).toBe(appearance);
      // Explicit selections must win even when the device asks for the other scheme.
      if (appearance !== 'system') {
        await session.prefer({ 'prefers-color-scheme': appearance === 'light' ? 'dark' : 'light' });
        expect((await session.op({ op: 'check' })).scheme).toBe(appearance);
      }
      await check('display-screen');
      await scrollPage('display-screen');
      await session.tap('back-settings');
      await check('settings-screen');
      await scrollPage('settings-screen');
      await session.tap('home-bar');
      await check('home-screen');
      await session.tap('home-lock');
      await check('lock-screen');
      await session.tap('lock-unlock');
      await session.tap('dock-messages');
      await check('messages-list');
      await session.tap('conversation-nico');
      await check('thread-screen');
      await session.type('composer', 'The new type looks good.');
      await session.tree('send');
      expect((await session.state()).slots.draft).toBe('The new type looks good.');
      await check('thread-screen', 'send visible');
      await session.tap('send');
      expect((await session.state()).slots.draft).toBe('');
      expect(failures).toEqual([]);
    } finally {
      await session.close();
    }
  }, 30000);
}

// Static search hints must remain readable even if a role exempts them from checkOp.
test('search hints clear 3:1 in Settings and Messages in both schemes', async () => {
  const session = await open({ host: 'web', app: 'typetour', size: [420, 900],
    env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
  try {
    const readings = [];
    await session.tap('lock-unlock');
    for (const [screen, button] of [['settings', 'dock-settings'], ['messages', 'dock-messages']]) {
      await session.tap(button);
      expect((await session.state()).slots.screen).toBe(screen);
      for (const scheme of ['light', 'dark']) {
        await session.prefer({ 'prefers-color-scheme': scheme });
        await session.clock('settle');
        const observed = await session.op({ op: 'check' });
        expect(observed.scheme).toBe(scheme);
        const hints = observed.items.filter(item => item.text === 'Search');
        expect(hints).toHaveLength(1);
        const hint = hints[0];
        expect(hint.y).toBeGreaterThanOrEqual(0);
        expect(hint.y + hint.h).toBeLessThanOrEqual(observed.viewport.h);
        expect(hint.bg).toHaveLength(3);
        // The observer's background includes ancestor fills; composite translucent ink onto it.
        const alpha = hint.fg[3] ?? 1;
        const ink = hint.fg.slice(0, 3).map((channel, i) => channel * alpha + hint.bg[i] * (1 - alpha));
        readings.push({ screen, scheme, contrast: ratio(ink, hint.bg) });
      }
      await session.tap('home-bar');
    }
    expect(readings.filter(reading => reading.contrast < 3)).toEqual([]);
  } finally {
    await session.close();
  }
}, 30000);
