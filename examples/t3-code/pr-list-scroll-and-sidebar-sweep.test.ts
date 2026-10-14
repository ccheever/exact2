// pr-list-scroll-and-sidebar-sweep (T3 Code 1e2ecbd975, MIT; see LICENSE-T3).
// PS-2 (X13), the sidebar's row-action sweep as Sidebar.tsx startActionSweep and Sidebar.pointer.ts
// SidebarPointerSensor run it: past 6 px the list takes `**:pointer-events-none`, so the row's card or a control's
// tooltip goes as the sweep takes over; Escape cancels it; a cancelled release applies nothing (r12-sidebar.test.ts holds
// the client's half); the rows then take the pointer again, so the row under it is the hovered one. The host keeps the
// pressed row hovered through a pan and hovers nothing new at the release until the pointer moves, so the section keeps
// that row as its rows' hover until the window's hover count moves on (`sweepRowHover`), and closes the card through the
// root's half of the hover when the sweep arms. The agent drive of the task record shows them on the app.
// PS-1 (X58): the Pull Requests list keeps its place across a window refocus; what moved it was the agent's first
// `prefer` re-telling the system appearance (task record, one-file repro), which remounts the per-scheme subtrees. The
// guard below keeps the list's mount free of the window's focus and visibility.
import { describe, expect, test } from 'bun:test';
import { contractFn } from './contract-fn-fixture';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** A component's text, from its `component` line to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const text = await source(file);
  const start = text.indexOf(`\ncomponent ${name}\n`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const rest = text.slice(start + 1);
  const end = rest.slice(1).search(/\n(component|fn|shape|style|use) /);
  return end < 0 ? rest : rest.slice(0, end + 1);
}
/** An action's body (its indented lines) inside a component's text. */
function action(text: string, name: string): string {
  const match = new RegExp(`\\n  action ${name}\\b[^\\n]*\\n((?:    [^\\n]*\\n)*)`).exec(text);
  if (!match) throw new Error(`no action ${name}`);
  return match[1];
}

