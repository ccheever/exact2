// composer-fidelity G11: the footer's overflow steps and the "More composer
// controls" rows (CompactComposerControlsMenu.tsx, T3 Code 1e2ecbd975, MIT; see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { footerLayout } from './composer-controls-view';
import { overflowMenu } from './composer-overflow';
import { traitsMenu } from './composer-presentation';

const base = { model: 'Claude Opus 4.6', traits: 'High', traitsIcon: false, runtime: 'Full access', plan: 'Build' };
const effort = { id: 'effort', label: 'Reasoning', type: 'select', options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High', isDefault: true }] };

describe('footerLayout overflow (G11)', () => {
  test('labels go first, then mode, then traits move into the menu, then the cluster hides; the same thresholds both ways', () => {
    const at = (host: number, previous?: ReturnType<typeof footerLayout>['steps']) => footerLayout({ ...base, host, previous });
    const order: string[] = [];
    let previous: ReturnType<typeof footerLayout>['steps'] | undefined;
    for (let host = 600; host >= 1; host -= 1) {
      const layout = at(host, previous); previous = layout.steps;
      const state = layout.controlsHidden ? 'hidden' : layout.traitsOverflow ? 'traits-menu' : layout.modeOverflow ? 'mode-menu' : layout.traitsIconOnly ? 'traits-icon' : layout.runtimeIconOnly ? 'mode-icon' : 'labels';
      if (order[order.length - 1] !== state) order.push(state);
    }
    expect(order).toEqual(['labels', 'mode-icon', 'traits-icon', 'mode-menu', 'traits-menu', 'hidden']);
    // Widening back needs a point of slack over each threshold, then restores in reverse order.
    const back: string[] = [];
    for (let host = 1; host <= 600; host += 1) {
      const layout = at(host, previous); previous = layout.steps;
      const state = layout.controlsHidden ? 'hidden' : layout.traitsOverflow ? 'traits-menu' : layout.modeOverflow ? 'mode-menu' : layout.traitsIconOnly ? 'traits-icon' : layout.runtimeIconOnly ? 'mode-icon' : 'labels';
      if (back[back.length - 1] !== state) back.push(state);
    }
    expect(back).toEqual(['hidden', 'traits-menu', 'mode-menu', 'traits-icon', 'mode-icon', 'labels']);
  });
  test('an unmeasured host keeps every control', () => {
    expect(footerLayout({ ...base, host: 0 })).toMatchObject({ modeOverflow: false, traitsOverflow: false, controlsHidden: false });
  });
});

describe('overflowMenu (CompactComposerControlsMenu)', () => {
  const runtimes = [{ mode: 'approval-required', label: 'Supervised', selected: false }, { mode: 'full-access', label: 'Full access', selected: true }];
  const traits = traitsMenu([effort], []).items;
  test('mode hidden: Mode (Chat, Plan) when Plan UI is on, then Access', () => {
    const menu = overflowMenu({ traits, traitsHidden: false, modeHidden: true, planVisible: true, interactionMode: 'plan', runtimes });
    expect(menu.items.map(item => item.kind === 'option' ? `${item.label}${item.selected ? '*' : ''}` : item.kind === 'header' ? `[${item.label}]` : '-'))
      .toEqual(['[Mode]', 'Chat', 'Plan*', '-', '[Access]', 'Supervised', 'Full access*']);
    expect(menu.count).toBe(4);
  });
  test('traits hidden too: the traits content leads; without Plan UI there is no Mode group', () => {
    const menu = overflowMenu({ traits, traitsHidden: true, modeHidden: true, planVisible: false, interactionMode: 'default', runtimes });
    expect(menu.items.map(item => item.kind === 'option' ? item.label : item.kind === 'header' ? `[${item.label}]` : '-'))
      .toEqual(['[Reasoning]', 'Medium', 'High', '-', '[Access]', 'Supervised', 'Full access']);
    expect(menu.items.filter(item => item.kind === 'option').map(item => item.index)).toEqual([0, 1, 2, 3]);
    expect(menu.items.find(item => item.label === 'Supervised')?.descriptor).toBe('__runtime');
  });
  test('nothing hidden: no menu', () => {
    expect(overflowMenu({ traits, traitsHidden: false, modeHidden: false, planVisible: true, interactionMode: 'default', runtimes })).toEqual({ items: [], count: 0 });
  });
});
