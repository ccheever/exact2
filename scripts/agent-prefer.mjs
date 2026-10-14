// `prefer`, the ninth operation's facts (LLP 1061 D5; LLP 1069.000 D6; LLP
// 1069.007 D2; LLP 1078 D7): the device facts by their web names, grouped
// on the wire as `media`, `page` and `fold`; and the web carrier's own path
// for them — the browser's emulation through CDP where it has one, the
// glue's substitute where it does not. agent.mjs re-exports the tables.

/** Every host's display preferences at launch under the agent (LLP 1069.007 D2). */
export const LAUNCH_MEDIA = { 'prefers-reduced-motion': 'no-preference', 'prefers-reduced-transparency': 'no-preference', 'prefers-color-scheme': 'light', 'prefers-contrast': 'no-preference', 'color-gamut': 'srgb', 'dynamic-range': 'standard' };
export const PREFERENCES = { 'display-mode': ['standalone', 'browser', 'fullscreen', 'minimal-ui', 'window-controls-overlay', 'picture-in-picture', 'tabbed'], 'prefers-reduced-motion': ['reduce', 'no-preference'], 'prefers-reduced-transparency': ['reduce', 'no-preference'], 'prefers-contrast': ['more', 'less', 'custom', 'no-preference'], 'prefers-color-scheme': ['dark', 'light'], 'color-gamut': ['srgb', 'p3', 'rec2020'], 'dynamic-range': ['standard', 'high'] }; // `prefer`'s CSS media features and values (the display's two: LLP 1100 D9)
export const PAGE_FACTS = { 'visibility-state': ['visible', 'hidden'], online: ['true', 'false'], 'can-share': ['true', 'false'], 'can-open-files': ['true', 'false'], 'has-focus': ['true', 'false'], 'keyboard-navigation': ['true', 'false'], 'root-font-size': ['<px>'], 'display-scale': ['<device pixels a px>'] }; // `prefer`'s page group (LLP 1069.000 D2, D3, D6; LLP 1069.007 D2; `has-focus`: #114; `keyboard-navigation`: the Mac's setting, whether Tab reaches every control, LLP 1115.000 D7.2, refused by name on the other hosts; `display-scale`: what a hairline is one device pixel of, LLP 1115.000 D10.3)
export const FOLD_FACTS = { posture: ['folded', 'continuous'], segments: ['<cols>x<rows>'], gap: ['<points>'] }; // `prefer`'s fold group (LLP 1078 D7): a host without a fold splits its viewport evenly; one with a fold refuses
export const MODE_FACT = { mode: ['<axis>:<value>'] }; // `prefer`'s mode group (LLP 1121 D5, D13): the app's own modes, `setMode` for each pin; `prefers-color-scheme` stays as it is

/** The CLI's facts (`prefer <name> <value> …`) as the wire's three groups; an unknown fact or value is refused naming what is expected.
 * A fact answers to its near spellings (`color-scheme`, `colorScheme`, `reduced-motion` for the `prefers-` names): round 9, three builds. */
export function preferGroups(facts) {
  const media = {}, page = {}, fold = {}, mode = {};
  const all = { ...PREFERENCES, ...PAGE_FACTS, ...FOLD_FACTS, ...MODE_FACT };
  const expected = () => Object.entries(all).map(([n, v]) => `${n} ${v.join('|')}`).join(', ');
  const norm = n => String(n).toLowerCase().replace(/[^a-z]/g, '').replace(/^prefers?/, '');
  for (const [given, value] of Object.entries(facts ?? {})) {
    const name = Object.keys(all).find(k => k === given || norm(k) === norm(given)) ?? given;
    if ((PREFERENCES[name] ?? []).includes(value)) media[name] = value;
    else if ((PAGE_FACTS[name] ?? []).includes(String(value))) page[name] = PAGE_FACTS[name][0] === 'true' ? String(value) === 'true' : String(value);
    else if ((name === 'root-font-size' || name === 'display-scale') && Number(value) > 0 && Number.isFinite(Number(value))) page[name] = Number(value);
    else if (name === 'posture' && FOLD_FACTS.posture.includes(value)) fold.posture = value;
    else if (name === 'segments' && /^\d+x\d+$/.test(String(value))) { const [c, r] = String(value).split('x').map(Number); fold.cols = c; fold.rows = r; }
    else if (name === 'gap' && Number(value) >= 0 && Number.isFinite(Number(value))) fold.gap = Number(value);
    else if (name === 'mode' && /^[\w.-]+[:=][\w.-]+([ ,]+[\w.-]+[:=][\w.-]+)*$/.test(String(value))) for (const pin of String(value).split(/[ ,]+/)) { const [axis, v] = pin.split(/[:=]/); mode[axis] = v; }
    else throw new Error(`prefer: ${name} ${value}: expected ${expected()}`);
  }
  if (fold.gap != null && fold.cols == null) throw new Error('prefer: gap needs segments <cols>x<rows>');
  return { media, page, fold, mode };
}

/** The request a native host answers: `media` rides along when nothing else does (an empty request is refused there by name). */
export function preferOp(media, page, fold, mode = {}) {
  const groups = Object.keys(media).length || (!Object.keys(page).length && !Object.keys(fold).length && !Object.keys(mode).length) ? { media } : {};
  return { op: 'prefer', ...groups, ...(Object.keys(page).length ? { page } : {}), ...(Object.keys(fold).length ? { fold } : {}), ...(Object.keys(mode).length ? { mode } : {}) };
}

