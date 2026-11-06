// ThreadAutomationsPanel (T3 Code 74541bab17): hovering a task's truncated title and schedule shows both in full in a
// tooltip on the left, which the window's hover layer draws as a medium title over the muted schedule.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';

const fns = contractFns('src/shell/hover-layer.contract', ['HoverTip']) as unknown as { hoverTipAtFrame: (...args: unknown[]) => Env };
const summary = contractComponent('src/details/thread-automations.contract', 'AutomationSummary');

test('the summary is the trigger of a tooltip holding the full title and schedule', () => {
  const calls: unknown[][] = [];
  const task = { id: 'task-1', title: 'Nightly dependency audit across every workspace package', line: 'Every day at 09:00 · next in 3h' };
  const geometry = { x: 900, y: 300, width: 180, height: 34, provisional: false, unavailable: false };
  const scope: Env = { ...fns, task, testKey: `details-automation-summary-${task.id}`, rem: 16, frame: () => geometry, hoverTipAt: (...args: unknown[]) => calls.push(args) };
  summary.call('hover', {}, scope, [true]);
  summary.call('hover', {}, scope, [false]);
  expect(calls.map(([tip, role, on]) => [(tip as Env).kind, (tip as Env).text, (tip as Env).lines, (tip as Env).side, role, on])).toEqual([
    ['tooltip', task.title, [task.line], 'left', 'trigger', true],
    ['tooltip', task.title, [task.line], 'left', 'trigger', false],
  ]);
  expect((calls[0]![0] as Env).testId).toBe('details-automation-summary-task-1-tooltip');
});
