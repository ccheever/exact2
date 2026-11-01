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
  const home = appText('mobile/home.contract'), thread = appText('mobile/thread.contract'), work = appText('mobile/thread-work.contract');
  expect(home).toContain('list id=listId appearance="none" flex=1 min-height=0 box-sizing="border-box"');
  expect(home).toContain('min-width=0\n        virtualized=true\n        estimated-item-height=88');
  expect(home).toContain('box position="relative" min-width=0 overflow="hidden"');
  expect(home).toContain('button appearance="none" press=selectThread(item.id)');
  expect(home).toContain('keyboardDismissMode="on-drag" scrollbar-width="none" touch-action="pan-y"');
  expect(thread).toContain('column flex=1 min-height=0 min-width=0 justify-content="flex-end"');
  expect(thread).toContain('min-width=0 box-sizing="border-box" overscroll-behavior="contain" touch-action="pan-y" virtualized=true');
  expect(thread).not.toContain('scroll-behavior="smooth"');
  expect(work).toContain('button appearance="none" press=toggleDetail');
  expect(work).toContain('scroll max-height=210 min-width=0 box-sizing="border-box" overflow-y="auto" overscroll-behavior="contain" scrollbar-width="none" touch-action="pan-y"');
  const markdown = appText('mobile/thread-markdown.contract');
  expect(markdown).toContain('button appearance="none" press=action("chatlocal:copy-code"');
  expect(markdown).toContain('view overflow-x="auto" overflow-y="hidden" overscroll-behavior="contain" scrollbar-width="none" touch-action="pan-x" min-width=0 max-width="100%" box-sizing="border-box"');
  expect(markdown).toContain('view display="flex" align-items="flex-start" min-width=0 max-width="100%" overflow-x="auto" overflow-y="hidden" overscroll-behavior="contain" scrollbar-width="none" touch-action="pan-x" box-sizing="border-box"');
  expect(home).toContain('view height=1 min-width=0 box-sizing="border-box" margin-left=17.5 margin-right=17.5');
  expect(home).toContain('view flex=1 min-width=0 height=1 box-sizing="border-box" background-color=colors.border');
  expect(home).toContain('if item.card and item.trailingDivider and not item.last and not sidebar');
  expect(home).toContain('if item.trailingDivider and not item.last and surface != "sidebar-list"');
  expect(thread).toContain('button appearance="none" press=action("fork-response", row.id, row.forkKey)');
});