/** `prefer segments 2x1 gap 40` as CDP's display features: one vertical feature per column divider, one horizontal per row divider, each `gap` wide and centred where the even split puts it (the same split every host makes). */
export function displayFeatures(width, height, cols, rows, gap) {
  const features = [];
  const span = (total, n) => (total - (n - 1) * gap) / n;
  for (let i = 1; i < cols; i++) features.push({ orientation: 'vertical', offset: Math.round(i * span(width, cols) + (i - 1) * gap), maskLength: Math.round(gap) });
  for (let i = 1; i < rows; i++) features.push({ orientation: 'horizontal', offset: Math.round(i * span(height, rows) + (i - 1) * gap), maskLength: Math.round(gap) });
  return features;
}

/** The web carrier's `prefer`. The browser's own emulation (LLP 1061 D5), which replaces its whole list: queries, CSS and the glue's listeners see it.
 * The page group is the glue's own value, told the runner as the page's observer tells it (LLP 1069.000 D6; LLP 1069.007 D5: not CDP).
 * The fold group (LLP 1078 D7): Chromium's own posture and display-feature overrides, so `navigator.devicePosture`, `window.viewport.segments`
 * and CSS's `env(viewport-segment-*)` all change — the parity oracle; a browser whose CDP lacks them gets the glue's substitute (the facts and
 * `layout.env`, not CSS). `emulated` is the carrier's media list, replaced in place. */
export async function preferWeb({ media, page, fold = {}, mode = {}, emulated, call, evaluate, frame, ask, scale = 0 }) {
  if (media['display-mode'] != null) throw new Error(`Chrome prefer cannot emulate display-mode ${media['display-mode']}: use the browser's fullscreen controls`);
  // LLP 1115.000 D10.3: a browser's scale is fixed when it launches (`EXACT_SCALE=2 bun scripts/agent.mjs web …`: Chrome's own device scale factor). CDP's emulated one moves `devicePixelRatio` and the resolution queries but not the device pixel a border snaps to (Chrome 154: a 0.5px border stays 1px under it), so it is not offered as the scale.
  if (page['display-scale'] != null) { const now = await evaluate('devicePixelRatio'); if (Math.abs(now - page['display-scale']) > 0.01) throw new Error(`prefer: display-scale ${page['display-scale']}: a browser's scale is fixed when it launches, and this one's is ${now}; launch it at another with EXACT_SCALE=${page['display-scale']}`); }
  if (Object.keys(media).length) { await call('Emulation.setEmulatedMedia', { features: Object.entries(Object.assign(emulated, media)).map(([name, value]) => ({ name, value })) }); await frame(); }
  let foldRequest = null;
  if (Object.keys(fold).length) {
    const v = await evaluate('({ w: innerWidth, h: innerHeight })'), grid = fold.cols != null || fold.rows != null || fold.gap != null;
    const cols = fold.cols ?? 1, rows = fold.rows ?? 1, gap = fold.gap ?? 0;
    if (grid && !(cols >= 1 && rows >= 1)) throw new Error(`prefer: segments ${cols}x${rows}: each count is at least 1`);
    if (grid && ((cols - 1) * gap >= v.w || (rows - 1) * gap >= v.h)) throw new Error(`prefer: segments ${cols}x${rows} gap ${gap}: the gap is wider than the viewport (${v.w} × ${v.h})`);
    try {
      if (fold.posture) await call(fold.posture === 'continuous' ? 'Emulation.clearDevicePostureOverride' : 'Emulation.setDevicePostureOverride', fold.posture === 'continuous' ? {} : { posture: { type: fold.posture } });
      if (grid) { // Chrome 154: a display feature takes effect only inside a device-metrics override (the carrier's own viewport, 1:1), carried on it when there is one divider; the list form joins it for a grid.
        const features = displayFeatures(v.w, v.h, cols, rows, gap);
        await call('Emulation.setDeviceMetricsOverride', { width: v.w, height: v.h, deviceScaleFactor: scale ? 0 : 1, mobile: false, ...(features.length === 1 ? { displayFeature: features[0] } : {}) });
        await call(features.length > 1 ? 'Emulation.setDisplayFeaturesOverride' : 'Emulation.clearDisplayFeaturesOverride', features.length > 1 ? { features } : {}); await frame();
        const reported = await evaluate(`(globalThis.viewport?.segments?.length ?? 1)`); if (reported !== cols * rows && cols * rows > 1) throw new Error(`the browser reports ${reported} segments for ${cols}x${rows}`);
      }
      foldRequest = {}; // the browser's own readings, re-read
    } catch (error) {
      if (!/wasn't found|not found|Invalid parameters|unknown|Only one display feature|reports \d+ segments/i.test(String(error?.message ?? error))) throw error;
      foldRequest = fold; // the glue's substitute: the facts and layout.env, not CSS's own resolution
    }
    await frame();
  }
  const hasMode = Object.keys(mode).length > 0;
  const pageReply = Object.keys(page).length || foldRequest || hasMode ? await ask({ op: 'prefer', ...(Object.keys(page).length ? { page } : {}), ...(foldRequest ? { fold: foldRequest } : {}), ...(hasMode ? { mode } : {}) }) : null;
  if (pageReply?.error) throw new Error(pageReply.error);
  if (pageReply) await frame();
  return { media: await evaluate(`Object.fromEntries(${JSON.stringify(Object.entries(PREFERENCES))}.map(([name, values]) => [name, values.find(v => matchMedia('(' + name + ': ' + v + ')').matches) ?? values.at(-1)]))`), ...(pageReply ? { page: pageReply.page, fold: pageReply.fold, ...(hasMode ? { mode: pageReply.mode } : {}) } : {}) };
}
