import { expect, test } from 'bun:test';
import { appText } from '../tests/support/app-files';

test('scroll-facing snapshots use sampled clocks instead of a frame clock', () => {
  const app = appText('mobile/app.contract');
  for (const resource of [
    'resource data = snapshot(',
    'resource sidebarHome = homeView(',
    'resource home = homeView(',
    'resource threadData = threadView(',
    'resource gitData = gitSnapshot(',
    'resource archiveData = archiveView(',
  ]) {
    const start = app.indexOf(resource);
    expect(start).toBeGreaterThanOrEqual(0);
    const end = app.indexOf('\n', start);
    expect(app.slice(start, end)).not.toContain('performanceNow()');
  }
  expect(app).toContain('resource data = snapshot(appearance.scheme, appearance.themeId, snapshotRead.serial, wallTime.epochAtZero + elapsed)');
  expect(app).toContain('resource home = homeView(data.revision, wallTime.epochAtZero + elapsed');
  expect(app).toContain('resource threadData = threadView(data.revision, wallTime.epochAtZero + max(elapsed, agentsElapsed)');
  expect(app).toContain('wallTime.epochAtZero + performanceNow()');
});

test('feed scrollports measure authored padding and avoid animating stream corrections', () => {
  const home = appText('mobile/home.contract'), thread = appText('mobile/thread.contract');
  expect(home).toContain('list id=listId appearance="none" flex=1 min-height=0 box-sizing="border-box"');
  expect(home).toContain('min-width=0\n        virtualized=true\n        estimated-item-height=88');
  expect(home).toContain('box position="relative" min-width=0 overflow="hidden"');
  expect(thread).toContain('column flex=1 min-height=0 min-width=0 justify-content="flex-end"');
  expect(thread).toContain('min-width=0 box-sizing="border-box" overscroll-behavior="contain" virtualized=true');
  expect(thread).not.toContain('scroll-behavior="smooth"');
});
