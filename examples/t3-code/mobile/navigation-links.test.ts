import { expect, test } from 'bun:test';
import { mobileAppLink } from './navigation-links';

test('wake deliveries preserve a warm stack and cold private URLs open Home', () => {
  for (const url of ['/', '//', '///', '/expo-sharing', '/expo-sharing?payload=x', '/expo-development-client/?url=x']) {
    expect(mobileAppLink(url, 'visit').ignore).toBe(true);
    expect(mobileAppLink(url, 'visit', true).location).toBe('/');
  }
  expect(mobileAppLink('/expo-sharing-other', 'visit').ignore).toBe(false);
});
test('public settings aliases retain query and the originating visit', () => {
  expect(mobileAppLink('/settings/keyboard?environmentId=server', '4')).toEqual({ location: '/settings/preferences/SettingsKeyboard?environmentId=server', ignore: false, requestRoute: '4' });
  expect(mobileAppLink('/settings/scheduled-tasks/new', '4').location).toBe('/settings/scheduled/new');
  expect(mobileAppLink('/settings/source-control', '4').location).toBe('/settings/server/SettingsEnvironmentSourceControl');
});
test('nested file links preserve encoded IDs and the whole file path', () => {
  expect(mobileAppLink('/threads/a%2Fb/c/files/src/a%20b.ts?view=source', '1').location).toBe('/threads/a%2Fb/c/files/src%2Fa%20b.ts?view=source');
  expect(mobileAppLink('/threads/a/b/files/%E0%A4%A', '1').location).toBe('/unmatched-link');
});
test('tap-time payload routes and malformed locations do not enter internal screens', () => {
  for (const url of ['/settings/usage/account/a/b/c', '/settings/scheduled/edit', '/threads/a/b/model', '/settings/server/SettingsEnvironmentMaintenance', '/new/model', '/threads/a/b/terminal/output', '//evil/threads/a/b', 'https://example.com', '/threads/a\\b/c']) expect(mobileAppLink(url, '1').location).toBe('/unmatched-link');
  expect(mobileAppLink('/threads/a/b/terminal', '1').location).toBe('/threads/a/b/terminal');
});

test('public review comment path maps to its mobile route', () => {
  expect(mobileAppLink('/threads/a/b/review-comment', '7').location).toBe('/threads/a/b/review/comment');
});

test('public links tolerate a trailing slash and reject malformed encoded segments', () => {
  expect(mobileAppLink('/settings/keyboard/', '1').location).toBe('/settings/preferences/SettingsKeyboard');
  expect(mobileAppLink('/threads/%ZZ/b', '1').location).toBe('/unmatched-link');
});

test('new-task public routes preserve explicit project and branch context', () => {
  const location = '/new/draft?environmentId=env%2Fone&projectId=project&branch=fix%2Fissue';
  expect(mobileAppLink(location, 'visit').location).toBe(location);
  expect(mobileAppLink('/new/draft/environment', 'visit').location).toBe('/new/draft/environment');
  expect(mobileAppLink('/new/draft/branch', 'visit').location).toBe('/new/draft/branch');
  expect(mobileAppLink('/new/draft/settings', 'visit').location).toBe('/new/draft/settings');
  expect(mobileAppLink('/new/projects', 'visit').location).toBe('/unmatched-link');
});
