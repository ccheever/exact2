// An Android implementation's manifest fragment (LLP 1133.030 D9, D12 item 4; LLP 1133.026 D19 item 3;
// LLP 1133.012 D12 item 3): the components and queries a package declares in its `exact.json` `"manifest"`, checked
// against an allow-list and written into the APK's generated manifest. Every package gets this seam (LLP 1133.001
// D8), so the list bounds what any package runs with no session and which packages and services it can see: what it
// names, and nothing the XML could say besides, is all a fragment can add. A package that needs another action or
// type amends the list in its RFC.
//
//   "manifest": {
//     "receivers": [{"name": "<class>", "directBootAware": true, "actions": ["android.intent.action.BOOT_COMPLETED"]}],
//     "services": [{"name": "<class>", "directBootAware": true, "foregroundServiceType": "systemExempted"}],
//     "queries": [{"package": "com.google.android.apps.healthdata"}, {"action": "android.intent.action.TTS_SERVICE"},
//                 {"action": "android.intent.action.SENDTO", "scheme": "mailto"}],
//     "activities": [{"name": "<class>", "actions": ["androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE"]}],
//     "aliases": [{"name": "<class>", "targetActivity": "<an activity above>", "action": "android.intent.action.VIEW_PERMISSION_USAGE",
//                  "category": "android.intent.category.HEALTH_PERMISSIONS"}],
//     "permissions": ["android.permission.ACCESS_NETWORK_STATE"],
//     "metadata": [{"name": "com.google.android.gms.wallet.api.enabled", "value": "true"}]
//   }
//
// Receivers and services are `exported="false"`, but for a service another app binds: the system's protected broadcasts still reach an unexported receiver,
// and a receiver for the package's own alarm needs no action, since its intent is explicit. An activity is exported,
// since the system starts it, but only for an action on the list, and never shows: it is `Theme.NoDisplay` and opens
// the app's privacy policy, which the application's `exact.privacy_policy` meta-data carries (Health Connect's
// rationale, [Health Connect: get started](https://developer.android.com/health-and-fitness/guides/health-connect/develop/get-started)).
// A service bound by another app through a permission only that app holds (`SERVICE_BINDS`: Google Pay's callbacks,
// LLP 1133.034 D5) is exported, since the binder is Play services' process; the permission is what guards it.
// An alias is guarded by `START_VIEW_PERMISSION_USAGE`, which only the system holds. A permission is only one whose
// protection level is `normal`, on `NORMAL_PERMISSIONS` (LLP 1133.031 D12 item 1), which the system grants at install
// and asks nothing for; a runtime permission comes only from a grant line, which derives its prompt (LLP 1133.002 D6).
// There is no `<provider>` and no `android:process`.

/** The broadcasts a package's receiver may name: the boot, the package replaced, the clock and zone, the exact-alarm
 * switch (LLP 1133.030 D9). */
export const RECEIVER_ACTIONS = new Set([
  'android.intent.action.BOOT_COMPLETED',
  'android.intent.action.LOCKED_BOOT_COMPLETED',
  'android.intent.action.TIME_SET',
  'android.intent.action.TIMEZONE_CHANGED',
  'android.intent.action.MY_PACKAGE_REPLACED',
  'android.app.action.SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED',
]);

/** The foreground service types a package's service may declare. */
export const SERVICE_TYPES = new Set(['systemExempted']);

/** The packages a fragment may make visible (`<queries>`): Health Connect's provider on Android 13 and earlier (LLP
 * 1133.026 D19 item 3). */
export const QUERY_PACKAGES = new Set(['com.google.android.apps.healthdata']);

/** The service intent actions a fragment may make visible (`<queries>`): speech recognition and synthesis on apps
 * targeting API 30 and later (LLP 1133.012 D12 item 3). */
export const QUERY_ACTIONS = new Set(['android.speech.RecognitionService', 'android.intent.action.TTS_SERVICE']);

