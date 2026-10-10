// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/keybinding-view.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Settings → Keybindings view model: rows, pills, labels, sources and conflicts
// from KeybindingsSettings.logic.ts, and the When builder of
// KeybindingsSettings.tsx as precomputed next expressions, so every builder
// control only replaces the draft expression with a string computed here.
import { arr, obj, str, type Obj } from './domain';
import { shortcutInput, whenExpression, keybindingDefaults, keybindingCommands } from './keybinding-settings';

export type WhenNode = { type: 'identifier'; name: string } | { type: 'not'; node: WhenNode } | { type: 'and' | 'or'; left: WhenNode; right: WhenNode };

const usageMetrics: Record<string, string> = { 'usage.cost': 'Cost', 'usage.tokens': 'Tokens', 'usage.limits': 'Limits' };
const usagePeriods: Record<string, string> = { 'usage.period.day': 'Past 24h', 'usage.period.week': '7 days', 'usage.period.month': '30 days', 'usage.period.quarter': '90 days' };
const usageOrder = ['usage.open', 'usage.cost', 'usage.tokens', 'usage.limits', 'usage.period.day', 'usage.period.week', 'usage.period.month', 'usage.period.quarter'];

function titleCase(segment: string): string {
  return segment.replace(/([a-z0-9])([A-Z])/g, '$1 $2').split(/[-_\s]+/).filter(Boolean).map(part => part.slice(0, 1).toUpperCase() + part.slice(1)).join(' ');
}
export function commandLabel(command: string): string {
  if (command === 'composer.sendAlternate') return 'Composer: Opposite Queue or Steer Action';
  if (command === 'composer.sendBackground') return 'Composer: Start in Background';
  if (command === 'composer.sendAndNewThread') return 'Composer: Send and Start New Thread';
  if (command === 'thread.steerQueuedMessage') return 'Queue: Send First Queued Message as Steer';
  if (command === 'thread.editQueuedMessage') return 'Queue: Edit Last Queued Message';
  if (command === 'thread.copyReference') return 'Pull Request: Copy Link or Thread ID';
  if (usageMetrics[command]) return `Usage: ${usageMetrics[command]}`;
  if (usagePeriods[command]) return `Usage: Period: ${usagePeriods[command]}`;
  if (command.startsWith('script.') && command.endsWith('.run')) return `Run Script: ${titleCase(command.slice(7, -4))}`;
  return command.split('.').map(titleCase).join(': ');
}
/** compareCommands: by `key`, with the Usage page commands as one block. */
export function compareCommands(left: string, right: string, key: (command: string) => string): number {
  const l = usageOrder.indexOf(left), r = usageOrder.indexOf(right);
  if (l >= 0 && r >= 0) return l - r;
  return key(l < 0 ? left : 'usage.cost').localeCompare(key(r < 0 ? right : 'usage.cost'));
}
/** KeybindingPill: one Kbd per `+` part, macOS glyphs. */
export function pillParts(value: string): string[] {
  return value.split('+').map(part => part === 'mod' ? '⌘' : part === 'shift' ? '⇧' : part === 'alt' ? '⌥' : part === 'ctrl' ? '⌃' : part.length === 1 ? part.toUpperCase() : part);
}
function keyLabel(key: string): string {
  if (key === ' ' || key === 'space') return 'Space';
  if (key.length === 1) return key.toUpperCase();
  if (key === 'escape' || key === 'esc') return 'Esc';
  const arrows: Record<string, string> = { arrowup: 'Up', arrowdown: 'Down', arrowleft: 'Left', arrowright: 'Right' };
  return arrows[key] || key.slice(0, 1).toUpperCase() + key.slice(1);
}
/** formatShortcutLabel on macOS: ⌃⌥⇧⌘ then the key. */
export function shortcutLabel(value: string): string {
  const tokens = value.toLowerCase().split('+');
  let key = tokens[tokens.length - 1] || '';
  if (key === '' && value.endsWith('+')) key = '+';
  const has = (name: string) => tokens.slice(0, -1).includes(name);
  return `${has('ctrl') || has('control') ? '⌃' : ''}${has('alt') || has('option') ? '⌥' : ''}${has('shift') ? '⇧' : ''}${has('mod') || has('meta') || has('cmd') ? '⌘' : ''}${keyLabel(key)}`;
}

