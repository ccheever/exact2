// `prefer`, the ninth operation's facts (LLP 1061 D5; LLP 1069.000 D6; LLP
// 1069.007 D2; LLP 1078 D7): the device facts by their web names, grouped
// on the wire as `media`, `page` and `fold`; and the web carrier's own path
// for them — the browser's emulation through CDP where it has one, the
// glue's substitute where it does not. agent.mjs re-exports the tables.

/** Every host's display preferences at launch under the agent (LLP 1069.007 D2). */
export const LAUNCH_MEDIA = { 'prefers-reduced-motion': 'no-preference', 'prefers-reduced-transparency': 'no-preference', 'prefers-color-scheme': 'light', 'prefers-contrast': 'no-preference' };
export const PREFERENCES = { 'prefers-reduced-motion': ['reduce', 'no-preference'], 'prefers-reduced-transparency': ['reduce', 'no-preference'], 'prefers-contrast': ['more', 'less', 'custom', 'no-preference'], 'prefers-color-scheme': ['dark', 'light'] }; // `prefer`'s CSS media features and values
export const PAGE_FACTS = { 'visibility-state': ['visible', 'hidden'], online: ['true', 'false'], 'can-share': ['true', 'false'], 'root-font-size': ['<px>'] }; // `prefer`'s page group (LLP 1069.000 D2, D3, D6; LLP 1069.007 D2)
export const FOLD_FACTS = { posture: ['folded', 'continuous'], segments: ['<cols>x<rows>'], gap: ['<points>'] }; // `prefer`'s fold group (LLP 1078 D7): a host without a fold splits its viewport evenly; one with a fold refuses

/** The CLI's facts (`prefer <name> <value> …`) as the wire's three groups; an unknown fact or value is refused naming what is expected. */
export function preferGroups(facts) {
  const media = {}, page = {}, fold = {};
  const expected = () => Object.entries({ ...PREFERENCES, ...PAGE_FACTS, ...FOLD_FACTS }).map(([n, v]) => `${n} ${v.join('|')}`).join(', ');
  for (const [name, value] of Object.entries(facts ?? {})) {
    if ((PREFERENCES[name] ?? []).includes(value)) media[name] = value;
    else if ((PAGE_FACTS[name] ?? []).includes(String(value))) page[name] = PAGE_FACTS[name][0] === 'true' ? String(value) === 'true' : String(value);
    else if (name === 'root-font-size' && Number(value) > 0 && Number.isFinite(Number(value))) page[name] = Number(value);
    else if (name === 'posture' && FOLD_FACTS.posture.includes(value)) fold.posture = value;
    else if (name === 'segments' && /^\d+x\d+$/.test(String(value))) { const [c, r] = String(value).split('x').map(Number); fold.cols = c; fold.rows = r; }
    else if (name === 'gap' && Number(value) >= 0 && Number.isFinite(Number(value))) fold.gap = Number(value);
    else throw new Error(`prefer: ${name} ${value}: expected ${expected()}`);
  }
  if (fold.gap != null && fold.cols == null) throw new Error('prefer: gap needs segments <cols>x<rows>');
  return { media, page, fold };
}

/** The request a native host answers: `media` rides along when nothing else does (an empty request is refused there by name). */
export function preferOp(media, page, fold) {
  const groups = Object.keys(media).length || (!Object.keys(page).length && !Object.keys(fold).length) ? { media } : {};
  return { op: 'prefer', ...groups, ...(Object.keys(page).length ? { page } : {}), ...(Object.keys(fold).length ? { fold } : {}) };
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
export async function preferWeb({ media, page, fold = {}, emulated, call, evaluate, frame, ask }) {
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
        await call('Emulation.setDeviceMetricsOverride', { width: v.w, height: v.h, deviceScaleFactor: 1, mobile: false, ...(features.length === 1 ? { displayFeature: features[0] } : {}) });
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
  const pageReply = Object.keys(page).length || foldRequest ? await ask({ op: 'prefer', ...(Object.keys(page).length ? { page } : {}), ...(foldRequest ? { fold: foldRequest } : {}) }) : null;
  if (pageReply?.error) throw new Error(pageReply.error);
  if (pageReply) await frame();
  return { media: await evaluate(`Object.fromEntries(${JSON.stringify(Object.entries(PREFERENCES))}.map(([name, values]) => [name, values.find(v => matchMedia('(' + name + ': ' + v + ')').matches) ?? values.at(-1)]))`), ...(pageReply ? { page: pageReply.page, fold: pageReply.fold } : {}) };
}
