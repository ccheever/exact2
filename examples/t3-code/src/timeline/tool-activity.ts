// T3 Code 9c161462c9 packages/shared/src/toolActivity.ts (MIT, LICENSE-T3).
import { obj } from '../client/domain';
import { highlight } from '../highlight/timeline-highlight';
import { toolCallLines } from './timeline-item-detail';
const trimmed = (value: unknown) => typeof value === 'string' && value.trim() ? value.trim() : undefined;
export function isJavaScriptReplTool(toolName: unknown): boolean {
  return toolName === 'js' || toolName === 'cua_repl.js' || toolName === 'node_repl.js';
}
export const COMPUTER_USE_TOOL_SOURCE = { key: 'computer-use', name: 'Computer Use', kind: 'computer' } satisfies Record<string, string>;
export function dynamicToolSurface(toolName: unknown, input: unknown): 'computer' | undefined {
  if (toolName === 'cua_repl.js') return 'computer';
  const code = obj(input).code;
  return isJavaScriptReplTool(toolName) && typeof code === 'string' && (/\bsky\s*\./u.test(code) || /["']@oai\/sky["']/u.test(code)) ? 'computer' : undefined;
}
export function replToolTitle(toolName: unknown, input: unknown, retainedTitle?: unknown): string | undefined {
  if (!isJavaScriptReplTool(toolName)) return undefined;
  const title = trimmed(retainedTitle);
  const meaningful = title && title.toLowerCase() !== 'js' && title.toLowerCase() !== (typeof toolName === 'string' ? toolName.toLowerCase() : '') ? title : undefined;
  return trimmed(obj(input).title) ?? meaningful ?? (dynamicToolSurface(toolName, input) === 'computer' ? 'Computer use' : 'Run JavaScript');
}

/** V2ItemInspector keeps each argument label, highlighting only a REPL's code value. */
export function replArguments(toolName: unknown, input: unknown) {
  if (!isJavaScriptReplTool(toolName)) return [];
  return (toolCallLines({ args: input }).args ?? []).map(([key, value], index) => ({ id: String(index), key, text: value,
    tokens: key === 'code' ? highlight(value, 'typescript').map((token, id) => ({ id: String(id), ...token })) : [] }));
}
