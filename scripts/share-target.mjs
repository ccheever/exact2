// @ref LLP 1133.024 D1, D4–D6 — the manifest's `share_target`: what each
// host derives from it, and what the bake refuses.
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { DOCUMENT_UTIS, resolveApp } from './app.mjs';

/** The UTI each wildcard names on Apple. */
const WILDCARD_UTIS = { 'image/*': 'public.image', 'video/*': 'public.movie', 'audio/*': 'public.audio', 'text/*': 'public.text' };
/** A file extension's MIME type, for an `accept` entry spelled as an extension. */
export const EXTENSION_TYPES = {
  '.txt': 'text/plain', '.text': 'text/plain', '.md': 'text/markdown', '.markdown': 'text/markdown', '.html': 'text/html', '.htm': 'text/html',
  '.csv': 'text/csv', '.tsv': 'text/tab-separated-values', '.json': 'application/json', '.pdf': 'application/pdf', '.zip': 'application/zip',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml',
  '.heic': 'image/heic', '.heif': 'image/heif', '.mp4': 'video/mp4', '.mov': 'video/quicktime', '.m4a': 'audio/mp4', '.mp3': 'audio/mpeg',
};
const EXTRA_UTIS = { 'image/heic': 'public.heic', 'image/heif': 'public.heif', 'video/mp4': 'public.mpeg-4', 'video/quicktime': 'com.apple.quicktime-movie', 'audio/mp4': 'public.mpeg-4-audio', 'audio/mpeg': 'public.mp3' };

const list = accept => [accept].flat();
/** The MIME type or pattern an `accept` entry names. */
export const acceptType = entry => entry.startsWith('.') ? EXTENSION_TYPES[entry.toLowerCase()] ?? null : entry.toLowerCase();
/** The UTI an `accept` entry names on Apple, or null. */
export function acceptUTI(entry) {
  const type = acceptType(entry);
  return type ? DOCUMENT_UTIS[type] ?? WILDCARD_UTIS[type] ?? EXTRA_UTIS[type] ?? null : null;
}

/** What the bake refuses in a manifest's `share_target`: an `accept` entry no host maps, files without
 * `POST` and multipart, a parameter name used twice. The compiler checks `action` against the app's routes
 * (`contract/cli/src/picker.rs`). */
export function shareTargetProblems(manifest) {
  const target = manifest.share_target;
  if (!target) return [];
  const problems = [];
  const files = target.params?.files ?? [];
  if (files.length && (target.method !== 'POST' || target.enctype !== 'multipart/form-data'))
    problems.push('share_target: params.files needs "method": "POST" and "enctype": "multipart/form-data", as the Web Share Target draft has it');
  files.forEach((file, i) => list(file.accept).forEach(entry => {
    if (!acceptUTI(entry)) problems.push(`share_target.params.files[${i}].accept: ${entry} names no type the hosts map (they map ${[...Object.keys(DOCUMENT_UTIS), ...Object.keys(WILDCARD_UTIS), ...Object.keys(EXTRA_UTIS)].join(', ')}, and the extensions ${Object.keys(EXTENSION_TYPES).join(' ')})`);
  }));
  const names = [target.params?.title, target.params?.text, target.params?.url, ...files.map(f => f.name)].filter(Boolean);
  for (const name of new Set(names.filter((n, i) => names.indexOf(n) !== i))) problems.push(`share_target.params: the parameter ${name} is named twice`);
  return problems;
}

/** The web's member (D6): always GET with the text members, since a POST share needs a service worker (LLP 1030
 * D1); `notes` says what was rewritten, for the build to print. */
export function webShareTarget(target, notes = []) {
  if (!target) return null;
  const { title, text, url, files } = target.params ?? {};
  if (target.method === 'POST') notes.push('share_target: the web member is published as GET; a POST share needs a service worker (LLP 1133.024 D6)');
  if (files?.length) notes.push('share_target: the web member leaves out params.files; file shares on the web wait for a service worker (LLP 1133.024 §19 question 4)');
  return { action: target.action, method: 'GET', enctype: 'application/x-www-form-urlencoded', params: Object.fromEntries(Object.entries({ title, text, url }).filter(([, v]) => v)) };
}

/** Apple's `NSExtensionActivationRule` (D4): the app's own, or derived from `params` — text when `text` is mapped, a
 * web URL when `url` is, and each accepted kind of file with no count cap, as the web has none. */
