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
  refuses({ activities: [] }, 'names "activities"');
  refuses({ receivers: [{ name: 'exact.alarms.R', actions: ['android.intent.action.PACKAGE_ADDED'] }] }, 'PACKAGE_ADDED');
  refuses({ receivers: [{ name: 'exact.alarms.R', exported: true }] }, '"exported"');
  refuses({ receivers: [{ name: 'exact.alarms.R', permission: 'x' }] }, '"permission"');
  refuses({ services: [{ name: 'exact.alarms.S', foregroundServiceType: 'location' }] }, '"location"');
  refuses({ services: [{ name: 'exact.alarms.S', process: ':ring' }] }, '"process"');
  refuses({ services: [{ name: 'dev.exact.canvas.Other' }] }, 'not a class in exact.alarms');
  refuses({ services: [{ name: 'exact.alarms.S"/><provider' }] }, 'not a class in exact.alarms');
  refuses({ services: [{ name: 'exact.alarms.S' }, { name: 'exact.alarms.S' }] }, 'twice');
});