// --- When expressions (parseKeybindingWhenExpression / whenAstToExpression) ---
export function parseWhen(expression: string): WhenNode | null {
  const tokens: string[] = [];
  let offset = 0;
  while (offset < expression.length) {
    const match = /^(\s+|&&|\|\||[!()]|[A-Za-z_][A-Za-z0-9_.-]*)/.exec(expression.slice(offset));
    if (!match) return null;
    if (!/^\s+$/.test(match[0])) tokens.push(match[0]);
    offset += match[0].length;
  }
  if (!tokens.length) return null;
  let index = 0;
  const primary = (depth: number): WhenNode | null => {
    if (depth > 64) return null;
    const token = tokens[index];
    if (token && /^[A-Za-z_]/.test(token)) { index++; return { type: 'identifier', name: token }; }
    if (token === '(') { index++; const node = or(depth + 1); if (!node || tokens[index] !== ')') return null; index++; return node; }
    return null;
  };
  const unary = (depth: number): WhenNode | null => {
    let nots = 0;
    while (tokens[index] === '!') { index++; if (++nots > 64) return null; }
    let node = primary(depth);
    if (!node) return null;
    while (nots-- > 0) node = { type: 'not', node };
    return node;
  };
  const and = (depth: number): WhenNode | null => {
    let left = unary(depth);
    while (left && tokens[index] === '&&') { index++; const right = unary(depth); if (!right) return null; left = { type: 'and', left, right }; }
    return left;
  };
  const or = (depth: number): WhenNode | null => {
    let left = and(depth);
    while (left && tokens[index] === '||') { index++; const right = and(depth); if (!right) return null; left = { type: 'or', left, right }; }
    return left;
  };
  const node = or(0);
  return node && index === tokens.length ? node : null;
}
export function printWhen(node: WhenNode | undefined): string {
  if (!node) return '';
  const wrap = (child: WhenNode) => child.type === 'identifier' || child.type === 'not' ? printWhen(child) : `(${printWhen(child)})`;
  if (node.type === 'identifier') return node.name;
  if (node.type === 'not') return `!${wrap(node.node)}`;
  return `${wrap(node.left)} ${node.type === 'and' ? '&&' : '||'} ${wrap(node.right)}`;
}

const coreVariables = ['terminalFocus', 'terminalOpen', 'isWeb', 'isDesktop', 'true', 'false'];
function identifiers(node: WhenNode | undefined, into: Set<string>): Set<string> {
  if (!node) return into;
  if (node.type === 'identifier') into.add(node.name);
  else if (node.type === 'not') identifiers(node.node, into);
  else { identifiers(node.left, into); identifiers(node.right, into); }
  return into;
}
/** buildWhenVariableOptions: core variables first, then the defaults' identifiers. */
export function whenVariables(): string[] {
  const known = new Set(coreVariables);
  for (const binding of keybindingDefaults) if (binding.when) identifiers(parseWhen(binding.when) || undefined, known);
  return [...known].sort((a, b) => {
    const l = coreVariables.indexOf(a), r = coreVariables.indexOf(b);
    if (l >= 0 || r >= 0) return (l < 0 ? 1e9 : l) - (r < 0 ? 1e9 : r);
    return a.localeCompare(b);
  });
}
const defaultVariable = () => whenVariables().find(name => name !== 'true' && name !== 'false') || 'terminalFocus';

