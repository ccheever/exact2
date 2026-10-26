// The Android manifest fragment's allow-list (LLP 1133.030 D9): what a package may add, and each refusal by name.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { manifestFragment } from './android-manifest.mjs';

const impl = (manifest) => [{ extension: 'alarms', className: 'exact.alarms.AlarmsModule', manifest }];

test("alarms' fragment is allowed, unexported, and nothing beyond what it names", () => {
  const json = JSON.parse(readFileSync(resolve(import.meta.dir, '../packages/alarms/android/exact.json'), 'utf8'));
  const xml = manifestFragment(impl(json.manifest));
  assert.equal((xml.match(/android:exported="false"/g) ?? []).length, 3);
  assert.doesNotMatch(xml, /exported="true"|<provider|<uses-permission|android:process|<activity/);
  assert.match(xml, /<service android:name="exact\.alarms\.AlarmService" android:exported="false" android:directBootAware="true" android:foregroundServiceType="systemExempted" \/>/);
  assert.match(xml, /<action android:name="android\.intent\.action\.LOCKED_BOOT_COMPLETED" \/>/);
  // The alarm's own receiver has no filter: its intent is explicit.
  assert.match(xml, /<receiver android:name="exact\.alarms\.AlarmReceiver" android:exported="false" android:directBootAware="true" \/>/);
});

test('an implementation with no fragment adds nothing', () => {
  assert.equal(manifestFragment(impl(undefined)), '');
  assert.equal(manifestFragment([]), '');
});

test('the allow-list refuses, naming what', () => {
  const refuses = (manifest, what) => assert.throws(() => manifestFragment(impl(manifest)), (e) => e.message.includes(what) && e.message.includes("alarms's Android implementation"));
  refuses({ providers: [] }, 'names "providers"');
  refuses({ activities: [{ name: 'exact.alarms.A', actions: ['android.intent.action.MAIN'] }] }, 'android.intent.action.MAIN');
  refuses({ activities: [{ name: 'exact.alarms.A' }] }, 'no actions');
  refuses({ activities: [{ name: 'exact.alarms.A', actions: ['androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE'], exported: false }] }, '"exported"');
  refuses({ queries: [{ package: 'com.android.chrome' }] }, 'com.android.chrome');
  refuses({ aliases: [{ name: 'exact.alarms.B', targetActivity: 'exact.alarms.A', action: 'android.intent.action.VIEW_PERMISSION_USAGE', category: 'android.intent.category.HEALTH_PERMISSIONS' }] }, 'not an activity it declares');
  refuses({ receivers: [{ name: 'exact.alarms.R', actions: ['android.intent.action.PACKAGE_ADDED'] }] }, 'PACKAGE_ADDED');
  refuses({ receivers: [{ name: 'exact.alarms.R', exported: true }] }, '"exported"');
  refuses({ receivers: [{ name: 'exact.alarms.R', permission: 'x' }] }, '"permission"');
  refuses({ services: [{ name: 'exact.alarms.S', foregroundServiceType: 'location' }] }, '"location"');
  refuses({ services: [{ name: 'exact.alarms.S', process: ':ring' }] }, '"process"');
  refuses({ services: [{ name: 'dev.exact.canvas.Other' }] }, 'not a class in exact.alarms');
  refuses({ services: [{ name: 'exact.alarms.S"/><provider' }] }, 'not a class in exact.alarms');
  refuses({ services: [{ name: 'exact.alarms.S' }, { name: 'exact.alarms.S' }] }, 'twice');
});

test("health's fragment: Health Connect queried, the rationale and the usage alias exported for the system alone", () => {
  const json = JSON.parse(readFileSync(resolve(import.meta.dir, '../packages/health/android/exact.json'), 'utf8'));
  const health = [{ extension: 'health', className: 'exact.health.HealthModule', manifest: json.manifest }];
  const xml = manifestFragment(health, { privacyPolicy: 'https://example.com/privacy?a=1&b=2' });
  assert.match(xml, /^ {4}<queries>\n {8}<package android:name="com\.google\.android\.apps\.healthdata" \/>\n {4}<\/queries>\n {4}<application>/);
  assert.match(xml, /<meta-data android:name="exact\.privacy_policy" android:value="https:\/\/example\.com\/privacy\?a=1&amp;b=2" \/>/);
  assert.match(xml, /<activity android:name="exact\.health\.PrivacyPolicyActivity" android:exported="true" android:theme="@android:style\/Theme\.NoDisplay">/);
  assert.match(xml, /<activity-alias android:name="exact\.health\.ViewPermissionUsageActivity" android:exported="true" android:targetActivity="exact\.health\.PrivacyPolicyActivity" android:permission="android\.permission\.START_VIEW_PERMISSION_USAGE">/);
  assert.match(xml, /<category android:name="android\.intent\.category\.HEALTH_PERMISSIONS" \/>/);
  assert.doesNotMatch(xml, /<provider|<uses-permission|android:process/);
  // No activity, no policy carried.
  assert.doesNotMatch(manifestFragment([{ ...health[0], manifest: { queries: json.manifest.queries } }], { privacyPolicy: 'https://example.com/privacy' }), /meta-data|<application/);
});
