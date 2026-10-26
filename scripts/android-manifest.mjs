// An Android implementation's manifest fragment (LLP 1133.030 D9, D12 item 4; LLP 1133.026 D19 item 3): the
// components a package declares in its `exact.json` `"manifest"`, checked against an allow-list and written as the
// `<application>` children of the APK's generated manifest. Every package gets this seam (LLP 1133.001 D8), so the
// list bounds what any package runs with no session: what it names, and nothing the XML could say besides, is all a
// fragment can add. A package that needs another action or type amends the list in its RFC.
//
//   "manifest": {
//     "receivers": [{"name": "<class>", "directBootAware": true, "actions": ["android.intent.action.BOOT_COMPLETED"]}],
//     "services": [{"name": "<class>", "directBootAware": true, "foregroundServiceType": "systemExempted"}]
//   }
//
// Every component is `exported="false"`: the system's protected broadcasts still reach an unexported receiver, and a
// receiver for the package's own alarm needs no action, since its intent is explicit. There is no `<provider>`, no
// `<uses-permission>` (permissions come only from grants, LLP 1133.002 D6), no `android:process` and no exported
// activity; LLP 1133.026 D19 item 3's activities, aliases and `<queries>` extend this list when they are built.

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

const FIELDS = { receivers: new Set(['name', 'directBootAware', 'actions']), services: new Set(['name', 'directBootAware', 'foregroundServiceType']) };

/** The `<application>` children for every bound implementation's fragment, or '' when none declares one. Throws,
 * naming the package and what the allow-list refuses. `bound` is `androidImplementations`' list. */
export function manifestFragment(bound) {
  const lines = [], names = new Set();
  for (const impl of bound) {
    const fragment = impl.manifest;
    if (fragment == null) continue;
    const who = `${impl.extension}'s Android implementation`, refuse = (what) => { throw new Error(`${who}: its manifest fragment ${what} (LLP 1133.030 D9's allow-list)`); };
    if (typeof fragment !== 'object' || Array.isArray(fragment)) refuse('is not an object');
    // A component is the package's own class: one in the Java package of its `class`.
    const own = impl.className.split('.').slice(0, -1).join('.');
    for (const [kind, list] of Object.entries(fragment)) {
      if (!FIELDS[kind]) refuse(`names "${kind}"; only receivers and services are allowed`);
      if (!Array.isArray(list)) refuse(`has ${kind} that is not a list`);
      for (const c of list) {
        for (const key of Object.keys(c ?? {})) if (!FIELDS[kind].has(key)) refuse(`gives a ${kind.slice(0, -1)} "${key}", which the list does not allow`);
        if (typeof c.name !== 'string' || !/^[A-Za-z_]\w*(\.[A-Za-z_]\w*)+$/.test(c.name) || !c.name.startsWith(`${own}.`)) refuse(`names ${JSON.stringify(c.name)}, not a class in ${own}`);
        if (names.has(c.name)) refuse(`declares ${c.name} twice`);
        names.add(c.name);
        if (c.directBootAware !== undefined && typeof c.directBootAware !== 'boolean') refuse(`gives ${c.name} a directBootAware that is not true or false`);
        const boot = c.directBootAware ? ' android:directBootAware="true"' : '';
        if (kind === 'receivers') {
          const actions = c.actions ?? [];
          if (!Array.isArray(actions)) refuse(`gives ${c.name} actions that are not a list`);
          for (const a of actions) if (!RECEIVER_ACTIONS.has(a)) refuse(`gives ${c.name} the action ${JSON.stringify(a)}, which is not on the list`);
          if (!actions.length) lines.push(`        <receiver android:name="${c.name}" android:exported="false"${boot} />`);
          else lines.push(`        <receiver android:name="${c.name}" android:exported="false"${boot}>`, '            <intent-filter>',
            ...actions.map((a) => `                <action android:name="${a}" />`), '            </intent-filter>', '        </receiver>');
        } else {
          const type = c.foregroundServiceType;
          if (type !== undefined && !SERVICE_TYPES.has(type)) refuse(`gives ${c.name} the foreground service type ${JSON.stringify(type)}, which is not on the list`);
          lines.push(`        <service android:name="${c.name}" android:exported="false"${boot}${type ? ` android:foregroundServiceType="${type}"` : ''} />`);
        }
      }
    }
  }
  return lines.join('\n');
}
