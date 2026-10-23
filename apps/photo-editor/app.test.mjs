import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { open } from '../../scripts/agent.mjs';
import { checkOp, ratio } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs photo-editor`.
for (const scheme of ['light', 'dark']) {
  test(`photo editor chrome uses dark roles under ${scheme} preference`, async () => {
    const session = await open({ host: 'web', app: 'photo-editor', size: [420, 900],
      env: { EXACT_AGENT_EPHEMERAL_PORT: '1' } });
    try {
      await session.prefer({ 'prefers-color-scheme': scheme });
      await session.clock('settle');
      const readPaint = () => {
        const node = id => document.querySelector(`[data-testid="${id}"]`);
        const rgba = value => value.match(/[\d.]+/g).map(Number);
        const over = (colour, behind) => colour.slice(0, 3).map((channel, i) =>
          channel * (colour[3] ?? 1) + behind[i] * (1 - (colour[3] ?? 1)));
        const background = element => element
          ? over(rgba(getComputedStyle(element).backgroundColor), background(element.parentElement))
          : [255, 255, 255];
        const paint = (element, label = element) => {
          const style = getComputedStyle(element), ink = rgba(getComputedStyle(label).color);
          const fill = background(element);
          return { scheme: style.colorScheme, rawFill: rgba(style.backgroundColor), rawInk: ink,
            fill, ink: over(ink, fill) };
        };
        const root = node('photo-editor-app'), panel = node('edit').parentElement;
        // Resolve the palette independently of the app's declared scheme and paints.
        const probe = document.createElement('span');
        probe.style.colorScheme = 'dark';
        root.append(probe);
        const roles = Object.fromEntries(['background', 'secondary-background', 'label', 'secondary-label', 'fill']
          .map(role => {
            probe.style.color = `var(--exact-${role})`;
            return [role, rgba(getComputedStyle(probe).color)];
          }));
        probe.remove();
        return {
          preference: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light', roles,
          surfaces: {
            page: paint(root), panel: paint(panel), status: paint(node('status')),
            heading: paint(panel.firstElementChild), edit: paint(node('edit')), edits: paint(node('edits')),
            rotate: paint(node('rotate'), node('rotate').firstElementChild),
            reset: paint(node('reset'), node('reset').firstElementChild),
          },
        };
      };
      const { preference, roles, surfaces } = await session.carrier.evaluate(`(${readPaint.toString()})()`);
      expect(preference).toBe(scheme);
      expect(Object.fromEntries(Object.entries(surfaces).map(([name, paint]) => [name, paint.scheme])))
        .toEqual(Object.fromEntries(Object.keys(surfaces).map(name => [name, 'dark'])));
      expect(surfaces.page.rawFill).toEqual(roles.background);
      expect(surfaces.page.fill).toEqual([0, 0, 0]);
      expect(surfaces.panel.rawFill).toEqual(roles['secondary-background']);
      expect(surfaces.reset.rawFill).toEqual(roles.fill);
      for (const name of ['page', 'panel', 'edit', 'reset']) {
        expect(surfaces[name].rawInk).toEqual(roles.label);
      }
      for (const name of ['status', 'heading', 'edits']) {
        expect(surfaces[name].rawInk).toEqual(roles['secondary-label']);
      }
      expect(surfaces.rotate.fill).toEqual([190, 214, 255]);
      // Explicit black also applies on Apple, where AccentColorText stays white.
      expect(surfaces.rotate.ink).toEqual([0, 0, 0]);
      for (const paint of Object.values(surfaces)) {
        expect(ratio(paint.ink, paint.fill)).toBeGreaterThanOrEqual(4.5);
      }
      const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
      expect(manifest.theme_color).toBe('#000000');
      expect(scheme === 'dark' ? manifest.background_color_dark ?? manifest.background_color
        : manifest.background_color).toBe('#000000');
      expect((await session.state()).slots.ready).toBe(true);
      await session.tap('rotate');
      await session.clock('settle');
      expect((await session.state()).slots.turns).toBe(1);
      expect((await session.state()).slots.edit).toContain('rotation 90.0°');
      await session.tap('reset');
      await session.clock('settle');
      expect((await session.state()).slots.turns).toBe(0);
      expect((await session.state()).slots.resets).toBe(1);
      expect((await session.state()).slots.edit).toContain('rotation 0.0°');
      const checked = await checkOp(session, [scheme]);
      expect(checked.check.map(report => report.scheme)).toEqual([scheme]);
      expect(checked.check[0].findings.filter(f => f.kind === 'contrast')).toEqual([]);
    } finally {
      await session.close();
    }
  }, 30000);
}
