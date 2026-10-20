// Adapted from T3 Code 9c161462c9 packages/shared/src/mcpApp.ts (MIT; LICENSE-T3).
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

export const MCP_APP_PROTOCOL_VERSION = "2026-01-26";
export const MCP_APP_MIME_TYPE = "text/html;profile=mcp-app";
export const MCP_APP_EXTENSION_ID = "io.modelcontextprotocol/ui";
export const MCP_APP_RESOURCE_SCHEME = "ui://";
export const MCP_APP_OUTPUT_KEY = "t3McpApp";

const MCP_APP_MIN_HEIGHT = 80;
export const MCP_APP_DEFAULT_HEIGHT = 320;
export const MCP_APP_MAX_HEIGHT = 2000;
export const MCP_APP_MAX_HTML_BYTES = 5 * 1024 * 1024;
const MAX_DOMAINS = 32;
const MAX_DOMAIN_LENGTH = 256;
const MAX_NAME_LENGTH = 256;
const MAX_RESOURCE_URI_LENGTH = 4096;

export interface McpAppCsp {
  readonly connectDomains?: ReadonlyArray<string>;
  readonly resourceDomains?: ReadonlyArray<string>;
  readonly frameDomains?: ReadonlyArray<string>;
  readonly baseUriDomains?: ReadonlyArray<string>;
}

export interface McpAppPermissions {
  readonly camera?: Record<string, never>;
  readonly microphone?: Record<string, never>;
  readonly geolocation?: Record<string, never>;
  readonly clipboardWrite?: Record<string, never>;
}

export interface McpAppReference {
  readonly attachmentId: string;
  readonly server: string;
  readonly tool: string;
  readonly resourceUri: string;
  readonly csp?: McpAppCsp;
  readonly permissions?: McpAppPermissions;
  readonly prefersBorder?: boolean;
}

// Only scheme-qualified origins, optionally with a leading `*.` subdomain
// wildcard, survive. Anything else (keywords, paths, quotes, whitespace) could
// widen or break the generated policy, so it is dropped.
const DOMAIN_PATTERN = /^(?:https?|wss?):\/\/(?:\*\.)?[a-z0-9-]+(?:\.[a-z0-9-]+)*(?::\d{1,5})?$/i;

function readDomains(value: unknown): ReadonlyArray<string> | undefined {
  if (!Array.isArray(value)) return undefined;
  const domains = value
    .filter(
      (domain): domain is string =>
        typeof domain === "string" &&
        domain.length <= MAX_DOMAIN_LENGTH &&
        DOMAIN_PATTERN.test(domain),
    )
    .slice(0, MAX_DOMAINS);
  return domains.length === 0 ? undefined : domains;
}

export function readMcpAppCsp(value: unknown): McpAppCsp | undefined {
  if (!isObject(value)) return undefined;
  const csp: {
    connectDomains?: ReadonlyArray<string>;
    resourceDomains?: ReadonlyArray<string>;
    frameDomains?: ReadonlyArray<string>;
    baseUriDomains?: ReadonlyArray<string>;
  } = {};
  for (const key of [
    "connectDomains",
    "resourceDomains",
    "frameDomains",
    "baseUriDomains",
  ] as const) {
    const domains = readDomains(value[key]);
    if (domains !== undefined) csp[key] = domains;
  }
  return Object.keys(csp).length === 0 ? undefined : csp;
}

const PERMISSION_KEYS = ["camera", "microphone", "geolocation", "clipboardWrite"] as const;

export function readMcpAppPermissions(value: unknown): McpAppPermissions | undefined {
  if (!isObject(value)) return undefined;
  const permissions: Partial<Record<(typeof PERMISSION_KEYS)[number], Record<string, never>>> = {};
  for (const key of PERMISSION_KEYS) {
    if (isObject(value[key])) permissions[key] = {};
  }
  return Object.keys(permissions).length === 0 ? undefined : permissions;
}

const boundedName = (value: unknown) =>
  typeof value === "string" && value.trim() !== "" && value.length <= MAX_NAME_LENGTH
    ? value
    : undefined;

export function readMcpAppReference(value: unknown): McpAppReference | undefined {
  if (!isObject(value)) return undefined;
  const attachmentId = boundedName(value.attachmentId);
  const server = boundedName(value.server);
  const tool = boundedName(value.tool);
  const resourceUri =
    typeof value.resourceUri === "string" && value.resourceUri.length <= MAX_RESOURCE_URI_LENGTH
      ? value.resourceUri
      : undefined;
  if (
    attachmentId === undefined ||
    server === undefined ||
    tool === undefined ||
    resourceUri === undefined ||
    !resourceUri.startsWith(MCP_APP_RESOURCE_SCHEME)
  ) {
    return undefined;
  }
  const csp = readMcpAppCsp(value.csp);
  const permissions = readMcpAppPermissions(value.permissions);
  return {
    attachmentId,
    server,
    tool,
    resourceUri,
    ...(csp === undefined ? {} : { csp }),
    ...(permissions === undefined ? {} : { permissions }),
    ...(typeof value.prefersBorder === "boolean" ? { prefersBorder: value.prefersBorder } : {}),
  };
}

export function mcpAppReferencesEqual(left: McpAppReference, right: McpAppReference): boolean {
  return (
    left.attachmentId === right.attachmentId &&
    left.server === right.server &&
    left.tool === right.tool &&
    left.resourceUri === right.resourceUri
  );
}

export function readMcpAppResourceUri(meta: unknown): string | undefined {
  if (!isObject(meta)) return undefined;
  const nested = isObject(meta.ui) ? meta.ui.resourceUri : undefined;
  const uri = nested ?? meta["ui/resourceUri"];
  return typeof uri === "string" && uri.startsWith(MCP_APP_RESOURCE_SCHEME) ? uri : undefined;
}

export function mcpAppToolCallableByApp(meta: unknown): boolean {
  const visibility =
    isObject(meta) && isObject(meta.ui) ? meta.ui.visibility : undefined;
  return !Array.isArray(visibility) || visibility.includes("app");
}

export function mcpAppAllowAttribute(permissions: McpAppPermissions | undefined): string {
  const features: Array<string> = [];
  if (permissions?.camera) features.push("camera");
  if (permissions?.microphone) features.push("microphone");
  if (permissions?.geolocation) features.push("geolocation");
  if (permissions?.clipboardWrite) features.push("clipboard-write");
  return features.join("; ");
}

export function clampMcpAppHeight(height: number): number {
  if (!Number.isFinite(height)) return MCP_APP_DEFAULT_HEIGHT;
  return Math.round(Math.min(MCP_APP_MAX_HEIGHT, Math.max(MCP_APP_MIN_HEIGHT, height)));
}

export function mcpAppFileName(reference: McpAppReference): string {
  const name = reference.tool.replace(/[^\w.-]+/g, "-").slice(0, 80) || "app";
  return `${name}.html`;
}
