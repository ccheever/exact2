// Frame operations run with the Native ticket of the answer draining this queue.
import type { T3Client } from '../client/client';
import { arr, obj, str } from '../client/domain';
import type { Files, Native } from '../client/protocol';
import { openLink } from '../browser/browser-links';
import { CLIENT_VERSION } from '../connections/version-skew';
import { saveBytes } from '../media/save-file';
import { makeMcpAppHost, mcpAppStyleVariables, mcpResourceBytes, McpAppHostRefusal, type McpAppHost, type McpAppHostContext } from './mcp-app-host';
import type { McpAppReference } from './mcp-app';
import type { HtmlRenderTheme } from './html-render';

type Job = { run: (native: Native, storage?: Files) => Promise<unknown>; resolve: (value: unknown) => void; reject: (error: unknown) => void };
export type DocumentRuntime = {
  host: McpAppHost; jobs: Job[]; deadlines: Map<() => void, number>; now: number;
  permission: { serial: number; text: string; settle: (approved: boolean) => void } | null;
  closed: boolean; fullscreen: boolean;
};
export function makeDocumentRuntime(client: T3Client, input: {
  app: McpAppReference; sourceThreadId: string; itemId: string; conversationThreadId: string;
  post: (message: unknown) => void; theme: () => HtmlRenderTheme; owned: () => boolean;
  height: (height: number) => void; width: () => number; viewportHeight: () => number; canExpand: () => boolean;
}): DocumentRuntime {
  let permissionSerial = 0;
  const runtime: DocumentRuntime = { host: null!, jobs: [], deadlines: new Map(), now: 0, permission: null, closed: false, fullscreen: false };
  const requireOwner = () => { if (!input.owned() || runtime.closed) throw new McpAppHostRefusal('This document is no longer open.'); };
  const queue = <T>(run: (native: Native, storage?: Files) => Promise<T>): Promise<T> => {
    requireOwner();
    if (runtime.jobs.length >= 16) return Promise.reject(new McpAppHostRefusal('Too many document requests.'));
    return new Promise<T>((resolve, reject) => { runtime.jobs.push({ run, resolve: value => resolve(value as T), reject }); client.revision++; });
  };
  const ask = (text: string): Promise<boolean> => {
    requireOwner();
    if (runtime.permission) return Promise.resolve(false);
    return new Promise(resolve => { runtime.permission = { serial: ++permissionSerial, text, settle: resolve }; client.revision++; });
  };
  const target = { threadId: input.sourceThreadId, itemId: input.itemId };
  const context = (): McpAppHostContext => ({
    theme: input.theme().appearance, styles: { variables: mcpAppStyleVariables(input.theme().variables) },
    displayMode: runtime.fullscreen ? 'fullscreen' : 'inline', availableDisplayModes: ['inline', 'fullscreen'],
    containerDimensions: runtime.fullscreen ? { width: input.width(), height: input.viewportHeight() } : { width: input.width(), maxHeight: 2000 }, platform: 'desktop',
  });
  runtime.host = makeMcpAppHost({
    app: input.app, hostVersion: CLIENT_VERSION, post: input.post, hostContext: context,
    schedule: (delay, run) => { runtime.deadlines.set(run, runtime.now + delay); return () => { runtime.deadlines.delete(run); }; },
    callTool: async ({ name, arguments: args }) => {
      const info = obj(await queue(native => client.rpc(native, 'mcpApps.toolInfo', { ...target, name })));
      if (info.callable !== true) throw new McpAppHostRefusal('This app cannot call that tool.');
      if (info.readOnly !== true && !await ask(`Allow ${input.app.server} to run ${str(info.title) || name}?\n${JSON.stringify(args, null, 2)}`)) throw new McpAppHostRefusal('Declined by the user.');
      return queue(async native => { requireOwner(); const result = obj(await client.rpc(native, 'mcpApps.callTool', { ...target, name, arguments: obj(args) }, info.readOnly !== true)); return { content: arr(result.content), ...result }; });
    },
    readResource: ({ uri }) => queue(native => client.rpc(native, 'mcpApps.readResource', { ...target, uri })),
    openLink: async url => {
      if (!await ask(`Open this link from ${input.app.server}?\n${url}`)) throw new McpAppHostRefusal('Declined by the user.');
      await queue(async native => { await openLink(client, native, url); });
    },
    sendMessage: async text => {
      if (!await ask(`Send this message from ${input.app.server}?\n${text}`)) throw new McpAppHostRefusal('Declined by the user.');
      await queue(async (native, storage) => { requireOwner(); if (!storage || client.threadId !== input.conversationThreadId) throw new McpAppHostRefusal('This conversation is no longer selected.'); await client.command('send', '', text, 0, native, storage); });
    },
    updateModelContext: context => queue(async native => { await client.rpc(native, 'mcpApps.updateModelContext', obj({ ...target, ...context, conversationThreadId: input.conversationThreadId }), true); }),
    requestDisplayMode: async mode => { requireOwner(); if (mode === 'inline') runtime.fullscreen = false; else if (mode === 'fullscreen' && input.canExpand()) runtime.fullscreen = true; client.revision++; return runtime.fullscreen ? 'fullscreen' : 'inline'; },
    downloadFile: async files => {
      if (!await ask(`Save ${files.map(file => file.name).join(', ')} from ${input.app.server}?`)) throw new McpAppHostRefusal('Declined by the user.');
      for (const file of files) await queue(async native => {
        requireOwner();
        const bytes = file._tag === 'embedded' ? file.bytes : mcpResourceBytes(arr(obj(await client.rpc(native, 'mcpApps.readResource', { ...target, uri: file.uri })).contents)[0]);
        requireOwner();
        if (!bytes || bytes.byteLength > 32 * 1024 * 1024) throw new McpAppHostRefusal(`${file.name} has no contents or is too large to save.`);
        await saveBytes(client, bytes, file.name);
      });
    },
    onRequestTeardown: () => { if (runtime.fullscreen) runtime.fullscreen = false; else runtime.closed = true; client.revision++; },
    onSizeChanged: size => { if (!runtime.fullscreen && size.height !== undefined) input.height(size.height); },
  });
  return runtime;
}
export async function drainDocumentRuntime(runtime: DocumentRuntime, native: Native, now: number, storage?: Files): Promise<void> {
  runtime.now = now;
  for (const [run, due] of runtime.deadlines) if (due <= now) { runtime.deadlines.delete(run); run(); }
  // Settling a queued operation lets the protocol continuation enqueue its next
  // operation. No Native object survives this drain in a callback or timer.
  for (let step = 0; step < 32; step++) {
    await Promise.resolve();
    const job = runtime.jobs.shift();
    if (!job) break;
    try { job.resolve(await job.run(native, storage)); } catch (error) { job.reject(error); }
  }
}
export function disposeDocumentRuntime(runtime: DocumentRuntime): void {
  runtime.closed = true; runtime.host.dispose(); runtime.deadlines.clear();
  runtime.permission?.settle(false); runtime.permission = null;
  for (const job of runtime.jobs.splice(0)) job.reject(new McpAppHostRefusal('This document is no longer open.'));
}
