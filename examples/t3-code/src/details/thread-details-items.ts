// What the thread details card shows (T3 Code 6d450a4284, MIT, see LICENSE-T3: components/chat/threadDetailsItems.ts and
// ThreadDetailsItemsMenu.tsx). Ten items, each on by default but Usage; the choices are kept as the reference keeps the
// client setting `threadDetailsItems`: only those that differ from the default, so a later default reaches everyone
// else. The record lives in the client's preference file under the same name. The card's right-click menu, its
// "Choose what to show" row and Settings › Appearance › Thread details share this list. The reference also reads the
// retired Labs flag `threadDetailsUsageEnabled` as Usage's default; the clone never offered that flag, so Usage is off.
import { obj, type Obj } from '../client/domain';

// Ids are stored in the preference file, so never rename one.
export const THREAD_DETAILS_ITEMS = [
  { id: 'environment', label: 'Environment and worktree', defaultVisible: true },
  { id: 'openIn', label: 'Open in editor', defaultVisible: true },
  { id: 'scripts', label: 'Project scripts', defaultVisible: true },
  { id: 'branch', label: 'Branch and pull request', defaultVisible: true },
  { id: 'gitActions', label: 'Git actions', defaultVisible: true },
  { id: 'changes', label: 'Changes', defaultVisible: true },
  { id: 'servers', label: 'Servers', defaultVisible: true },
  { id: 'automations', label: 'Automations', defaultVisible: true },
  { id: 'lineage', label: 'Lineage', defaultVisible: true },
  { id: 'usage', label: 'Usage', defaultVisible: false },
] as const;

export type ThreadDetailsItemId = (typeof THREAD_DETAILS_ITEMS)[number]['id'];
export type ThreadDetailsVisibility = Readonly<Record<ThreadDetailsItemId, boolean>>;
type Choices = Partial<Record<ThreadDetailsItemId, boolean>>;
type Holder = { local: object };
type Stored = { threadDetailsItems?: Choices };

const isItem = (id: string): id is ThreadDetailsItemId => THREAD_DETAILS_ITEMS.some(item => item.id === id);
const defaultOf = (id: ThreadDetailsItemId) => THREAD_DETAILS_ITEMS.find(item => item.id === id)!.defaultVisible;

/** The saved choices (an empty record when there are none). */
export const threadDetailsChoices = (client: Holder): Choices => (client.local as Stored).threadDetailsItems ?? {};

export function resolveThreadDetailsVisibility(choices: Choices): ThreadDetailsVisibility {
  return Object.fromEntries(THREAD_DETAILS_ITEMS.map(item => [item.id, choices[item.id] ?? item.defaultVisible])) as Record<ThreadDetailsItemId, boolean>;
}
export const threadDetailsVisibility = (client: Holder): ThreadDetailsVisibility => resolveThreadDetailsVisibility(threadDetailsChoices(client));

/** Keeps only choices that differ from the default. */
export function setThreadDetailsItem(choices: Choices, id: ThreadDetailsItemId, visible: boolean): Choices {
  const { [id]: _previous, ...rest } = choices;
  return visible === defaultOf(id) ? rest : { ...rest, [id]: visible };
}

export const isDefaultThreadDetailsVisibility = (visibility: ThreadDetailsVisibility): boolean => THREAD_DETAILS_ITEMS.every(item => visibility[item.id] === item.defaultVisible);

/** "Thread details: N of 10 on" (Settings › Appearance's row). */
export const shownItemCount = (visibility: ThreadDetailsVisibility): number => THREAD_DETAILS_ITEMS.filter(item => visibility[item.id]).length;

/** The menu's rows: one checkbox per item, then Reset to default (disabled at the defaults). */
export function threadDetailsMenu(client: Holder) {
  const visibility = threadDetailsVisibility(client);
  return {
    items: THREAD_DETAILS_ITEMS.map(item => ({ id: item.id, label: item.label, checked: visibility[item.id] })),
    resetDisabled: isDefaultThreadDetailsVisibility(visibility),
  };
}

/** A menu row: `item` shows or hides one item (`value` "true" or "false"); `reset` clears every choice. */
export function threadDetailsItemsLocal(client: Holder, op: string, id: string, value: string): string {
  const stored = client.local as Stored;
  if (op === 'reset') { stored.threadDetailsItems = {}; return ''; }
  if (op === 'item' && isItem(id)) { stored.threadDetailsItems = setThreadDetailsItem(threadDetailsChoices(client), id, value === 'true'); return ''; }
  return '';
}

/** load(): the saved record, keeping only known items with a boolean that is not the default. */
export function adoptThreadDetailsItems(next: object, saved: Obj): void {
  let choices: Choices = {};
  for (const [id, visible] of Object.entries(obj(saved.threadDetailsItems))) if (isItem(id) && typeof visible === 'boolean') choices = setThreadDetailsItem(choices, id, visible);
  if (Object.keys(choices).length > 0) (next as Stored).threadDetailsItems = choices;
}
