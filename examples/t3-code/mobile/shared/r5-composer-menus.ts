// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r5-composer-menus.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r5-composer: the composer's content-sized menus at the width their measured
// texts give, adapted from T3 Code (MIT; see LICENSE-T3): ui/menu.tsx MenuPopup
// (min-w-40, max-w calc(100vw - 2rem), p-1 inside a 1pt border, items px-2 with
// gap-2) holds TraitsPicker's radio groups, the runtime Select and
// BranchToolbar's MobileRunContextSelector ("Run on", then Workspace). Each text
// is laid out by r5-composer.contract's R5TextProbes and reported through the
// t3-anchor hook (T3Composer.swift); until a probe reports, the estimate stands in.
import { obj, type Obj } from './domain';
import { textWidth as estimate14 } from './composer-controls-view';
import { textWidth as pagesWidth } from './pages-text-width';

export type Probe = { key: string; text: string; size: number; weight: number };
export const probeKey = (text: string, size: number, weight: number) => `t:${size}:${weight}:${text}`;
export function probe(text: string, size: number, weight: number): Probe { return { key: probeKey(text, size, weight), text, size, weight }; }

/** The probes' unique entries (a menu may repeat a text). */
export function uniqueProbes(probes: Probe[]): Probe[] {
  const seen = new Set<string>();
  return probes.filter(entry => entry.text !== '' && !seen.has(entry.key) && (seen.add(entry.key), true)).slice(0, 96);
}

/** A text's laid-out width, or the estimate before its probe reports. */
export function measured(presentation: Obj, text: string, size: number, weight: number): number {
  if (!text) return 0;
  const value = obj(presentation.anchors)[probeKey(text, size, weight)];
  const width = Array.isArray(value) ? Number(value[1]) : NaN;
  if (Number.isFinite(width) && width > 0) return width;
  if (size === 14 && weight === 500) return estimate14(text);
  if (size === 12 && weight === 400) return estimate14(text, true);
  return pagesWidth(text, size >= 13 ? 14 : 12) * (weight >= 500 ? 1.04 : 1) * (size >= 13 ? size / 14 : size / 12);
}

const MENU_MIN = 160, CHROME = 8 + 2; // MenuPopup: p-1 and the 1pt border on both sides.
const ITEM = 16; // px-2

type Trait = { kind: string; label: string; description: string; isDefault: boolean };
type Runtime = { label: string; description: string };

/** Every text the composer's menus size by. */
export function menuProbes(traits: Trait[], runtimes: Runtime[], runOn: string[]): Probe[] {
  const list: Probe[] = [];
  for (const item of traits) {
    if (item.kind === 'header') list.push(probe(item.label, 12, 500));
    if (item.kind === 'option') { list.push(probe(item.label, 14, 400)); if (item.description) list.push(probe(item.description, 12, 400)); }
  }
  if (traits.some(item => item.isDefault)) list.push(probe('Default', 10, 500));
  for (const option of runtimes) list.push(probe(option.label, 14, 500), probe(option.description, 12, 400));
  if (runOn.length) for (const text of ['Run on', 'Workspace']) list.push(probe(text, 12, 500));
  for (const text of runOn) list.push(probe(text, 14, 400));
  return uniqueProbes(list);
}

/**
 * TraitsPicker's MenuPopup: headers (MenuGroupLabel px-2), radio rows with the
 * label, a 4pt gap and the "Default" badge (px-[3px] + 1pt border), and a
 * description of at most 224pt (max-w-56) under it.
 */
export function effortMenuWidth(presentation: Obj, traits: Trait[], viewport = 0): number {
  const badge = measured(presentation, 'Default', 10, 500) + 8;
  let content = 0;
  for (const item of traits) {
    if (item.kind === 'header') content = Math.max(content, measured(presentation, item.label, 12, 500));
    if (item.kind !== 'option') continue;
    content = Math.max(content, measured(presentation, item.label, 14, 400) + (item.isDefault ? 4 + badge : 0));
    if (item.description) content = Math.max(content, Math.min(224, measured(presentation, item.description, 12, 400)));
  }
  return cap(Math.max(MENU_MIN, content + ITEM + CHROME), viewport);
}

/** The runtime mode Select: a 14pt icon, 6pt, the medium label; the description under it. */
export function runtimeMenuWidth(presentation: Obj, runtimes: Runtime[], viewport = 0): number {
  let content = 0;
  for (const option of runtimes) {
    content = Math.max(content, 14 + 6 + measured(presentation, option.label, 14, 500), measured(presentation, option.description, 12, 400));
  }
  return cap(Math.max(MENU_MIN, content + ITEM + CHROME), viewport);
}