/** The activity intents, by action and data scheme, a fragment may make visible: the mail and message composers'
 * (LLP 1133.024 D22 item 6), whether one resolves being `composeStatus`. */
export const QUERY_SCHEMES = { 'android.intent.action.SENDTO': new Set(['mailto', 'smsto']) };

/** The services another app binds: each binder's permission, and the actions its intent may name (LLP 1133.034 D5). */
export const SERVICE_BINDS = new Map([
  ['com.google.android.gms.permission.BIND_PAYMENTS_CALLBACK_SERVICE', new Set(['com.google.android.gms.wallet.callback.PAYMENT_DATA_CALLBACKS'])],
]);

/** The `<meta-data>` a package may declare, by name, with its one value (LLP 1133.034 D13: Google Pay's switch). */
export const METADATA = new Map([['com.google.android.gms.wallet.api.enabled', 'true']]);

/** The actions an activity may answer: Health Connect's request to show the app's privacy policy. */
export const ACTIVITY_ACTIONS = new Set(['androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE']);

/** An alias's action and category: Android 14's permission-usage entry for Health (LLP 1133.026 D19 item 3). */
export const ALIAS_ACTIONS = new Set(['android.intent.action.VIEW_PERMISSION_USAGE']);
export const ALIAS_CATEGORIES = new Set(['android.intent.category.HEALTH_PERMISSIONS']);

/** The normal permissions a fragment may declare (LLP 1133.031 D12 item 1): `protectionLevel="normal"` in the
 * platform's own manifest, granted at install with no prompt. */
export const NORMAL_PERMISSIONS = new Set(['android.permission.ACCESS_NETWORK_STATE']);

/** The normal permissions every bound implementation's fragment declares, which the build adds as `<uses-permission>`
 * beside the grants' (`-Ppermissions`). Throws, naming the package, for any permission not on the list. */
export function fragmentPermissions(bound) {
  const names = new Set();
  for (const impl of bound) {
    const list = impl.manifest?.permissions;
    if (list == null) continue;
    const refuse = (what) => { throw new Error(`${impl.extension}'s Android implementation: its manifest fragment ${what} (LLP 1133.031 D12 item 1: normal permissions only; a runtime permission comes from a grant line)`); };
    if (!Array.isArray(list)) refuse('has permissions that are not a list');
    for (const p of list) {
      if (!NORMAL_PERMISSIONS.has(p)) refuse(`asks for ${JSON.stringify(p)}, which is not on the list`);
      names.add(p);
    }
  }
  return [...names];
}

const FIELDS = {
  receivers: new Set(['name', 'directBootAware', 'actions']),
  services: new Set(['name', 'directBootAware', 'foregroundServiceType', 'permission', 'actions']),
  queries: new Set(['package', 'action', 'scheme']),
  activities: new Set(['name', 'actions']),
  aliases: new Set(['name', 'targetActivity', 'action', 'category']),
  metadata: new Set(['name', 'value']),
};

const ONE = { receivers: 'receiver', services: 'service', queries: 'query', activities: 'activity', aliases: 'alias', metadata: 'meta-data' };

const escape = (s) => s.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

/** The application meta-data a package's configuration may become (LLP 1133.021 §9 item 7, D18): by interface and
 * configuration key, the meta-data name. The Maps SDK reads its key from `com.google.android.geo.API_KEY`. */
export const CONFIG_META = { '@exact/maps': { apiKey: 'com.google.android.geo.API_KEY' } };

/** The `<meta-data>` lines `config` (`packageConfig`'s map, extension to values) carries under [CONFIG_META]. */
export function configMetaData(config) {
  const lines = [];
  for (const [name, keys] of Object.entries(CONFIG_META)) {
    const values = config.get(name.split('/').at(-1)) ?? {};
    for (const [key, meta] of Object.entries(keys)) if (typeof values[key] === 'string') lines.push(`        <meta-data android:name="${meta}" android:value="${escape(values[key])}" />`);
  }
  return lines;
}

