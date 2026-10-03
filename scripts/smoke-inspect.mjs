// The smoke's inspection steps (`smoke.mjs` imports them as it does
// `smoke-duo.mjs`): 2b, `layout <node>` (LLP 1035.002 D1); 2c, `state`'s
// host sections (D2, D3); 7a, `layout agree` (LLP 1080.001 D5) at the drive's
// settled points; and the pooling drive on Carousel's virtualized feed.
import { render } from './agent-inspect.mjs';

const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const box = (l, id) => l.nodes.find((n) => n.testId === id);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 2b. `layout <node>` (LLP 1035.002 D1): the runner's half names where
// each value came from, the host's half the spaces it has; the explained
// box is the listing's box; a stale id is refused by name.
export async function explainNode(s, { tree, layout, check }) {
  const explained = await s.layout('station-name');
  const n = explained.node;
  const listed = box(layout, 'station-name');
  check(n && n.id === byTestId(tree, 'station-name')?.id && n.style && n.space?.viewport, `layout station-name carries no node detail: ${JSON.stringify(explained.node)}`);
  check(n && ['authored', 'inherited', 'initial'].includes(n.style.text_color?.source), `text_color has no source: ${JSON.stringify(n?.style?.text_color)}`);
  check(n && listed && Math.abs(n.space.viewport.x - listed.x) < 0.01 && Math.abs(n.space.viewport.w - listed.w) < 0.01, `the explained box ${JSON.stringify(n?.space?.viewport)} disagrees with the listing ${JSON.stringify(listed)}`);
  check(await s.op({ op: 'layout', id: 999999 }).then(() => false, (e) => /stale node/.test(e.message)), 'a stale node id was not refused by name');
}

// 2c. `state`'s host sections (LLP 1035.002 D2) are present on every host
// (Linux says `unavailable`, never nothing); where the host has a focus,
// the field just typed into is the editor; on iOS the keyboard comes up
// (its notification lands asynchronously, so poll). Every reply is
// tagged with the runner's epoch and incarnation (D3). Answers the state
// and tree it read last.
export async function hostSections(s, { host, tree, state, check }) {
  check(state.focus && state.keyboard && state.navigation, `state lacks a host section: ${Object.keys(state).join(', ')}`);
  check(Number.isInteger(state.epoch) && Number.isInteger(state.incarnation), `state is untagged: epoch ${state.epoch}, incarnation ${state.incarnation}`);
  check(Number.isInteger((await s.layout()).epoch), 'the layout reply is untagged');
  if (host !== 'linux') {
    const field = byTestId(tree, 'station-search')?.id;
    check(state.focus.editor === field && state.focus.logical === field, `the typed field is not the focus: ${JSON.stringify(state.focus)}`);
    if (host === 'ios') {
      for (let i = 0; i < 40 && !state.keyboard.visible; i++) { await sleep(50); state = await s.state(); }
      check(state.keyboard.visible === true && state.keyboard.overlap > 0, `the keyboard is not up: ${JSON.stringify(state.keyboard)}`);
    }
    check(state.slots.searchFocused === true, `typing did not focus the field (searchFocused ${state.slots.searchFocused})`);
    await s.type('station-search', { key: 'Enter' });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.lastKey === 'Enter' && byTestId(tree, 'search-hint')?.props.text === 'searching · last key Enter', `Enter at the field: lastKey ${JSON.stringify(state.slots.lastKey)}, hint ${JSON.stringify(byTestId(tree, 'search-hint')?.props.text)}`);
    check(byTestId(tree, 'station-search')?.props.value === 'Palo', `Enter changed the field's text to ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  }
  return { state, tree };
}

// 7a. `layout agree` (LLP 1080.001 D5) after `clock settle`: a failure for a
// drive that does not settle, a missing reply, an incomplete walk (each
// reason named), a required kind that covered nothing, and each listed
// disagreement as the transcript renders it. The web and Linux answer
// `unavailable`, which is asserted instead. Answers the report.
export async function agree(s, label, { host, check }) {
  const settled = await s.clock('settle');
  if (settled?.settled === false) { check(false, `${label}: clock settle did not settle (${settled.reason ?? 'bound'}); agreement not asserted`); return null; }
  const reply = await s.layout(undefined, undefined, { agree: true });
  const a = reply?.agreement;
  if (!check(a, `${label}: layout agree answered no agreement`)) return null;
  if (host === 'web' || host === 'linux') { check(typeof a.unavailable === 'string', `${label}: ${host} must answer agreement unavailable, not ${JSON.stringify(a).slice(0, 200)}`); return a; }
  check(a.complete === true, `${label}: the agreement walk is incomplete (${(a.incomplete ?? []).join(', ')})`);
  const c = a.coverage ?? {};
  check(c.stray?.judged > 0 && c.hidden?.compared > 0 && c.frame?.compared > 0, `${label}: a required kind covered nothing: ${JSON.stringify({ stray: c.stray, hidden: c.hidden?.compared, frame: c.frame?.compared })}`);
  for (const line of render('layout', reply).split('\n').slice(2)) check(false, `${label}: ${line.trim()}`);
  console.log(`${host} agree, ${label}: ${a.complete ? 'complete' : 'INCOMPLETE'}, ${Object.values(a.counts ?? {}).reduce((x, y) => x + y, 0)} disagreements; ${c.stray?.judged} judged, ${c.frame?.compared} frames compared, ${c.hidden?.compared} hidden compared, ${c.kept?.parkedRoots} parked roots`);
  return a;
}

// The pooling drive (LLP 1080.001 D5), when the app is Carousel: on the
// feed page, `Down` moves the virtualized feed far, so rows retire and park and the rows built
// there take them. Both counters must grow — retirement and reuse happened —
// then the walk is clean and counts every parked root `state` reports.
export async function poolingDrive(s, { host, check }) {
  // Carousel opens on its strips; `feed-page` shows the virtualized feed.
  if (!byTestId(await s.tree(), 'feed')) { await s.tap('feed-page'); await s.clock('settle'); }
  const before = (await s.state()).pool;
  if (!check(before && Number.isInteger(before.parks), `carousel: state has no pool section (${JSON.stringify(before)})`)) return;
  await s.tap('feed-far');
  const a = await agree(s, 'carousel: after the feed moved far', { host, check });
  const after = (await s.state()).pool;
  check(after.parks > before.parks && after.takes > before.takes, `carousel: the feed's far move parked ${after.parks - before.parks} and took ${after.takes - before.takes} rows; both must grow`);
  if (a) check(a.coverage?.kept?.parkedRoots === after.parked, `carousel: layout agree counted ${a.coverage?.kept?.parkedRoots} parked roots, state.pool ${after.parked}`);
  console.log(`${host} carousel pool: parks ${before.parks}→${after.parks}, takes ${before.takes}→${after.takes}, parked ${after.parked}`);
}
