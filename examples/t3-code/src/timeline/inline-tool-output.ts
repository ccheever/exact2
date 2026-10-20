// Adapted from T3 Code 9c161462c9 packages/shared/src/toolOutput.ts (MIT; LICENSE-T3).
import { HTML_RENDER_TOOL_NAME, readHtmlRenderReference, type HtmlRenderReference } from './html-render';
import { MCP_APP_OUTPUT_KEY, readMcpAppReference, type McpAppReference } from './mcp-app';
import { resolveName } from './timeline-t3tools';
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const resolveT3McpToolId = (name: string | null | undefined) => name == null ? null : resolveName(name);

const MAX_PARSED_BYTES = 16_384;
const MAX_METADATA_BYTES = 8_192;
const MAX_ID_LENGTH = 256;
const MAX_THREADS = 100;
const MAX_CONTENT_BLOCKS = 32;
const MAX_ENVELOPE_DEPTH = 4;
const MAX_ENVELOPE_NODES = 128;
const encoder = new TextEncoder();

interface ResultReadBudget {
  remainingBytes: number;
  remainingNodes: number;
  exceeded: boolean;
}

interface ResultEnvelope {
  data?: Record<string, unknown>;
  failed: boolean;
}

interface CompactToolOutput {
  isError?: true;
  threadId?: string;
  messageId?: string;
  taskId?: string;
  scheduledTaskId?: string;
  status?: "rolled_back";
  htmlRender?: HtmlRenderReference;
  [MCP_APP_OUTPUT_KEY]?: McpAppReference;
  thread?: { threadId: string };
  threads?: Array<{ threadId?: string; status?: "rolled_back" }>;
}

function readResult(value: unknown, budget: ResultReadBudget, depth = 0): ResultEnvelope {
  if (depth > MAX_ENVELOPE_DEPTH || budget.remainingNodes-- <= 0) {
    budget.exceeded = true;
    return { failed: false };
  }
  if (typeof value === "string") {
    if (value.length > budget.remainingBytes) {
      budget.exceeded = true;
      return { failed: false };
    }
    const bytes = encoder.encode(value).byteLength;
    if (bytes > budget.remainingBytes) {
      budget.exceeded = true;
      return { failed: false };
    }
    budget.remainingBytes -= bytes;
    try {
      return readResult(JSON.parse(value), budget, depth + 1);
    } catch {
      return { failed: false };
    }
  }
  if (Array.isArray(value)) {
    if (value.length > MAX_CONTENT_BLOCKS) {
      budget.exceeded = true;
      return { failed: false };
    }
    let data: ResultEnvelope["data"];
    let failed = false;
    for (const block of value) {
      const text = isObject(block) ? block.text : undefined;
      const result = readResult(
        isObject(text) ? (text.text ?? text) : text,
        budget,
        depth + 1,
      );
      data ??= result.data;
      failed ||= result.failed;
      if (budget.exceeded) break;
    }
    return { ...(data === undefined ? {} : { data }), failed };
  }
  if (!isObject(value)) return { failed: false };
  const failed =
    value.isError === true ||
    value.is_error === true ||
    value._tag === "OrchestratorMcpFailure" ||
    value.error != null;
  const content = value.structuredContent ?? value.content;
  if (content !== undefined) {
    const nested = readResult(content, budget, depth + 1);
    return { ...nested, failed: failed || nested.failed };
  }
  return { data: value, failed };
}

function boundedId(value: unknown): string | undefined {
  if (typeof value !== "string" || value.length > MAX_ID_LENGTH || value.trim() === "") {
    return undefined;
  }
  return Array.from(value).join("");
}

export function compactDynamicToolOutput(value: unknown): CompactToolOutput | undefined {
  const budget: ResultReadBudget = {
    remainingBytes: MAX_PARSED_BYTES,
    remainingNodes: MAX_ENVELOPE_NODES,
    exceeded: false,
  };
  const result = readResult(value, budget);
  const output: CompactToolOutput = result.failed ? { isError: true } : {};
  const data = budget.exceeded ? undefined : result.data;
  if (data !== undefined) {
    for (const key of ["threadId", "messageId", "taskId", "scheduledTaskId"] as const) {
      const id = boundedId(data[key]);
      if (id !== undefined) output[key] = id;
    }
    if (data.status === "rolled_back") output.status = "rolled_back";
    const htmlRender = readHtmlRenderReference(data.htmlRender);
    if (htmlRender !== undefined) output.htmlRender = htmlRender;
    const mcpApp = readMcpAppReference(data[MCP_APP_OUTPUT_KEY]);
    if (mcpApp !== undefined) output[MCP_APP_OUTPUT_KEY] = mcpApp;
    const nestedThreadId = isObject(data.thread)
      ? boundedId(data.thread.threadId)
      : undefined;
    if (nestedThreadId !== undefined) output.thread = { threadId: nestedThreadId };

    if (Array.isArray(data.threads)) {
      let complete = data.threads.length <= MAX_THREADS;
      const threads: NonNullable<CompactToolOutput["threads"]> = [];
      if (complete) {
        for (const entry of data.threads) {
          if (!isObject(entry)) {
            complete = false;
            break;
          }
          const threadId = boundedId(entry.threadId);
          const rolledBack = entry.status === "rolled_back";
          if (threadId === undefined && !rolledBack) {
            complete = false;
            break;
          }
          threads.push({
            ...(threadId === undefined ? {} : { threadId }),
            ...(rolledBack ? { status: "rolled_back" as const } : {}),
          });
        }
      }
      if (complete) output.threads = threads;
      else {
        // A partial batch or a top-level ID would turn an unknown creation
        // count into a confidently wrong one in the existing summary parser.
        delete output.threadId;
        delete output.status;
      }
    }
  }
  const oversized = () => encoder.encode(JSON.stringify(output)).byteLength > MAX_METADATA_BYTES;
  if (oversized()) {
    delete output.threads;
    delete output.threadId;
    delete output.status;
  }
  const app = output[MCP_APP_OUTPUT_KEY];
  if (app?.csp !== undefined && oversized()) {
    const { csp: _csp, ...rest } = app;
    output[MCP_APP_OUTPUT_KEY] = rest;
  }
  if (oversized()) delete output[MCP_APP_OUTPUT_KEY];
  return Object.keys(output).length === 0 ? undefined : output;
}

export function htmlRenderFromToolItem(item: {
  readonly toolName: string | null | undefined;
  readonly output?: unknown;
}): HtmlRenderReference | undefined {
  if (resolveT3McpToolId(item.toolName) !== HTML_RENDER_TOOL_NAME) return undefined;
  const output = compactDynamicToolOutput(item.output);
  return output?.isError ? undefined : output?.htmlRender;
}

/**
 * The MCP App a completed tool call carries, if any. The adapter that captured
 * it put the reference in the output, beside the tool's own result. A tool's
 * result can imitate that shape, so the reference only counts when it names
 * the very server and tool the item records: a server can then only ever
 * point at an app of its own.
 */
export function mcpAppFromToolItem(item: {
  readonly toolName: string | null | undefined;
  readonly output?: unknown;
}): McpAppReference | undefined {
  const app = compactDynamicToolOutput(item.output)?.[MCP_APP_OUTPUT_KEY];
  return app !== undefined && item.toolName === `${app.server}.${app.tool}` ? app : undefined;
}
