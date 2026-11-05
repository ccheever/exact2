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
  const app = appText('mobile/app.contract'), home = appText('mobile/home.contract'), thread = appText('mobile/thread.contract'), work = appText('mobile/thread-work.contract'), route = appText('mobile/route-content.contract'), pending = appText('mobile/pending-input.contract');
  expect(home).toContain('list id=listId appearance="none" flex=1 min-height=0 box-sizing="border-box"');
  expect(home).toContain('min-width=0\n        virtualized=true\n        estimated-item-height=88');
  expect(home).toContain('box position="relative" min-width=0 overflow="hidden"');
  expect(home).toContain('button appearance="none" press=selectThread(item.id)');
  expect(home).toContain('keyboardDismissMode="on-drag" scrollbar-width="none" touch-action="pan-y"');
  expect(thread).toContain('column flex=1 min-height=0 min-width=0 justify-content="flex-end"');
  expect(thread).toContain('min-width=0 box-sizing="border-box" overflow-x="hidden" overscroll-behavior="contain" keyboardDismissMode="none" touch-action="pan-y" virtualized=true');
  expect(thread).toContain('scroll-start="end" scrollFollowEnd=endFollowEnabled scroll=historyScroll');
  expect(thread).not.toContain('scroll-start="end" scrollFollowEnd=true scroll=historyScroll');
  expect(route).toContain('derive threadScrollToEndVisible = not threadEndFollowEnabled and threadData.loaded and not threadData.loading and threadData.error == "" and threadData.rows.length > 0');
  expect(route).toContain('(workingControl.visible or threadScrollToEndVisible) ? 48.5 : 0');
  expect(route).toContain('derive workingControlLift = (threadData.pendingInput.active and pendingInputCoverageOwner == threadData.pendingInput.cardId) ? pendingInputCoverage : 0');
  expect(route).toContain('lift=workingControlLift');
  expect(route).toContain('reducedMotion=viewport.prefersReducedMotion, act=pendingInputAct');
  expect(pending).toContain('reducedMotion: bool');
  expect(pending).toContain('transition=(reducedMotion ? "none" : "translate 220ms ease-out")');
  expect(thread).not.toContain('scroll-behavior="smooth"');
  expect(work).toContain('button appearance="none" press=toggleDetail');
  expect(work).toContain('row align-items="center" gap=3.5 width="100%" min-width=0');
  expect(work).toContain('button appearance="none" press=action("open-agent", activity.targetId, "") flex-shrink=0');
  expect(work).toContain('for picture in activity.toolImages key=picture.id');
  expect(work).toContain('button appearance="none" press=openMedia(picture.id)');
  expect(work).toContain('scroll max-height=210 min-width=0 box-sizing="border-box" overflow-y="auto" overscroll-behavior="contain" scrollbar-width="none" touch-action="pan-y"');
  const markdown = appText('mobile/thread-markdown.contract');
  expect(markdown).toContain('button appearance="none" press=action("chatlocal:copy-code"');
  expect(markdown).toContain('view overflow-x="auto" overflow-y="hidden" overscroll-behavior="contain" scrollbar-width="none" min-width=0 max-width="100%" box-sizing="border-box"');
  expect(markdown).toContain('view display="flex" align-items="flex-start" min-width=0 max-width="100%" overflow-x="auto" overflow-y="hidden" overscroll-behavior="contain" scrollbar-width="none" box-sizing="border-box"');
  expect(markdown).not.toContain('overflow-x="auto" overflow-y="hidden" overscroll-behavior="contain" scrollbar-width="none" touch-action="pan-x"');
  expect(home).toContain('view height=1 min-width=0 box-sizing="border-box" margin-left=17.5 margin-right=17.5');
  expect(home).toContain('view flex=1 min-width=0 height=1 box-sizing="border-box" background-color=colors.border');
  expect(home).toContain('if item.card and item.trailingDivider and not item.last and not sidebar');
  expect(home).toContain('if item.trailingDivider and not item.last and surface != "sidebar-list"');
  expect(thread).toContain('button appearance="none" press=action("fork-response", row.id, row.forkKey)');
  expect(app).toContain('width=(mobileRoutePresentation(entry.name, `${entry.id}`, newTaskSession, workspace.usesSplitView) == "fullscreen" ? viewport.width : workspace.contentSettledWidth)');
  expect(app).toContain('max-width=(mobileRoutePresentation(entry.name, `${entry.id}`, newTaskSession, workspace.usesSplitView) == "fullscreen" ? viewport.width : "100%")');
  expect(app).toContain('overflow-x="hidden" overflow-y="hidden" background-color=look.sheet');
});

test('Home keeps menus outside the virtualized feed at one bounded resident pair', () => {
  const home = appText('mobile/home.contract'), thread = appText('mobile/thread.contract');
  const menuHost = home.slice(home.indexOf('        virtualized=true'), home.indexOf('\ncomponent HomeListItem'));
  expect((menuHost.match(/HomeMenu\(/g) ?? []).length).toBe(1);
  expect((menuHost.match(/HomeSwipeMenu\(/g) ?? []).length).toBe(1);
  expect(menuHost).toContain('HomeMenu(menuId=contextMenuId, items=selectedMenuItems');
  expect(menuHost).toContain('HomeSwipeMenu(token=swipeMenuToken, menuId=swipeMenuId, items=selectedSwipeItems');
  expect(home).toContain('derive contextMenuId = `home-menu-${listId}`');
  expect(home).toContain('derive swipeMenuId = `home-snooze-${listId}`');
  expect(home).toContain('column contextmenu=selectMenu(item.key, item.environmentId, item.threadId) contextPopover=(item.menuItems.length > 0 ? sharedMenuId : "")');
  const rowBody = home.slice(home.indexOf('component HomeListItem'), home.indexOf('\ncomponent HomeContextListItem'));
  expect(rowBody).not.toContain('contextmenu=');
  expect(home).toContain('data-mobile-home-menu-id=(surface == "sidebar-list" ? menuId : "")');
  expect(home).toContain('popoverMenuId=sharedSwipeMenuId');
  expect(thread).toContain('virtualized=true estimated-item-height=180');
});

test('Home keeps its native scroll owner through filtered empty results', () => {
  const app = appText('mobile/app.contract');
  const home = appText('mobile/home.contract');
  expect(app).toContain('entry.name == "home" ? "home-list"');

  const filteredStart = home.indexOf('else if items.length == 0 and not fullPageEmpty');
  const fullPageStart = home.indexOf('else if items.length == 0\n', filteredStart + 1);
  const nextBranch = home.indexOf('\n    else\n', fullPageStart);
  expect(filteredStart).toBeGreaterThanOrEqual(0);
  expect(fullPageStart).toBeGreaterThan(filteredStart);
  expect(nextBranch).toBeGreaterThan(fullPageStart);
  expect(home.slice(filteredStart, fullPageStart)).toContain('scroll id=listId');
  expect(home.slice(fullPageStart, nextBranch)).not.toContain('scroll id=listId');
});

test('New Task virtualizes project rows while empty chooser content has no stale scroll owner', () => {
  const app = appText('mobile/app.contract'), task = appText('mobile/new-task.contract');
  expect(task).toContain('list id="new-task-projects-scroll" virtualized=true estimated-item-height=49');
  expect(task).toContain('for project in data.projects key=project.id');
  expect(app).toContain('entry.name == "newTask" ? "new-task-projects-scroll"');
});