type Path = number[];
function flatten(node: WhenNode, operator: 'and' | 'or'): WhenNode[] {
  return node.type === operator ? [...flatten(node.left, operator), ...flatten(node.right, operator)] : [node];
}
function group(children: WhenNode[], operator: 'and' | 'or'): WhenNode | undefined {
  return children.length ? children.slice(1).reduce<WhenNode>((left, right) => ({ type: operator, left, right }), children[0]) : undefined;
}
function condition(node: WhenNode): { identifier: string; negated: boolean } | null {
  if (node.type === 'identifier') return { identifier: node.name, negated: false };
  if (node.type === 'not' && node.node.type === 'identifier') return { identifier: node.node.name, negated: true };
  return null;
}
/** Replace the node at `path` (child indexes in the builder's flattened tree). */
function replaceAt(node: WhenNode, path: Path, next: (target: WhenNode) => WhenNode | undefined): WhenNode | undefined {
  if (!path.length) return next(node);
  if (condition(node)) return node;
  if (node.type === 'not') {
    const inner = replaceAt(node.node, path.slice(1), next);
    return inner ? { type: 'not', node: inner } : undefined;
  }
  const operator = node.type === 'or' ? 'or' : 'and';
  const children = flatten(node, operator);
  const replaced = replaceAt(children[path[0]], path.slice(1), next);
  const nextChildren = replaced ? children.map((child, index) => index === path[0] ? replaced : child) : children.filter((_, index) => index !== path[0]);
  return group(nextChildren, operator) ?? { type: 'identifier', name: defaultVariable() };
}

export type WhenLine = { id: string; depth: number; kind: string; label: string; negated: boolean; operator: string; removeLabel: string;
  negate: string; remove: string; useAnd: string; useOr: string; addCondition: string; addGroup: string; options: { value: string; label: string; selected: boolean }[]; operators: { value: string; label: string; selected: boolean }[] };
/** The builder as indented lines; every control's result precomputed. */
export function whenEditor(expression: string) {
  const parsed = expression.trim() ? parseWhen(expression.trim()) : null;
  const error = expression.trim() && !parsed ? 'Use variables with !, &&, ||, and parentheses.' : '';
  const root = parsed || undefined;
  const variables = whenVariables();
  const known = new Set(variables);
  const unknown = [...identifiers(root, new Set())].filter(name => !known.has(name)).sort();
  const lines: WhenLine[] = [];
  const print = (node: WhenNode | undefined) => printWhen(node);
  const at = (path: Path, next: (target: WhenNode) => WhenNode | undefined) => root ? print(path.length ? replaceAt(root, path, next) : next(root)) : '';
  const fresh = (): WhenNode => ({ type: 'identifier', name: defaultVariable() });
  const walk = (node: WhenNode, path: Path, depth: number) => {
    const id = path.join('.') || 'root';
    const removeLabel = depth === 0 ? 'Clear all conditions' : condition(node) ? 'Remove condition' : 'Remove group and its conditions';
    const base = { id, depth, removeLabel, remove: at(path, () => undefined), negate: '', useAnd: '', useOr: '', addCondition: '', addGroup: '', options: [] as WhenLine['options'], operators: [] as WhenLine['operators'], label: '', negated: false, operator: '' };
    const parts = condition(node);
    if (parts) {
      const values = known.has(parts.identifier) ? variables : [parts.identifier, ...variables];
      lines.push({ ...base, kind: 'condition', label: parts.identifier, negated: parts.negated,
        negate: at(path, target => { const c = condition(target)!; const leaf: WhenNode = { type: 'identifier', name: c.identifier }; return c.negated ? leaf : { type: 'not', node: leaf }; }),
        options: values.map(value => ({ value: at(path, target => { const leaf: WhenNode = { type: 'identifier', name: value }; return condition(target)!.negated ? { type: 'not', node: leaf } : leaf; }), label: value, selected: value === parts.identifier })) });
      return;
    }
    if (node.type === 'not') {
      lines.push({ ...base, kind: 'not', negated: true, negate: at(path, target => target.type === 'not' ? target.node : target) });
      walk(node.node, [...path, 0], depth + 1);
      return;
    }
    const operator = node.type === 'or' ? 'or' : 'and', children = flatten(node, operator);
    const and = at(path, target => group(flatten(target, operator), 'and')), or = at(path, target => group(flatten(target, operator), 'or'));
    lines.push({ ...base, kind: 'group', operator, useAnd: and, useOr: or, operators: [{ value: and, label: 'and', selected: operator === 'and' }, { value: or, label: 'or', selected: operator === 'or' }],
      addCondition: at(path, target => group([...flatten(target, operator), fresh()], operator)),
      addGroup: at(path, target => group([...flatten(target, operator), { type: operator === 'and' ? 'or' : 'and', left: fresh(), right: { type: 'not', node: fresh() } }], operator)) });
    children.forEach((child, index) => walk(child, [...path, index], depth + 1));
  };
  if (root) walk(root, [], 0);
  const rootGroup: WhenNode = { type: 'or', left: fresh(), right: { type: 'not', node: fresh() } };
  return { error, unknown: unknown.length === 1 ? `Unknown condition: ${unknown[0]}` : unknown.length ? `Unknown conditions: ${unknown.join(', ')}` : '', lines,
    addRootCondition: print(root ? { type: 'and', left: root, right: fresh() } : fresh()), addRootGroup: print(root ? { type: 'and', left: root, right: rootGroup } : rootGroup) };
}

