// T3 Code 9c161462c9 packages/shared/src/toolActivity.ts (MIT, LICENSE-T3).
import { obj } from '../client/domain';
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
