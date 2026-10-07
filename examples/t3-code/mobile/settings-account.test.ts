import { expect, test } from 'bun:test';
import { mobileAccountRouteEntry, mobileAccountSettings } from './settings-account';
import { mobileNotificationsSettings } from './settings-notifications';

test('unconfigured direct auth and source waitlist links replace Settings rather than remaining on an auth page', () => {
  for (const route of ['settingsAuth', 'settingsWaitlist']) {
    for (const hasHistory of [true, false]) expect(mobileAccountRouteEntry(route, hasHistory)).toEqual({kind: 'replace', route: 'settings'});
  }
});
test('unconfigured onboarding dismisses when possible and falls back to Home on a cold link', () => {
  expect(mobileAccountRouteEntry('connectOnboarding', true)).toEqual({kind: 'back'});
  expect(mobileAccountRouteEntry('connectOnboarding', false)).toEqual({kind: 'replace', route: 'home'});
});
test('ordinary settings and notification links keep their route; configuration remains absent', () => {
  for (const route of ['home', 'settings', 'settingsNotifications', 'settingsProviderAccounts', '']) expect(mobileAccountRouteEntry(route, true)).toBeNull();
  expect(mobileAccountSettings()).toEqual({cloudConfigured: false});
  expect(mobileNotificationsSettings()).toEqual({title: 'Notifications', detail: 'Notifications require T3 Connect in this app build.'});
});