// --- Rows (buildKeybindingRows) ---
export type KeybindingRowView = { id: string; command: string; label: string; key: string; condition: string; whenLabel: string; source: string; pills: { id: string; text: string }[]; aria: string;
  defaultKey: string; defaultWhen: string; canReset: boolean; canRemove: boolean; conflict: string; first: boolean };
export function conflictLabels(rows: { id: string; command: string; key: string; when: string }[], input: { id: string; key: string; when: string }): string[] {
  if (!input.key.trim()) return [];
  const conflicts = rows.filter(row => row.id !== input.id && row.key === input.key && (!row.when || !input.when || row.when === input.when)).map(row => commandLabel(row.command));
  return [...new Set(conflicts)].sort();
}
export function conflictText(labels: string[]): string {
  if (!labels.length) return '';
  return labels.length === 1 ? `Conflicts with ${labels[0]}.` : `Conflicts with ${labels.slice(0, 3).join(', ')}${labels.length > 3 ? ', and more' : ''}.`;
}
/** Stable row identity: command, key, when (bindingId's triple), encoded for Contract. */
export function rowId(command: string, key: string, when: string): string {
  return encodeURIComponent(JSON.stringify([command, key, when]));
}
export function buildRows(bindings: Obj[], query: string): KeybindingRowView[] {
  const defaults = keybindingDefaults.map(binding => ({ command: binding.command, key: binding.key, when: binding.when || '' }));
  const base = bindings.map(binding => {
    const command = str(binding.command), key = shortcutInput(obj(binding.shortcut)), when = whenExpression(binding.whenAst);
    const exact = defaults.some(entry => entry.command === command && entry.key === key && entry.when === when);
    const fallback = defaults.find(entry => entry.command === command && entry.key === key && entry.when === when) || defaults.find(entry => entry.command === command && entry.when === when) || defaults.find(entry => entry.command === command);
    const source = command.startsWith('script.') ? 'Project' : exact ? 'Default' : 'Custom';
    return { id: rowId(command, key, when), command, key, when, source, defaultKey: fallback ? fallback.key : '', defaultWhen: fallback ? fallback.when : '' };
  });
  const rows = base.map(row => ({ ...row, condition: row.when, label: commandLabel(row.command), whenLabel: row.when || 'Always', pills: pillParts(row.key).map((text, index) => ({ id: `${index}`, text })), aria: shortcutLabel(row.key),
    canReset: row.source === 'Custom' && row.defaultKey !== '', canRemove: row.source !== 'Default', conflict: conflictText(conflictLabels(base, row)), first: false }));
  rows.sort((left, right) => compareCommands(left.command, right.command, command => command) || left.key.localeCompare(right.key));
  const term = query.trim().toLowerCase();
  const filtered = term ? rows.filter(row => [row.command, row.label, row.key, row.condition, row.source].some(text => text.toLowerCase().includes(term))) : rows;
  return filtered.map(({ when, ...row }, index) => ({ ...row, first: index === 0 }));
}
/** buildKeybindingCommandOptions: the static catalog plus configured commands, by label. */
export function commandOptions(bindings: Obj[]): { value: string; label: string; selected: boolean }[] {
  const commands = new Set([...keybindingCommands, ...arr(bindings).map(binding => str(binding.command))]);
  return [...commands].sort((left, right) => compareCommands(left, right, commandLabel)).map(value => ({ value, label: commandLabel(value), selected: false }));
}
