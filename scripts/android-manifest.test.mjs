// The Android manifest fragment's allow-list (LLP 1133.030 D9): what a package may add, and each refusal by name.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fragmentPermissions, manifestFragment } from './android-manifest.mjs';
import { androidImplementations } from '../js/bake/src/typescript.mjs';

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

test('speech action queries and package queries share one element and deduplicate across implementations', () => {
  const queries = [
    { action: 'android.speech.RecognitionService' },
    { package: 'com.google.android.apps.healthdata' },
    { action: 'android.intent.action.TTS_SERVICE' },
  ];
  const xml = manifestFragment([...impl({ queries: [...queries, ...queries] }), ...impl({ queries })]);
  assert.equal(xml, [
    '    <queries>',
    '        <intent><action android:name="android.speech.RecognitionService" /></intent>',
    '        <package android:name="com.google.android.apps.healthdata" />',
    '        <intent><action android:name="android.intent.action.TTS_SERVICE" /></intent>',
    '    </queries>',
  ].join('\n'));
});

test('an unlisted query action is refused with the package and action named', () => {
  assert.throws(() => manifestFragment([{
    extension: 'speech-recognition', className: 'exact.speechrecognition.SpeechRecognitionModule',
    manifest: { queries: [{ action: 'android.intent.action.MAIN' }] },
  }]), (e) => e.message.includes("speech-recognition's Android implementation") &&
    e.message.includes('android.intent.action.MAIN') && e.message.includes('not on the list'));
});

test('a query names exactly one package or action', () => {
  for (const query of [{}, null, { package: 'com.google.android.apps.healthdata', action: 'android.intent.action.TTS_SERVICE' }]) {
    assert.throws(() => manifestFragment(impl({ queries: [query] })),
      (e) => e.message.includes("alarms's Android implementation") && e.message.includes('exactly one of package or action'));
  }
});

test('androidImplementations carries the bound speech packages through manifest fragments', () => {
  const app = mkdtempSync(resolve(tmpdir(), 'exact-speech-manifest-'));
  const packages = ['speech-recognition', 'speech-synthesis'];
  try {
    mkdirSync(resolve(app, 'node_modules/@exact'), { recursive: true });
    for (const name of packages) symlinkSync(resolve(import.meta.dir, '../packages', name), resolve(app, 'node_modules/@exact', name), 'dir');
    writeFileSync(resolve(app, 'app.ts'), 'import { startSpeechRecognition } from "@exact/speech-recognition";\nimport { speak } from "@exact/speech-synthesis";\n');
    const bound = androidImplementations(app).sort((a, b) => a.extension.localeCompare(b.extension));
    assert.deepEqual(bound.map((b) => b.extension), packages);
    assert.deepEqual(bound.map((b) => b.manifest), [
      { queries: [{ action: 'android.speech.RecognitionService' }] },
      { queries: [{ action: 'android.intent.action.TTS_SERVICE' }] },
    ]);
    assert.ok(bound.every((b) => !Object.hasOwn(b, 'queries')));
    assert.equal(manifestFragment(bound), [
      '    <queries>',
      '        <intent><action android:name="android.speech.RecognitionService" /></intent>',
      '        <intent><action android:name="android.intent.action.TTS_SERVICE" /></intent>',
      '    </queries>',
    ].join('\n'));
    writeFileSync(resolve(app, 'app.ts'), 'export const unused = true;\n');
    assert.deepEqual(androidImplementations(app), []);
  } finally { rmSync(app, { recursive: true, force: true }); }
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

test("device's normal permission is declared, and a runtime one refused", () => {
  const json = JSON.parse(readFileSync(resolve(import.meta.dir, '../packages/device/android/exact.json'), 'utf8'));
  const bound = [{ extension: 'device', className: 'exact.device.DeviceModule', manifest: json.manifest }];
  assert.deepEqual(fragmentPermissions(bound), ['android.permission.ACCESS_NETWORK_STATE']);
  assert.equal(manifestFragment(bound), '', 'a permission adds no component');
  assert.deepEqual(fragmentPermissions(impl(undefined)), []);
  assert.throws(() => fragmentPermissions(impl({ permissions: ['android.permission.ACCESS_FINE_LOCATION'] })), (e) => e.message.includes('ACCESS_FINE_LOCATION') && e.message.includes('normal permissions only'));
});

test('a query with a data scheme: SENDTO mailto: and smsto: only (LLP 1133.024 D22 item 6)', () => {
  const xml = manifestFragment(impl({ queries: [{ action: 'android.intent.action.SENDTO', scheme: 'mailto' }, { action: 'android.intent.action.SENDTO', scheme: 'smsto' }] }));
  assert.match(xml, /<intent><action android:name="android\.intent\.action\.SENDTO" \/><data android:scheme="mailto" \/><\/intent>/);
  assert.match(xml, /<data android:scheme="smsto" \/>/);
  assert.throws(() => manifestFragment(impl({ queries: [{ action: 'android.intent.action.SENDTO', scheme: 'https' }] })), /not on the list/);
  assert.throws(() => manifestFragment(impl({ queries: [{ action: 'android.intent.action.VIEW', scheme: 'mailto' }] })), /not on the list/);
});

test("payments' callbacks service is exported only to Play services' binder, with Google Pay's switch", () => {
  const json = JSON.parse(readFileSync(resolve(import.meta.dir, '../packages/payments/android/exact.json'), 'utf8'));
  const payments = (manifest) => manifestFragment([{ extension: 'payments', className: 'exact.payments.PaymentsModule', manifest }]);
  const xml = payments(json.manifest);
  assert.match(xml, /<service android:name="exact\.payments\.PaymentCallbacks" android:exported="true" android:permission="com\.google\.android\.gms\.permission\.BIND_PAYMENTS_CALLBACK_SERVICE">/);
  assert.match(xml, /<action android:name="com\.google\.android\.gms\.wallet\.callback\.PAYMENT_DATA_CALLBACKS" \/>/);
  assert.match(xml, /<meta-data android:name="com\.google\.android\.gms\.wallet\.api\.enabled" android:value="true" \/>/);
  const refuses = (manifest, what) => assert.throws(() => payments(manifest), (e) => e.message.includes(what));
  refuses({ services: [{ name: 'exact.payments.S', permission: 'android.permission.BIND_JOB_SERVICE', actions: ['x'] }] }, 'BIND_JOB_SERVICE');
  refuses({ services: [{ name: 'exact.payments.S', permission: 'com.google.android.gms.permission.BIND_PAYMENTS_CALLBACK_SERVICE', actions: ['android.intent.action.MAIN'] }] }, 'does not bind');
  refuses({ metadata: [{ name: 'com.google.android.geo.API_KEY', value: 'k' }] }, 'API_KEY');
});
