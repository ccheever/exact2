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