describe('PS-2: the rows a sweep passes over, and after it (sweepRowHover)', () => {
  test('none while live; a cancelled sweep\'s resting row until the window\'s hover moves on; else the window\'s', async () => {
    const hover = await contractFn('sidebar.contract', 'sweepRowHover', ['live', 'rest', 'restSeq', 'seq', 'hoverId']);
    // Live: the list takes no pointer, so no row is hovered, the pressed one included.
    expect(hover(true, '', 0, 7, 'sweep-4~active#settle')).toBe('');
    // Cancelled with Escape over Sweep three: Sweep three, through the release, while the host's stale hover is the pressed row.
    expect(hover(false, 'sweep-3~active', 7, 7, '')).toBe('sweep-3~active');
    expect(hover(false, 'sweep-3~active', 7, 7, 'sweep-4~active#settle')).toBe('sweep-3~active');
    // The pointer moved and the window hovered something: its hover again.
    expect(hover(false, 'sweep-3~active', 7, 8, 'sweep-3~active')).toBe('sweep-3~active');
    expect(hover(false, 'sweep-3~active', 7, 9, '')).toBe('');
    // No sweep: the window's hover.
    expect(hover(false, '', 0, 3, 'sweep-2~active')).toBe('sweep-2~active');
  });

  test('the rows take that hover; the resting row is the one under the pointer when Escape cancelled', async () => {
    const section = await component('sidebar.contract', 'ThreadSection');
    expect(section).toContain('hoverId=sweepRowHover(sweepLive, sweepRest, sweepRestSeq, hoverSeq, hoverId)');
    const cancel = action(section, 'cancelSweep');
    expect(cancel).toContain('sweepCancelled = true');
    expect(cancel).toMatch(/sweepRest = join\(map\(filter\(rows, \(t\) => frame\(`row-\$\{t\.id\}`\)\.y <= sweepAt and .* > sweepAt\), \(t\) => t\.hoverKey\), ""\)/);
    expect(cancel).toContain('sweepRestSeq = hoverSeq');
    // A new sweep forgets it. The pointer's first move (out of the section too) ends it through the window's hover
    // count: the host sends the leave of the control it kept hovered through the pan.
    expect(action(section, 'sweep')).toContain('sweepRest = ""');
    // The section takes no hover of its own: the macOS host hovered one node at a time before main #327 (X62), so a hover on the
    // section would trade it with the row under the pointer on every move.
    const view = section.slice(section.indexOf('\n  view\n'));
    expect(view).not.toMatch(/\n    column [^\n]*\bhover=/);
    expect(section).not.toContain('sectionHover');
  });

  test('the card or a control\'s tooltip closes as the sweep takes over, through the window\'s hover', async () => {
    const section = await component('sidebar.contract', 'ThreadSection');
    const sweep = action(section, 'sweep');
    // The 6 pt rule arms the sweep; its first arming closes the window's hover (whatever the press did to the focus).
    // (`armed` is the sweep's rows from before this move, "" on a fresh sweep: sidebar-sweep.test.ts runs it.)
    expect(sweep).toMatch(/if not cancelled and \(armed != "" or movedX \* movedX \+ movedY \* movedY >= 36\)\n(\s*\/\/[^\n]*\n)*\s*if armed == ""\n\s*hoverCard\(hoverId, false\)\n\s*sweepKeys = /);
    // `hoverCard` is the window's sidebarHover, which counts the hover and calls the root's half (it clears the hover it
    // is told about), as a pointer leaving the row does. adopt-main-fixes-r11: since main #478 an action bound to `pan=`
    // may call it; #468's sweep called the root's half (`unhover`), handed down as it is, which is no prop now.
    const windowText = await source('app-window.contract');
    expect(windowText).not.toContain('unhover=');
    expect(windowText.match(/hoverCard=sidebarHover, [^\n]*hoverSeq=sidebarHoverSeq, /g)?.length).toBe(2);
    expect(action(windowText, 'sidebarHover').split('\n')[0]).toBe('    sidebarHoverSeq = sidebarHoverSeq + 1');
    expect(action(windowText, 'sidebarHover').trimEnd().split('\n').at(-1)).toBe('    sidebarHoverRoot(id, over)');
    const root = await source('app.contract');
    expect(root).toContain('sidebarHoverRoot=sidebarHover,');
    expect(/\n  action sidebarHover\(id: string, over: bool\)[^\n]*\n    if over\n[\s\S]*?\n    else if sidebarHoverId == id\n      sidebarHoverId = ""\n/.test(root)).toBe(true);
    // Every section hears both.
    const sidebar = await source('sidebar.contract');
    expect(sidebar.match(/ThreadSection\(/g)?.length).toBe(5);
    expect(sidebar.match(/hoverCard=hoverCard, [^\n]*hoverSeq=hoverSeq, /g)?.length).toBe(5);
    expect(sidebar).not.toContain('unhover');
  });

  test('Escape still cancels a live sweep from anywhere in the window, and only while it is live', async () => {
    const section = await component('sidebar.contract', 'ThreadSection');
    expect(section).toMatch(/\n      when sweepLive\n        button press=cancelSweep aria-keyshortcuts="Escape" aria-label="Cancel sweep" tabindex=-1/);
    // The release of a cancelled sweep says so, and the client applies nothing (r12-sidebar.test.ts).
    expect(section).toContain('sweepKeys=(sweepCancelled and sweepEpoch == data.sidebar.sweepEpoch ? "|cancel|" : sweepLive ? sweepKeys : "")');
  });
});

describe('PS-1: the Pull Requests list across a window refocus', () => {
  test('the list mounts on the page and the scheme only: the window\'s focus or visibility never remounts it', async () => {
    const pages = await component('app-main.contract', 'PagesCover');
    const view = pages.slice(pages.indexOf('\n  view\n'));
    const mount = view.slice(0, view.indexOf('PrList(controlsLeft'));
    const conditions = [...mount.matchAll(/\n\s*when ([^\n]+)/g)].map(match => match[1]);
    expect(conditions).toEqual(['pageCover', 'utilityPage == "usage"', 'scheme == "dark"', 'utilityPage == "pull-requests"', 'scheme == "dark"']);
    // Its rows are keyed by group and row; the scroll keeps no offset of its own, so a re-read moves nothing.
    const list = await component('pages-prs.contract', 'PrList');
    expect(list).toContain('each group in page.groups key=group.key');
    expect(list).toContain('each row in group.rows key=row.key');
    expect(list).toMatch(/scroll flex=1 min-height=0 width="100%" testId="pull-requests-scroll"\n/);
  });
});
