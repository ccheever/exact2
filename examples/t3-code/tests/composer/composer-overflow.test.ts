// The footer's overflow steps and the "More composer
// controls" rows (CompactComposerControlsMenu.tsx, T3 Code 2359643cff, MIT; see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { footerLayout } from '../../src/composer/composer-controls-view';
import { overflowMenu } from '../../src/composer/composer-overflow';

// The effort reads inside the model trigger now, so only the mode block steps.
const base = { model: 'Claude Opus 4.6', suffix: 40, runtime: 'Full access', runtimeShown: true, mode: true, plan: 'Build' };

describe('footerLayout overflow (G11)', () => {
  test('the mode labels go first, then the mode moves into the menu, then the cluster hides; the same thresholds both ways', () => {
    const at = (host: number, previous?: ReturnType<typeof footerLayout>['steps']) => footerLayout({ ...base, host, previous });
    const state = (layout: ReturnType<typeof footerLayout>) => layout.controlsHidden ? 'hidden' : layout.measuredHidden === 1 ? 'mode-menu' : layout.runtimeIconOnly ? 'mode-icon' : 'labels';
    const order: string[] = [];
    let previous: ReturnType<typeof footerLayout>['steps'] | undefined;
    for (let host = 600; host >= 1; host -= 1) {
      const layout = at(host, previous); previous = layout.steps;
      if (order[order.length - 1] !== state(layout)) order.push(state(layout));
    }
    expect(order).toEqual(['labels', 'mode-icon', 'mode-menu', 'hidden']);
    // Widening back needs a point of slack over each threshold, then restores in reverse order.
    const back: string[] = [];
    for (let host = 1; host <= 600; host += 1) {
      const layout = at(host, previous); previous = layout.steps;
      if (back[back.length - 1] !== state(layout)) back.push(state(layout));
    }
    expect(back).toEqual(['hidden', 'mode-menu', 'mode-icon', 'labels']);
  });
  test('an unmeasured host keeps every control', () => {
    expect(footerLayout({ ...base, host: 0 })).toMatchObject({ measuredHidden: 0, controlsHidden: false });
  });
});

describe('overflowMenu (CompactComposerControlsMenu)', () => {
  const label = (item: { kind: string; label: string; selected: boolean }) => item.kind === 'option' ? `${item.label}${item.selected ? '*' : ''}` : item.kind === 'header' ? `[${item.label}]` : '-';
  test('mode hidden: Mode (Chat, Plan) when Plan UI is on, then a divider before Permissions while they live here', () => {
    const menu = overflowMenu({ modeHidden: true, planVisible: true, interactionMode: 'plan', showPermissions: true });
    expect(menu.items.map(label)).toEqual(['[Mode]', 'Chat', 'Plan*', '-']);
    expect(menu.count).toBe(2);
    expect(overflowMenu({ modeHidden: true, planVisible: true, interactionMode: 'plan', showPermissions: false }).items.map(label))
      .toEqual(['[Mode]', 'Chat', 'Plan*']);
  });
  test('without Plan UI there is no Mode group: the menu is Permissions alone', () => {
    expect(overflowMenu({ modeHidden: true, planVisible: false, interactionMode: 'default', showPermissions: true })).toEqual({ items: [], count: 0 });
  });
  test('nothing hidden: no menu', () => {
    expect(overflowMenu({ modeHidden: false, planVisible: true, interactionMode: 'default', showPermissions: false })).toEqual({ items: [], count: 0 });
  });
});
