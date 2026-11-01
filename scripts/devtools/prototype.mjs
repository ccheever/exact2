// Throwaway LLP 1155 Stage 0a prototype. Synthetic observations, no app or host connection.
// D3's two platform candidates are compared; neither is a native CSS cascade.
import { randomBytes } from 'node:crypto';
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const excerpt = readFileSync(new URL('./prototype.contract', import.meta.url), 'utf8');
const sourceURL = 'exact://stage0/prototype.contract';
const range = (line, start = 0, end = 1) => ({ startLine: line, startColumn: start, endLine: line, endColumn: end });
const property = (name, value) => ({ name, value });
const style = (properties, sheet) => ({ cssProperties: properties, shorthandEntries: [], ...(sheet ? { styleSheetId: sheet } : {}) });
const rule = (selector, properties, sheet, line, origin = 'regular') => ({
  matchingSelectors: [0],
  rule: { selectorList: { text: selector, range: range(line), selectors: [{ text: selector, range: range(line) }] },
    origin, ...(sheet ? { styleSheetId: sheet } : {}), style: {
      ...style(properties.map(p => sheet ? { ...p, range: range(line, excerpt.split('\n')[line].indexOf(p.name), excerpt.split('\n')[line].length) } : p), sheet),
      ...(sheet ? { range: range(line, 0, excerpt.split('\n')[line].length) } : {}),
    } },
});

