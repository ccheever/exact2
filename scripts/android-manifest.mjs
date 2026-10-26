// An Android implementation's manifest fragment (LLP 1133.030 D9, D12 item 4; LLP 1133.026 D19 item 3): the
// components a package declares in its `exact.json` `"manifest"`, checked against an allow-list and written into the
// APK's generated manifest. Every package gets this seam (LLP 1133.001 D8), so the list bounds what any package runs
// with no session: what it names, and nothing the XML could say besides, is all a fragment can add. A package that
// needs another action or type amends the list in its RFC.
//
//   "manifest": {
//     "receivers": [{"name": "<class>", "directBootAware": true, "actions": ["android.intent.action.BOOT_COMPLETED"]}],
//     "services": [{"name": "<class>", "directBootAware": true, "foregroundServiceType": "systemExempted"}],
//     "queries": [{"package": "com.google.android.apps.healthdata"}],
//     "activities": [{"name": "<class>", "actions": ["androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE"]}],
//     "aliases": [{"name": "<class>", "targetActivity": "<an activity above>", "action": "android.intent.action.VIEW_PERMISSION_USAGE",
//                  "category": "android.intent.category.HEALTH_PERMISSIONS"}]
//   }
//
// Receivers and services are `exported="false"`: the system's protected broadcasts still reach an unexported receiver,
// and a receiver for the package's own alarm needs no action, since its intent is explicit. An activity is exported,
// since the system starts it, but only for an action on the list, and never shows: it is `Theme.NoDisplay` and opens
// the app's privacy policy, which the application's `exact.privacy_policy` meta-data carries (Health Connect's
// rationale, [Health Connect: get started](https://developer.android.com/health-and-fitness/guides/health-connect/develop/get-started)).
// An alias is guarded by `START_VIEW_PERMISSION_USAGE`, which only the system holds. There is no `<provider>`, no
// `<uses-permission>` (permissions come only from grants, LLP 1133.002 D6) and no `android:process`.

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

/** The actions an activity may answer: Health Connect's request to show the app's privacy policy. */
export const ACTIVITY_ACTIONS = new Set(['androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE']);

/** An alias's action and category: Android 14's permission-usage entry for Health (LLP 1133.026 D19 item 3). */
export const ALIAS_ACTIONS = new Set(['android.intent.action.VIEW_PERMISSION_USAGE']);
export const ALIAS_CATEGORIES = new Set(['android.intent.category.HEALTH_PERMISSIONS']);

const FIELDS = {
  receivers: new Set(['name', 'directBootAware', 'actions']),
  services: new Set(['name', 'directBootAware', 'foregroundServiceType']),
  queries: new Set(['package']),
  activities: new Set(['name', 'actions']),
  aliases: new Set(['name', 'targetActivity', 'action', 'category']),
};

const ONE = { receivers: 'receiver', services: 'service', queries: 'query', activities: 'activity', aliases: 'alias' };

const escape = (s) => s.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

/** The manifest's children for every bound implementation's fragment (a `<queries>` element and an `<application>`
 * one, each when it has entries), or '' when none declares one. Throws, naming the package and what the allow-list
 * refuses. `bound` is `androidImplementations`' list; `privacyPolicy` is the app's `privacy_policy`, carried as
 * meta-data when a fragment declares an activity. */
export function manifestFragment(bound, { privacyPolicy = null } = {}) {
  const lines = [], queries = [], names = new Set();
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
      if (!FIELDS[kind]) refuse(`names "${kind}"; only ${Object.keys(FIELDS).join(', ')} are allowed`);
      if (!Array.isArray(list)) refuse(`has ${kind} that is not a list`);
      for (const c of list) {
        for (const key of Object.keys(c ?? {})) if (!FIELDS[kind].has(key)) refuse(`gives a ${ONE[kind]} "${key}", which the list does not allow`);
        if (kind === 'queries') {
          listed(QUERY_PACKAGES, c.package, 'a query the package');
          if (!queries.includes(c.package)) queries.push(c.package);
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
          lines.push(`        <service android:name="${c.name}" android:exported="false"${boot}${type ? ` android:foregroundServiceType="${type}"` : ''} />`);
        }
      }
    }
  }
  if (activities && privacyPolicy) lines.unshift(`        <meta-data android:name="exact.privacy_policy" android:value="${escape(privacyPolicy)}" />`);
  return [
    ...(queries.length ? ['    <queries>', ...queries.map((p) => `        <package android:name="${p}" />`), '    </queries>'] : []),
    ...(lines.length ? ['    <application>', ...lines, '    </application>'] : []),
  ].join('\n');
}