/** `fragment` (from [manifestFragment]) with `meta` lines inside its `<application>`, made if it has none. */
export function withMetaData(fragment, meta) {
  if (!meta.length) return fragment;
  if (fragment.includes('    <application>\n')) return fragment.replace('    <application>\n', `    <application>\n${meta.join('\n')}\n`);
  return [fragment, '    <application>', ...meta, '    </application>'].filter(Boolean).join('\n');
}

/** The manifest's children for every bound implementation's fragment (a `<queries>` element and an `<application>`
 * one, each when it has entries), or '' when none declares one. Throws, naming the package and what the allow-list
 * refuses. `bound` is `androidImplementations`' list; `privacyPolicy` is the app's `privacy_policy`, carried as
 * meta-data when a fragment declares an activity. */
export function manifestFragment(bound, { privacyPolicy = null } = {}) {
  const lines = [], queries = new Set(), names = new Set();
  let activities = 0;
  for (const impl of bound) {
    const fragment = impl.manifest;
    if (fragment == null) continue;
    const who = `${impl.extension}'s Android implementation`, refuse = (what) => { throw new Error(`${who}: its manifest fragment ${what} (LLP 1133.030 D9's allow-list)`); };
    if (typeof fragment !== 'object' || Array.isArray(fragment)) refuse('is not an object');
    // A component is the package's own class: one in the Java package of its `class`.
    const own = impl.className.split('.').slice(0, -1).join('.'), mine = new Set();
    const component = (kind, c) => {
      if (typeof c.name !== 'string' || !/^[A-Za-z_]\w*(\.[A-Za-z_]\w*)+$/.test(c.name) || !c.name.startsWith(`${own}.`)) refuse(`names ${JSON.stringify(c.name)}, not a class in ${own}`);
      if (names.has(c.name)) refuse(`declares ${c.name} twice`);
      names.add(c.name);
    };
    const listed = (set, value, what) => { if (!set.has(value)) refuse(`gives ${what} ${JSON.stringify(value)}, which is not on the list`); };
    // Aliases name an activity, so activities come first whatever the object's order.
    const kinds = Object.keys(fragment).sort((a, b) => (a === 'aliases') - (b === 'aliases'));
    for (const kind of kinds) {
      const list = fragment[kind];
      if (kind === 'permissions') continue; // fragmentPermissions'
      if (!FIELDS[kind]) refuse(`names "${kind}"; only ${[...Object.keys(FIELDS), 'permissions'].join(', ')} are allowed`);
      if (!Array.isArray(list)) refuse(`has ${kind} that is not a list`);
      for (const c of list) {
        for (const key of Object.keys(c ?? {})) if (!FIELDS[kind].has(key)) refuse(`gives a ${ONE[kind]} "${key}", which the list does not allow`);
        if (kind === 'queries') {
          const keys = Object.keys(c ?? {}).sort().join();
          if (c == null || typeof c !== 'object' || Array.isArray(c) || !['package', 'action', 'action,scheme'].includes(keys)) refuse('gives a query without exactly one of package or action (with a scheme or not)');
          if (Object.hasOwn(c, 'package')) {
            listed(QUERY_PACKAGES, c.package, 'a query the package');
            queries.add(`        <package android:name="${c.package}" />`);
          } else if (Object.hasOwn(c, 'scheme')) {
            if (!QUERY_SCHEMES[c.action]?.has(c.scheme)) refuse(`gives a query the action ${JSON.stringify(c.action)} with the scheme ${JSON.stringify(c.scheme)}, which is not on the list`);
            queries.add(`        <intent><action android:name="${c.action}" /><data android:scheme="${c.scheme}" /></intent>`);
          } else {
            listed(QUERY_ACTIONS, c.action, 'a query the action');
            queries.add(`        <intent><action android:name="${c.action}" /></intent>`);
          }
          continue;
        }
        if (kind === 'metadata') {
          if (METADATA.get(c.name) !== c.value) refuse(`gives the meta-data ${JSON.stringify(c.name)} = ${JSON.stringify(c.value)}, which is not on the list`);
          if (names.has(c.name)) refuse(`declares ${c.name} twice`);
          names.add(c.name);
          lines.push(`        <meta-data android:name="${c.name}" android:value="${c.value}" />`);
          continue;
        }
        component(kind, c);
        if (kind === 'activities') {
          const actions = c.actions ?? [];
          if (!Array.isArray(actions) || !actions.length) refuse(`gives ${c.name} no actions; an activity is only for one the system starts`);
          for (const a of actions) listed(ACTIVITY_ACTIONS, a, `${c.name} the action`);
          mine.add(c.name);
          activities++;
          lines.push(`        <activity android:name="${c.name}" android:exported="true" android:theme="@android:style/Theme.NoDisplay">`, '            <intent-filter>',
            ...actions.map((a) => `                <action android:name="${a}" />`), '            </intent-filter>', '        </activity>');
          continue;
        }
        if (kind === 'aliases') {
          if (!mine.has(c.targetActivity)) refuse(`gives ${c.name} the target ${JSON.stringify(c.targetActivity)}, not an activity it declares`);
          listed(ALIAS_ACTIONS, c.action, `${c.name} the action`);
          listed(ALIAS_CATEGORIES, c.category, `${c.name} the category`);
          lines.push(`        <activity-alias android:name="${c.name}" android:exported="true" android:targetActivity="${c.targetActivity}" android:permission="android.permission.START_VIEW_PERMISSION_USAGE">`,
            '            <intent-filter>', `                <action android:name="${c.action}" />`, `                <category android:name="${c.category}" />`, '            </intent-filter>', '        </activity-alias>');
          continue;
        }
        if (c.directBootAware !== undefined && typeof c.directBootAware !== 'boolean') refuse(`gives ${c.name} a directBootAware that is not true or false`);
        const boot = c.directBootAware ? ' android:directBootAware="true"' : '';
        if (kind === 'receivers') {
          const actions = c.actions ?? [];
          if (!Array.isArray(actions)) refuse(`gives ${c.name} actions that are not a list`);
          for (const a of actions) listed(RECEIVER_ACTIONS, a, `${c.name} the action`);
          if (!actions.length) lines.push(`        <receiver android:name="${c.name}" android:exported="false"${boot} />`);
          else lines.push(`        <receiver android:name="${c.name}" android:exported="false"${boot}>`, '            <intent-filter>',
            ...actions.map((a) => `                <action android:name="${a}" />`), '            </intent-filter>', '        </receiver>');
        } else {
          const type = c.foregroundServiceType;
          if (type !== undefined) listed(SERVICE_TYPES, type, `${c.name} the foreground service type`);
          if (c.permission === undefined && c.actions === undefined) {
            lines.push(`        <service android:name="${c.name}" android:exported="false"${boot}${type ? ` android:foregroundServiceType="${type}"` : ''} />`);
            continue;
          }
          const actions = SERVICE_BINDS.get(c.permission);
          if (!actions) refuse(`gives ${c.name} the permission ${JSON.stringify(c.permission)}, which is not on the list`);
          if (!Array.isArray(c.actions) || !c.actions.length || c.actions.some((a) => !actions.has(a))) refuse(`gives ${c.name} actions that ${c.permission} does not bind`);
          if (type !== undefined || c.directBootAware) refuse(`gives the bound service ${c.name} a foreground type or direct boot`);
          lines.push(`        <service android:name="${c.name}" android:exported="true" android:permission="${c.permission}">`, '            <intent-filter>',
            ...c.actions.map((a) => `                <action android:name="${a}" />`), '            </intent-filter>', '        </service>');
        }
      }
    }
  }
  if (activities && privacyPolicy) lines.unshift(`        <meta-data android:name="exact.privacy_policy" android:value="${escape(privacyPolicy)}" />`);
  return [
    ...(queries.size ? ['    <queries>', ...queries, '    </queries>'] : []),
    ...(lines.length ? ['    <application>', ...lines, '    </application>'] : []),
  ].join('\n');
}