/**
 * MobileRunContextSelector with its "Run on" group: rows are a 12pt icon (its
 * -2pt margins), 6pt and the label; the group labels and the Workspace rows
 * ("Current checkout" / "Current worktree", "New worktree") join the widest.
 */
export function runOnMenuWidth(presentation: Obj, labels: string[], workspace: string[], viewport = 0): number {
  let content = Math.max(measured(presentation, 'Run on', 12, 500), measured(presentation, 'Workspace', 12, 500));
  for (const label of [...labels, ...workspace]) content = Math.max(content, 8 + 6 + measured(presentation, label, 14, 400));
  return cap(Math.max(MENU_MIN, content + ITEM + CHROME), viewport);
}

/**
 * A menu's height from its rows as ComposerMenus draws them (4pt padding and a 1pt border each side;
 * headers 26–28pt, dividers 9pt, notes 22pt, option rows 28pt or 44pt with a description), so the
 * popup can open below its trigger when it fits there, as MenuPopup's side="bottom" does.
 */
export function traitsMenuHeight(items: Trait[]): number {
  return items.reduce((sum, item) => sum + (item.kind === 'header' ? 28 : item.kind === 'divider' ? 9 : item.kind === 'note' ? 22
    : item.kind === 'option' ? (item.description ? 44 : 28) : 0), 10);
}
/** The runtime Select's rows are 46pt (label and description). */
export const runtimeMenuHeight = (runtimes: Runtime[]) => 10 + runtimes.length * 46;

/** max-w-[calc(100vw-2rem)] once the viewport is known. */
const cap = (width: number, viewport: number) => viewport > 0 ? Math.min(width, viewport - 32) : width;

/** The workspace rows under "Run on" (MobileRunContextSelector). */
export function workspaceLabels(worktree: boolean): string[] { return [worktree ? 'Current worktree' : 'Current checkout', 'New worktree']; }

/** The snapshot's measured menu fields (composer-presentation.ts composerSnapshot). */
export function composerMenus(presentation: Obj, view: { traits: Trait[]; runtimes: Runtime[] }, runOn: string[], extra: Probe[] = []) {
  return {
    menuProbes: uniqueProbes([...menuProbes(view.traits, view.runtimes, runOn.length ? [...runOn, ...workspaceLabels(false), ...workspaceLabels(true)] : []), ...extra]),
    effortMenuWidth: effortMenuWidth(presentation, view.traits),
    runtimeMenuWidth: runtimeMenuWidth(presentation, view.runtimes),
    effortMenuHeight: traitsMenuHeight(view.traits), runtimeMenuHeight: runtimeMenuHeight(view.runtimes),
  };
}

type CheckRow = { label: string; status: string };
/** pages-text-width.ts checkMenuWidth with measured texts: MenuCheckboxItems (8 + 16pt check + 8, the label, 12, the status, 16) in a min-w-40 MenuPopup. */
export function checkMenu(presentation: Obj, rows: CheckRow[], extra: string[] = []): { width: number; probes: Probe[] } {
  const widest = rows.reduce((max, row) => Math.max(max, 32 + measured(presentation, row.label, 14, 400)
    + (row.status ? 12 + measured(presentation, row.status, 12, 400) : 0) + 16), 0);
  // An action row (Model prices): 8 + 16pt icon + 8 + 6, the label, 8.
  const actions = extra.reduce((max, label) => Math.max(max, 10 + 28 + measured(presentation, label, 14, 400) + 8), 0);
  return { width: Math.max(MENU_MIN, widest + 10, actions),
    probes: uniqueProbes([...rows.flatMap(row => [probe(row.label, 14, 400), probe(row.status, 12, 400)]), ...extra.map(label => probe(label, 14, 400))]) };
}

/**
 * The menu and tooltip sizes above are px at the root size 16 (the probes lay their texts out in px and
 * the paddings are the reference's rem utilities at 16); the menus' own rows are rem, so each size follows
 * the Interface font size (the root font size) as the reference's rem-sized popups do.
 */
export const ROOT_SIZED_MENU_FIELDS = ['effortMenuWidth', 'runtimeMenuWidth', 'effortMenuHeight', 'runtimeMenuHeight',
  'moreMenuWidth', 'restingMoreMenuWidth', 'moreMenuHeight', 'restingMoreMenuHeight', 'sendTipWidth'] as const;
export function atRootFontSize<T extends object>(row: T, rootFontSize: number): T {
  if (rootFontSize === 16) return row;
  const out = { ...row } as Record<string, unknown>;
  for (const field of ROOT_SIZED_MENU_FIELDS) if (typeof out[field] === 'number') out[field] = (out[field] as number) * rootFontSize / 16;
  return out as T;
}

