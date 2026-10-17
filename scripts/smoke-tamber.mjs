// The Tamber binding's sample (LLP 1121 D12, stage 5): `apps/tamber-host`'s
// native module fills itself through `ExactTamber(view).color(…)` and says
// each colour it applies, which the app shows with a count. The window's
// swatch follows `setMode`; the pinned one keeps its pin's (`tone=cool`); both
// follow the scheme; a root font size (what Dynamic Type sets) fires their
// observations. A live edit is the dev loop's (the stage 5 gate drives it).
const said = async (s, id) => (await s.tree()).nodes.find(n => n.props?.testId === id)?.props?.text ?? '';
const colour = (text) => text.match(/#[0-9a-f]{8}/)?.[0] ?? null;
const times = (text) => Number(text.match(/×(\d+)/)?.[1] ?? 0);

export async function tamberSmoke({ host, open, check }) {
  const s = await open({ host });
  try {
    const at = async () => ({ window: await said(s, 'window-said'), pinned: await said(s, 'pinned-said') });
    const settle = () => new Promise(r => setTimeout(r, 600));
    await settle();
    const boot = await at();
    check(colour(boot.window) === '#c8102eff', `${host} tamber: the window's swatch is the warm tone: ${boot.window}`);
    check(colour(boot.pinned) === '#0a7affff', `${host} tamber: the pinned swatch is its pin's tone: ${boot.pinned}`);
    await s.tap('flip'); await settle();
    const moded = await at();
    check(colour(moded.window) === '#0a7affff', `${host} tamber: setMode moves the window's swatch: ${moded.window}`);
    check(times(moded.pinned) === times(boot.pinned), `${host} tamber: the pinned swatch's inputs did not move: ${moded.pinned}`);
    await s.prefer({ 'prefers-color-scheme': 'dark' }); await settle();
    const dark = await at();
    check(colour(dark.window) === '#64b5f6ff' && colour(dark.pinned) === '#64b5f6ff', `${host} tamber: both follow the scheme: ${dark.window} | ${dark.pinned}`);
    await s.prefer({ 'root-font-size': '22' }); await settle();
    const font = await at();
    check(times(font.window) > times(dark.window) && times(font.pinned) > times(dark.pinned), `${host} tamber: a root font size fires both observations: ${font.window} | ${font.pinned}`);
    // Stage 6 G: the label's font and the inset follow the text size, the
    // font Bold Text too (`length(…)`, `font(…)`).
    const inset = (text) => Number(text.match(/inset (\d+)/)?.[1] ?? 0);
    const size = (text) => Number(text.match(/font (\d+)/)?.[1] ?? 0);
    const face = (text) => text.match(/font \d+ (\S+ w\d+)/)?.[1] ?? '';
    check(inset(font.window) > inset(dark.window) && size(font.window) > size(dark.window), `${host} tamber: a length and a font follow the text size: ${dark.window} → ${font.window}`);
    await s.prefer({ 'bold-text': true }); await settle();
    const bold = await at();
    check(times(bold.window) > times(font.window) && face(bold.window) !== face(font.window), `${host} tamber: Bold Text fires the observation and weights the font: ${font.window} → ${bold.window}`);
    await s.prefer({ 'bold-text': false }); await settle();
  } finally {
    await s.close?.();
  }
}