export function prototype({ directory, port = 0, platform = 'computed' }) {
  if (!['computed', 'rule'].includes(platform)) throw new Error('platform is computed or rule');
  mkdirSync(directory, { recursive: true });
  const transcript = resolve(directory, 'wire.jsonl');
  const capability = randomBytes(24).toString('hex');
  const parent = `/${capability}`, path = `${parent}/page`;
  const until = Date.now() + 10 * 60_000;
  let socket, claimed = false;
  const record = (direction, message, category) => appendFileSync(transcript, JSON.stringify({ direction, category, message }) + '\n');
  const nodes = new Map([
    [100, { nodeId: 100, backendNodeId: 100, nodeType: 9, nodeName: '#document', localName: '', nodeValue: '', documentURL: 'exact://stage0/', baseURL: 'exact://stage0/', childNodeCount: 1 }],
    [1, { nodeId: 1, backendNodeId: 1, parentId: 100, nodeType: 1, nodeName: 'MAIN', localName: 'main', nodeValue: '', attributes: ['id', 'page'], childNodeCount: 1 }],
    [2, { nodeId: 2, backendNodeId: 2, parentId: 1, nodeType: 1, nodeName: 'BUTTON', localName: 'button', nodeValue: '', attributes: ['id', 'tinted-control'], childNodeCount: 0 }],
  ]);
  const children = id => id === 100 ? [1] : id === 1 ? [2] : [];
  const expanded = new Set();
  const node = (id, depth = 0) => {
    if (!nodes.has(id)) throw new Error(`unknown prototype node ${id}`);
    if (depth) expanded.add(id);
    return { ...nodes.get(id), ...(depth ? { children: children(id).map(n => node(n, depth < 0 ? -1 : depth - 1)) } : {}) };
  };
  const inherited = rule('prototype.contract:3:5', [property('color', 'rgb(190, 30, 45)')], 'contract', 2);
  const authored = rule('prototype.contract:4:7', [property('font-size', '24px')], 'contract', 3);
  const frame = { id: 'stub', loaderId: 'stub', url: 'exact://stage0/', domainAndRegistry: '', securityOrigin: 'exact://stage0', mimeType: 'text/plain', secureContextType: 'Secure', crossOriginIsolatedContextType: 'NotIsolated', gatedAPIFeatures: [] };
  const emit = (ws, method, params) => { const message = { method, params }; record('event', message, 'essential'); ws.send(JSON.stringify(message)); };
  const methods = {
    'DOM.enable': () => ({}),
    'CSS.enable': (_, ws) => {
      emit(ws, 'CSS.styleSheetAdded', { header: { styleSheetId: 'contract', frameId: 'stub', sourceURL, origin: 'regular', title: '', disabled: false, isInline: false, isMutable: false, isConstructed: false, startLine: 0, startColumn: 0, length: excerpt.length, endLine: excerpt.split('\n').length - 1, endColumn: 0 } });
      return {};
    },
    // The RFC explicitly allows a declared context that refuses every evaluation.
    'Runtime.enable': (_, ws) => {
      emit(ws, 'Runtime.executionContextCreated', { context: { id: 1, origin: 'exact://stage0', name: 'Synthetic Exact tree (no JavaScript)', uniqueId: 'exact-stage0', auxData: { isDefault: true, type: 'default', frameId: 'stub' } } });
      return {};
    },
    'Page.getResourceTree': () => ({ frameTree: { frame, resources: [] } }),
    'DOM.getDocument': ({ depth = 1 }) => ({ root: node(100, depth) }),
    'DOM.requestChildNodes': ({ nodeId, depth = 1 }, ws) => {
      node(nodeId);
      // Re-sending known children replaces the frontend's DOMNode objects and strands its selection.
      if (!expanded.has(nodeId)) { expanded.add(nodeId); emit(ws, 'DOM.setChildNodes', { parentId: nodeId, nodes: children(nodeId).map(id => node(id, depth < 0 ? -1 : depth - 1)) }); }
      else if (depth < 0 || depth > 1) for (const id of children(nodeId)) methods['DOM.requestChildNodes']({ nodeId: id, depth: depth < 0 ? -1 : depth - 1 }, ws);
      return {};
    },
    'CSS.getMatchedStylesForNode': ({ nodeId }) => {
      node(nodeId);
      return { matchedCSSRules: nodeId === 2 ? [authored, ...(platform === 'rule' ? [rule('AppKit tint', [property('color', 'rgb(0, 122, 255)')], null, 0, 'inspector')] : [])] : nodeId === 1 ? [inherited] : [],
        inherited: nodeId === 2 ? [{ matchedCSSRules: [inherited] }] : [] };
    },
    'CSS.getComputedStyleForNode': ({ nodeId }) => {
      node(nodeId);
      // font-family is deliberately unavailable. D3 permits omission, not invalid CSS.
      return { computedStyle: nodeId === 2 ? [property('font-size', '24px'), property('color', 'rgb(0, 122, 255)')] : [property('color', 'rgb(190, 30, 45)')], extraFields: { isAppearanceBase: false } };
    },
    'CSS.getStyleSheetText': ({ styleSheetId }) => { if (styleSheetId !== 'contract') throw new Error('unknown prototype sheet'); return { text: excerpt }; },
  };
  const refused = method => /^(Runtime\.(evaluate|callFunctionOn|compileScript|runScript)|Page\.(addScript|navigate|reload)|CSS\.(set|add|create|force)|DOM\.(setAttribute|setNode|setOuter|setFile|remove|move|copy|undo|redo|focus|scroll))/.test(method);
  const server = Bun.serve({
    hostname: '127.0.0.1', port,
    fetch(req, server) {
      const url = new URL(req.url), origin = req.headers.get('origin');
      const host = `127.0.0.1:${server.port}`;
      const address = server.requestIP(req)?.address;
      if (address !== '127.0.0.1' || req.headers.get('host') !== host || (origin !== null && origin !== 'devtools://devtools')) return new Response('forbidden', { status: 403 });
      record('http', { method: req.method, path: url.pathname.replace(capability, '<capability>'), origin }, 'attachment');
      if (req.method !== 'GET') return new Response('read only', { status: 405 });
      if (Date.now() >= until || claimed && !socket) return new Response('capability expired or revoked', { status: 410 });
      const ws = `ws://${host}${path}`;
      if (url.pathname === `${parent}/json/list`) return Response.json([{ id: 'stub', type: 'page', title: 'Exact Stage 0a — synthetic mounted tree', url: 'exact://stage0/', webSocketDebuggerUrl: ws, devtoolsFrontendUrl: `devtools://devtools/bundled/inspector.html?ws=${host}${path}` }]);
      if (url.pathname === `${parent}/json/version`) return Response.json({ Browser: 'Exact-Stage0a', 'Protocol-Version': '1.3' });
      if (url.pathname === path && !claimed && server.upgrade(req)) { claimed = true; return; }
      return new Response('not found', { status: 404 });
    },
    websocket: {
      open(ws) { socket = ws; },
      message(ws, bytes) {
        let request;
        try { request = JSON.parse(String(bytes)); } catch { ws.close(1003, 'JSON required'); return; }
        const { id, method, params = {} } = request;
        const category = refused(method) ? 'refused' : methods[method] ? 'essential' : 'tolerated-as-error';
        record('request', request, category);
        let response;
        if (category !== 'essential') response = { id, error: { code: -32601, message: category === 'refused' ? `Read-only prototype refuses ${method}` : `Prototype does not implement ${method}` } };
        else try { response = { id, result: methods[method](params, ws) }; } catch (error) { response = { id, error: { code: -32602, message: error.message } }; }
        record('response', response, category); ws.send(JSON.stringify(response));
      },
      close() { socket = null; },
    },
  });
  return { port: server.port, parent, path, url: `devtools://devtools/bundled/inspector.html?ws=127.0.0.1:${server.port}${path}`, stop() { socket?.close(); server.stop(true); } };
}

if (import.meta.main) {
  const directory = process.argv[2];
  if (!directory) throw new Error('bun scripts/devtools/prototype.mjs <scratch-evidence-directory> [computed|rule]');
  const server = prototype({ directory, platform: process.argv[3] ?? 'computed' });
  console.log(JSON.stringify({ pid: process.pid, ...server }));
  for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => { server.stop(); process.exit(0); });
}