export function appleActivationRule(target) {
  if (target.apple?.NSExtensionActivationRule) return target.apple.NSExtensionActivationRule;
  const { text, url, files = [] } = target.params ?? {};
  const types = files.flatMap(f => list(f.accept).map(acceptType));
  const rule = {};
  if (text) rule.NSExtensionActivationSupportsText = true;
  if (url || text) rule.NSExtensionActivationSupportsWebURLWithMaxCount = 1;
  if (types.some(t => t?.startsWith('image/'))) rule.NSExtensionActivationSupportsImageWithMaxCount = 1000;
  if (types.some(t => t?.startsWith('video/'))) rule.NSExtensionActivationSupportsMovieWithMaxCount = 1000;
  if (types.some(t => t && !t.startsWith('image/') && !t.startsWith('video/'))) rule.NSExtensionActivationSupportsFileWithMaxCount = 1000;
  return rule;
}

/** What the extension and the claim read about each files member: its name and the UTIs it accepts. */
export const appleShareMembers = target => (target.params?.files ?? []).map(f => ({ name: f.name, types: [...new Set(list(f.accept).map(acceptUTI))] }));

/** The app's `share_target`, which the driver hands the host with each share. */
export function appShareTarget(app) {
  const { dir } = resolveApp(app);
  const target = JSON.parse(readFileSync(join(dir, 'app.json'), 'utf8')).share_target;
  if (!target) throw new Error(`share: ${join(dir, 'app.json')} declares no share_target (LLP 1133.024 D1)`);
  return target;
}

/** The driver's `share:` form (D9): `share:?title=…&text=…&url=…&files=<path>&files=<path>`, a URL only the driver
 * speaks. Each path is made absolute here, where the host copies it from. Returns the wire's `share`. */
export function driverShare(href, base) {
  const query = new URLSearchParams(href.replace(/^share:\??/, ''));
  const share = Object.fromEntries(['title', 'text', 'url'].filter(k => query.has(k)).map(k => [k, query.get(k)]));
  const files = query.getAll('files').filter(Boolean).map(path => ({ path: resolve(base, path) }));
  return files.length ? { ...share, files } : share;
}

/** Where the web delivers a share (D6): its GET member's location, with a link joined to the text when no `url`
 * member is mapped, as the hosts' claim joins it. A file is refused: web file shares wait for a service worker. */
export function webShareLocation(target, share) {
  if (share.files?.length) throw new Error('share: the web takes text and links only; a file share waits for a service worker (LLP 1133.024 §19 question 4)');
  const { title, text, url } = target.params ?? {};
  let body = share.text;
  if (!url && share.url) body = !body ? share.url : body.includes(share.url) ? body : `${body}\n${share.url}`;
  const pairs = [[title, share.title], [text, body], [url, share.url]].filter(([name, value]) => name && value);
  return pairs.length ? `${target.action}?${pairs.map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`).join('&')}` : target.action;
}

/** Android's intent filters (D5): `SEND text/plain` when text or a link is mapped, and `SEND` and `SEND_MULTIPLE`
 * with each accepted MIME pattern for files, or the app's own `mimeTypes`. */
export function androidShareTypes(target) {
  if (target.android?.mimeTypes?.length) return { text: target.android.mimeTypes.includes('text/plain'), files: target.android.mimeTypes.filter(t => t !== 'text/plain') };
  const { text, url, files = [] } = target.params ?? {};
  const types = [...new Set(files.flatMap(f => list(f.accept).map(acceptType)).filter(Boolean))];
  return { text: !!(text || url), files: types };
}

/** The app activity's share filters and its `share_target` (D5), as lines inside a manifest's `<application>`: the
 * activity becomes `singleTask`, so a share brings the app's one task forward rather than starting it in the
 * sender's, and carries the member as `dev.exact.share_target` meta-data, which the reader hands exact2 with each
 * share (`Shares.kt`). The platform words stay out of it. Null when the app declares no target. */
export function androidShareActivity(target) {
  if (!target) return null;
  const { text, files } = androidShareTypes(target);
  const send = [...new Set([...(text ? ['text/plain'] : []), ...files])], multiple = files;
  const member = JSON.stringify({ action: target.action, params: target.params ?? {} }).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
  const filter = (action, types) => types.length ? ['            <intent-filter>', `                <action android:name="android.intent.action.${action}" />`,
    '                <category android:name="android.intent.category.DEFAULT" />', ...types.map((t) => `                <data android:mimeType="${t}" />`), '            </intent-filter>'] : [];
  return ['        <activity android:name="dev.exact.app.MainActivity" android:launchMode="singleTask" tools:replace="android:launchMode">',
    ...filter('SEND', send), ...filter('SEND_MULTIPLE', multiple),
    `            <meta-data android:name="dev.exact.share_target" android:value="${member}" />`, '        </activity>'];
}
